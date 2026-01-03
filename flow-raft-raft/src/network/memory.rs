//! In-memory network implementation for testing and single-node mode

use std::collections::HashMap;
use std::sync::Arc;

use futures::lock::Mutex;
use openraft::BasicNode;
use openraft::RaftTypeConfig;
use openraft::Vote;
use openraft::error::RPCError;
use openraft::error::RaftError;
use openraft::network::RPCOption;
use openraft::network::RaftNetworkFactory;
use openraft::raft::AppendEntriesRequest;
use openraft::raft::AppendEntriesResponse;
use openraft::raft::InstallSnapshotRequest;
use openraft::raft::InstallSnapshotResponse;
use openraft::raft::VoteRequest;
use openraft::raft::VoteResponse;

use crate::types::{NodeId, TypeConfig};

/// In-memory network for single-node and testing
#[derive(Debug, Clone, Default)]
pub struct MemoryNetworkFactory {
    nodes: Arc<Mutex<HashMap<NodeId, Arc<Mutex<MemoryNode>>>>>,
}

#[derive(Debug, Default)]
struct MemoryNode {
    // Node state would be stored here
}

impl MemoryNetworkFactory {
    /// Create a new memory network factory
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a node in the network
    pub async fn register_node(&self, node_id: NodeId) {
        let mut nodes = self.nodes.lock().await;
        nodes.insert(node_id, Arc::new(Mutex::new(MemoryNode::default())));
    }
}

/// In-memory network connection
#[derive(Debug)]
pub struct MemoryNetwork {
    target: NodeId,
    /// Shared node registry (reserved for future multi-node routing implementation)
    ///
    /// # Note
    /// This field is currently unused as the in-memory network implementation
    /// is simplified for single-node and testing scenarios. It's reserved for
    /// future multi-node routing where messages would be routed through this registry.
    #[allow(dead_code)] // Reserved for future multi-node routing
    nodes: Arc<Mutex<HashMap<NodeId, Arc<Mutex<MemoryNode>>>>>,
}

impl RaftNetworkFactory<TypeConfig> for MemoryNetworkFactory {
    type Network = MemoryNetwork;

    async fn new_client(&mut self, target: NodeId, _node: &BasicNode) -> Self::Network {
        MemoryNetwork {
            target,
            nodes: self.nodes.clone(),
        }
    }
}

impl openraft::RaftNetwork<TypeConfig> for MemoryNetwork {
    async fn append_entries(
        &mut self,
        _rpc: AppendEntriesRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<
        AppendEntriesResponse<<TypeConfig as RaftTypeConfig>::NodeId>,
        RPCError<
            <TypeConfig as RaftTypeConfig>::NodeId,
            <TypeConfig as RaftTypeConfig>::Node,
            RaftError<<TypeConfig as RaftTypeConfig>::NodeId>,
        >,
    > {
        // In-memory implementation: for now, just return success
        // This will be properly implemented for multi-node scenarios
        Ok(AppendEntriesResponse::Success)
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<<TypeConfig as RaftTypeConfig>::NodeId>,
        _option: RPCOption,
    ) -> Result<
        VoteResponse<<TypeConfig as RaftTypeConfig>::NodeId>,
        RPCError<
            <TypeConfig as RaftTypeConfig>::NodeId,
            <TypeConfig as RaftTypeConfig>::Node,
            RaftError<<TypeConfig as RaftTypeConfig>::NodeId>,
        >,
    > {
        // In-memory: always grant vote for testing
        Ok(VoteResponse {
            vote: rpc.vote,
            vote_granted: true,
            last_log_id: rpc.last_log_id,
        })
    }

    async fn install_snapshot(
        &mut self,
        _req: InstallSnapshotRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<<TypeConfig as RaftTypeConfig>::NodeId>,
        RPCError<
            <TypeConfig as RaftTypeConfig>::NodeId,
            <TypeConfig as RaftTypeConfig>::Node,
            RaftError<
                <TypeConfig as RaftTypeConfig>::NodeId,
                openraft::error::InstallSnapshotError,
            >,
        >,
    > {
        // In-memory implementation: snapshot is already "transmitted"
        Ok(InstallSnapshotResponse {
            vote: Vote::new(1, self.target),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[fixture]
    fn test_network_factory() -> MemoryNetworkFactory {
        MemoryNetworkFactory::new()
    }

    #[tokio::test]
    async fn test_register_node() {
        let network = test_network_factory();
        network.register_node(1).await;
        // Node should be registered (no error)
    }

    #[tokio::test]
    async fn test_create_network_client() {
        let mut factory = test_network_factory();
        let node = BasicNode {
            addr: "".to_string(),
        };
        let client = factory.new_client(1, &node).await;
        assert_eq!(client.target, 1);
    }
}
