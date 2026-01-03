//! Parallel workflow example
//!
//! Demonstrates split/merge edges with:
//! - Function-based nodes
//! - Parallel task execution
//! - Split operations
//! - Merge operations

use flow_raft::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Order {
    id: String,
    items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ItemResult {
    item: String,
    processed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderResult {
    order_id: String,
    items_processed: usize,
}

// Simple Rust functions for workflow nodes
fn split_order(order: Order) -> Result<Order, String> {
    println!(
        "Splitting order: {} into {} items",
        order.id,
        order.items.len()
    );
    Ok(order)
}

fn process_item(item: String) -> Result<ItemResult, String> {
    println!("Processing item: {}", item);
    Ok(ItemResult {
        item: item.clone(),
        processed: true,
    })
}

fn finalize_order(results: OrderResult) -> Result<OrderResult, String> {
    println!(
        "Finalizing order: {} with {} items processed",
        results.order_id, results.items_processed
    );
    Ok(results)
}

// Split object for split edges
#[derive(Debug)]
struct ItemSplitter;

impl SplitObject for ItemSplitter {
    fn evaluate(&self, input: serde_json::Value) -> Result<Vec<NodeName>, String> {
        let items = input
            .get("items")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing items array".to_string())?;

        let mut node_names = Vec::new();
        for (i, _) in items.iter().enumerate() {
            node_names.push(NodeName::new(format!("process_item_{}", i)));
        }
        Ok(node_names)
    }
}

// Merge object for merge edges
#[derive(Debug)]
struct ItemMerger;

impl MergeObject for ItemMerger {
    fn merge(&self, inputs: Vec<serde_json::Value>) -> Result<serde_json::Value, String> {
        let mut items_processed = 0;
        for input in inputs {
            if input
                .get("processed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                items_processed += 1;
            }
        }

        let result = OrderResult {
            order_id: "order_1".to_string(),
            items_processed,
        };

        serde_json::to_value(result).map_err(|e| format!("Failed to serialize result: {}", e))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Define workflow with function-based nodes and split/merge edges
    let mut builder = GraphBuilder::new("parallel_workflow");
    builder.add_node_fn("split", wrap_function(split_order), None);

    // Add parallel processing nodes
    for i in 0..3 {
        builder.add_node_fn(
            format!("process_item_{}", i),
            wrap_function(process_item),
            None,
        );
    }

    builder
        .add_node_fn("finalize", wrap_function(finalize_order), None)
        .add_split_edge(
            "split",
            Arc::new(ItemSplitter),
            vec!["process_item_0", "process_item_1", "process_item_2"],
        )
        .add_merge_edge(
            vec!["process_item_0", "process_item_1", "process_item_2"],
            Arc::new(ItemMerger),
            "finalize",
        )
        .set_root("split");

    let workflow = builder.build()?;

    let workflow_def = WorkflowDef::from_graph("parallel", workflow, RetryConfig::default());

    // Create single-node app
    let app = FlowRaftApp::builder()
        .with_node_id(1)
        .with_workflows(vec![workflow_def.clone()])
        .enable_metrics(true)
        .build_single_node()
        .await?;

    println!("✓ FlowRaft app created successfully!");

    // Setup execution infrastructure
    let workflow_id = workflow_def.workflow_id;
    let executor = Arc::new(WorkflowExecutor::new(
        app.raft().clone(),
        app.state_machine().clone(),
        1,
    ));
    let registry = Arc::new(HandlerRegistry::new());

    // Register handlers for function-based nodes
    registry
        .register_handler(
            workflow_id,
            "fn_split".to_string(),
            Arc::new(FunctionHandler {
                name: "split_order".to_string(),
                func: Box::new(|inputs: serde_json::Value| {
                    let order: Order = serde_json::from_value(inputs)
                        .map_err(|e| format!("Failed to deserialize order: {}", e))?;
                    let result = split_order(order)?;
                    serde_json::to_value(result)
                        .map_err(|e| format!("Failed to serialize result: {}", e))
                }),
            }) as Arc<dyn TaskHandler>,
        )
        .await;

    // Register handlers for parallel processing nodes
    for i in 0..3 {
        registry
            .register_handler(
                workflow_id,
                format!("fn_process_item_{}", i),
                Arc::new(FunctionHandler {
                    name: format!("process_item_{}", i),
                    func: Box::new(move |inputs: serde_json::Value| {
                        // Extract item from inputs (simplified - in real scenario would parse properly)
                        let item = inputs
                            .get("items")
                            .and_then(|v| v.as_array())
                            .and_then(|arr| arr.get(i))
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let result = process_item(item)?;
                        serde_json::to_value(result)
                            .map_err(|e| format!("Failed to serialize result: {}", e))
                    }),
                }) as Arc<dyn TaskHandler>,
            )
            .await;
    }

    registry
        .register_handler(
            workflow_id,
            "fn_finalize".to_string(),
            Arc::new(FunctionHandler {
                name: "finalize_order".to_string(),
                func: Box::new(|inputs: serde_json::Value| {
                    // The merge object will have already merged the results
                    // This handler receives the merged result
                    let result: OrderResult = serde_json::from_value(inputs)
                        .map_err(|e| format!("Failed to deserialize order result: {}", e))?;
                    let finalized = finalize_order(result)?;
                    serde_json::to_value(finalized)
                        .map_err(|e| format!("Failed to serialize finalized order: {}", e))
                }),
            }) as Arc<dyn TaskHandler>,
        )
        .await;

    // Wait for workflow to be registered
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Update workflow with inputs before execution
    if let Some(mut workflow) = app.get_workflow(&workflow_id).await {
        workflow.inputs = serde_json::to_value(Order {
            id: "order_789".to_string(),
            items: vec![
                "item1".to_string(),
                "item2".to_string(),
                "item3".to_string(),
            ],
        })
        .unwrap();

        use flow_raft_raft::command::WorkflowCommandBuilder;
        let request = WorkflowCommandBuilder::transition_workflow(workflow_id, workflow);
        app.create_workflow(request)
            .await
            .map_err(|e| format!("Failed to update workflow with inputs: {:?}", e))?;
    }

    // Execute workflow
    let handler_executor = HandlerExecutor::new(executor, registry);
    execute_and_display(&handler_executor, &app, workflow_id, 100).await?;

    println!("✓ Parallel workflow example completed!");
    Ok(())
}

// Helper handler that wraps functions
struct FunctionHandler {
    name: String,
    func: Box<dyn Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync>,
}

impl TaskHandler for FunctionHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("[{}] Executing with inputs: {:?}", self.name, inputs);
        (self.func)(inputs)
    }
}

// Helper function for execution
async fn execute_and_display(
    handler_executor: &HandlerExecutor,
    app: &FlowRaftApp,
    workflow_id: WorkflowId,
    max_iterations: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\nExecuting workflow {}...", workflow_id);

    match handler_executor
        .execute_workflow(workflow_id, max_iterations)
        .await
    {
        Ok(()) => {
            println!("\n✓ Workflow execution completed successfully!");

            if let Some(final_workflow) = app.get_workflow(&workflow_id).await {
                println!("\nFinal workflow state:");
                println!("  State: {:?}", final_workflow.state);
                println!(
                    "  Tasks completed: {}/{}",
                    final_workflow.executions.len(),
                    final_workflow.task_definitions.len()
                );

                if let Some(outputs) = &final_workflow.outputs {
                    println!(
                        "  Final outputs: {}",
                        serde_json::to_string_pretty(outputs).unwrap_or_default()
                    );
                }
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("✗ Workflow execution failed: {:?}", e);
            Err(Box::new(std::io::Error::other(format!(
                "Execution failed: {:?}",
                e
            ))))
        }
    }
}
