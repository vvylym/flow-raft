//! Advanced error handling example
//!
//! Typed API: single node with retry configuration.
//! - process_order(Order) -> Result<PaymentResult, String>
//! - Graph built with with_retry_config for retries
//!
//! **Intent:** Demonstrates retry-then-fail when a handler errors. This example does *not*
//! create a workflow with inputs, so the engine runs the first task with empty/missing data;
//! process_order fails to deserialize ("missing field `id`"), retries up to the configured
//! limit, then the workflow fails. Check the logs for retries and the final error.

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

fn process_order(order: Order) -> Result<PaymentResult, String> {
    println!("  [process_order] Processing order: {}", order.id);
    Ok(PaymentResult {
        order_id: order.id,
        success: true,
        retry_count: 1,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let mut builder =
        TypedGraphBuilder::new("error_handling_workflow").with_retry_config(RetryConfig::new(3));
    builder
        .add_node("process_order", node(process_order), None)
        .set_root("process_order");
    let typed_graph = builder.build()?;
    let workflow_def = typed_graph.workflow_def("error_handling")?;

    let app = FlowRaftAppBuilder::new()
        .with_node_id(1)
        .with_workflows(vec![workflow_def.clone()])
        .enable_metrics(true)
        .build_single_node()
        .await?;

    let workflow_id = workflow_def.workflow_id;
    let executor = Arc::new(WorkflowExecutor::new(
        app.raft().clone(),
        app.state_machine().clone(),
        1,
    ));
    let registry = Arc::new(HandlerRegistry::new());
    register_typed_graph_handlers(registry.as_ref(), workflow_id, &typed_graph).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let handler_executor = HandlerExecutor::new(executor, registry);
    println!("\n=== Executing workflow with error handling ===");

    match handler_executor.execute_workflow(workflow_id, 100).await {
        Ok(()) => {
            println!("✓ Workflow completed successfully!");
            if let Some(final_workflow) = app.get_workflow(&workflow_id).await {
                println!("Final state: {:?}", final_workflow.state);
            }
        }
        Err(e) => {
            println!("✗ Workflow failed: {:?}", e);
        }
    }

    Ok(())
}
