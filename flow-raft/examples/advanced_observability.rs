//! Advanced observability example
//!
//! Demonstrates:
//! - Metrics collection
//! - Distributed tracing
//! - Execution history
//! - Real-time monitoring

use flow_raft::prelude::*;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create metrics collector
    let metrics = Arc::new(MetricsCollector::new());

    // Define workflow
    let workflow_graph = GraphBuilder::new("observable_workflow")
        .add_node(
            "task1",
            "handler1",
            vec!["input".to_string()],
            vec!["output".to_string()],
            None,
        )
        .set_root("task1")
        .build()?;

    let workflow_def =
        WorkflowDef::from_graph("observable", workflow_graph, RetryConfig::default());

    // Create app with metrics and tracing
    // Note: Tracing is initialized by the builder, so we don't initialize it again
    let _app = FlowRaftApp::builder()
        .with_node_id(1)
        .with_workflows(vec![workflow_def.clone()])
        .enable_metrics(true)
        .with_metrics(metrics.clone())
        .with_tracing(flow_raft_observability::TracingExporter::None, None)
        .with_metrics_port(9090)
        .build_single_node()
        .await?;

    println!("✓ Observability example setup complete!");
    println!("  - Metrics available at http://localhost:9090/metrics");
    println!("  - Health check at http://localhost:9090/health");
    println!("  - Tracing enabled (console output)");

    // Monitor metrics
    tokio::spawn(async move {
        loop {
            let summary = metrics.get_metrics_summary().await;
            println!("Metrics summary: {:?}", summary);
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    println!("\n✓ Advanced observability example completed!");
    Ok(())
}
