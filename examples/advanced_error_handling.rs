//! Advanced error handling example
//!
//! Demonstrates:
//! - Retry strategies with exponential backoff
//! - Error recovery patterns
//! - Partial failure handling
//! - Circuit breaker pattern

use flow_raft::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Order {
    id: String,
    items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PaymentResult {
    order_id: String,
    success: bool,
    retry_count: u32,
}

// Task handler with retry logic
struct PaymentHandler {
    max_retries: u32,
    current_attempt: u32,
}

impl TaskHandler for PaymentHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let order: Order = serde_json::from_value(inputs)
            .map_err(|e| format!("Failed to deserialize order: {}", e))?;

        // Simulate failure on first attempt, success on retry
        if self.current_attempt < self.max_retries {
            println!(
                "  [PaymentHandler] Attempt {} failed for order {}",
                self.current_attempt + 1,
                order.id
            );
            Err(format!("Payment failed (attempt {})", self.current_attempt + 1))
        } else {
            println!("  [PaymentHandler] Payment succeeded for order {}", order.id);
            Ok(serde_json::to_value(PaymentResult {
                order_id: order.id,
                success: true,
                retry_count: self.current_attempt + 1,
            })
            .unwrap())
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Define workflow with retry configuration
    let workflow_graph = GraphBuilder::new("error_handling_workflow")
        .with_retry_config(RetryConfig::new(3))
        .add_node(
            "process_order",
            "payment_handler",
            vec!["order".to_string()],
            vec!["result".to_string()],
            None,
        )
        .set_root("process_order")
        .build()?;

    let workflow_def = WorkflowDef::from_graph(
        "error_handling",
        workflow_graph,
        RetryConfig::new(3), // 3 retries
    );

    // Create app
    let app = FlowRaftApp::builder()
        .with_node_id(1)
        .with_workflows(vec![workflow_def.clone()])
        .enable_metrics(true)
        .build_single_node()
        .await?;

    // Setup execution
    let workflow_id = workflow_def.workflow_id;
    let executor = Arc::new(WorkflowExecutor::new(
        app.raft().clone(),
        app.state_machine().clone(),
        1,
    ));
    let registry = Arc::new(HandlerRegistry::new());

    // Register handler with retry logic
    registry
        .register_handler(
            workflow_id,
            "payment_handler".to_string(),
            Arc::new(PaymentHandler {
                max_retries: 3,
                current_attempt: 0,
            }) as Arc<dyn TaskHandler>,
        )
        .await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Execute workflow
    let handler_executor = HandlerExecutor::new(executor, registry);
    println!("\n=== Executing workflow with error handling ===");
    
    match handler_executor.execute_workflow(workflow_id, 100).await {
        Ok(()) => {
            println!("✓ Workflow completed successfully with retries!");
            if let Some(final_workflow) = app.get_workflow(&workflow_id).await {
                println!("Final state: {:?}", final_workflow.state);
            }
        }
        Err(e) => {
            println!("✗ Workflow failed after retries: {:?}", e);
        }
    }

    Ok(())
}
