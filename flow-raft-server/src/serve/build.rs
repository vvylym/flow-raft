//! Build Raft, app, executor, and gRPC service for NodeServer.

use std::sync::Arc;

use flow_raft_observability::WorkflowWatcher;
use flow_raft_raft::app::FlowRaftApp;
use flow_raft_raft::config::default_config;
use flow_raft_raft::executor::WorkflowExecutor;
use flow_raft_raft::network::{TcpNetworkFactory, TcpRaftRpcServer};
use flow_raft_raft::node::FlowRaftNode;
use flow_raft_raft::storage::{LogStore, StateMachineStore};
use openraft::error::RaftError;

use crate::grpc::FlowRaftServiceImpl;
use crate::handlers::{HandlerExecutor, HandlerRegistry};

use super::ServeConfig;

/// Error building serve components.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// Raft node creation failed.
    #[error("raft: {0}")]
    Raft(#[from] RaftError<u64, openraft::error::Infallible>),
}

/// Built components for [NodeServer::run](super::NodeServer::run).
pub struct ServeComponents {
    /// Raft node (used for bootstrap).
    pub node: FlowRaftNode,
    /// Join handle for the spawned Raft RPC server. Abort on shutdown.
    pub raft_rpc_handle: tokio::task::JoinHandle<std::io::Result<()>>,
    /// gRPC service implementation.
    pub service: FlowRaftServiceImpl,
    /// HTTP port for Prometheus (from config.http_bind).
    pub http_port: u16,
    /// gRPC bind address.
    pub grpc_bind: std::net::SocketAddr,
}

/// Build Raft node, RPC server, app, executor, handler registry, and gRPC service.
///
/// Uses in-memory [LogStore] and [StateMachineStore]. [ServeConfig::data_path] is
/// ignored in 0.2.0.
pub async fn build_components(config: &ServeConfig) -> Result<ServeComponents, BuildError> {
    let log_store = LogStore::default();
    let state_machine = StateMachineStore::default();
    let network = TcpNetworkFactory::new();
    let raft_config = default_config();

    let node = FlowRaftNode::new(
        config.node_id,
        raft_config,
        network,
        log_store,
        state_machine,
    )
    .await?;

    let raft_rpc = TcpRaftRpcServer::new(node.raft.clone(), config.raft_bind);
    let raft_rpc_handle = raft_rpc.spawn();

    let app = Arc::new(FlowRaftApp::new(
        node.raft.clone(),
        node.state_machine.clone(),
    ));
    let executor = Arc::new(WorkflowExecutor::new(
        node.raft.clone(),
        node.state_machine.clone(),
        config.node_id,
    ));
    let registry = Arc::new(HandlerRegistry::new());
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

    Ok(ServeComponents {
        node,
        raft_rpc_handle,
        service,
        http_port: config.http_bind.port(),
        grpc_bind: config.grpc_bind,
    })
}
