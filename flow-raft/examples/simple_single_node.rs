//! Simple single-node workflow example
//!
//! Demonstrates the simplified API with:
//! - Function-based node definitions using simple Rust functions
//! - Builder pattern for FlowRaftApp
//! - Simple workflow execution
//! - Execution tracking

use flow_raft::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Order {
    id: String,
    amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Payment {
    order_id: String,
    amount: f64,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt {
    order_id: String,
    payment_id: String,
    total: f64,
}

// Simple Rust functions for workflow nodes
fn process_order(order: Order) -> Result<Payment, String> {
    println!("Processing order: {}", order.id);
    Ok(Payment {
        order_id: order.id.clone(),
        amount: order.amount,
        status: "processed".to_string(),
    })
}

fn charge_payment(payment: Payment) -> Result<Receipt, String> {
    println!("Charging payment for order: {}", payment.order_id);
    Ok(Receipt {
        order_id: payment.order_id.clone(),
        payment_id: format!("pay_{}", payment.order_id),
        total: payment.amount,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Define workflow using function-based nodes
    let workflow_graph = GraphBuilder::new("order_processing")
        .with_retry_config(RetryConfig::new(3))
        .add_node_fn("process", wrap_function(process_order), None)
        .add_node_fn("charge", wrap_function(charge_payment), None)
        .add_edge("process", "charge")
        .set_root("process")
        .build()?;

    // Convert graph to workflow definition
    let workflow_def =
        WorkflowDef::from_graph("order_processing", workflow_graph, RetryConfig::new(3));

    // Create single-node app using builder pattern
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
    // Handler names are "fn_{node_name}" when using add_node_fn
    registry
        .register_handler(
            workflow_id,
            "fn_process".to_string(),
            Arc::new(FunctionHandler {
                name: "process_order".to_string(),
                func: Box::new(|inputs: serde_json::Value| {
                    let order: Order = serde_json::from_value(inputs)
                        .map_err(|e| format!("Failed to deserialize order: {}", e))?;
                    let payment = process_order(order)?;
                    serde_json::to_value(payment)
                        .map_err(|e| format!("Failed to serialize payment: {}", e))
                }),
            }) as Arc<dyn TaskHandler>,
        )
        .await;
    registry
        .register_handler(
            workflow_id,
            "fn_charge".to_string(),
            Arc::new(FunctionHandler {
                name: "charge_payment".to_string(),
                func: Box::new(|inputs: serde_json::Value| {
                    let payment: Payment = serde_json::from_value(inputs)
                        .map_err(|e| format!("Failed to deserialize payment: {}", e))?;
                    let receipt = charge_payment(payment)?;
                    serde_json::to_value(receipt)
                        .map_err(|e| format!("Failed to serialize receipt: {}", e))
                }),
            }) as Arc<dyn TaskHandler>,
        )
        .await;

    // Wait for workflow to be registered
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Update workflow with inputs before execution
    if let Some(mut workflow) = app.get_workflow(&workflow_id).await {
        workflow.inputs = serde_json::to_value(Order {
            id: "order_123".to_string(),
            amount: 99.99,
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

    println!("✓ Example completed successfully!");
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
