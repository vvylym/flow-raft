//! Helper to run gRPC on a shared-network Raft cluster.
//!
//! Registers workflows and handlers, builds the gRPC service with executor and
//! watcher, and starts the gRPC server bound to the given address.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use flow_raft_api::WorkflowDef;
use flow_raft_api::graph::converter::graph_to_workflow;
use flow_raft_core::WorkflowSnapshot;
use flow_raft_observability::WorkflowWatcher;
use flow_raft_raft::command::WorkflowCommandBuilder;
use flow_raft_raft::executor::WorkflowExecutor;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

use crate::grpc::{FlowRaftServiceImpl, FlowRaftServiceServer};
use crate::handlers::{HandlerExecutor, HandlerRegistry};
use crate::raft_cluster::RaftClusterHandle;

/// Default timeout when waiting for cluster leader.
const DEFAULT_LEADER_TIMEOUT: Duration = Duration::from_secs(10);

/// Errors from [run_grpc_on_cluster].
#[derive(Debug, thiserror::Error)]
pub enum RunGrpcOnClusterError {
    /// No leader elected within timeout
    #[error("no leader elected within {:?}", _0)]
    NoLeader(Duration),
    /// Failure creating a workflow from definition
    #[error("workflow creation failed: {0}")]
    WorkflowCreation(String),
    /// Raft write error when creating workflows
    #[error("raft write failed: {0}")]
    RaftWrite(String),
    /// Bind error
    #[error("bind: {0}")]
    Bind(#[from] std::io::Error),
    /// Server serve error
    #[error("server: {0}")]
    Server(#[from] tonic::transport::Error),
}

/// Runs a gRPC server against the leader of a shared-network Raft cluster.
///
/// Waits for a leader, creates draft workflows in the app for each definition,
/// builds [FlowRaftServiceImpl] with [HandlerExecutor] and [WorkflowWatcher],
/// and starts the gRPC server on `bind_addr`. The handler registry should
/// already contain handlers for the workflows in `workflow_defs`.
///
/// Returns the spawned server task. The server runs until the task is aborted
/// or the process exits.
pub async fn run_grpc_on_cluster(
    handle: &RaftClusterHandle,
    registry: Arc<HandlerRegistry>,
    workflow_defs: Vec<WorkflowDef>,
    bind_addr: SocketAddr,
) -> Result<
    (
        tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
        SocketAddr,
    ),
    RunGrpcOnClusterError,
> {
    let (leader_id, app) = handle
        .wait_for_leader_with_id(DEFAULT_LEADER_TIMEOUT)
        .await
        .ok_or(RunGrpcOnClusterError::NoLeader(DEFAULT_LEADER_TIMEOUT))?;

    for def in &workflow_defs {
        let draft = graph_to_workflow(
            def.graph.clone(),
            def.workflow_id,
            def.default_retry_config.clone(),
            serde_json::json!({}),
        )
        .map_err(|e| RunGrpcOnClusterError::WorkflowCreation(e.to_string()))?;
        let snapshot = WorkflowSnapshot::from_workflow(&draft);
        let request = WorkflowCommandBuilder::create_workflow(snapshot);
        app.create_workflow(request).await.map_err(|e| {
            RunGrpcOnClusterError::RaftWrite(format!("create_workflow failed: {:?}", e))
        })?;
    }

    let raft = app.raft().clone();
    let state_machine = app.state_machine().clone();
    let executor = Arc::new(WorkflowExecutor::new(raft, state_machine, leader_id));
    let watcher = Arc::new(WorkflowWatcher::new());
    let handler_executor = Arc::new(HandlerExecutor::with_watcher(
        executor.clone(),
        registry.clone(),
        watcher.clone(),
    ));

    let service = FlowRaftServiceImpl::with_handler_executor(
        app,
        executor,
        registry,
        handler_executor,
        watcher,
    );

    let listener = TcpListener::bind(bind_addr).await?;
    let actual_addr = listener.local_addr()?;
    let incoming = TcpListenerStream::new(listener);
    let server = Server::builder()
        .add_service(FlowRaftServiceServer::new(service))
        .serve_with_incoming(incoming);

    Ok((tokio::spawn(server), actual_addr))
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use crate::raft_cluster::launch_raft_cluster;

    use super::*;

    #[tokio::test]
    async fn run_grpc_on_cluster_starts_server_with_empty_workflows() {
        let node_ids = [1u64, 2, 3];
        let handle = launch_raft_cluster(&node_ids).await.unwrap();
        let registry = Arc::new(HandlerRegistry::new());
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let result = run_grpc_on_cluster(&handle, registry, vec![], addr).await;
        assert!(result.is_ok(), "{:?}", result.err());
        let (join, _bound_addr) = result.unwrap();
        join.abort();
        let _ = join.await;
    }
}
