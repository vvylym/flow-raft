//! gRPC service implementation for FlowRaft
//!
//! Implements all gRPC endpoints for workflow management and observability.

use std::sync::Arc;

use crate::handlers::{HandlerExecutor, HandlerRegistry};
use flow_raft_api::graph::graph_to_workflow;
use flow_raft_api::workflow::parse_workflow_from_json;
use flow_raft_core::{WorkflowId, WorkflowSnapshot};
use flow_raft_observability::{HistoryStore, WorkflowWatcher};
use flow_raft_proto::proto::flow_raft_service_server::FlowRaftService;
use flow_raft_proto::proto::*;
use flow_raft_raft::app::FlowRaftApp;
#[allow(unused_imports)] // Used in trigger_workflow
use flow_raft_raft::command::WorkflowCommandBuilder;
use flow_raft_raft::executor::WorkflowExecutor;
use std::sync::Arc as StdArc;

use super::types::{
    parse_inputs, parse_task_id, parse_workflow_id, task_execution_to_status,
    workflow_snapshot_to_status,
};

/// Default maximum iterations for workflow execution when triggered via gRPC.
const DEFAULT_TRIGGER_MAX_ITERATIONS: usize = 10_000;

/// FlowRaft gRPC service implementation
pub struct FlowRaftServiceImpl {
    /// FlowRaft application layer
    app: Arc<FlowRaftApp>,
    /// Workflow executor (reserved for future task execution endpoints)
    ///
    /// # Note
    /// This field is reserved for future gRPC endpoints that will allow
    /// direct task execution management, task status queries, and execution control.
    #[allow(dead_code)] // Reserved for future task execution endpoints
    executor: Arc<WorkflowExecutor>,
    /// Handler registry (reserved for future handler management endpoints)
    ///
    /// # Note
    /// This field is reserved for future gRPC endpoints that will allow
    /// dynamic handler registration, handler discovery, and handler management.
    #[allow(dead_code)] // Reserved for future handler management endpoints
    registry: Arc<HandlerRegistry>,
    /// When set, trigger_workflow spawns this executor so the task loop runs.
    handler_executor: Option<Arc<HandlerExecutor>>,
    /// History store for execution history
    history_store: Option<StdArc<HistoryStore>>,
    /// Workflow watcher for real-time event streaming
    watcher: Option<Arc<WorkflowWatcher>>,
}

impl FlowRaftServiceImpl {
    /// Creates a new gRPC service implementation
    pub fn new(
        app: Arc<FlowRaftApp>,
        executor: Arc<WorkflowExecutor>,
        registry: Arc<HandlerRegistry>,
    ) -> Self {
        Self {
            app,
            executor,
            registry,
            handler_executor: None,
            history_store: None,
            watcher: None,
        }
    }

    /// Creates a new gRPC service implementation with workflow watcher
    pub fn with_watcher(
        app: Arc<FlowRaftApp>,
        executor: Arc<WorkflowExecutor>,
        registry: Arc<HandlerRegistry>,
        watcher: Arc<WorkflowWatcher>,
    ) -> Self {
        Self {
            app,
            executor,
            registry,
            handler_executor: None,
            history_store: None,
            watcher: Some(watcher),
        }
    }

    /// Creates a gRPC service that runs the handler executor when a workflow is triggered.
    /// Use this when the service should drive task execution: after transitioning the
    /// workflow to Running, the executor loop is spawned so tasks run and watch_workflow
    /// receives events.
    pub fn with_handler_executor(
        app: Arc<FlowRaftApp>,
        executor: Arc<WorkflowExecutor>,
        registry: Arc<HandlerRegistry>,
        handler_executor: Arc<HandlerExecutor>,
        watcher: Arc<WorkflowWatcher>,
    ) -> Self {
        Self {
            app,
            executor,
            registry,
            handler_executor: Some(handler_executor),
            history_store: None,
            watcher: Some(watcher),
        }
    }

    /// Sets the workflow watcher
    pub fn set_watcher(&mut self, watcher: Arc<WorkflowWatcher>) {
        self.watcher = Some(watcher);
    }

    /// Creates a new gRPC service implementation with history store
    pub fn with_history_store(
        app: Arc<FlowRaftApp>,
        executor: Arc<WorkflowExecutor>,
        registry: Arc<HandlerRegistry>,
        history_store: StdArc<HistoryStore>,
    ) -> Self {
        Self {
            app,
            executor,
            registry,
            handler_executor: None,
            history_store: Some(history_store),
            watcher: None,
        }
    }

    /// Sets the history store
    pub fn set_history_store(&mut self, history_store: StdArc<HistoryStore>) {
        self.history_store = Some(history_store);
    }
}

#[tonic::async_trait]
impl FlowRaftService for FlowRaftServiceImpl {
    /// Gets node status
    async fn get_node_status(
        &self,
        _request: tonic::Request<GetNodeStatusRequest>,
    ) -> Result<tonic::Response<NodeStatus>, tonic::Status> {
        // Node status can be derived from Raft metrics
        // For now, return basic status
        Ok(tonic::Response::new(NodeStatus {
            node_id: 1,
            mode: "leader".to_string(),
            is_leader: true,
            cluster_nodes: vec![1],
        }))
    }

    /// Defines a workflow by parsing the JSON payload, persisting it in the Raft-backed
    /// app, and returning a stable workflow_id for use in trigger_workflow / trigger_workflow_by_id.
    ///
    /// The JSON format must match what [FlowRaftClient::submit_workflow](flow_raft_api::client::FlowRaftClient::submit_workflow)
    /// sends: `{ "name", "graph": { "name", "nodes", "edges", "root" }, "default_retry_config": { ... } }`.
    async fn define_workflow(
        &self,
        request: tonic::Request<DefineWorkflowRequest>,
    ) -> Result<tonic::Response<WorkflowDefinition>, tonic::Status> {
        let req = request.into_inner();

        let parsed = parse_workflow_from_json(&req.definition).map_err(|e| {
            tonic::Status::invalid_argument(format!("define_workflow parse error: {}", e))
        })?;

        let workflow_id = WorkflowId::default();

        let draft = graph_to_workflow(
            parsed.graph,
            workflow_id,
            parsed.default_retry_config.clone(),
            serde_json::json!({}),
        )
        .map_err(|e| {
            tonic::Status::invalid_argument(format!(
                "define_workflow graph_to_workflow error: {}",
                e
            ))
        })?;

        let snapshot = WorkflowSnapshot::from_workflow(&draft);
        let create_request = WorkflowCommandBuilder::create_workflow(snapshot);

        self.app
            .create_workflow(create_request)
            .await
            .map_err(|e| {
                tonic::Status::internal(format!("define_workflow create_workflow failed: {:?}", e))
            })?;

        Ok(tonic::Response::new(WorkflowDefinition {
            workflow_id: workflow_id.as_ref().to_string(),
            name: parsed.name,
            status: "draft".to_string(),
        }))
    }

    /// Triggers a workflow execution
    #[tracing::instrument(level = "info", skip(self))]
    async fn trigger_workflow(
        &self,
        request: tonic::Request<TriggerWorkflowRequest>,
    ) -> Result<tonic::Response<WorkflowExecution>, tonic::Status> {
        let req = request.into_inner();
        let workflow_id =
            parse_workflow_id(&req.workflow_id).map_err(tonic::Status::invalid_argument)?;

        let inputs = parse_inputs(req.inputs).map_err(tonic::Status::invalid_argument)?;

        // Get workflow to check if it exists
        let workflow = self.app.get_workflow(&workflow_id).await;
        let Some(workflow_snapshot) = workflow else {
            return Err(tonic::Status::not_found(format!(
                "Workflow {} not found",
                workflow_id
            )));
        };

        // If workflow is not running, we need to start it
        // For workflows that are in Draft or Scheduled state, transition to Running
        use flow_raft_core::WorkflowState;
        let execution_id = workflow_id;

        match workflow_snapshot.state {
            WorkflowState::Draft | WorkflowState::Scheduled => {
                // Transition to running state
                use flow_raft_raft::command::WorkflowCommandBuilder;
                let mut updated = workflow_snapshot.clone();
                updated.state = WorkflowState::Running;
                updated.started_at = Some(chrono::Utc::now());
                updated.inputs = inputs.clone();

                let request = WorkflowCommandBuilder::transition_workflow(workflow_id, updated);
                if let Err(e) = self.app.create_workflow(request).await {
                    return Err(tonic::Status::internal(format!(
                        "Failed to start workflow: {:?}",
                        e
                    )));
                }
            }
            WorkflowState::Running => {
                // Already running, no action needed
            }
            _ => {
                return Err(tonic::Status::failed_precondition(format!(
                    "Workflow {} is in state {:?} and cannot be started",
                    workflow_id, workflow_snapshot.state
                )));
            }
        }

        // Spawn handler executor so the task loop runs and watcher receives events
        if let Some(he) = &self.handler_executor {
            let he = he.clone();
            let wf_id = workflow_id;
            tokio::spawn(async move {
                let _ = he
                    .execute_workflow(wf_id, DEFAULT_TRIGGER_MAX_ITERATIONS)
                    .await;
            });
        }

        Ok(tonic::Response::new(WorkflowExecution {
            execution_id: execution_id.as_ref().to_string(),
            workflow_id: workflow_id.as_ref().to_string(),
            status: "running".to_string(),
            error: None,
        }))
    }

    /// Gets workflow status
    async fn get_workflow(
        &self,
        request: tonic::Request<GetWorkflowRequest>,
    ) -> Result<tonic::Response<WorkflowStatus>, tonic::Status> {
        let req = request.into_inner();
        let workflow_id =
            parse_workflow_id(&req.workflow_id).map_err(tonic::Status::invalid_argument)?;

        let workflow = self.app.get_workflow(&workflow_id).await.ok_or_else(|| {
            tonic::Status::not_found(format!("Workflow {} not found", workflow_id))
        })?;

        let status = workflow_snapshot_to_status(&workflow);
        Ok(tonic::Response::new(status))
    }

    /// Lists workflows
    async fn list_workflows(
        &self,
        _request: tonic::Request<ListWorkflowsRequest>,
    ) -> Result<tonic::Response<WorkflowList>, tonic::Status> {
        // Get all workflows from app
        let workflows: std::collections::BTreeMap<
            flow_raft_core::WorkflowId,
            flow_raft_core::WorkflowSnapshot,
        > = self.app.get_all_workflows().await;

        let summaries: Vec<WorkflowSummary> = workflows
            .iter()
            .map(|(id, snapshot)| {
                let status = snapshot.status();
                WorkflowSummary {
                    workflow_id: id.as_ref().to_string(),
                    state: match &snapshot.state {
                        flow_raft_core::WorkflowState::Draft => "draft",
                        flow_raft_core::WorkflowState::Scheduled => "scheduled",
                        flow_raft_core::WorkflowState::Running => "running",
                        flow_raft_core::WorkflowState::Paused => "paused",
                        flow_raft_core::WorkflowState::Completed => "completed",
                        flow_raft_core::WorkflowState::Failed { .. } => "failed",
                        flow_raft_core::WorkflowState::Cancelled => "cancelled",
                    }
                    .to_string(),
                    total_tasks: status.total_tasks as i32,
                    completed_tasks: status.completed as i32,
                    failed_tasks: status.failed as i32,
                    created_at: snapshot.created_at.timestamp(),
                }
            })
            .collect();

        Ok(tonic::Response::new(WorkflowList {
            workflows: summaries,
            total: workflows.len() as i32,
        }))
    }

    /// Watches workflow for real-time updates
    type WatchWorkflowStream =
        tokio_stream::wrappers::ReceiverStream<Result<WorkflowUpdate, tonic::Status>>;

    #[tracing::instrument(level = "info", skip(self))]
    async fn watch_workflow(
        &self,
        request: tonic::Request<WatchWorkflowRequest>,
    ) -> Result<tonic::Response<Self::WatchWorkflowStream>, tonic::Status> {
        let req = request.into_inner();
        let workflow_id =
            parse_workflow_id(&req.workflow_id).map_err(tonic::Status::invalid_argument)?;

        // Get watcher or return error
        let watcher = self
            .watcher
            .as_ref()
            .ok_or_else(|| tonic::Status::unavailable("Workflow watcher not configured"))?;

        // Subscribe to workflow updates
        let mut receiver = watcher.watch_workflow(workflow_id).await;

        // Create channel for streaming
        let (tx, rx) = tokio::sync::mpsc::channel(128);

        // Spawn task to forward updates from watcher to gRPC stream
        tokio::spawn(async move {
            while let Ok(update) = receiver.recv().await {
                let proto_update = WorkflowUpdate {
                    workflow_id: update.workflow_id.as_ref().to_string(),
                    event_type: update.event_type,
                    data: update.data,
                    timestamp: update.timestamp.timestamp(),
                };

                if tx.send(Ok(proto_update)).await.is_err() {
                    // Receiver dropped, stop forwarding
                    break;
                }
            }
        });

        Ok(tonic::Response::new(
            tokio_stream::wrappers::ReceiverStream::new(rx),
        ))
    }

    /// Gets execution history
    async fn get_execution_history(
        &self,
        request: tonic::Request<GetExecutionHistoryRequest>,
    ) -> Result<tonic::Response<ExecutionHistory>, tonic::Status> {
        let req = request.into_inner();
        let workflow_id =
            parse_workflow_id(&req.workflow_id).map_err(tonic::Status::invalid_argument)?;

        // Get history from history store if available
        if let Some(history_store) = &self.history_store
            && let Some(history) = history_store.get_history(&workflow_id, None).await
        {
            let events: Vec<ExecutionEvent> = history
                .events
                .iter()
                .map(|event| ExecutionEvent {
                    event_type: match event.event_type {
                        flow_raft_observability::ExecutionEventType::WorkflowStateChange => {
                            "workflow_state_change".to_string()
                        }
                        flow_raft_observability::ExecutionEventType::TaskStarted => {
                            "task_started".to_string()
                        }
                        flow_raft_observability::ExecutionEventType::TaskCompleted => {
                            "task_completed".to_string()
                        }
                        flow_raft_observability::ExecutionEventType::TaskFailed => {
                            "task_failed".to_string()
                        }
                        flow_raft_observability::ExecutionEventType::TaskCancelled => {
                            "task_cancelled".to_string()
                        }
                    },
                    task_id: event.task_id.map(|id| id.as_ref().to_string()),
                    data: event.data.clone(),
                    timestamp: event.timestamp.timestamp(),
                })
                .collect();

            return Ok(tonic::Response::new(ExecutionHistory {
                workflow_id: workflow_id.as_ref().to_string(),
                events,
            }));
        }

        // If no history store or no history found, return empty history
        Ok(tonic::Response::new(ExecutionHistory {
            workflow_id: workflow_id.as_ref().to_string(),
            events: vec![],
        }))
    }

    /// Gets task results
    async fn get_task_results(
        &self,
        request: tonic::Request<GetTaskResultsRequest>,
    ) -> Result<tonic::Response<TaskResults>, tonic::Status> {
        let req = request.into_inner();
        let workflow_id =
            parse_workflow_id(&req.workflow_id).map_err(tonic::Status::invalid_argument)?;
        let task_id = parse_task_id(&req.task_id).map_err(tonic::Status::invalid_argument)?;

        let workflow = self.app.get_workflow(&workflow_id).await.ok_or_else(|| {
            tonic::Status::not_found(format!("Workflow {} not found", workflow_id))
        })?;

        let execution = workflow
            .executions
            .get(&task_id)
            .ok_or_else(|| tonic::Status::not_found(format!("Task {} not found", task_id)))?;

        let status = task_execution_to_status(task_id, execution);

        Ok(tonic::Response::new(TaskResults {
            task_id: task_id.to_string(),
            state: status.state,
            outputs: status.outputs,
            error: status.error,
            attempts: status.attempts,
            started_at: status.started_at,
            completed_at: status.completed_at,
        }))
    }

    /// Runs a single task on this node (for distributed execution).
    /// The caller (orchestrator/leader) is responsible for applying the task result to the Raft state.
    async fn run_task(
        &self,
        request: tonic::Request<RunTaskRequest>,
    ) -> Result<tonic::Response<RunTaskResponse>, tonic::Status> {
        let req = request.into_inner();
        let workflow_id =
            parse_workflow_id(&req.workflow_id).map_err(tonic::Status::invalid_argument)?;
        let task_id = parse_task_id(&req.task_id).map_err(tonic::Status::invalid_argument)?;
        let inputs = parse_inputs(req.inputs).map_err(tonic::Status::invalid_argument)?;

        let handler = self
            .registry
            .get_handler(&workflow_id, &req.handler_name)
            .await;
        let Some(handler) = handler else {
            return Ok(tonic::Response::new(RunTaskResponse {
                outputs: None,
                error: Some(format!(
                    "handler not found: workflow_id={} handler_name={}",
                    workflow_id, req.handler_name
                )),
            }));
        };

        match handler.execute(task_id, inputs) {
            Ok(outputs) => Ok(tonic::Response::new(RunTaskResponse {
                outputs: Some(serde_json::to_string(&outputs).unwrap_or_else(|_| "{}".to_string())),
                error: None,
            })),
            Err(e) => Ok(tonic::Response::new(RunTaskResponse {
                outputs: None,
                error: Some(e),
            })),
        }
    }

    /// Pauses a running workflow
    async fn pause_workflow(
        &self,
        request: tonic::Request<PauseWorkflowRequest>,
    ) -> Result<tonic::Response<WorkflowStatus>, tonic::Status> {
        let req = request.into_inner();
        let workflow_id =
            parse_workflow_id(&req.workflow_id).map_err(tonic::Status::invalid_argument)?;

        let workflow = self.app.get_workflow(&workflow_id).await.ok_or_else(|| {
            tonic::Status::not_found(format!("Workflow {} not found", workflow_id))
        })?;

        // Check if workflow is in Running state
        use flow_raft_core::WorkflowState;
        if !matches!(workflow.state, WorkflowState::Running) {
            return Err(tonic::Status::failed_precondition(format!(
                "Workflow {} is in state {:?} and cannot be paused. Only running workflows can be paused.",
                workflow_id, workflow.state
            )));
        }

        // Transition to Paused state
        let mut updated = workflow.clone();
        updated.state = WorkflowState::Paused;

        let request = WorkflowCommandBuilder::transition_workflow(workflow_id, updated);
        self.app
            .create_workflow(request)
            .await
            .map_err(|e| tonic::Status::internal(format!("Failed to pause workflow: {:?}", e)))?;

        // Get updated workflow and return status
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let updated_workflow = self.app.get_workflow(&workflow_id).await.ok_or_else(|| {
            tonic::Status::not_found(format!("Workflow {} not found after pause", workflow_id))
        })?;

        let status = workflow_snapshot_to_status(&updated_workflow);
        Ok(tonic::Response::new(status))
    }

    /// Resumes a paused workflow
    async fn resume_workflow(
        &self,
        request: tonic::Request<ResumeWorkflowRequest>,
    ) -> Result<tonic::Response<WorkflowStatus>, tonic::Status> {
        let req = request.into_inner();
        let workflow_id =
            parse_workflow_id(&req.workflow_id).map_err(tonic::Status::invalid_argument)?;

        let workflow = self.app.get_workflow(&workflow_id).await.ok_or_else(|| {
            tonic::Status::not_found(format!("Workflow {} not found", workflow_id))
        })?;

        // Check if workflow is in Paused state
        use flow_raft_core::WorkflowState;
        if !matches!(workflow.state, WorkflowState::Paused) {
            return Err(tonic::Status::failed_precondition(format!(
                "Workflow {} is in state {:?} and cannot be resumed. Only paused workflows can be resumed.",
                workflow_id, workflow.state
            )));
        }

        // Transition to Running state
        let mut updated = workflow.clone();
        updated.state = WorkflowState::Running;

        let request = WorkflowCommandBuilder::transition_workflow(workflow_id, updated);
        self.app
            .create_workflow(request)
            .await
            .map_err(|e| tonic::Status::internal(format!("Failed to resume workflow: {:?}", e)))?;

        // Get updated workflow and return status
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let updated_workflow = self.app.get_workflow(&workflow_id).await.ok_or_else(|| {
            tonic::Status::not_found(format!("Workflow {} not found after resume", workflow_id))
        })?;

        let status = workflow_snapshot_to_status(&updated_workflow);
        Ok(tonic::Response::new(status))
    }

    /// Cancels a workflow (from any state)
    async fn cancel_workflow(
        &self,
        request: tonic::Request<CancelWorkflowRequest>,
    ) -> Result<tonic::Response<WorkflowStatus>, tonic::Status> {
        let req = request.into_inner();
        let workflow_id =
            parse_workflow_id(&req.workflow_id).map_err(tonic::Status::invalid_argument)?;

        let workflow = self.app.get_workflow(&workflow_id).await.ok_or_else(|| {
            tonic::Status::not_found(format!("Workflow {} not found", workflow_id))
        })?;

        // Check if workflow is already in a terminal state
        use flow_raft_core::WorkflowState;
        if matches!(
            workflow.state,
            WorkflowState::Completed | WorkflowState::Cancelled
        ) {
            // Already in terminal state, return current status
            let status = workflow_snapshot_to_status(&workflow);
            return Ok(tonic::Response::new(status));
        }

        // Transition to Cancelled state
        let mut updated = workflow.clone();
        updated.state = WorkflowState::Cancelled;
        updated.completed_at = Some(chrono::Utc::now());

        let request = WorkflowCommandBuilder::transition_workflow(workflow_id, updated);
        self.app
            .create_workflow(request)
            .await
            .map_err(|e| tonic::Status::internal(format!("Failed to cancel workflow: {:?}", e)))?;

        // Get updated workflow and return status
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let updated_workflow = self.app.get_workflow(&workflow_id).await.ok_or_else(|| {
            tonic::Status::not_found(format!("Workflow {} not found after cancel", workflow_id))
        })?;

        let status = workflow_snapshot_to_status(&updated_workflow);
        Ok(tonic::Response::new(status))
    }
}
