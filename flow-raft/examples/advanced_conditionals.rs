//! Advanced conditionals example
//!
//! Demonstrates:
//! - Complex branching logic
//! - Multi-way conditionals
//! - State-dependent routing
//! - Conditional retries

use flow_raft::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Request {
    id: String,
    priority: String,
    amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ValidationResult {
    request_id: String,
    valid: bool,
    priority: String,
}

// Complex condition for routing
#[derive(Debug)]
struct PriorityRouter;

impl ConditionObject for PriorityRouter {
    fn evaluate(&self, input: serde_json::Value) -> Result<NodeName, String> {
        let priority = input
            .get("priority")
            .and_then(|v| v.as_str())
            .unwrap_or("standard");

        match priority {
            "high" => Ok(NodeName::new("high_priority_handler")),
            "medium" => Ok(NodeName::new("medium_priority_handler")),
            _ => Ok(NodeName::new("standard_handler")),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Define workflow with complex conditionals
    let workflow = GraphBuilder::new("conditional_routing")
        .add_node_fn("validate", wrap_function(validate_request), None)
        .add_node(
            "high_priority_handler",
            "high_handler",
            vec!["request".to_string()],
            vec!["result".to_string()],
            None,
        )
        .add_node(
            "medium_priority_handler",
            "medium_handler",
            vec!["request".to_string()],
            vec!["result".to_string()],
            None,
        )
        .add_node(
            "standard_handler",
            "standard_handler",
            vec!["request".to_string()],
            vec!["result".to_string()],
            None,
        )
        .add_conditional_edge(
            "validate",
            Arc::new(PriorityRouter),
            "high_priority_handler",
            "medium_priority_handler",
        )
        .set_root("validate")
        .build()?;

    let workflow_def =
        WorkflowDef::from_graph("conditional_routing", workflow, RetryConfig::default());

    // Create app
    let _app = FlowRaftApp::builder()
        .with_node_id(1)
        .with_workflows(vec![workflow_def.clone()])
        .enable_metrics(true)
        .build_single_node()
        .await?;

    println!("✓ Advanced conditionals example setup complete!");
    println!("This demonstrates complex branching based on request priority.");
    Ok(())
}

fn validate_request(request: Request) -> Result<ValidationResult, String> {
    Ok(ValidationResult {
        request_id: request.id,
        valid: true,
        priority: request.priority,
    })
}
