//! Node launcher for FlowRaft
//!
//! Provides functions to launch nodes in leader or follower mode.

use std::collections::BTreeSet;

use crate::raft::network::MemoryNetworkFactory;
use crate::raft::node::FlowRaftNode;
use crate::raft::storage::{LogStore, StateMachineStore};
use crate::raft::types::NodeId;

// Note: FlowRaftNode doesn't expose network field, so we need to handle it differently
// For now, we'll create nodes directly

use super::config::{NodeConfig, NodeMode};

/// Error type for node launching
#[derive(Debug, thiserror::Error)]
pub enum NodeLaunchError {
    /// Raft error
    #[error("Raft error: {0}")]
    RaftError(
        openraft::error::RaftError<
            NodeId,
            openraft::error::Infallible,
        >,
    ),
    /// Initialization error
    #[error("Initialization error: {0}")]
    InitializeError(
        openraft::error::RaftError<
            NodeId,
            openraft::error::InitializeError<NodeId, openraft::BasicNode>,
        >,
    ),
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

    let node = FlowRaftNode::new(
        config.node_id,
        config.raft_config,
        network,
        log_store,
        state_machine,
    )
    .await
    .map_err(NodeLaunchError::RaftError)?;

    // Initialize as single-node cluster if in leader mode
    if config.mode == NodeMode::Leader {
        node.initialize_single_node()
            .await
            .map_err(NodeLaunchError::InitializeError)?;
    }

    Ok(node)
}

/// Launches a follower node
///
/// # Arguments
/// * `config` - Node configuration
/// * `network` - Network factory (shared across nodes)
/// * `cluster_nodes` - Set of all node IDs in the cluster
///
/// # Returns
/// The launched node
pub async fn launch_follower(
    config: NodeConfig,
    network: MemoryNetworkFactory,
    cluster_nodes: BTreeSet<NodeId>,
) -> Result<FlowRaftNode, NodeLaunchError> {
    let log_store = LogStore::default();
    let state_machine = StateMachineStore::default();

    let node = FlowRaftNode::new(
        config.node_id,
        config.raft_config,
        network,
        log_store,
        state_machine,
    )
    .await
    .map_err(NodeLaunchError::RaftError)?;

    // Join the cluster
    node.initialize_cluster(cluster_nodes)
        .await
        .map_err(NodeLaunchError::InitializeError)?;

    Ok(node)
}

/// Joins a node to an existing cluster
///
/// # Arguments
/// * `node` - The node to join
/// * `cluster_nodes` - Set of all node IDs in the cluster
///
/// # Returns
/// Ok(()) if successful, error otherwise
pub async fn join_cluster(
    node: &FlowRaftNode,
    cluster_nodes: BTreeSet<NodeId>,
) -> Result<(), NodeLaunchError> {
    node.initialize_cluster(cluster_nodes)
        .await
        .map_err(NodeLaunchError::InitializeError)
}
