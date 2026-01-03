//! Simple sequential workflow example
//!
//! Demonstrates a basic workflow with 3 sequential tasks using the simplified API.

use flow_raft::prelude::*;
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

    // Convert graph to workflow definition
    let default_retry = RetryConfig::default();
    let workflow_def = WorkflowDef::from_graph("simple_workflow", graph, default_retry.clone());

    println!("Workflow definition created");

    // Create single-node app using builder pattern
    let app = FlowRaftApp::builder()
        .with_node_id(1)
        .with_workflows(vec![workflow_def.clone()])
        .enable_metrics(true)
        .build_single_node()
        .await?;

    println!("FlowRaft app created using builder pattern");

    // Get workflow ID from the definition
    let workflow_id = workflow_def.workflow_id;

    // Setup handlers for execution
    let executor = Arc::new(WorkflowExecutor::new(
        app.raft().clone(),
        app.state_machine().clone(),
        1,
    ));
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

    // Workflow is already registered via builder pattern
    println!("Workflow registered in Raft cluster");
    
    // Wait for workflow to be stored
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Execute the workflow
    println!("\nExecuting workflow...");
    let handler_executor = HandlerExecutor::new(
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
