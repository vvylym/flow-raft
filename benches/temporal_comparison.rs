//! FlowRaft Workflow Benchmarks
//!
//! Benchmarks for FlowRaft workflow execution performance.
//! These benchmarks measure FlowRaft's own performance characteristics.

#![allow(missing_docs)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use flow_raft::prelude::*;
use std::time::Instant;

async fn benchmark_flowraft_simple_workflow() -> u64 {
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
    let _ = app.register_workflow(workflow_def).await.unwrap();

    start.elapsed().as_micros() as u64
}

fn benchmark_flowraft_latency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("flowraft_simple_workflow_latency", |b| {
        b.iter(|| {
            rt.block_on(async {
                let latency = benchmark_flowraft_simple_workflow().await;
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
                    let _ = benchmark_flowraft_simple_workflow().await;
                }
            })
        })
    });
}

// Additional benchmarks for comprehensive performance evaluation

fn benchmark_flowraft_conditional_workflow(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("flowraft_conditional_workflow", |b| {
        b.iter(|| {
            rt.block_on(async {
                let start = Instant::now();
                let app = FlowRaftApp::builder()
                    .with_node_id(1)
                    .enable_metrics(false)
                    .build_single_node()
                    .await
                    .unwrap();

                // Create workflow with conditional branches
                let mut builder = GraphBuilder::new("conditional");
                builder
                    .add_node("task1", "handler1", vec![], vec![], None)
                    .add_node("task2", "handler2", vec![], vec![], None)
                    .add_node("task3", "handler3", vec![], vec![], None)
                    .add_simple_edge("task1", "task2")
                    .add_simple_edge("task1", "task3")
                    .set_root("task1");
                let workflow_graph = builder.build().unwrap();

                let workflow_def =
                    WorkflowDef::from_graph("conditional", workflow_graph, RetryConfig::default());
                let _ = app.register_workflow(workflow_def).await;
                black_box(start.elapsed().as_micros() as u64)
            })
        })
    });
}

fn benchmark_flowraft_workflow_with_retries(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("flowraft_workflow_with_retries", |b| {
        b.iter(|| {
            rt.block_on(async {
                let start = Instant::now();
                let app = FlowRaftApp::builder()
                    .with_node_id(1)
                    .enable_metrics(false)
                    .build_single_node()
                    .await
                    .unwrap();

                // Create workflow with retry configuration
                let retry_config = RetryConfig::with_backoff(3, 2.0, 100);

                let mut builder = GraphBuilder::new("retry");
                builder
                    .add_node("task1", "handler1", vec![], vec![], None)
                    .set_root("task1");
                let workflow_graph = builder.build().unwrap();

                let workflow_def = WorkflowDef::from_graph("retry", workflow_graph, retry_config);
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
    benchmark_flowraft_conditional_workflow,
    benchmark_flowraft_workflow_with_retries
);
criterion_main!(benches);
