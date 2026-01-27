//! Shared-network Raft cluster launcher.
//!
//! Builds one N-node Raft cluster with a shared in-memory network so all
//! nodes participate in the same replicated log and state machine.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use flow_raft_raft::network::MemoryNetworkFactory;
use flow_raft_raft::storage::{LogStore, StateMachineStore};
use flow_raft_raft::types::NodeId;
use flow_raft_raft::{FlowRaftApp, FlowRaftNode};
use tokio::time::sleep;

use crate::node::launcher::NodeLaunchError;

#[cfg(test)]
mod run;

/// Fast Raft config for tests and local clusters (quick elections).
fn fast_config() -> flow_raft_raft::config::RaftConfig {
    openraft::Config {
        heartbeat_interval: 100,
        election_timeout_min: 300,
        election_timeout_max: 500,
        ..Default::default()
    }
}

/// Handle to a shared-network Raft cluster.
///
/// Holds all nodes and their apps. Use [RaftClusterHandle::leader_app] to get
/// the leader's app for creating workflows; use [RaftClusterHandle::node_apps]
/// to verify replication on all nodes.
pub struct RaftClusterHandle {
    /// (node_id, node, app) for each node. All share the same network.
    nodes: Vec<(NodeId, FlowRaftNode, Arc<FlowRaftApp>)>,
    /// Shared network; used to unregister dropped nodes.
    network: MemoryNetworkFactory,
}

impl RaftClusterHandle {
    /// Returns the number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns (node_id, app) for every node. Use this to assert replication
    /// by calling `get_workflow` on each node's app.
    pub fn node_apps(&self) -> Vec<(NodeId, Arc<FlowRaftApp>)> {
        self.nodes
            .iter()
            .map(|(id, _, app)| (*id, app.clone()))
            .collect()
    }

    /// Returns the leader's app if a leader exists. Polls each node's
    /// linearizable read; first node that reports leader wins.
    pub async fn leader_app(&self) -> Option<Arc<FlowRaftApp>> {
        let rafts_and_apps: Vec<_> = self
            .nodes
            .iter()
            .map(|(_, n, a)| (n.raft.clone(), a.clone()))
            .collect();
        for (_raft, app) in rafts_and_apps {
            if app.raft().ensure_linearizable().await.is_ok() {
                return Some(app);
            }
        }
        None
    }

    /// Wait for a leader to be elected, up to `timeout`. Returns the leader's
    /// app or None on timeout.
    pub async fn wait_for_leader(&self, timeout: Duration) -> Option<Arc<FlowRaftApp>> {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if let Some(app) = self.leader_app().await {
                return Some(app);
            }
            sleep(Duration::from_millis(50)).await;
        }
        None
    }

    /// Like [Self::wait_for_leader] but returns the leader's node ID and app.
    pub async fn wait_for_leader_with_id(
        &self,
        timeout: Duration,
    ) -> Option<(NodeId, Arc<FlowRaftApp>)> {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            let rafts_apps: Vec<_> = self
                .nodes
                .iter()
                .map(|(id, n, a)| (*id, n.raft.clone(), a.clone()))
                .collect();
            for (node_id, raft, app) in rafts_apps {
                if raft.ensure_linearizable().await.is_ok() {
                    return Some((node_id, app));
                }
            }
            sleep(Duration::from_millis(50)).await;
        }
        None
    }

    /// Removes and drops the node with the given ID, and unregisters it from
    /// the network. Use to simulate node failure in tests.
    pub async fn drop_node(&mut self, node_id: NodeId) {
        self.network.unregister_raft(node_id).await;
        self.nodes.retain(|(id, _, _)| *id != node_id);
    }
}

/// Launches a single shared-network Raft cluster with the given node IDs.
///
/// All nodes use the same [MemoryNetworkFactory], so they form one logical
/// cluster with one replicated log and state machine. The first node
/// initializes the cluster with all IDs; a leader is elected shortly after.
///
/// Use [RaftClusterHandle::wait_for_leader] then create workflows on the
/// leader's app; use [RaftClusterHandle::node_apps] to verify replication.
pub async fn launch_raft_cluster(
    node_ids: &[NodeId],
) -> Result<RaftClusterHandle, NodeLaunchError> {
    if node_ids.is_empty() {
        return Err(NodeLaunchError::Config(
            "launch_raft_cluster requires at least one node".to_string(),
        ));
    }

    let config = fast_config();
    let network = MemoryNetworkFactory::new();
    let mut nodes = Vec::with_capacity(node_ids.len());

    for &node_id in node_ids {
        let log_store = LogStore::default();
        let state_machine = StateMachineStore::default();
        let node = FlowRaftNode::new(
            node_id,
            config.clone(),
            network.clone(),
            log_store,
            state_machine.clone(),
        )
        .await
        .map_err(NodeLaunchError::RaftError)?;
        network.register_raft(node_id, node.raft.clone()).await;
        let app = Arc::new(FlowRaftApp::new(node.raft.clone(), state_machine));
        nodes.push((node_id, node, app));
    }

    let node_ids_set: BTreeSet<NodeId> = node_ids.iter().copied().collect();
    nodes[0]
        .1
        .initialize_cluster(node_ids_set)
        .await
        .map_err(NodeLaunchError::InitializeError)?;

    Ok(RaftClusterHandle { nodes, network })
}
