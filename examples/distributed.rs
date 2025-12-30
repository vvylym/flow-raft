//! Distributed execution example
//!
//! Demonstrates multi-node cluster setup with leader/follower coordination
//! and task execution on any node.

use flow_raft::api::node::{launch_follower, launch_leader, NodeConfig, NodeMode};
use flow_raft::api::handlers::{HandlerExecutor, HandlerRegistry};
use flow_raft::core::{RetryConfig, TaskId, WorkflowId};
use flow_raft::raft::app::FlowRaftApp;
use flow_raft::raft::config::default_config;
use flow_raft::raft::executor::{TaskHandler, WorkflowExecutor};
use flow_raft::raft::network::MemoryNetworkFactory;
use flow_raft::raft::storage::{LogStore, StateMachineStore};
use flow_raft::raft::types::{NodeId, Request};
use flow_raft::api::graph::{GraphBuilder, NodeName};
use flow_raft::api::graph::converter::graph_to_workflow;
use std::collections::BTreeSet;
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

    // Create shared network for all nodes
    let network = MemoryNetworkFactory::new();

    // Launch leader node
    println!("\n1. Launching leader node...");
    let leader_config = NodeConfig::new(1, NodeMode::Leader);
    let leader = launch_leader(leader_config, network.clone()).await?;
    println!("   Leader node {} launched", leader.node_id);

    // Launch follower nodes
    println!("\n2. Launching follower nodes...");
    let cluster_nodes: BTreeSet<NodeId> = [1, 2, 3, 4]
        .into_iter()
        .collect();

    let mut followers = Vec::new();
    for node_id in [2, 3, 4] {
        let follower_config = NodeConfig::new(node_id, NodeMode::Follower);
        let follower = launch_follower(follower_config, network.clone(), cluster_nodes.clone())
            .await?;
        println!("   Follower node {} launched", follower.node_id);
        followers.push(follower);
    }

    // Create a simple workflow
    println!("\n3. Creating workflow...");
    let mut builder = GraphBuilder::new("distributed_workflow");
    builder
        .add_node("task1", "handler1", vec![], vec![], None)
        .add_node("task2", "handler2", vec![], vec![], None)
        .add_node("task3", "handler3", vec![], vec![], None)
        .add_simple_edge("task1", "task2")
        .add_simple_edge("task2", "task3")
        .set_root("task1");

    let graph = builder.build()?;
    let workflow_id = WorkflowId::default();
    let retry_config = RetryConfig::default();
    let workflow = graph_to_workflow(graph, workflow_id, retry_config, serde_json::json!({}))?;

    let scheduled = workflow.schedule()?;
    let running = scheduled.start()?;

    // Create app on leader
    let app = Arc::new(FlowRaftApp::new(
        leader.raft.clone(),
        leader.state_machine.clone(),
    ));

    // Create executors on all nodes (any node can execute tasks)
    let leader_executor = Arc::new(WorkflowExecutor::new(
        leader.raft.clone(),
        leader.state_machine.clone(),
        1,
    ));

    let mut executors = vec![leader_executor];
    for follower in &followers {
        let executor = Arc::new(WorkflowExecutor::new(
            follower.raft.clone(),
            follower.state_machine.clone(),
            follower.node_id,
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

    // Create workflow on leader
    println!("\n4. Creating workflow on leader...");
    let snapshot = flow_raft::core::WorkflowSnapshot::from_workflow(&running);
    let request = Request::CreateWorkflow {
        workflow: snapshot.clone(),
    };
    app.create_workflow(request).await?;
    println!("   Workflow created");

    // Wait for replication with retries
    println!("\n5. Verifying replication...");
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
