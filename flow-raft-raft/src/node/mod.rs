//! FlowRaft node implementation
//!
//! Provides node setup and initialization for Raft cluster.

use std::collections::BTreeSet;
use std::sync::Arc;

use openraft::Raft;
use openraft::RaftNetworkFactory;

use crate::config::RaftConfig;
use crate::storage::{LogStore, StateMachineStore};
use crate::types::{NodeId, TypeConfig};

/// FlowRaft node
pub struct FlowRaftNode {
    /// The unique identifier for this node
    pub node_id: NodeId,
    /// The Raft instance for this node
    pub raft: Arc<Raft<TypeConfig>>,
    /// The state machine store for this node
    pub state_machine: StateMachineStore<TypeConfig>,
}

impl FlowRaftNode {
    /// Create a new FlowRaft node.
    ///
    /// Use [MemoryNetworkFactory] for in-memory/single-node or tests; use
    /// [TcpNetworkFactory](crate::network::TcpNetworkFactory) for production over TCP.
    pub async fn new<N>(
        node_id: NodeId,
        config: RaftConfig,
        network: N,
        log_store: LogStore<TypeConfig>,
        state_machine: StateMachineStore<TypeConfig>,
    ) -> Result<
        Self,
        openraft::error::RaftError<
            <TypeConfig as openraft::RaftTypeConfig>::NodeId,
            openraft::error::Infallible,
        >,
    >
    where
        N: RaftNetworkFactory<TypeConfig>,
    {
        let config = Arc::new(config.validate().unwrap());
        let raft = Raft::new(node_id, config, network, log_store, state_machine.clone()).await?;

        Ok(Self {
            node_id,
            raft: Arc::new(raft),
            state_machine,
        })
    }

    /// Initialize a single-node cluster
    pub async fn initialize_single_node(
        &self,
    ) -> Result<
        (),
        openraft::error::RaftError<
            <TypeConfig as openraft::RaftTypeConfig>::NodeId,
            openraft::error::InitializeError<
                <TypeConfig as openraft::RaftTypeConfig>::NodeId,
                <TypeConfig as openraft::RaftTypeConfig>::Node,
            >,
        >,
    > {
        // Initialize with a single node
        // initialize() accepts IntoNodes - BTreeSet<NodeId> implements it
        let nodes: BTreeSet<NodeId> = [self.node_id].into_iter().collect();
        self.raft.initialize(nodes).await?;
        Ok(())
    }

    /// Initialize a multi-node cluster.
    /// This should be called on the first node with all node IDs.
    /// For in-memory transport, peer addrs are not required.
    pub async fn initialize_cluster(
        &self,
        node_ids: BTreeSet<NodeId>,
    ) -> Result<
        (),
        openraft::error::RaftError<
            <TypeConfig as openraft::RaftTypeConfig>::NodeId,
            openraft::error::InitializeError<
                <TypeConfig as openraft::RaftTypeConfig>::NodeId,
                <TypeConfig as openraft::RaftTypeConfig>::Node,
            >,
        >,
    > {
        self.raft.initialize(node_ids).await?;
        Ok(())
    }

    /// Initialize a multi-node cluster with peer addresses.
    /// Use this for TCP transport so Raft can connect to each peer.
    /// Build the map with [tcp_nodes](crate::network::tcp_nodes).
    pub async fn initialize_cluster_with_nodes(
        &self,
        nodes: std::collections::BTreeMap<NodeId, openraft::BasicNode>,
    ) -> Result<
        (),
        openraft::error::RaftError<
            <TypeConfig as openraft::RaftTypeConfig>::NodeId,
            openraft::error::InitializeError<
                <TypeConfig as openraft::RaftTypeConfig>::NodeId,
                <TypeConfig as openraft::RaftTypeConfig>::Node,
            >,
        >,
    > {
        self.raft.initialize(nodes).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_config;
    use crate::network::MemoryNetworkFactory;

    #[tokio::test]
    async fn test_node_creation() {
        let node_id = 1;
        let config = default_config();
        let network = MemoryNetworkFactory::new();
        let log_store = LogStore::default();
        let state_machine = StateMachineStore::default();

        let node = FlowRaftNode::new(node_id, config, network, log_store, state_machine).await;
        assert!(node.is_ok());
    }
}
