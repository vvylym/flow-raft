//! Bootstrap or join Raft cluster for NodeServer.

use flow_raft_raft::network::tcp_nodes;
use flow_raft_raft::node::FlowRaftNode;
use openraft::BasicNode;

use super::ServeConfig;

/// Error during bootstrap/join.
#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    /// Peers string is invalid: expected "id=addr,id=addr" (e.g. "2=127.0.0.1:5011,3=127.0.0.1:5012").
    #[error("invalid peers (expected id=addr): {0}")]
    InvalidPeers(String),
    /// Self node_id appears in peers; each node ID must be unique.
    #[error("node_id {0} appears in peers")]
    DuplicateNodeId(u64),
    /// Raft initialize failed.
    #[error("raft initialize: {0}")]
    Raft(#[from] openraft::error::RaftError<u64, openraft::error::InitializeError<u64, BasicNode>>),
}

/// Run bootstrap or join based on [ServeConfig].
///
/// - If [ServeConfig::peers] is empty and [ServeConfig::bootstrap] is true:
///   [FlowRaftNode::initialize_single_node].
/// - If [ServeConfig::peers] is empty and `bootstrap` is false: do nothing (joining node).
/// - If [ServeConfig::peers] is non-empty: parse as `id=addr`, add self, and call
///   [FlowRaftNode::initialize_cluster_with_nodes].
pub async fn run_bootstrap(
    config: &ServeConfig,
    node: &FlowRaftNode,
) -> Result<(), BootstrapError> {
    if config.peers.is_empty() {
        if config.bootstrap {
            node.initialize_single_node().await?;
        }
        return Ok(());
    }

    let mut entries: Vec<(u64, String)> = Vec::new();
    for s in &config.peers {
        let s = s.trim();
        if s.is_empty() {
            continue;
        }
        let (id, addr) = s
            .split_once('=')
            .ok_or_else(|| BootstrapError::InvalidPeers(s.to_string()))?;
        let id: u64 = id
            .trim()
            .parse()
            .map_err(|_| BootstrapError::InvalidPeers(s.to_string()))?;
        let addr = addr.trim().to_string();
        if id == config.node_id {
            return Err(BootstrapError::DuplicateNodeId(config.node_id));
        }
        entries.push((id, addr));
    }

    let self_addr = format!("{}:{}", config.raft_bind.ip(), config.raft_bind.port());
    let self_entry = (config.node_id, self_addr);
    entries.insert(0, self_entry);
    let nodes = tcp_nodes(entries);
    node.initialize_cluster_with_nodes(nodes).await?;
    Ok(())
}
