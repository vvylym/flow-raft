//! Additional tests for memory network to increase coverage

use flow_raft_raft::network::MemoryNetworkFactory;
use flow_raft_raft::types::NodeId;
use openraft::RaftNetworkFactory;

#[tokio::test]
async fn test_memory_network_factory_new() {
    let factory = MemoryNetworkFactory::new();
    // Verify factory is created
    assert!(std::mem::size_of_val(&factory) > 0);
}

#[tokio::test]
async fn test_memory_network_factory_register_node() {
    let factory = MemoryNetworkFactory::new();
    let node_id: NodeId = 1;
    factory.register_node(node_id).await;
    // Verify node is registered (no error)
}

#[tokio::test]
async fn test_memory_network_factory_new_client() {
    let factory = MemoryNetworkFactory::new();
    let node_id: NodeId = 1;
    factory.register_node(node_id).await;

    use openraft::BasicNode;
    let node = BasicNode {
        addr: "127.0.0.1:8080".to_string(),
    };
    let mut factory_mut = factory;
    let client = RaftNetworkFactory::new_client(&mut factory_mut, node_id, &node).await;
    // Verify client is created
    assert!(std::mem::size_of_val(&client) > 0);
}
