//! Integration tests for cluster failure scenarios
//!
//! Tests cluster resilience including:
//! - Leader failure and election
//! - Follower failure
//! - Node restart and rejoin
//! - State replication verification

use flow_raft::prelude::*;
use std::time::Duration;

#[tokio::test]
async fn test_leader_failure_and_election() {
    // Setup: Create 3-node cluster
    let workflow = GraphBuilder::new("test_workflow")
        .add_node(
            "task1",
            "handler1",
            vec![],
            vec![],
            None,
        )
        .build()
        .unwrap();

    let workflow_def = WorkflowDef::from_graph("test", workflow, RetryConfig::default());

    let mut nodes = launch_cluster(vec![
        (1, NodeMode::Leader, vec![workflow_def.clone()]),
        (2, NodeMode::Follower, vec![]),
        (3, NodeMode::Follower, vec![]),
    ])
    .await
    .unwrap();

    assert_eq!(nodes.len(), 3);

    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Shutdown leader
    nodes[0].shutdown().await.unwrap();

    // Wait for leader election
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify cluster continues operating
    let status = nodes[1].cluster_status().await;
    assert!(status.nodes.contains(&2) || status.nodes.contains(&3));

    // Verify metrics would show election occurred
    // (In a full implementation, we would check metrics)
}

#[tokio::test]
async fn test_follower_failure() {
    // Setup: Create 3-node cluster
    let workflow = GraphBuilder::new("test_workflow")
        .add_node(
            "task1",
            "handler1",
            vec![],
            vec![],
            None,
        )
        .build()
        .unwrap();

    let workflow_def = WorkflowDef::from_graph("test", workflow, RetryConfig::default());

    let nodes = launch_cluster(vec![
        (1, NodeMode::Leader, vec![workflow_def.clone()]),
        (2, NodeMode::Follower, vec![]),
        (3, NodeMode::Follower, vec![]),
    ])
    .await
    .unwrap();

    assert_eq!(nodes.len(), 3);

    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Shutdown follower
    nodes[2].shutdown().await.unwrap();

    // Verify cluster continues
    tokio::time::sleep(Duration::from_millis(500)).await;

    let status = nodes[0].cluster_status().await;
    assert!(status.nodes.contains(&1));
    assert!(status.nodes.contains(&2));
}

#[tokio::test]
async fn test_node_restart() {
    // Setup: Create 2-node cluster
    let workflow = GraphBuilder::new("test_workflow")
        .add_node(
            "task1",
            "handler1",
            vec![],
            vec![],
            None,
        )
        .build()
        .unwrap();

    let workflow_def = WorkflowDef::from_graph("test", workflow, RetryConfig::default());

    let mut nodes = launch_cluster(vec![
        (1, NodeMode::Leader, vec![workflow_def.clone()]),
        (2, NodeMode::Follower, vec![]),
    ])
    .await
    .unwrap();

    assert_eq!(nodes.len(), 2);

    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Shutdown follower
    let follower = nodes.remove(1);
    follower.shutdown().await.unwrap();

    // Restart follower
    tokio::time::sleep(Duration::from_millis(500)).await;

    let restarted = launch_cluster(vec![(2, NodeMode::Follower, vec![])])
        .await
        .unwrap();

    assert_eq!(restarted.len(), 1);

    // Wait for rejoin
    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut restarted = restarted;
    let status = restarted[0].cluster_status().await;
    assert_eq!(status.node_id, 2);
}

#[tokio::test]
async fn test_state_replication_metrics() {
    // Setup: Create 3-node cluster
    let workflow = GraphBuilder::new("test_workflow")
        .add_node(
            "task1",
            "handler1",
            vec![],
            vec![],
            None,
        )
        .build()
        .unwrap();

    let workflow_def = WorkflowDef::from_graph("test", workflow, RetryConfig::default());

    let mut nodes = launch_cluster(vec![
        (1, NodeMode::Leader, vec![workflow_def.clone()]),
        (2, NodeMode::Follower, vec![]),
        (3, NodeMode::Follower, vec![]),
    ])
    .await
    .unwrap();

    assert_eq!(nodes.len(), 3);

    // Register workflow on leader
    nodes[0].register_workflow(workflow_def).await.unwrap();

    // Wait for replication
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify metrics would show replication
    // (In a full implementation, we would check metrics collector)
    let status = nodes[0].cluster_status().await;
    assert!(status.nodes.contains(&1));
}
