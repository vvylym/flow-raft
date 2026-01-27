//! Handler execution integration
//!
//! Integrates handler registry with workflow executor.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use flow_raft_core::{TaskExecution, TaskId, WorkflowId};
use flow_raft_observability::{
    HistoryStore, WorkflowWatcher,
    history::{ExecutionEvent, ExecutionEventType},
};
use flow_raft_raft::command::WorkflowCommandBuilder;
use flow_raft_raft::executor::WorkflowExecutor;
use flow_raft_raft::types::TypeConfig;
use openraft::RaftTypeConfig;
use openraft::error::RaftError;

use super::registry::HandlerRegistry;
use super::task_router::{RunTaskCaller, TaskRouter};

/// Error type for handler execution
#[derive(Debug, thiserror::Error)]
pub enum HandlerExecutionError {
    /// Handler not found
    #[error("Handler not found: {handler_name} for workflow {workflow_id}")]
    HandlerNotFound {
        /// The workflow ID
        workflow_id: WorkflowId,
        /// The handler name that was not found
        handler_name: String,
    },
    /// Raft error
    #[error("Raft error: {0}")]
    RaftError(
        RaftError<
            <TypeConfig as RaftTypeConfig>::NodeId,
            openraft::error::ClientWriteError<
                <TypeConfig as RaftTypeConfig>::NodeId,
                <TypeConfig as RaftTypeConfig>::Node,
            >,
        >,
    ),
}

/// Handler executor that integrates registry with workflow executor
pub struct HandlerExecutor {
    /// Workflow executor
    executor: Arc<WorkflowExecutor>,
    /// Handler registry
    registry: Arc<HandlerRegistry>,
    /// Workflow watcher for broadcasting events
    watcher: Option<Arc<WorkflowWatcher>>,
    /// History store for recording execution events
    history_store: Option<Arc<HistoryStore>>,
    /// When set, tasks may be routed to other nodes via [RunTaskCaller]
    task_router: Option<Arc<dyn TaskRouter>>,
    /// This node's gRPC endpoint (used to avoid self-routing when router returns it)
    self_endpoint: Option<String>,
    /// Caller used to run a task on a remote endpoint (RunTask gRPC)
    run_task_caller: Option<Arc<dyn RunTaskCaller>>,
}

impl HandlerExecutor {
    /// Creates a new handler executor
    pub fn new(executor: Arc<WorkflowExecutor>, registry: Arc<HandlerRegistry>) -> Self {
        Self {
            executor,
            registry,
            watcher: None,
            history_store: None,
            task_router: None,
            self_endpoint: None,
            run_task_caller: None,
        }
    }

    /// Creates a new handler executor with workflow watcher
    pub fn with_watcher(
        executor: Arc<WorkflowExecutor>,
        registry: Arc<HandlerRegistry>,
        watcher: Arc<WorkflowWatcher>,
    ) -> Self {
        Self {
            executor,
            registry,
            watcher: Some(watcher),
            history_store: None,
            task_router: None,
            self_endpoint: None,
            run_task_caller: None,
        }
    }

    /// Creates a new handler executor with history store
    pub fn with_history_store(
        executor: Arc<WorkflowExecutor>,
        registry: Arc<HandlerRegistry>,
        history_store: Arc<HistoryStore>,
    ) -> Self {
        Self {
            executor,
            registry,
            watcher: None,
            history_store: Some(history_store),
            task_router: None,
            self_endpoint: None,
            run_task_caller: None,
        }
    }

    /// Enables distributed routing: tasks can be run on other nodes when the router
    /// returns an endpoint different from `self_endpoint`. The caller is used to
    /// run the task via RunTask gRPC; the leader applies the result to the Raft state.
    pub fn with_distributed_routing(
        mut self,
        task_router: Arc<dyn TaskRouter>,
        self_endpoint: String,
        run_task_caller: Arc<dyn RunTaskCaller>,
    ) -> Self {
        self.task_router = Some(task_router);
        self.self_endpoint = Some(self_endpoint);
        self.run_task_caller = Some(run_task_caller);
        self
    }

    /// Sets the workflow watcher
    pub fn set_watcher(&mut self, watcher: Arc<WorkflowWatcher>) {
        self.watcher = Some(watcher);
    }

    /// Sets the history store
    pub fn set_history_store(&mut self, history_store: Arc<HistoryStore>) {
        self.history_store = Some(history_store);
    }

    /// Applies a task result (from a remote RunTask call) to the Raft state.
    async fn apply_task_result(
        &self,
        workflow_id: WorkflowId,
        task_id: TaskId,
        result: Result<serde_json::Value, String>,
    ) -> Result<(), HandlerExecutionError> {
        let now = chrono::Utc::now();
        let (state, last_error, outputs) = match result {
            Ok(output) => (flow_raft_core::TaskState::Completed, None, Some(output)),
            Err(e) => (
                flow_raft_core::TaskState::Failed {
                    error_message: Some(e.clone()),
                    failure_kind: flow_raft_core::FailureKind::Retryable,
                },
                Some(e),
                None,
            ),
        };
        let execution = TaskExecution {
            task_id,
            state,
            attempts: 1,
            started_at: Some(now),
            completed_at: Some(now),
            last_error,
            outputs,
        };
        let request =
            WorkflowCommandBuilder::update_task_execution(workflow_id, task_id, execution);
        self.executor
            .raft()
            .client_write(request)
            .await
            .map(|_| ())
            .map_err(HandlerExecutionError::RaftError)
    }

    /// Executes a task using the registered handler
    ///
    /// # Arguments
    /// * `workflow_id` - The workflow ID
    /// * `task_id` - The task ID
    /// * `handler_name` - The handler name (from task definition)
    /// * `inputs` - Task inputs
    ///
    /// # Returns
    /// Ok(()) if execution succeeded, error otherwise
    pub async fn execute_task(
        &self,
        workflow_id: WorkflowId,
        task_id: TaskId,
        handler_name: &str,
        inputs: serde_json::Value,
    ) -> Result<(), HandlerExecutionError> {
        // Get handler from registry
        let handler = self
            .registry
            .get_handler(&workflow_id, handler_name)
            .await
            .ok_or_else(|| HandlerExecutionError::HandlerNotFound {
                workflow_id,
                handler_name: handler_name.to_string(),
            })?;

        // Execute task using workflow executor
        self.executor
            .execute_task(workflow_id, task_id, handler.as_ref(), inputs)
            .await
            .map_err(HandlerExecutionError::RaftError)
    }

    /// Gets the workflow executor
    pub fn executor(&self) -> &Arc<WorkflowExecutor> {
        &self.executor
    }

    /// Gets the handler registry
    pub fn registry(&self) -> &Arc<HandlerRegistry> {
        &self.registry
    }

    /// Executes a workflow until completion
    ///
    /// This method:
    /// 1. Gets ready tasks
    /// 2. Executes them using registered handlers
    /// 3. Waits for state updates
    /// 4. Repeats until workflow is complete
    ///
    /// # Arguments
    /// * `workflow_id` - The workflow ID to execute
    /// * `max_iterations` - Maximum number of execution cycles (prevents infinite loops)
    ///
    /// # Returns
    /// Ok(()) if workflow completed successfully, error otherwise
    #[tracing::instrument(level = "info", skip(self), fields(workflow_id = %workflow_id, max_iterations = max_iterations))]
    pub async fn execute_workflow(
        &self,
        workflow_id: WorkflowId,
        max_iterations: usize,
    ) -> Result<(), HandlerExecutionError> {
        // Create a span for the entire workflow execution
        let span =
            tracing::span!(tracing::Level::INFO, "workflow_execution", workflow_id = %workflow_id);
        let _guard = span.enter();
        for _iteration in 0..max_iterations {
            // Wait a bit for state machine to catch up
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // Get current workflow state
            let workflow = Arc::as_ref(&self.executor)
                .state_machine()
                .get_workflow(&workflow_id)
                .await;

            let Some(workflow) = workflow else {
                return Err(HandlerExecutionError::HandlerNotFound {
                    workflow_id,
                    handler_name: "workflow_not_found".to_string(),
                });
            };

            // Check if workflow is complete
            use flow_raft_core::WorkflowState;
            match &workflow.state {
                WorkflowState::Completed => {
                    println!("✓ Workflow {} completed successfully!", workflow_id);
                    if let Some(outputs) = &workflow.outputs {
                        println!(
                            "  Final outputs: {}",
                            serde_json::to_string_pretty(outputs).unwrap_or_default()
                        );
                    }
                    // Broadcast workflow completed event
                    if let Some(watcher) = &self.watcher {
                        watcher.broadcast_update(flow_raft_observability::WorkflowUpdate {
                            workflow_id,
                            event_type: "workflow_completed".to_string(),
                            data: workflow
                                .outputs
                                .as_ref()
                                .map(|o| serde_json::to_string(o).unwrap_or_default()),
                            timestamp: chrono::Utc::now(),
                        });
                    }
                    return Ok(());
                }
                WorkflowState::Failed { .. } => {
                    println!("✗ Workflow {} failed", workflow_id);
                    if let Some(error) = &workflow.error_message {
                        println!("  Error: {}", error);
                    }
                    // Broadcast workflow failed event
                    if let Some(watcher) = &self.watcher {
                        watcher.broadcast_update(flow_raft_observability::WorkflowUpdate {
                            workflow_id,
                            event_type: "workflow_failed".to_string(),
                            data: workflow.error_message.clone(),
                            timestamp: chrono::Utc::now(),
                        });
                    }
                    return Err(HandlerExecutionError::HandlerNotFound {
                        workflow_id,
                        handler_name: format!(
                            "workflow_failed: {}",
                            workflow.error_message.as_deref().unwrap_or("unknown")
                        ),
                    });
                }
                WorkflowState::Cancelled => {
                    println!("⚠ Workflow {} was cancelled", workflow_id);
                    // Broadcast workflow cancelled event
                    if let Some(watcher) = &self.watcher {
                        watcher.broadcast_update(flow_raft_observability::WorkflowUpdate {
                            workflow_id,
                            event_type: "workflow_cancelled".to_string(),
                            data: None,
                            timestamp: chrono::Utc::now(),
                        });
                    }
                    return Ok(());
                }
                _ => {
                    // Workflow is still running, continue execution
                }
            }

            // Get ready tasks
            let mut ready_tasks = Arc::as_ref(&self.executor)
                .get_ready_tasks(&workflow_id)
                .await;

            // Build name -> task_id for merge/conditional resolution
            let name_to_id: HashMap<String, TaskId> = workflow
                .task_definitions
                .iter()
                .map(|(id, d)| (d.name.clone(), *id))
                .collect();

            // Filter conditionals: only the chosen branch runs
            let cond_edges = self.registry.get_conditional_edges(&workflow_id).await;
            let mut to_remove = HashSet::new();
            for (source_name, then_name, else_name, condition) in cond_edges {
                let Some(&source_id) = name_to_id.get(&source_name) else {
                    continue;
                };
                let Some(&then_id) = name_to_id.get(&then_name) else {
                    continue;
                };
                let Some(&else_id) = name_to_id.get(&else_name) else {
                    continue;
                };
                let Some(exec) = workflow.executions.get(&source_id) else {
                    continue;
                };
                if !exec.state.is_terminal() {
                    continue;
                }
                let Some(out) = exec.outputs.clone() else {
                    continue;
                };
                let chosen = match condition.evaluate(out) {
                    Ok(n) => n,
                    Err(_) => {
                        to_remove.insert(then_id);
                        to_remove.insert(else_id);
                        continue;
                    }
                };
                let chosen_str = chosen.as_ref();
                if chosen_str == then_name {
                    to_remove.insert(else_id);
                } else if chosen_str == else_name {
                    to_remove.insert(then_id);
                } else {
                    to_remove.insert(then_id);
                    to_remove.insert(else_id);
                }
            }
            ready_tasks.retain(|id| !to_remove.contains(id));

            if ready_tasks.is_empty() {
                // No ready tasks, check if all tasks are complete.
                // For conditionals, the non-chosen branch and any task that depends only on
                // skipped/unreachable tasks count as "done".
                let cond_edges = self.registry.get_conditional_edges(&workflow_id).await;
                let mut skipped = HashSet::new();
                for (source_name, then_name, else_name, condition) in cond_edges {
                    let Some(&source_id) = name_to_id.get(&source_name) else {
                        continue;
                    };
                    let Some(&then_id) = name_to_id.get(&then_name) else {
                        continue;
                    };
                    let Some(&else_id) = name_to_id.get(&else_name) else {
                        continue;
                    };
                    let Some(exec) = workflow.executions.get(&source_id) else {
                        continue;
                    };
                    if !exec.state.is_terminal() {
                        continue;
                    }
                    let Some(out) = exec.outputs.clone() else {
                        continue;
                    };
                    let chosen = match condition.evaluate(out) {
                        Ok(n) => n,
                        Err(_) => continue,
                    };
                    let chosen_str = chosen.as_ref();
                    if chosen_str == then_name {
                        skipped.insert(else_id);
                    } else if chosen_str == else_name {
                        skipped.insert(then_id);
                    }
                }
                // Transitively add tasks whose prerequisites include a skipped task
                loop {
                    let mut added = false;
                    for (task_id, deps) in &workflow.dependencies {
                        if skipped.contains(task_id) {
                            continue;
                        }
                        if deps.prerequisites.iter().any(|p| skipped.contains(p)) {
                            skipped.insert(*task_id);
                            added = true;
                        }
                    }
                    if !added {
                        break;
                    }
                }
                let all_complete = workflow.task_definitions.keys().all(|task_id| {
                    if skipped.contains(task_id) {
                        true
                    } else {
                        workflow
                            .executions
                            .get(task_id)
                            .map(|e| e.state.is_terminal())
                            .unwrap_or(false)
                    }
                });

                if all_complete {
                    // All tasks complete - transition workflow to completed
                    // Get the workflow as a running workflow and complete it
                    // Workflow types used for state transitions
                    use flow_raft_raft::command::WorkflowCommandBuilder;

                    // Create updated snapshot with completed state
                    let mut completed_snapshot = workflow.clone();
                    completed_snapshot.state = flow_raft_core::WorkflowState::Completed;
                    completed_snapshot.completed_at = Some(chrono::Utc::now());

                    // Collect outputs from completed tasks
                    let mut outputs = serde_json::Map::new();
                    for (task_id, exec) in &workflow.executions {
                        if let Some(task_outputs) = &exec.outputs
                            && let Some(output) = task_outputs.get("output")
                        {
                            outputs.insert(format!("task_{}", task_id), output.clone());
                        }
                    }
                    if !outputs.is_empty() {
                        completed_snapshot.outputs = Some(serde_json::Value::Object(outputs));
                    }

                    // Update workflow state via Raft
                    let request = WorkflowCommandBuilder::transition_workflow(
                        workflow_id,
                        completed_snapshot,
                    );

                    // Use the executor's raft method which returns &Arc<Raft>
                    let executor = Arc::as_ref(&self.executor);
                    let raft = executor.raft();
                    if let Err(e) = (*raft).client_write(request).await {
                        println!("  Warning: Failed to mark workflow as completed: {:?}", e);
                    } else {
                        // Wait for state update
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }

                    // Check again if workflow is now completed
                    if let Some(updated) = Arc::as_ref(&self.executor)
                        .state_machine()
                        .get_workflow(&workflow_id)
                        .await
                        && matches!(updated.state, flow_raft_core::WorkflowState::Completed)
                    {
                        println!("✓ Workflow {} completed successfully!", workflow_id);
                        if let Some(outputs) = &updated.outputs {
                            println!(
                                "  Final outputs: {}",
                                serde_json::to_string_pretty(outputs).unwrap_or_default()
                            );
                        }
                        // Record workflow state change event
                        if let Some(history_store) = &self.history_store {
                            history_store
                                .record_event(
                                    workflow_id,
                                    ExecutionEvent {
                                        event_type: ExecutionEventType::WorkflowStateChange,
                                        task_id: None,
                                        data: serde_json::json!({
                                            "state": "completed",
                                            "outputs": updated.outputs
                                        })
                                        .to_string(),
                                        timestamp: chrono::Utc::now(),
                                    },
                                )
                                .await;
                        }
                        // Broadcast workflow completed event
                        if let Some(watcher) = &self.watcher {
                            watcher.broadcast_update(flow_raft_observability::WorkflowUpdate {
                                workflow_id,
                                event_type: "workflow_completed".to_string(),
                                data: updated
                                    .outputs
                                    .as_ref()
                                    .map(|o| serde_json::to_string(o).unwrap_or_default()),
                                timestamp: chrono::Utc::now(),
                            });
                        }
                        return Ok(());
                    }
                }
                // No ready tasks and not all complete - might be waiting or stuck
                continue;
            }

            // Execute ready tasks
            for task_id in ready_tasks {
                let task_def = workflow.task_definitions.get(&task_id);
                let Some(task_def) = task_def else {
                    continue;
                };

                // Get task inputs: merge target uses merge fn; others use workflow.inputs + prereq outputs
                let task_inputs = if let Some((source_names, merge_obj)) = self
                    .registry
                    .get_merge_spec(&workflow_id, &task_def.name)
                    .await
                {
                    let mut inputs = Vec::new();
                    let mut ok = true;
                    for sn in &source_names {
                        if let Some(&sid) = name_to_id.get(sn) {
                            if let Some(out) = workflow
                                .executions
                                .get(&sid)
                                .and_then(|e| e.outputs.clone())
                            {
                                inputs.push(out);
                            } else {
                                ok = false;
                                break;
                            }
                        } else {
                            ok = false;
                            break;
                        }
                    }
                    if ok && inputs.len() == source_names.len() {
                        merge_obj.merge(inputs).unwrap_or_else(|e| {
                            tracing::warn!("merge error: {}", e);
                            serde_json::Value::Null
                        })
                    } else {
                        serde_json::Value::Null
                    }
                } else {
                    let mut inputs = workflow.inputs.clone();
                    if let Some(deps) = workflow.dependencies.get(&task_id) {
                        for prereq_id in &deps.prerequisites {
                            if let Some(prereq_exec) = workflow.executions.get(prereq_id)
                                && let Some(outputs) = &prereq_exec.outputs
                                && let Some(obj) = inputs.as_object_mut()
                                && let Some(prereq_obj) = outputs.as_object()
                            {
                                for (k, v) in prereq_obj {
                                    obj.insert(k.clone(), v.clone());
                                }
                            }
                        }
                    }
                    inputs
                };

                println!(
                    "  → Executing task: {} (handler: {})",
                    task_id, task_def.handler
                );

                // Create span for task execution
                let task_span = tracing::span!(
                    tracing::Level::INFO,
                    "task_execution",
                    task_id = %task_id,
                    handler = %task_def.handler,
                    workflow_id = %workflow_id
                );
                let _task_guard = task_span.enter();

                // Record task started event
                if let Some(history_store) = &self.history_store {
                    history_store
                        .record_event(
                            workflow_id,
                            ExecutionEvent {
                                event_type: ExecutionEventType::TaskStarted,
                                task_id: Some(task_id),
                                data: serde_json::json!({
                                    "task_id": task_id,
                                    "handler": task_def.handler
                                })
                                .to_string(),
                                timestamp: chrono::Utc::now(),
                            },
                        )
                        .await;
                }
                // Broadcast task started event
                if let Some(watcher) = &self.watcher {
                    watcher.broadcast_update(flow_raft_observability::WorkflowUpdate {
                        workflow_id,
                        event_type: "task_started".to_string(),
                        data: Some(
                            serde_json::json!({
                                "task_id": task_id,
                                "handler": task_def.handler
                            })
                            .to_string(),
                        ),
                        timestamp: chrono::Utc::now(),
                    });
                }

                // Route task: remote via RunTask or local
                let run_remote = self
                    .task_router
                    .as_ref()
                    .zip(self.run_task_caller.as_ref())
                    .zip(self.self_endpoint.as_ref())
                    .and_then(|((router, _caller), self_ep)| {
                        router
                            .route(&workflow_id, &task_def.handler)
                            .filter(|ep| ep != self_ep)
                    });

                if let Some(ep) = run_remote
                    && let Some(caller) = &self.run_task_caller
                {
                    let result = caller
                        .run_task_on(
                            ep.as_str(),
                            workflow_id,
                            task_id,
                            &task_def.handler,
                            task_inputs,
                        )
                        .await;
                    self.apply_task_result(workflow_id, task_id, result).await?;
                    // Wait for state update then continue loop (same as local path)
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    continue;
                }

                // Execute task locally
                match self
                    .execute_task(workflow_id, task_id, &task_def.handler, task_inputs)
                    .await
                {
                    Ok(()) => {
                        // Wait for state update
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                        // Get updated workflow to see task result
                        if let Some(updated_workflow) = Arc::as_ref(&self.executor)
                            .state_machine()
                            .get_workflow(&workflow_id)
                            .await
                            && let Some(exec) = updated_workflow.executions.get(&task_id)
                        {
                            match &exec.state {
                                flow_raft_core::TaskState::Completed => {
                                    // Record task completed event
                                    if let Some(history_store) = &self.history_store {
                                        history_store
                                            .record_event(
                                                workflow_id,
                                                ExecutionEvent {
                                                    event_type: ExecutionEventType::TaskCompleted,
                                                    task_id: Some(task_id),
                                                    data: exec
                                                        .outputs
                                                        .as_ref()
                                                        .map(|o| {
                                                            serde_json::to_string(o)
                                                                .unwrap_or_default()
                                                        })
                                                        .unwrap_or_else(|| "{}".to_string()),
                                                    timestamp: chrono::Utc::now(),
                                                },
                                            )
                                            .await;
                                    }
                                    // Broadcast task completed event
                                    if let Some(watcher) = &self.watcher {
                                        watcher.broadcast_update(
                                            flow_raft_observability::WorkflowUpdate {
                                                workflow_id,
                                                event_type: "task_completed".to_string(),
                                                data: exec.outputs.as_ref().map(|o| {
                                                    serde_json::to_string(o).unwrap_or_default()
                                                }),
                                                timestamp: chrono::Utc::now(),
                                            },
                                        );
                                    }
                                    if let Some(outputs) = &exec.outputs {
                                        println!("    ✓ Task {} completed", task_id);
                                        if let Some(output) = outputs.get("output") {
                                            println!(
                                                "      Output: {}",
                                                serde_json::to_string(output).unwrap_or_default()
                                            );
                                        }
                                    }
                                }
                                flow_raft_core::TaskState::Failed { error_message, .. } => {
                                    // Record task failed event
                                    if let Some(history_store) = &self.history_store {
                                        history_store
                                            .record_event(
                                                workflow_id,
                                                ExecutionEvent {
                                                    event_type: ExecutionEventType::TaskFailed,
                                                    task_id: Some(task_id),
                                                    data: serde_json::json!({
                                                        "error": error_message
                                                    })
                                                    .to_string(),
                                                    timestamp: chrono::Utc::now(),
                                                },
                                            )
                                            .await;
                                    }
                                    // Broadcast task failed event
                                    if let Some(watcher) = &self.watcher {
                                        watcher.broadcast_update(
                                            flow_raft_observability::WorkflowUpdate {
                                                workflow_id,
                                                event_type: "task_failed".to_string(),
                                                data: error_message.clone(),
                                                timestamp: chrono::Utc::now(),
                                            },
                                        );
                                    }
                                    println!(
                                        "    ✗ Task {} failed: {}",
                                        task_id,
                                        error_message.as_deref().unwrap_or("unknown")
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        println!("    ✗ Task {} execution error: {:?}", task_id, e);
                        // Continue with other tasks
                    }
                }
            }
        }

        Err(HandlerExecutionError::HandlerNotFound {
            workflow_id,
            handler_name: format!("max_iterations_reached: {}", max_iterations),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::{MapTaskRouter, RunTaskCaller};
    use flow_raft_raft::config::default_config;
    use flow_raft_raft::executor::TaskHandler;
    use flow_raft_raft::network::MemoryNetworkFactory;
    use flow_raft_raft::storage::{LogStore, StateMachineStore};
    use flow_raft_raft::types::NodeId;
    use openraft::Raft;
    use std::sync::Arc;

    struct MockHandler;

    impl TaskHandler for MockHandler {
        fn execute(
            &self,
            _task_id: TaskId,
            _inputs: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({"result": "success"}))
        }
    }

    async fn create_test_executor() -> Arc<WorkflowExecutor> {
        let node_id = 1;
        let config = Arc::new(default_config().validate().unwrap());
        let network = MemoryNetworkFactory::new();
        let log_store = LogStore::default();
        let state_machine = StateMachineStore::default();

        let raft = Raft::new(node_id, config, network, log_store, state_machine.clone())
            .await
            .unwrap();

        Arc::new(WorkflowExecutor::new(
            Arc::new(raft),
            state_machine,
            node_id,
        ))
    }

    #[tokio::test]
    async fn test_execute_task_with_registered_handler() {
        let executor = create_test_executor().await;
        let registry = Arc::new(HandlerRegistry::new());
        let handler_executor = HandlerExecutor::new(executor.clone(), registry.clone());

        let workflow_id = WorkflowId::default();
        let task_id = TaskId::default();
        let handler_name = "test_handler";

        // Register handler
        registry
            .register_handler(
                workflow_id,
                handler_name.to_string(),
                Arc::new(MockHandler) as Arc<dyn TaskHandler>,
            )
            .await;

        // Initialize cluster
        executor
            .raft()
            .initialize(
                [1u64]
                    .into_iter()
                    .collect::<std::collections::BTreeSet<NodeId>>(),
            )
            .await
            .unwrap();

        // Create workflow first
        let workflow = flow_raft_core::WorkflowSnapshot {
            workflow_id,
            state: flow_raft_core::WorkflowState::Running,
            task_definitions: indexmap::IndexMap::new(),
            executions: indexmap::IndexMap::new(),
            dependencies: indexmap::IndexMap::new(),
            retry_configs: indexmap::IndexMap::new(),
            created_at: chrono::Utc::now(),
            started_at: Some(chrono::Utc::now()),
            completed_at: None,
            inputs: serde_json::json!({}),
            outputs: None,
            error_message: None,
        };

        let request = flow_raft_raft::types::Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        executor.raft().client_write(request).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Execute task
        let result = handler_executor
            .execute_task(workflow_id, task_id, handler_name, serde_json::json!({}))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_task_without_handler() {
        let executor = create_test_executor().await;
        let registry = Arc::new(HandlerRegistry::new());
        let handler_executor = HandlerExecutor::new(executor, registry);

        let workflow_id = WorkflowId::default();
        let task_id = TaskId::default();
        let handler_name = "nonexistent_handler";

        // Try to execute without registering handler
        let result = handler_executor
            .execute_task(workflow_id, task_id, handler_name, serde_json::json!({}))
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            HandlerExecutionError::HandlerNotFound { .. }
        ));
    }

    #[tokio::test]
    async fn test_execute_workflow_distributed_routing_applies_remote_result() {
        use flow_raft_core::{TaskDefinition, TaskDependencies};
        use flow_raft_raft::command::WorkflowCommandBuilder;
        use std::collections::HashSet;

        let executor = create_test_executor().await;
        let registry = Arc::new(HandlerRegistry::new());
        let workflow_id = WorkflowId::default();
        let task_id = TaskId::default();
        let handler_name = "h";

        let mut router = MapTaskRouter::new();
        router.add_route(workflow_id.as_ref(), handler_name, "http://remote:99");

        struct MockCaller;
        #[tonic::async_trait]
        impl RunTaskCaller for MockCaller {
            async fn run_task_on(
                &self,
                _ep: &str,
                _wf: WorkflowId,
                _t: TaskId,
                _name: &str,
                _inp: serde_json::Value,
            ) -> Result<serde_json::Value, String> {
                Ok(serde_json::json!({"ok": true}))
            }
        }

        let mut handler_executor = HandlerExecutor::new(executor.clone(), registry);
        handler_executor = handler_executor.with_distributed_routing(
            Arc::new(router),
            "http://self:98".to_string(),
            Arc::new(MockCaller),
        );

        executor
            .raft()
            .initialize(
                [1u64]
                    .into_iter()
                    .collect::<std::collections::BTreeSet<NodeId>>(),
            )
            .await
            .unwrap();

        let mut task_defs = indexmap::IndexMap::new();
        task_defs.insert(
            task_id,
            TaskDefinition {
                id: task_id,
                name: "t1".to_string(),
                handler: handler_name.to_string(),
                inputs: HashSet::new(),
                outputs: HashSet::new(),
                timeout_secs: None,
            },
        );
        let mut deps = indexmap::IndexMap::new();
        deps.insert(task_id, TaskDependencies::default());

        let workflow = flow_raft_core::WorkflowSnapshot {
            workflow_id,
            state: flow_raft_core::WorkflowState::Running,
            task_definitions: task_defs,
            executions: indexmap::IndexMap::new(),
            dependencies: deps,
            retry_configs: indexmap::IndexMap::new(),
            created_at: chrono::Utc::now(),
            started_at: Some(chrono::Utc::now()),
            completed_at: None,
            inputs: serde_json::json!({}),
            outputs: None,
            error_message: None,
        };
        executor
            .raft()
            .client_write(WorkflowCommandBuilder::create_workflow(workflow))
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

        let result = handler_executor.execute_workflow(workflow_id, 5).await;

        assert!(result.is_ok());
    }
}
