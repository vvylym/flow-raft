//! Conditional workflow example
//!
//! Demonstrates conditional edges with:
//! - Function-based nodes
//! - Conditional edges
//! - Branching logic

use flow_raft::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Order {
    id: String,
    amount: f64,
    valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ValidationResult {
    order_id: String,
    valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProcessedOrder {
    order_id: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RejectedOrder {
    order_id: String,
    reason: String,
}

// Simple Rust functions for workflow nodes
fn validate_order(order: Order) -> Result<ValidationResult, String> {
    println!("Validating order: {}", order.id);
    Ok(ValidationResult {
        order_id: order.id.clone(),
        valid: order.valid,
    })
}

fn process_order(result: ValidationResult) -> Result<ProcessedOrder, String> {
    println!("Processing order: {}", result.order_id);
    Ok(ProcessedOrder {
        order_id: result.order_id.clone(),
        status: "processed".to_string(),
    })
}

fn reject_order(result: ValidationResult) -> Result<RejectedOrder, String> {
    println!("Rejecting order: {}", result.order_id);
    Ok(RejectedOrder {
        order_id: result.order_id.clone(),
        reason: "validation failed".to_string(),
    })
}

// Condition object for conditional edges
#[derive(Debug)]
struct ValidationCondition;

impl ConditionObject for ValidationCondition {
    fn evaluate(&self, input: serde_json::Value) -> Result<NodeName, String> {
        let valid = input
            .get("valid")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if valid {
            Ok(NodeName::new("process"))
        } else {
            Ok(NodeName::new("reject"))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Define workflow with function-based nodes and conditional edge
    let workflow = GraphBuilder::new("conditional_workflow")
        .add_node_fn("validate", wrap_function(validate_order), None)
        .add_node_fn("process", wrap_function(process_order), None)
        .add_node_fn("reject", wrap_function(reject_order), None)
        .add_conditional_edge(
            "validate",
            Arc::new(ValidationCondition),
            "process",
            "reject",
        )
        .set_root("validate")
        .build()?;

    let workflow_def = WorkflowDef::from_graph("conditional", workflow, RetryConfig::default());

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
            "fn_validate".to_string(),
            Arc::new(FunctionHandler {
                name: "validate_order".to_string(),
                func: Box::new(|inputs: serde_json::Value| {
                    let order: Order = serde_json::from_value(inputs)
                        .map_err(|e| format!("Failed to deserialize order: {}", e))?;
                    let result = validate_order(order)?;
                    serde_json::to_value(result)
                        .map_err(|e| format!("Failed to serialize result: {}", e))
                }),
            }) as Arc<dyn TaskHandler>,
        )
        .await;

    registry
        .register_handler(
            workflow_id,
            "fn_process".to_string(),
            Arc::new(FunctionHandler {
                name: "process_order".to_string(),
                func: Box::new(|inputs: serde_json::Value| {
                    let result: ValidationResult = serde_json::from_value(inputs)
                        .map_err(|e| format!("Failed to deserialize validation result: {}", e))?;
                    let processed = process_order(result)?;
                    serde_json::to_value(processed)
                        .map_err(|e| format!("Failed to serialize processed order: {}", e))
                }),
            }) as Arc<dyn TaskHandler>,
        )
        .await;

    registry
        .register_handler(
            workflow_id,
            "fn_reject".to_string(),
            Arc::new(FunctionHandler {
                name: "reject_order".to_string(),
                func: Box::new(|inputs: serde_json::Value| {
                    let result: ValidationResult = serde_json::from_value(inputs)
                        .map_err(|e| format!("Failed to deserialize validation result: {}", e))?;
                    let rejected = reject_order(result)?;
                    serde_json::to_value(rejected)
                        .map_err(|e| format!("Failed to serialize rejected order: {}", e))
                }),
            }) as Arc<dyn TaskHandler>,
        )
        .await;

    // Wait for workflow to be registered
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Update workflow with inputs before execution
    if let Some(mut workflow) = app.get_workflow(&workflow_id).await {
        workflow.inputs = serde_json::to_value(Order {
            id: "order_456".to_string(),
            amount: 150.0,
            valid: true,
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

    println!("✓ Conditional workflow example completed!");
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
