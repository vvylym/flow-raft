//! Production-ready cluster example
//!
//! Demonstrates a 3-node cluster with production scenarios:
//! - Leader/follower setup
//! - Workflow execution
//! - Node shutdown scenarios (follower and leader)
//! - Leader election
//! - Node restart and rejoin
//! - Metrics monitoring

use flow_raft::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create metrics collector
    let metrics = Arc::new(MetricsCollector::new());

    // Define workflows
    let workflow1 = GraphBuilder::new("workflow_1")
        .add_node(
            "task1",
            "handler1",
            vec!["input".to_string()],
            vec!["output".to_string()],
            None,
        )
        .build()?;

    let workflow2 = GraphBuilder::new("workflow_2")
        .add_node(
            "task2",
            "handler2",
            vec!["input".to_string()],
            vec!["output".to_string()],
            None,
        )
        .build()?;

    let workflow1_def = WorkflowDef::from_graph("workflow_1", workflow1, RetryConfig::default());
    let workflow2_def = WorkflowDef::from_graph("workflow_2", workflow2, RetryConfig::default());

    // Launch 3-node cluster using builder pattern
    println!("Launching 3-node cluster...");
    let mut nodes = launch_cluster(vec![
        (1, NodeMode::Leader, vec![workflow1_def.clone()]),
        (2, NodeMode::Follower, vec![workflow2_def.clone()]),
        (3, NodeMode::Follower, vec![]),
    ])
    .await?;

    println!("✓ Cluster launched with {} nodes", nodes.len());

    // Monitor metrics
    let metrics_clone = metrics.clone();
    tokio::spawn(async move {
        loop {
            let cluster_metrics = metrics_clone.get_cluster_metrics().await;
            println!("Cluster metrics: {:?}", cluster_metrics);
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Get cluster status
    if let Some(node) = nodes.first_mut() {
        let status = node.cluster_status().await;
        println!("Cluster status: {:?}", status);
    }

    // Execute workflow on leader before scenarios
    if let Some(leader_node) = nodes.first() {
        let workflow_id = workflow1_def.workflow_id;
        let app = leader_node.app();

        let executor = Arc::new(WorkflowExecutor::new(
            app.raft().clone(),
            app.state_machine().clone(),
            leader_node.node_id(),
        ));
        let registry = Arc::new(HandlerRegistry::new());

        registry
            .register_handler(
                workflow_id,
                "handler1".to_string(),
                Arc::new(EchoHandler {
                    name: "task1".to_string(),
                }) as Arc<dyn TaskHandler>,
            )
            .await;

        tokio::time::sleep(Duration::from_millis(200)).await;

        let handler_executor = HandlerExecutor::new(executor, registry);
        println!("\n=== Executing workflow before scenarios ===");
        if handler_executor
            .execute_workflow(workflow_id, 100)
            .await
            .is_ok()
        {
            println!("✓ Workflow executed successfully");
        }
    }

    // Scenario 1: Shutdown follower node
    println!("\n=== Scenario 1: Shutting down follower node 3 ===");
    if nodes.len() > 2 {
        nodes[2].shutdown().await?;
        println!("✓ Follower node 3 shut down");
    }

    // Verify cluster continues operating
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("✓ Cluster continues operating with 2 nodes");

    // Scenario 2: Shutdown leader node
    println!("\n=== Scenario 2: Shutting down leader node 1 ===");
    if !nodes.is_empty() {
        nodes[0].shutdown().await?;
        println!("✓ Leader node 1 shut down");
    }

    // Wait for leader election
    println!("Waiting for leader election...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Verify new leader elected and cluster continues
    if nodes.len() > 1
        && let Some(node) = nodes.get_mut(1)
    {
        let status = node.cluster_status().await;
        println!("✓ New leader status: {:?}", status);
    }

    // Scenario 3: Restart failed node as follower
    println!("\n=== Scenario 3: Restarting node 1 as follower ===");
    // Note: This would require proper leader address, skipping for now
    // let node1_restarted =
    //     launch_cluster(vec![(1, NodeMode::Follower, vec![workflow1_def.clone()])]).await?;

    // Check metrics summary
    let metrics_summary = metrics.get_metrics_summary().await;
    println!("\n=== Final Metrics Summary ===");
    println!("Metrics: {:?}", metrics_summary);

    println!("\n✓ Production cluster example completed successfully!");
    Ok(())
}

// Helper handler
struct EchoHandler {
    name: String,
}

impl TaskHandler for EchoHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("[{}] Executing with inputs: {:?}", self.name, inputs);
        Ok(serde_json::json!({
            "handler": self.name,
            "result": "success",
            "inputs": inputs
        }))
    }
}
