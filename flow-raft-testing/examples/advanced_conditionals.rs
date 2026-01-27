//! Advanced conditionals example
//!
//! Typed API: multi-way switch based on priority.
//! - validate_request(Request) -> ValidationResult
//! - switch(|r: ValidationResult| ...) routes to high / medium / standard
//! - Branch handlers take ValidationResult and return a result type
//!
//! Runs a workflow with Request { priority: "high" } and asserts the outcome is
//! HandlerOutput { status: "processed_high" }.

use flow_raft::prelude::*;
use flow_raft_raft::command::WorkflowCommandBuilder;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HandlerOutput {
    request_id: String,
    priority: String,
    status: String,
}

fn validate_request(request: Request) -> Result<ValidationResult, String> {
    Ok(ValidationResult {
        request_id: request.id,
        valid: true,
        priority: request.priority,
    })
}

fn high_priority_handler(r: ValidationResult) -> Result<HandlerOutput, String> {
    Ok(HandlerOutput {
        request_id: r.request_id,
        priority: "high".into(),
        status: "processed_high".into(),
    })
}

fn medium_priority_handler(r: ValidationResult) -> Result<HandlerOutput, String> {
    Ok(HandlerOutput {
        request_id: r.request_id,
        priority: "medium".into(),
        status: "processed_medium".into(),
    })
}

fn standard_handler(r: ValidationResult) -> Result<HandlerOutput, String> {
    Ok(HandlerOutput {
        request_id: r.request_id,
        priority: "standard".into(),
        status: "processed_standard".into(),
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let mut builder = TypedGraphBuilder::new("conditional_routing");
    builder
        .add_node("validate", node(validate_request), None)
        .add_node("high_priority_handler", node(high_priority_handler), None)
        .add_node(
            "medium_priority_handler",
            node(medium_priority_handler),
            None,
        )
        .add_node("standard_handler", node(standard_handler), None)
        .add_switch_edge(
            "validate",
            switch(|r: ValidationResult| match r.priority.as_str() {
                "high" => "high_priority_handler".to_string(),
                "medium" => "medium_priority_handler".to_string(),
                _ => "standard_handler".to_string(),
            }),
            vec![
                "high_priority_handler",
                "medium_priority_handler",
                "standard_handler",
            ],
        )
        .set_root("validate");

    let typed_graph = builder.build()?;
    let workflow_def = typed_graph.workflow_def("conditional_routing")?;

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

    let request = Request {
        id: "req1".to_string(),
        priority: "high".to_string(),
        amount: 10.0,
    };
    if let Some(mut w) = app.get_workflow(&workflow_id).await {
        w.inputs = serde_json::to_value(&request).unwrap();
        app.create_workflow(WorkflowCommandBuilder::transition_workflow(workflow_id, w))
            .await?;
    }

    let handler_executor = HandlerExecutor::new(executor, registry);
    handler_executor.execute_workflow(workflow_id, 100).await?;

    let s = app.get_workflow(&workflow_id).await.expect("workflow");
    let tid = s
        .task_definitions
        .iter()
        .find(|(_, d)| d.name == "high_priority_handler")
        .map(|(id, _)| *id)
        .expect("high_priority_handler task");
    let out = s
        .executions
        .get(&tid)
        .and_then(|e| e.outputs.as_ref())
        .cloned()
        .expect("high_priority_handler output");
    let out: HandlerOutput = serde_json::from_value(out).expect("HandlerOutput");
    assert_eq!(out.status, "processed_high", "switch must route 'high' to high_priority_handler");

    println!("✓ Advanced conditionals example completed (status={})", out.status);
    Ok(())
}
