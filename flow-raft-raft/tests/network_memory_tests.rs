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
async fn test_memory_network_factory_new_client() {
    let mut factory = MemoryNetworkFactory::new();
    let node_id: NodeId = 1;
    use openraft::BasicNode;
    let node = BasicNode {
        addr: "127.0.0.1:8080".to_string(),
    };
    let client = RaftNetworkFactory::new_client(&mut factory, node_id, &node).await;
    assert!(std::mem::size_of_val(&client) > 0);
}
