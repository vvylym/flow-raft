//! Workflow registration benchmarks.
//!
//! Measures time to build and register workflows of different shapes and sizes
//! using the shared flow_raft_testing::workflows::bench_workflows builders.

#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use flow_raft::prelude::*;
use flow_raft_testing::workflows::{conditional_nop_graph, linear_nop_graph, parallel_nop_graph};
use std::hint::black_box;

async fn register_linear(n: usize) {
    let g = linear_nop_graph(n, "reg_linear");
    let def = g.workflow_def("reg_linear").unwrap();
    let app = FlowRaftAppBuilder::new()
        .with_node_id(1)
        .enable_metrics(false)
        .build_single_node()
        .await
        .unwrap();
    let _ = app.register_workflow(def).await;
    black_box(());
}

async fn register_parallel(n: usize) {
    let g = parallel_nop_graph(n, "reg_parallel");
    let def = g.workflow_def("reg_parallel").unwrap();
    let app = FlowRaftAppBuilder::new()
        .with_node_id(1)
        .enable_metrics(false)
        .build_single_node()
        .await
        .unwrap();
    let _ = app.register_workflow(def).await;
    black_box(());
}

async fn register_conditional() {
    let g = conditional_nop_graph("reg_conditional");
    let def = g.workflow_def("reg_conditional").unwrap();
    let app = FlowRaftAppBuilder::new()
        .with_node_id(1)
        .enable_metrics(false)
        .build_single_node()
        .await
        .unwrap();
    let _ = app.register_workflow(def).await;
    black_box(());
}

fn bench_registration(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut g = c.benchmark_group("registration");
    for n in [10, 50, 100] {
        g.bench_with_input(BenchmarkId::new("linear", n), &n, |b, &n| {
            b.iter(|| rt.block_on(register_linear(black_box(n))));
        });
        g.bench_with_input(BenchmarkId::new("parallel", n), &n, |b, &n| {
            b.iter(|| rt.block_on(register_parallel(black_box(n))));
        });
    }
    g.bench_function("conditional", |b| {
        b.iter(|| rt.block_on(register_conditional()));
    });
    g.finish();
}

criterion_group!(benches, bench_registration);
criterion_main!(benches);
