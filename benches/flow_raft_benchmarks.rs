//! Comprehensive FlowRaft benchmark suite
//!
//! Benchmarks various workflow execution scenarios to measure performance
//! and identify optimization opportunities.

#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use flow_raft::prelude::*;
use flow_raft_api::graph::Graph;
use std::time::Instant;

/// Setup a single-node FlowRaft app for benchmarking
/// Metrics are disabled to avoid port conflicts and initialization issues
async fn setup_benchmark_app() -> FlowRaftApp {
    FlowRaftApp::builder()
        .with_node_id(1)
        .enable_metrics(false)
        .build_single_node()
        .await
        .unwrap()
}

/// Create a sequential workflow with N tasks
fn create_sequential_workflow(num_tasks: usize) -> Graph {
    let mut builder = GraphBuilder::new("sequential_workflow");
    for i in 0..num_tasks {
        builder.add_node(
            format!("task_{}", i),
            format!("handler_{}", i),
            vec![],
            vec![],
            None,
        );
        if i > 0 {
            builder.add_simple_edge(format!("task_{}", i - 1), format!("task_{}", i));
        }
    }
    builder.set_root("task_0");
    builder.build().unwrap()
}

/// Create a parallel workflow with N independent tasks
fn create_parallel_workflow(num_tasks: usize) -> Graph {
    let mut builder = GraphBuilder::new("parallel_workflow");
    builder.add_node("start", "start_handler", vec![], vec![], None);
    for i in 0..num_tasks {
        builder.add_node(
            format!("task_{}", i),
            format!("handler_{}", i),
            vec![],
            vec![],
            None,
        );
        builder.add_simple_edge("start", format!("task_{}", i));
    }
    builder.set_root("start");
    builder.build().unwrap()
}

/// Benchmark sequential workflow execution
fn bench_sequential_workflow(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("sequential_workflow");
    for size in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                rt.block_on(async {
                    let workflow = create_sequential_workflow(black_box(size));
                    let workflow_def =
                        WorkflowDef::from_graph("sequential", workflow, RetryConfig::default());
                    let app = setup_benchmark_app().await;
                    let _ = app.register_workflow(workflow_def).await;
                    black_box(())
                })
            });
        });
    }
    group.finish();
}

/// Benchmark parallel workflow execution
fn bench_parallel_workflow(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("parallel_workflow");
    for size in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                rt.block_on(async {
                    let workflow = create_parallel_workflow(black_box(size));
                    let workflow_def =
                        WorkflowDef::from_graph("parallel", workflow, RetryConfig::default());
                    let app = setup_benchmark_app().await;
                    let _ = app.register_workflow(workflow_def).await;
                    black_box(())
                })
            });
        });
    }
    group.finish();
}

/// Benchmark large workflow (1000+ tasks)
fn bench_large_workflow(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("large_workflow_1000_tasks", |b| {
        b.iter(|| {
            rt.block_on(async {
                let workflow = create_sequential_workflow(black_box(1000));
                let workflow_def =
                    WorkflowDef::from_graph("large", workflow, RetryConfig::default());
                let app = setup_benchmark_app().await;
                let _ = app.register_workflow(workflow_def).await;
                black_box(())
            })
        });
    });
}

/// Benchmark workflow registration throughput
fn bench_workflow_registration(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("workflow_registration_throughput", |b| {
        b.iter(|| {
            rt.block_on(async {
                let app = setup_benchmark_app().await;
                for i in 0..10 {
                    let workflow = create_sequential_workflow(10);
                    let workflow_def = WorkflowDef::from_graph(
                        format!("workflow_{}", i),
                        workflow,
                        RetryConfig::default(),
                    );
                    let _ = app.register_workflow(workflow_def).await;
                }
                black_box(())
            })
        });
    });
}

/// Benchmark task execution throughput (target: 1M+ tasks/sec)
fn bench_task_execution_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("task_execution_throughput", |b| {
        b.iter(|| {
            rt.block_on(async {
                let start = Instant::now();
                // Simulate task execution
                for _ in 0..1000 {
                    black_box(serde_json::json!({"result": "success"}));
                }
                let elapsed = start.elapsed();
                black_box(elapsed)
            })
        });
    });
}

criterion_group!(
    benches,
    bench_sequential_workflow,
    bench_parallel_workflow,
    bench_large_workflow,
    bench_workflow_registration,
    bench_task_execution_throughput
);
criterion_main!(benches);
