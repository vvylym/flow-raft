//! Handler execution integration
//!
//! Integrates handler registry with workflow executor.

use std::sync::Arc;

use crate::core::{TaskId, WorkflowId};
use crate::raft::executor::WorkflowExecutor;
use crate::raft::types::TypeConfig;
use openraft::error::RaftError;
use openraft::RaftTypeConfig;

use super::registry::HandlerRegistry;

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
}

impl HandlerExecutor {
    /// Creates a new handler executor
    pub fn new(executor: Arc<WorkflowExecutor>, registry: Arc<HandlerRegistry>) -> Self {
        Self { executor, registry }
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
    pub async fn execute_workflow(
        &self,
        workflow_id: WorkflowId,
        max_iterations: usize,
    ) -> Result<(), HandlerExecutionError> {
        for iteration in 0..max_iterations {
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
            use crate::core::WorkflowState;
            match &workflow.state {
                WorkflowState::Completed => {
                    println!("✓ Workflow {} completed successfully!", workflow_id);
                    if let Some(outputs) = &workflow.outputs {
                        println!("  Final outputs: {}", serde_json::to_string_pretty(outputs).unwrap_or_default());
                    }
                    return Ok(());
                }
                WorkflowState::Failed { .. } => {
                    println!("✗ Workflow {} failed", workflow_id);
                    if let Some(error) = &workflow.error_message {
                        println!("  Error: {}", error);
                    }
                    return Err(HandlerExecutionError::HandlerNotFound {
                        workflow_id,
                        handler_name: format!("workflow_failed: {}", workflow.error_message.as_deref().unwrap_or("unknown")),
                    });
                }
                WorkflowState::Cancelled => {
                    println!("⚠ Workflow {} was cancelled", workflow_id);
                    return Ok(());
                }
                _ => {
                    // Workflow is still running, continue execution
                }
            }

            // Get ready tasks
            let ready_tasks = Arc::as_ref(&self.executor).get_ready_tasks(&workflow_id).await;

            if ready_tasks.is_empty() {
                // No ready tasks, check if all tasks are complete
                let all_complete = workflow
                    .task_definitions
                    .keys()
                    .all(|task_id| {
                        workflow
                            .executions
                            .get(task_id)
                            .map(|exec| exec.state.is_terminal())
                            .unwrap_or(false)
                    });

                if all_complete {
                    // All tasks complete - transition workflow to completed
                    // Get the workflow as a running workflow and complete it
                    use crate::core::{Workflow, WorkflowRunning};
                    use crate::raft::command::WorkflowCommandBuilder;
                    
                    // Create updated snapshot with completed state
                    let mut completed_snapshot = workflow.clone();
                    completed_snapshot.state = crate::core::WorkflowState::Completed;
                    completed_snapshot.completed_at = Some(chrono::Utc::now());
                    
                    // Collect outputs from completed tasks
                    let mut outputs = serde_json::Map::new();
                    for (task_id, exec) in &workflow.executions {
                        if let Some(task_outputs) = &exec.outputs {
                            if let Some(output) = task_outputs.get("output") {
                                outputs.insert(format!("task_{}", task_id), output.clone());
                            }
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
                    if let Some(updated) = Arc::as_ref(&self.executor).state_machine().get_workflow(&workflow_id).await {
                        if matches!(updated.state, crate::core::WorkflowState::Completed) {
                            println!("✓ Workflow {} completed successfully!", workflow_id);
                            if let Some(outputs) = &updated.outputs {
                                println!("  Final outputs: {}", serde_json::to_string_pretty(outputs).unwrap_or_default());
                            }
                            return Ok(());
                        }
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

                // Get task inputs from workflow inputs and previous task outputs
                let mut task_inputs = workflow.inputs.clone();
                
                // Merge outputs from prerequisite tasks
                if let Some(deps) = workflow.dependencies.get(&task_id) {
                    for prereq_id in &deps.prerequisites {
                        if let Some(prereq_exec) = workflow.executions.get(prereq_id) {
                            if let Some(outputs) = &prereq_exec.outputs {
                                // Merge prerequisite outputs into task inputs
                                if let Some(obj) = task_inputs.as_object_mut() {
                                    if let Some(prereq_obj) = outputs.as_object() {
                                        for (k, v) in prereq_obj {
                                            obj.insert(k.clone(), v.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                println!("  → Executing task: {} (handler: {})", task_id, task_def.handler);

                // Execute task
                match self
                    .execute_task(workflow_id, task_id, &task_def.handler, task_inputs)
                    .await
                {
                    Ok(()) => {
                        // Wait for state update
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                        // Get updated workflow to see task result
                        if let Some(updated_workflow) = Arc::as_ref(&self.executor).state_machine().get_workflow(&workflow_id).await {
                            if let Some(exec) = updated_workflow.executions.get(&task_id) {
                                match &exec.state {
                                    crate::core::TaskState::Completed => {
                                        if let Some(outputs) = &exec.outputs {
                                            println!("    ✓ Task {} completed", task_id);
                                            if let Some(output) = outputs.get("output") {
                                                println!("      Output: {}", serde_json::to_string(output).unwrap_or_default());
                                            }
                                        }
                                    }
                                    crate::core::TaskState::Failed { error_message, .. } => {
                                        println!("    ✗ Task {} failed: {}", task_id, error_message.as_deref().unwrap_or("unknown"));
                                    }
                                    _ => {}
                                }
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
    use crate::raft::config::default_config;
    use crate::raft::executor::TaskHandler;
    use crate::raft::network::MemoryNetworkFactory;
    use crate::raft::storage::{LogStore, StateMachineStore};
    use crate::raft::types::NodeId;
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

        Arc::new(WorkflowExecutor::new(Arc::new(raft), state_machine, node_id))
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
            .initialize([1u64].into_iter().collect::<std::collections::BTreeSet<NodeId>>())
            .await
            .unwrap();

        // Create workflow first
        let workflow = crate::core::WorkflowSnapshot {
            workflow_id,
            state: crate::core::WorkflowState::Running,
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

        let request = crate::raft::types::Request::CreateWorkflow {
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
}
