//! Integration tests for multi-node cluster scenarios

use flow_raft::cluster::{ClusterNode, NodeRole};
use flow_raft::node::config::NodeConfig;
use flow_raft::node::launcher::launch_single_node;
use flow_raft::raft::config::default_config;
use flow_raft::raft::types::NodeId;
use flow_raft::WorkflowId;

#[tokio::test]
async fn test_single_node_cluster() {
    let config = NodeConfig {
        node_id: 1,
        mode: flow_raft::node::config::NodeMode::Leader,
        raft_config: default_config(),
        network_config: Default::default(),
        storage_path: None,
    };
    
    let node = launch_single_node(config).await;
    assert!(node.is_ok());
}

#[tokio::test]
async fn test_cluster_node_workflow_registration() {
    let config = NodeConfig {
        node_id: 1,
        mode: flow_raft::node::config::NodeMode::Leader,
        raft_config: default_config(),
        network_config: Default::default(),
        storage_path: None,
    };
    
    let node = launch_single_node(config).await.unwrap();
    
    // Test workflow registration
    use flow_raft::WorkflowDef;
    use flow_raft::graph::GraphBuilder;
    use flow_raft::RetryConfig;
    
    let mut builder = GraphBuilder::new("test_workflow")
        .with_default_retry_config(RetryConfig::default());
    builder.add_node("task1", "handler1", vec![], vec![], None);
    builder.set_root("task1");
    
    let graph = builder.build().unwrap();
    let workflow_def = WorkflowDef::from_graph("test", graph, RetryConfig::default());
    
    let workflow_id = node.register_workflow(workflow_def).await;
    assert!(workflow_id.is_ok());
    
    let workflows = node.list_workflows().await;
    assert_eq!(workflows.len(), 1);
}
