//! Simple sequential workflow example
//!
//! Demonstrates a basic workflow with 3 sequential tasks.

use flow_raft::api::graph::GraphBuilder;
use flow_raft::api::handlers::HandlerRegistry;
use flow_raft::core::{RetryConfig, TaskId, WorkflowId};
use flow_raft::raft::app::FlowRaftApp;
use flow_raft::raft::config::default_config;
use flow_raft::raft::executor::{TaskHandler, WorkflowExecutor};
use flow_raft::raft::network::MemoryNetworkFactory;
use flow_raft::raft::storage::{LogStore, StateMachineStore};
use flow_raft::raft::types::Request;
use flow_raft::api::graph::converter::graph_to_workflow;
use std::sync::Arc;

struct SimpleTaskHandler {
    name: String,
}

impl TaskHandler for SimpleTaskHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("Executing task: {} with inputs: {}", self.name, inputs);
        Ok(serde_json::json!({
            "task": self.name,
            "result": "success",
            "inputs": inputs
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Simple Sequential Workflow Example");

    // Create a simple graph with 3 sequential tasks
    let mut builder = GraphBuilder::new("simple_workflow");
    
    builder
        .add_node("task1", "handler1", vec!["input1".to_string()], vec!["output1".to_string()], None)
        .add_node("task2", "handler2", vec!["input2".to_string()], vec!["output2".to_string()], None)
        .add_node("task3", "handler3", vec!["input3".to_string()], vec!["output3".to_string()], None)
        .add_simple_edge("task1", "task2")
        .add_simple_edge("task2", "task3")
        .set_root("task1");

    let graph = builder.build()?;
    println!("Graph built with {} nodes", graph.nodes.len());

    // Convert graph to workflow
    let workflow_id = WorkflowId::default();
    let default_retry = RetryConfig::default();
    let workflow = graph_to_workflow(graph, workflow_id, default_retry.clone(), serde_json::json!({}))?;

    // Schedule and start workflow
    let scheduled = workflow.schedule()?;
    let running = scheduled.start()?;

    println!("Workflow scheduled and started");

    // Create Raft infrastructure
    let node_id = 1;
    let config = Arc::new(default_config().validate().unwrap());
    let network = MemoryNetworkFactory::new();
    let log_store = LogStore::default();
    let state_machine = StateMachineStore::default();

    let raft = openraft::Raft::new(node_id, config, network, log_store, state_machine.clone())
        .await?;
    let raft = Arc::new(raft);

    // Initialize cluster
    raft.initialize([1u64].into_iter().collect::<std::collections::BTreeSet<_>>())
        .await?;

    // Create app and executor
    let app = Arc::new(FlowRaftApp::new(raft.clone(), state_machine.clone()));
    let executor = Arc::new(WorkflowExecutor::new(raft, state_machine.clone(), node_id));
    let registry = Arc::new(HandlerRegistry::new());

    // Register handlers
    registry
        .register_handler(
            workflow_id,
            "handler1".to_string(),
            Arc::new(SimpleTaskHandler {
                name: "task1".to_string(),
            }) as Arc<dyn TaskHandler>,
        )
        .await;
    registry
        .register_handler(
            workflow_id,
            "handler2".to_string(),
            Arc::new(SimpleTaskHandler {
                name: "task2".to_string(),
            }) as Arc<dyn TaskHandler>,
        )
        .await;
    registry
        .register_handler(
            workflow_id,
            "handler3".to_string(),
            Arc::new(SimpleTaskHandler {
                name: "task3".to_string(),
            }) as Arc<dyn TaskHandler>,
        )
        .await;

    // Create workflow snapshot and store it
    let snapshot = flow_raft::core::WorkflowSnapshot::from_workflow(&running);
    let request = Request::CreateWorkflow {
        workflow: snapshot.clone(),
    };
    app.create_workflow(request).await?;

    println!("Workflow created in Raft cluster");
    
    // Wait for workflow to be stored
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Execute the workflow
    println!("\nExecuting workflow...");
    let handler_executor = flow_raft::api::handlers::executor::HandlerExecutor::new(
        executor.clone(),
        registry.clone(),
    );
    
    match handler_executor.execute_workflow(workflow_id, 100).await {
        Ok(()) => {
            println!("\n✓ Workflow execution completed successfully!");
            
            // Show final workflow state
            if let Some(final_workflow) = app.get_workflow(&workflow_id).await {
                println!("\nFinal workflow state:");
                println!("  State: {:?}", final_workflow.state);
                println!("  Tasks completed: {}/{}", 
                    final_workflow.executions.len(),
                    final_workflow.task_definitions.len());
                
                if let Some(outputs) = &final_workflow.outputs {
                    println!("  Final outputs: {}", serde_json::to_string_pretty(outputs).unwrap_or_default());
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Workflow execution failed: {:?}", e);
        }
    }

    println!("\nExample completed!");
    Ok(())
}
