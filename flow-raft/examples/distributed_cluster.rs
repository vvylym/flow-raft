//! Distributed cluster example
//!
//! Demonstrates multi-node cluster setup with:
//! - 3-node cluster (1 leader, 2 followers)
//! - Workflow registration on different nodes
//! - Metrics collection
//! - Cluster operations

use flow_raft::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create metrics collector
    let _metrics = Arc::new(MetricsCollector::new());

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
    let mut nodes = launch_cluster(vec![
        (1, NodeMode::Leader, vec![workflow1_def.clone()]),
        (2, NodeMode::Follower, vec![workflow2_def.clone()]),
        (3, NodeMode::Follower, vec![]),
    ])
    .await?;

    println!("Cluster launched with {} nodes", nodes.len());

    // Monitor metrics (commented out for now - would require metrics integration)
    // let metrics_clone = metrics.clone();
    // tokio::spawn(async move {
    //     loop {
    //         let cluster_metrics = metrics_clone.get_cluster_metrics().await;
    //         println!("Cluster metrics: {:?}", cluster_metrics);
    //         tokio::time::sleep(Duration::from_secs(5)).await;
    //     }
    // });

    // Wait a bit for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Get cluster status
    if let Some(node) = nodes.first_mut() {
        let status = node.cluster_status().await;
        println!("Cluster status: {:?}", status);
    }

    // Setup execution on leader node
    if let Some(leader_node) = nodes.first() {
        let workflow_id = workflow1_def.workflow_id;
        let app = leader_node.app();

        // Setup executor and registry
        let executor = Arc::new(WorkflowExecutor::new(
            app.raft().clone(),
            app.state_machine().clone(),
            leader_node.node_id(),
        ));
        let registry = Arc::new(HandlerRegistry::new());

        // Register handlers
        registry
            .register_handler(
                workflow_id,
                "handler1".to_string(),
                Arc::new(EchoHandler {
                    name: "task1".to_string(),
                }) as Arc<dyn TaskHandler>,
            )
            .await;

        // Wait for workflow to be registered
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Execute workflow
        let handler_executor = HandlerExecutor::new(executor, registry);
        if let Err(e) = handler_executor.execute_workflow(workflow_id, 100).await {
            eprintln!("Workflow execution error: {:?}", e);
        } else {
            println!("✓ Workflow executed successfully on leader node");

            // Show final state
            if let Some(final_workflow) = app.get_workflow(&workflow_id).await {
                println!("Final workflow state: {:?}", final_workflow.state);
            }
        }
    }

    println!("Distributed cluster example completed!");
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
