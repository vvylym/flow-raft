//! gRPC service implementation for FlowRaft
//!
//! Implements all gRPC endpoints for workflow management and observability.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::api::grpc::types::proto::flow_raft_service_server::FlowRaftService;
use crate::api::grpc::types::proto::*;
use crate::api::handlers::HandlerRegistry;
use crate::core::WorkflowId;
use crate::raft::app::FlowRaftApp;
use crate::raft::command::WorkflowCommandBuilder;
use crate::raft::executor::WorkflowExecutor;
use crate::raft::types::Request;

use super::types::{parse_inputs, parse_task_id, parse_workflow_id, task_execution_to_status, workflow_snapshot_to_status};

/// FlowRaft gRPC service implementation
pub struct FlowRaftServiceImpl {
    /// FlowRaft application layer
    app: Arc<FlowRaftApp>,
    /// Workflow executor
    executor: Arc<WorkflowExecutor>,
    /// Handler registry
    registry: Arc<HandlerRegistry>,
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
        }
    }
}

#[tonic::async_trait]
impl FlowRaftService for FlowRaftServiceImpl {
    /// Launches a node
    async fn launch_node(
        &self,
        _request: tonic::Request<LaunchNodeRequest>,
    ) -> Result<tonic::Response<LaunchNodeResponse>, tonic::Status> {
        // Node launching is handled by the CLI/launcher, not via gRPC
        // This endpoint is a placeholder for future remote node management
        Err(tonic::Status::unimplemented("Node launching is handled via CLI"))
    }

    /// Gets node status
    async fn get_node_status(
        &self,
        _request: tonic::Request<GetNodeStatusRequest>,
    ) -> Result<tonic::Response<NodeStatus>, tonic::Status> {
        // TODO: Implement node status retrieval
        Err(tonic::Status::unimplemented("Node status retrieval not yet implemented"))
    }

    /// Defines a workflow
    async fn define_workflow(
        &self,
        request: tonic::Request<DefineWorkflowRequest>,
    ) -> Result<tonic::Response<WorkflowDefinition>, tonic::Status> {
        let req = request.into_inner();
        
        // Parse workflow definition from JSON
        let definition: serde_json::Value = serde_json::from_str(&req.definition)
            .map_err(|e| tonic::Status::invalid_argument(format!("Invalid workflow definition: {}", e)))?;

        // TODO: Convert definition to WorkflowSnapshot and create workflow
        // For now, return a placeholder
        let workflow_id = WorkflowId::default();
        
        Ok(tonic::Response::new(WorkflowDefinition {
            workflow_id: workflow_id.to_string(),
            name: req.name,
            status: "draft".to_string(),
        }))
    }

    /// Triggers a workflow execution
    async fn trigger_workflow(
        &self,
        request: tonic::Request<TriggerWorkflowRequest>,
    ) -> Result<tonic::Response<WorkflowExecution>, tonic::Status> {
        let req = request.into_inner();
        let workflow_id = parse_workflow_id(&req.workflow_id)
            .map_err(|e| tonic::Status::invalid_argument(e))?;
        
        let inputs = parse_inputs(req.inputs)
            .map_err(|e| tonic::Status::invalid_argument(e))?;

        // Get workflow to check if it exists
        let workflow = self.app.get_workflow(&workflow_id).await;
        if workflow.is_none() {
            return Err(tonic::Status::not_found(format!("Workflow {} not found", workflow_id)));
        }

        // TODO: Start workflow execution
        // For now, return a placeholder
        Ok(tonic::Response::new(WorkflowExecution {
            execution_id: uuid::Uuid::new_v4().to_string(),
            workflow_id: workflow_id.to_string(),
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
        let workflow_id = parse_workflow_id(&req.workflow_id)
            .map_err(|e| tonic::Status::invalid_argument(e))?;

        let workflow = self.app.get_workflow(&workflow_id).await
            .ok_or_else(|| tonic::Status::not_found(format!("Workflow {} not found", workflow_id)))?;

        let status = workflow_snapshot_to_status(&workflow);
        Ok(tonic::Response::new(status))
    }

    /// Lists workflows
    async fn list_workflows(
        &self,
        _request: tonic::Request<ListWorkflowsRequest>,
    ) -> Result<tonic::Response<WorkflowList>, tonic::Status> {
        // Get all workflows from app
        let workflows: std::collections::BTreeMap<crate::core::WorkflowId, crate::core::WorkflowSnapshot> = self.app.get_all_workflows().await;
        
        let summaries: Vec<WorkflowSummary> = workflows
            .iter()
            .map(|(id, snapshot)| {
                let status = snapshot.status();
                WorkflowSummary {
                    workflow_id: id.to_string(),
                    state: match &snapshot.state {
                        crate::core::WorkflowState::Draft => "draft",
                        crate::core::WorkflowState::Scheduled => "scheduled",
                        crate::core::WorkflowState::Running => "running",
                        crate::core::WorkflowState::Paused => "paused",
                        crate::core::WorkflowState::Completed => "completed",
                        crate::core::WorkflowState::Failed { .. } => "failed",
                        crate::core::WorkflowState::Cancelled => "cancelled",
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
    type WatchWorkflowStream = tokio_stream::wrappers::ReceiverStream<Result<WorkflowUpdate, tonic::Status>>;

    async fn watch_workflow(
        &self,
        request: tonic::Request<WatchWorkflowRequest>,
    ) -> Result<tonic::Response<Self::WatchWorkflowStream>, tonic::Status> {
        let req = request.into_inner();
        let workflow_id = parse_workflow_id(&req.workflow_id)
            .map_err(|e| tonic::Status::invalid_argument(e))?;

        // TODO: Implement real-time watching using observability service
        // For now, return an empty stream
        let (_tx, rx) = tokio::sync::mpsc::channel(128);
        Ok(tonic::Response::new(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }

    /// Gets execution history
    async fn get_execution_history(
        &self,
        request: tonic::Request<GetExecutionHistoryRequest>,
    ) -> Result<tonic::Response<ExecutionHistory>, tonic::Status> {
        let req = request.into_inner();
        let workflow_id = parse_workflow_id(&req.workflow_id)
            .map_err(|e| tonic::Status::invalid_argument(e))?;

        // TODO: Implement execution history retrieval
        // For now, return empty history
        Ok(tonic::Response::new(ExecutionHistory {
            workflow_id: workflow_id.to_string(),
            events: vec![],
        }))
    }

    /// Gets task results
    async fn get_task_results(
        &self,
        request: tonic::Request<GetTaskResultsRequest>,
    ) -> Result<tonic::Response<TaskResults>, tonic::Status> {
        let req = request.into_inner();
        let workflow_id = parse_workflow_id(&req.workflow_id)
            .map_err(|e| tonic::Status::invalid_argument(e))?;
        let task_id = parse_task_id(&req.task_id)
            .map_err(|e| tonic::Status::invalid_argument(e))?;

        let workflow = self.app.get_workflow(&workflow_id).await
            .ok_or_else(|| tonic::Status::not_found(format!("Workflow {} not found", workflow_id)))?;

        let execution = workflow.executions.get(&task_id)
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
}
