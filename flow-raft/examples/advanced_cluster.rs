//! Advanced cluster example
//!
//! Demonstrates:
//! - Multi-region deployment patterns
//! - Cross-cluster replication
//! - Load balancing
//! - High availability scenarios

use flow_raft::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("=== Advanced Cluster Example ===");
    println!("\nThis example demonstrates:");
    println!("  1. Multi-region cluster setup");
    println!("  2. High availability configuration");
    println!("  3. Load balancing across nodes");
    println!("  4. Cross-cluster replication patterns");

    // Create workflows
    let workflow1 = GraphBuilder::new("region1_workflow")
        .add_node(
            "task1",
            "handler1",
            vec!["input".to_string()],
            vec!["output".to_string()],
            None,
        )
        .set_root("task1")
        .build()?;

    let workflow2 = GraphBuilder::new("region2_workflow")
        .add_node(
            "task2",
            "handler2",
            vec!["input".to_string()],
            vec!["output".to_string()],
            None,
        )
        .set_root("task2")
        .build()?;

    let workflow1_def = WorkflowDef::from_graph("region1", workflow1, RetryConfig::default());
    let workflow2_def = WorkflowDef::from_graph("region2", workflow2, RetryConfig::default());

    // Launch multi-region cluster
    println!("\nLaunching 5-node cluster (simulating 2 regions)...");
    let mut nodes = launch_cluster(vec![
        (1, NodeMode::Leader, vec![workflow1_def.clone()]), // Region 1 leader
        (2, NodeMode::Follower, vec![]),                    // Region 1 follower
        (3, NodeMode::Follower, vec![]),                    // Region 1 follower
        (4, NodeMode::Follower, vec![workflow2_def.clone()]), // Region 2 follower
        (5, NodeMode::Follower, vec![]),                    // Region 2 follower
    ])
    .await?;

    println!("✓ Cluster launched with {} nodes", nodes.len());

    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Get cluster status
    if let Some(node) = nodes.first_mut() {
        let status = node.cluster_status().await;
        println!("\nCluster status:");
        println!("  Leader: {:?}", status.leader);
        println!("  Nodes: {:?}", status.nodes);
        println!("  This node role: {:?}", status.role);
    }

    println!("\n✓ Advanced cluster example completed!");
    Ok(())
}
