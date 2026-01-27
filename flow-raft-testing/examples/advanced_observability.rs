//! Advanced observability example
//!
//! Typed API: single typed node, metrics and tracing.
//! - task1(input) -> output via node(...)
//!
//! Runs a workflow with Input { value: "test" } and asserts the outcome is
//! Output { value: "processed: test" }. Also demonstrates metrics and a metrics loop.

use flow_raft::prelude::*;
use flow_raft_raft::command::WorkflowCommandBuilder;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Input {
    value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Output {
    value: String,
}

fn task1(input: Input) -> Result<Output, String> {
    Ok(Output {
        value: format!("processed: {}", input.value),
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let metrics = Arc::new(MetricsCollector::new());

    let mut builder = TypedGraphBuilder::new("observable_workflow");
    builder
        .add_node("task1", node(task1), None)
        .set_root("task1");
    let typed_graph = builder.build()?;
    let workflow_def = typed_graph.workflow_def("observable")?;

    let app = FlowRaftAppBuilder::new()
        .with_node_id(1)
        .with_workflows(vec![workflow_def.clone()])
        .enable_metrics(true)
        .with_metrics(metrics.clone())
        .with_tracing(flow_raft_observability::TracingExporter::None, None)
        .with_metrics_port(9090)
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

    let input = Input { value: "test".to_string() };
    if let Some(mut w) = app.get_workflow(&workflow_id).await {
        w.inputs = serde_json::to_value(&input).unwrap();
        app.create_workflow(WorkflowCommandBuilder::transition_workflow(workflow_id, w))
            .await?;
    }

    let handler_executor = HandlerExecutor::new(executor, registry);
    handler_executor.execute_workflow(workflow_id, 100).await?;

    let s = app.get_workflow(&workflow_id).await.expect("workflow");
    let tid = s
        .task_definitions
        .iter()
        .find(|(_, d)| d.name == "task1")
        .map(|(id, _)| *id)
        .expect("task1");
    let out = s
        .executions
        .get(&tid)
        .and_then(|e| e.outputs.as_ref())
        .cloned()
        .expect("task1 output");
    let out: Output = serde_json::from_value(out).expect("Output");
    assert_eq!(out.value, "processed: test", "task1 must transform input correctly");

    println!("✓ Observability example completed (output={})", out.value);
    Ok(())
}
