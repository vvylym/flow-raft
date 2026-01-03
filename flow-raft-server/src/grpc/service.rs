//! gRPC service implementation for FlowRaft
//!
//! Implements all gRPC endpoints for workflow management and observability.

use std::sync::Arc;

use crate::handlers::HandlerRegistry;
use flow_raft_core::WorkflowId;
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
    /// Launches a node
    ///
    /// # Note
    /// This endpoint is currently unimplemented. Node launching is handled via
    /// the CLI interface (`launch_single_node`, `launch_cluster_node`) or
    /// programmatically using `ClusterNode::new_leader` or `ClusterNode::join_cluster`.
    /// Remote node management via gRPC may be implemented in a future release.
    async fn launch_node(
        &self,
        _request: tonic::Request<LaunchNodeRequest>,
    ) -> Result<tonic::Response<LaunchNodeResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Node launching is handled via CLI or programmatic API. See launch_single_node() or launch_cluster_node()",
        ))
    }

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

    /// Defines a workflow
    ///
    /// # Note
    /// This is a partial implementation. Currently, it creates a workflow ID and returns
    /// a basic workflow definition. A full implementation would:
    /// 1. Parse the JSON definition into a `Graph` structure
    /// 2. Convert graph to `WorkflowSnapshot` using `graph_to_workflow`
    /// 3. Register the workflow via `self.app.create_workflow()`
    ///
    /// For now, workflows should be defined programmatically using `GraphBuilder` and
    /// registered via `ClusterNode::register_workflow` or `FlowRaftApp::register_workflow`.
    async fn define_workflow(
        &self,
        request: tonic::Request<DefineWorkflowRequest>,
    ) -> Result<tonic::Response<WorkflowDefinition>, tonic::Status> {
        let req = request.into_inner();

        // Partial implementation: create workflow ID and return basic definition
        // Full implementation would parse JSON and register workflow via Raft
        let workflow_id = WorkflowId::default();

        Ok(tonic::Response::new(WorkflowDefinition {
            workflow_id: workflow_id.as_ref().to_string(),
            name: req.name,
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
