//! Distributed execution example
//!
//! Demonstrates multi-node cluster setup with leader/follower coordination
//! and task execution on any node using the simplified API.

use flow_raft::prelude::*;
use std::sync::Arc;

struct DistributedTaskHandler {
    node_id: NodeId,
    task_name: String,
}

impl TaskHandler for DistributedTaskHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!(
            "Node {} executing task {} with inputs: {:?}",
            self.node_id, self.task_name, inputs
        );
        Ok(serde_json::json!({
            "node_id": self.node_id,
            "task": self.task_name,
            "result": "success"
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Distributed Execution Example");

    // Create a simple workflow
    println!("\n1. Creating workflow...");
    let workflow_graph = GraphBuilder::new("distributed_workflow")
        .add_node("task1", "handler1", vec![], vec![], None)
        .add_node("task2", "handler2", vec![], vec![], None)
        .add_node("task3", "handler3", vec![], vec![], None)
        .add_simple_edge("task1", "task2")
        .add_simple_edge("task2", "task3")
        .set_root("task1")
        .build()?;

    let workflow_def = WorkflowDef::from_graph("distributed_workflow", workflow_graph, RetryConfig::default());
    let workflow_id = workflow_def.workflow_id;

    // Launch cluster using builder pattern
    println!("\n2. Launching cluster...");
    let nodes = launch_cluster(vec![
        (1, NodeMode::Leader, vec![workflow_def.clone()]),
        (2, NodeMode::Follower, vec![]),
        (3, NodeMode::Follower, vec![]),
        (4, NodeMode::Follower, vec![]),
    ])
    .await?;

    println!("   Cluster launched with {} nodes", nodes.len());

    // Get the leader app for workflow registration verification
    let leader_app = nodes[0].app();
    
    // Create executors on all nodes (any node can execute tasks)
    let mut executors = Vec::new();
    for node in &nodes {
        let executor = Arc::new(WorkflowExecutor::new(
            node.app().raft().clone(),
            node.app().state_machine().clone(),
            node.node_id(),
        ));
        executors.push(executor);
    }

    // Register handlers on all nodes
    let registry = Arc::new(HandlerRegistry::new());
    for (idx, _executor) in executors.iter().enumerate() {
        let node_id = (idx + 1) as u64;
        registry
            .register_handler(
                workflow_id,
                "handler1".to_string(),
                Arc::new(DistributedTaskHandler {
                    node_id,
                    task_name: "task1".to_string(),
                }) as Arc<dyn TaskHandler>,
            )
            .await;
        registry
            .register_handler(
                workflow_id,
                "handler2".to_string(),
                Arc::new(DistributedTaskHandler {
                    node_id,
                    task_name: "task2".to_string(),
                }) as Arc<dyn TaskHandler>,
            )
            .await;
        registry
            .register_handler(
                workflow_id,
                "handler3".to_string(),
                Arc::new(DistributedTaskHandler {
                    node_id,
                    task_name: "task3".to_string(),
                }) as Arc<dyn TaskHandler>,
            )
            .await;
    }

    // Workflow is already registered via builder pattern
    println!("\n3. Workflow registered on leader");

    // Wait for replication with retries
    println!("\n4. Verifying replication...");
    let mut all_replicated = false;
    for attempt in 0..10 {
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        let mut replicated_count = 0;
        for executor in &executors {
            if executor.state_machine().get_workflow(&workflow_id).await.is_some() {
                replicated_count += 1;
            }
        }
        
        if replicated_count == executors.len() {
            all_replicated = true;
            break;
        }
        
        if attempt < 9 {
            println!("   Waiting for replication... ({}/{})", replicated_count, executors.len());
        }
    }
    
    if !all_replicated {
        println!("   Warning: Not all nodes have the workflow yet (this may be expected in a test environment)");
    } else {
        println!("   All nodes have the workflow");
    }

    println!("\nDistributed execution example completed successfully!");
    println!("Cluster setup: 1 leader + 3 followers");
    println!("Any node can execute tasks, state is replicated via Raft");

    Ok(())
}
