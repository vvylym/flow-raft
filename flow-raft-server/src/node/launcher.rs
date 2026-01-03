//! Node launcher for FlowRaft
//!
//! Provides functions to launch nodes in leader or follower mode.

use std::collections::BTreeSet;
use std::sync::Arc;

use flow_raft_observability::MetricsCollector;
use flow_raft_observability::PrometheusExporter;
// RaftConfig is accessed via NodeConfig::raft_config, not directly imported
use flow_raft_raft::network::MemoryNetworkFactory;
use flow_raft_raft::node::FlowRaftNode;
use flow_raft_raft::storage::{LogStore, StateMachineStore};
use flow_raft_raft::types::NodeId;
use tracing_subscriber::EnvFilter;

use super::config::{NodeConfig, NodeMode};
use crate::cluster::{ClusterNode, NodeRole};

// RaftConfig is used via NodeConfig::raft_config field

/// Error type for node launching
#[derive(Debug, thiserror::Error)]
pub enum NodeLaunchError {
    /// Raft error
    #[error("Raft error: {0}")]
    RaftError(openraft::error::RaftError<NodeId, openraft::error::Infallible>),
    /// Initialization error
    #[error("Initialization error: {0}")]
    InitializeError(
        openraft::error::RaftError<
            NodeId,
            openraft::error::InitializeError<NodeId, openraft::BasicNode>,
        >,
    ),
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),
}

/// Initialize tracing with environment-based filtering
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}

/// Launches a single node (leader)
///
/// # Arguments
/// * `config` - Node configuration
///
/// # Returns
/// The launched cluster node
pub async fn launch_single_node(config: NodeConfig) -> Result<ClusterNode, NodeLaunchError> {
    init_tracing();

    let raft_config = config.raft_config.clone();
    let node_id = config.node_id;

    ClusterNode::new_leader(node_id, raft_config)
        .await
        .map_err(|e| NodeLaunchError::Config(format!("Failed to create leader: {}", e)))
}

/// Launches a single node with workflows using builder pattern
///
/// # Arguments
/// * `node_id` - Node ID
/// * `workflows` - Workflows to register
///
/// # Returns
/// The FlowRaftApp instance
pub async fn launch_single_node_with_workflows(
    node_id: NodeId,
    workflows: Vec<flow_raft_api::WorkflowDef>,
) -> Result<flow_raft_raft::FlowRaftApp, NodeLaunchError> {
    use flow_raft_raft::FlowRaftApp;

    FlowRaftApp::builder()
        .with_node_id(node_id)
        .with_workflows(workflows)
        .enable_metrics(true)
        .build_single_node()
        .await
        .map_err(|e| NodeLaunchError::Config(format!("Failed to build app: {}", e)))
}

/// Launches a cluster node (leader or follower)
///
/// # Arguments
/// * `config` - Node configuration
/// * `role` - Node role (Leader or Follower)
/// * `leader_address` - Leader address (for followers)
///
/// # Returns
/// The launched cluster node
pub async fn launch_cluster_node(
    config: NodeConfig,
    role: NodeRole,
    leader_address: Option<String>,
) -> Result<ClusterNode, NodeLaunchError> {
    init_tracing();

    let raft_config = config.raft_config.clone();
    let node_id = config.node_id;

    match role {
        NodeRole::Leader => ClusterNode::new_leader(node_id, raft_config)
            .await
            .map_err(|e| NodeLaunchError::Config(format!("Failed to create leader: {}", e))),
        NodeRole::Follower => {
            let leader_addr = leader_address.ok_or_else(|| {
                NodeLaunchError::Config("Leader address required for follower".to_string())
            })?;
            ClusterNode::join_cluster(node_id, raft_config, leader_addr)
                .await
                .map_err(|e| NodeLaunchError::Config(format!("Failed to join cluster: {}", e)))
        }
    }
}

/// Launches a cluster with multiple nodes using builder pattern
///
/// # Arguments
/// * `nodes` - Vector of (node_id, mode, workflows) tuples
///
/// # Returns
/// Vector of cluster nodes
pub async fn launch_cluster(
    nodes: Vec<(NodeId, NodeMode, Vec<flow_raft_api::WorkflowDef>)>,
) -> Result<Vec<ClusterNode>, NodeLaunchError> {
    let mut cluster_nodes = Vec::with_capacity(nodes.len());
    let mut leader_id: Option<NodeId> = None;

    // First, identify the leader
    for (node_id, mode, _) in &nodes {
        if matches!(mode, NodeMode::Leader) {
            if leader_id.is_some() {
                return Err(NodeLaunchError::Config(
                    "Multiple leaders specified".to_string(),
                ));
            }
            leader_id = Some(*node_id);
        }
    }

    // Create all nodes using builder pattern for consistency
    for (node_id, mode, workflows) in nodes {
        let role = match mode {
            NodeMode::Leader => NodeRole::Leader,
            NodeMode::Follower => NodeRole::Follower,
            NodeMode::Auto => {
                // Auto mode: first node becomes leader, others become followers
                if leader_id.is_none() {
                    let new_leader_id = node_id;
                    leader_id = Some(new_leader_id);
                    NodeRole::Leader
                } else {
                    NodeRole::Follower
                }
            }
        };

        let leader_address = leader_id.and_then(|lid| {
            if lid != node_id {
                Some(format!("http://node{}:8080", lid))
            } else {
                None
            }
        });

        let config = flow_raft_raft::config::default_config();
        let cluster_node = match role {
            NodeRole::Leader => ClusterNode::new_leader(node_id, config)
                .await
                .map_err(|e| NodeLaunchError::Config(format!("Failed to create leader: {}", e)))?,
            NodeRole::Follower => {
                let leader_addr = leader_address.ok_or_else(|| {
                    NodeLaunchError::Config("Leader address required for follower".to_string())
                })?;
                ClusterNode::join_cluster(node_id, config, leader_addr)
                    .await
                    .map_err(|e| {
                        NodeLaunchError::Config(format!("Failed to join cluster: {}", e))
                    })?
            }
        };

        // Register workflows using the cluster node's registration method
        for workflow_def in workflows {
            cluster_node
                .register_workflow(workflow_def)
                .await
                .map_err(|e| {
                    NodeLaunchError::Config(format!("Failed to register workflow: {}", e))
                })?;
        }

        cluster_nodes.push(cluster_node);
    }

    Ok(cluster_nodes)
}

/// Start metrics server
///
/// # Arguments
/// * `port` - Port for metrics endpoint (default: 9090)
/// * `metrics_collector` - Metrics collector instance
///
/// # Returns
/// Join handle for the metrics server task
pub async fn start_metrics_server(
    port: u16,
    metrics_collector: Arc<MetricsCollector>,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error + Send + Sync>> {
    let exporter = PrometheusExporter::new(port, metrics_collector)?;
    exporter.start_server().await
}

/// Launches a leader node
///
/// # Arguments
/// * `config` - Node configuration
/// * `network` - Network factory (shared across nodes)
///
/// # Returns
/// The launched node
pub async fn launch_leader(
    config: NodeConfig,
    network: MemoryNetworkFactory,
) -> Result<FlowRaftNode, NodeLaunchError> {
    let log_store = LogStore::default();
    let state_machine = StateMachineStore::default();
    let node_id = config.node_id;
    let raft_config = config.raft_config.clone();

    let node = FlowRaftNode::new(node_id, raft_config, network, log_store, state_machine)
        .await
        .map_err(NodeLaunchError::RaftError)?;

    node.initialize_single_node()
        .await
        .map_err(NodeLaunchError::InitializeError)?;

    Ok(node)
}

/// Launches a follower node
///
/// # Arguments
/// * `config` - Node configuration
/// * `network` - Network factory (shared across nodes)
/// * `cluster_nodes` - Set of node IDs in the cluster
///
/// # Returns
/// The launched node
pub async fn launch_follower(
    config: NodeConfig,
    network: MemoryNetworkFactory,
    _cluster_nodes: BTreeSet<NodeId>,
) -> Result<FlowRaftNode, NodeLaunchError> {
    let log_store = LogStore::default();
    let state_machine = StateMachineStore::default();
    let node_id = config.node_id;
    let raft_config = config.raft_config.clone();

    let node = FlowRaftNode::new(node_id, raft_config, network, log_store, state_machine)
        .await
        .map_err(NodeLaunchError::RaftError)?;

    // Note: This function creates a follower node but does not join it to a cluster.
    // For cluster join functionality, use `join_cluster` or `launch_cluster_node`
    // with `NodeRole::Follower` instead. This function is kept for backward
    // compatibility but may be deprecated in favor of the cluster
    // join methods.

    Ok(node)
}

/// Joins a cluster
///
/// # Arguments
/// * `config` - Node configuration
/// * `network` - Network factory (shared across nodes)
/// * `cluster_nodes` - Set of node IDs in the cluster
///
/// # Returns
/// The launched node
pub async fn join_cluster(
    config: NodeConfig,
    network: MemoryNetworkFactory,
    cluster_nodes: BTreeSet<NodeId>,
) -> Result<FlowRaftNode, NodeLaunchError> {
    let log_store = LogStore::default();
    let state_machine = StateMachineStore::default();
    let node_id = config.node_id;
    let raft_config = config.raft_config.clone();

    let node = FlowRaftNode::new(node_id, raft_config, network, log_store, state_machine)
        .await
        .map_err(NodeLaunchError::RaftError)?;

    node.initialize_cluster(cluster_nodes)
        .await
        .map_err(NodeLaunchError::InitializeError)?;

    Ok(node)
}
