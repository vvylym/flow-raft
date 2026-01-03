//! Tests for FlowRaft node

use flow_raft_raft::config::default_config;
use flow_raft_raft::network::MemoryNetworkFactory;
use flow_raft_raft::node::FlowRaftNode;
use flow_raft_raft::storage::{LogStore, StateMachineStore};
use flow_raft_raft::types::NodeId;
use std::collections::BTreeSet;

#[tokio::test]
async fn test_node_creation() {
    let node_id: NodeId = 1;
    let config = default_config();
    let network = MemoryNetworkFactory::new();
    let log_store = LogStore::default();
    let state_machine = StateMachineStore::default();

    let node = FlowRaftNode::new(node_id, config, network, log_store, state_machine).await;
    assert!(node.is_ok());
    let node = node.unwrap();
    assert_eq!(node.node_id, node_id);
}

#[tokio::test]
async fn test_node_initialize_single_node() {
    let node_id: NodeId = 1;
    let config = default_config();
    let network = MemoryNetworkFactory::new();
    let log_store = LogStore::default();
    let state_machine = StateMachineStore::default();

    let node = FlowRaftNode::new(node_id, config, network, log_store, state_machine)
        .await
        .unwrap();
    let result = node.initialize_single_node().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_node_initialize_cluster() {
    let node_id: NodeId = 1;
    let config = default_config();
    let network = MemoryNetworkFactory::new();
    let log_store = LogStore::default();
    let state_machine = StateMachineStore::default();

    let node = FlowRaftNode::new(node_id, config, network, log_store, state_machine)
        .await
        .unwrap();
    let node_ids: BTreeSet<NodeId> = [1, 2, 3].into_iter().collect();
    let result = node.initialize_cluster(node_ids).await;
    assert!(result.is_ok());
}
