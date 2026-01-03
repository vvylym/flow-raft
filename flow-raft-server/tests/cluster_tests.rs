//! Comprehensive tests for cluster management

use flow_raft_api::WorkflowDef;
use flow_raft_core::RetryConfig;
use flow_raft_raft::config::default_config;
use flow_raft_server::cluster::{ClusterNode, NodeRole};

#[tokio::test]
async fn test_cluster_node_new_leader() {
    let node_id = 1;
    let config = default_config();
    let node = ClusterNode::new_leader(node_id, config).await;
    assert!(node.is_ok());
    let node = node.unwrap();
    assert_eq!(node.node_id(), node_id);
    assert_eq!(node.role(), NodeRole::Leader);
}

#[tokio::test]
async fn test_cluster_node_join_cluster() {
    let node_id = 2;
    let config = default_config();
    let node = ClusterNode::join_cluster(node_id, config, "127.0.0.1:8080".to_string()).await;
    assert!(node.is_ok());
    let node = node.unwrap();
    assert_eq!(node.node_id(), node_id);
    assert_eq!(node.role(), NodeRole::Follower);
}

#[tokio::test]
async fn test_cluster_node_cluster_status() {
    let node_id = 1;
    let config = default_config();
    let node = ClusterNode::new_leader(node_id, config).await.unwrap();
    let mut node = node;
    let status = node.cluster_status().await;
    assert_eq!(status.node_id, node_id);
    assert_eq!(status.role, NodeRole::Leader);
    assert!(status.leader.is_some());
    assert!(status.nodes.contains(&node_id));
}

#[tokio::test]
async fn test_cluster_node_register_workflow() {
    let node_id = 1;
    let config = default_config();
    let node = ClusterNode::new_leader(node_id, config).await.unwrap();

    let workflow = WorkflowDef::from_graph(
        "test_workflow",
        flow_raft_api::graph::GraphBuilder::new("test")
            .add_node("task1", "handler1", vec![], vec![], None)
            .set_root("task1")
            .build()
            .unwrap(),
        RetryConfig::default(),
    );

    let workflow_id = node.register_workflow(workflow).await;
    assert!(workflow_id.is_ok());
}

#[tokio::test]
async fn test_cluster_node_list_workflows() {
    let node_id = 1;
    let config = default_config();
    let node = ClusterNode::new_leader(node_id, config).await.unwrap();

    let workflow = WorkflowDef::from_graph(
        "test_workflow",
        flow_raft_api::graph::GraphBuilder::new("test")
            .add_node("task1", "handler1", vec![], vec![], None)
            .set_root("task1")
            .build()
            .unwrap(),
        RetryConfig::default(),
    );

    let workflow_id = workflow.workflow_id;
    node.register_workflow(workflow).await.unwrap();

    let workflows = node.list_workflows().await;
    assert!(workflows.contains(&workflow_id));
}

#[tokio::test]
async fn test_cluster_node_unregister_workflow() {
    let node_id = 1;
    let config = default_config();
    let node = ClusterNode::new_leader(node_id, config).await.unwrap();

    let workflow = WorkflowDef::from_graph(
        "test_workflow",
        flow_raft_api::graph::GraphBuilder::new("test")
            .add_node("task1", "handler1", vec![], vec![], None)
            .set_root("task1")
            .build()
            .unwrap(),
        RetryConfig::default(),
    );

    let workflow_id = workflow.workflow_id;
    node.register_workflow(workflow).await.unwrap();

    let result = node.unregister_workflow(workflow_id).await;
    assert!(result.is_ok());

    let workflows = node.list_workflows().await;
    assert!(!workflows.contains(&workflow_id));
}

#[tokio::test]
async fn test_cluster_node_submit_workflow() {
    let node_id = 1;
    let config = default_config();
    let node = ClusterNode::new_leader(node_id, config).await.unwrap();

    let workflow = WorkflowDef::from_graph(
        "test_workflow",
        flow_raft_api::graph::GraphBuilder::new("test")
            .add_node("task1", "handler1", vec![], vec![], None)
            .set_root("task1")
            .build()
            .unwrap(),
        RetryConfig::default(),
    );

    let workflow_id = node.submit_workflow(workflow, serde_json::json!({})).await;
    assert!(workflow_id.is_ok());
}

#[tokio::test]
async fn test_cluster_node_app() {
    let node_id = 1;
    let config = default_config();
    let node = ClusterNode::new_leader(node_id, config).await.unwrap();
    let app = node.app();
    // Verify app is accessible
    assert!(std::mem::size_of_val(app) > 0);
}

#[tokio::test]
async fn test_cluster_node_node() {
    let node_id = 1;
    let config = default_config();
    let node = ClusterNode::new_leader(node_id, config).await.unwrap();
    let raft_node = node.node();
    // Verify node is accessible
    assert!(std::mem::size_of_val(raft_node) > 0);
}
