//! FlowRaft node implementation
//!
//! Provides node setup and initialization for Raft cluster.

use std::collections::BTreeSet;
use std::sync::Arc;

use openraft::Raft;

use crate::raft::config::RaftConfig;
use crate::raft::network::MemoryNetworkFactory;
use crate::raft::storage::{LogStore, StateMachineStore};
use crate::raft::types::{NodeId, TypeConfig};

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
    /// Create a new FlowRaft node
    pub async fn new(
        node_id: NodeId,
        config: RaftConfig,
        network: MemoryNetworkFactory,
        log_store: LogStore<TypeConfig>,
        state_machine: StateMachineStore<TypeConfig>,
    ) -> Result<
        Self,
        openraft::error::RaftError<
            <TypeConfig as openraft::RaftTypeConfig>::NodeId,
            openraft::error::Infallible,
        >,
    > {
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

    /// Initialize a multi-node cluster
    /// This should be called on the first node with all node IDs
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raft::config::default_config;

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
