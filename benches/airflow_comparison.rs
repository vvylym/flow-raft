//! FlowRaft Workflow Benchmarks
//!
//! Benchmarks for FlowRaft workflow execution performance.
//! These benchmarks measure FlowRaft's own performance characteristics.

#![allow(missing_docs)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use flow_raft::prelude::*;
use std::time::Instant;

async fn benchmark_flowraft_simple_workflow() -> Result<u64, String> {
    let start = Instant::now();

    // Setup using builder pattern (metrics disabled for benchmarks)
    let app = FlowRaftApp::builder()
        .with_node_id(1)
        .enable_metrics(false)
        .build_single_node()
        .await
        .unwrap();

    // Create workflow using simplified API
    let workflow_graph = GraphBuilder::new("simple")
        .add_node("task1", "handler1", vec![], vec![], None)
        .add_node("task2", "handler2", vec![], vec![], None)
        .add_node("task3", "handler3", vec![], vec![], None)
        .add_simple_edge("task1", "task2")
        .add_simple_edge("task2", "task3")
        .set_root("task1")
        .build()
        .unwrap();

    let workflow_def = WorkflowDef::from_graph("simple", workflow_graph, RetryConfig::default());
    app.register_workflow(workflow_def)
        .await
        .map_err(|e| format!("Failed to register workflow: {}", e))?;

    Ok(start.elapsed().as_micros() as u64)
}

fn benchmark_flowraft_latency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("flowraft_simple_workflow_latency", |b| {
        b.iter(|| {
            rt.block_on(async {
                let latency = benchmark_flowraft_simple_workflow().await.unwrap_or(0);
                black_box(latency)
            })
        })
    });
}

fn benchmark_flowraft_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("flowraft_workflow_throughput", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Create and store 10 workflows
                for _ in 0..10 {
                    let _ = benchmark_flowraft_simple_workflow().await.unwrap_or(0);
                }
            })
        })
    });
}

// Additional benchmarks for comprehensive performance evaluation

fn benchmark_flowraft_large_workflow(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("flowraft_large_workflow_100_tasks", |b| {
        b.iter(|| {
            rt.block_on(async {
                let start = Instant::now();
                let app = FlowRaftApp::builder()
                    .with_node_id(1)
                    .enable_metrics(false)
                    .build_single_node()
                    .await
                    .unwrap();

                // Create workflow with 100 tasks
                let mut builder = GraphBuilder::new("large");
                for i in 1..=100 {
                    builder.add_node(
                        format!("task{}", i),
                        format!("handler{}", i),
                        vec![],
                        vec![],
                        None,
                    );
                    if i > 1 {
                        builder.add_simple_edge(format!("task{}", i - 1), format!("task{}", i));
                    }
                }
                builder.set_root("task1");

                let workflow_graph = builder.build().unwrap();
                let workflow_def =
                    WorkflowDef::from_graph("large", workflow_graph, RetryConfig::default());
                let _ = app.register_workflow(workflow_def).await;
                black_box(start.elapsed().as_micros() as u64)
            })
        })
    });
}

fn benchmark_flowraft_parallel_workflow(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("flowraft_parallel_workflow", |b| {
        b.iter(|| {
            rt.block_on(async {
                let start = Instant::now();
                let app = FlowRaftApp::builder()
                    .with_node_id(1)
                    .enable_metrics(false)
                    .build_single_node()
                    .await
                    .unwrap();

                // Create workflow with parallel branches
                let mut builder = GraphBuilder::new("parallel");
                builder
                    .add_node("task1", "handler1", vec![], vec![], None)
                    .add_node("task2", "handler2", vec![], vec![], None)
                    .add_node("task3", "handler3", vec![], vec![], None)
                    .add_node("task4", "handler4", vec![], vec![], None)
                    .add_node("task5", "handler5", vec![], vec![], None)
                    .add_simple_edge("task1", "task2")
                    .add_simple_edge("task1", "task3")
                    .add_simple_edge("task2", "task4")
                    .add_simple_edge("task3", "task4")
                    .add_simple_edge("task4", "task5")
                    .set_root("task1");
                let workflow_graph = builder.build().unwrap();

                let workflow_def =
                    WorkflowDef::from_graph("parallel", workflow_graph, RetryConfig::default());
                let _ = app.register_workflow(workflow_def).await;
                black_box(start.elapsed().as_micros() as u64)
            })
        })
    });
}

criterion_group!(
    benches,
    benchmark_flowraft_latency,
    benchmark_flowraft_throughput,
    benchmark_flowraft_large_workflow,
    benchmark_flowraft_parallel_workflow
);
criterion_main!(benches);
