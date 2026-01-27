//! Workflow creation, scheduling, and execution benchmarks.
//!
//! - **create_and_schedule**: build workflow from graph, schedule, start (no Raft).
//! - **run_order_pipeline**: full run (single-node app, register, set input, execute) using
//!   flow_raft_testing::workflows::order_pipeline.

#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use flow_raft::graph::graph_to_workflow;
use flow_raft::prelude::*;
use flow_raft_core::{RetryConfig, WorkflowId};
use flow_raft_testing::workflows::{linear_nop_graph, order_pipeline_cases, order_pipeline_graph};
use std::hint::black_box;

fn create_and_schedule_linear(n: usize) {
    let g = linear_nop_graph(n, "exec_linear");
    let graph = g.graph().clone();
    let w = graph_to_workflow(
        graph,
        WorkflowId::default(),
        RetryConfig::default(),
        serde_json::Value::Null,
    )
    .unwrap();
    let s = w.schedule().unwrap();
    let r = s.start().unwrap();
    black_box(r);
}

async fn run_order_pipeline_once() {
    let (input, _expected) = order_pipeline_cases().into_iter().next().unwrap();
    let g = order_pipeline_graph();
    let def = g.workflow_def("order_pipeline").unwrap();
    let app = FlowRaftAppBuilder::new()
        .with_node_id(1)
        .with_workflows(vec![def.clone()])
        .enable_metrics(false)
        .build_single_node()
        .await
        .unwrap();
    let reg = std::sync::Arc::new(HandlerRegistry::new());
    register_typed_graph_handlers(reg.as_ref(), def.workflow_id, &g).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    if let Some(mut w) = app.get_workflow(&def.workflow_id).await {
        w.inputs = serde_json::to_value(&input).unwrap();
        app.create_workflow(
            flow_raft_raft::command::WorkflowCommandBuilder::transition_workflow(
                def.workflow_id,
                w,
            ),
        )
        .await
        .unwrap();
    }
    let exec = std::sync::Arc::new(WorkflowExecutor::new(
        app.raft().clone(),
        app.state_machine().clone(),
        1,
    ));
    HandlerExecutor::new(exec, reg)
        .execute_workflow(def.workflow_id, 100)
        .await
        .unwrap();
    black_box(());
}

fn bench_create_schedule(c: &mut Criterion) {
    let mut g = c.benchmark_group("create_and_schedule");
    for n in [10, 100] {
        g.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| create_and_schedule_linear(black_box(n)));
        });
    }
    g.finish();
}

fn bench_run_order_pipeline(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    c.bench_function("run_order_pipeline", |b| {
        b.iter(|| rt.block_on(run_order_pipeline_once()));
    });
}

criterion_group!(benches, bench_create_schedule, bench_run_order_pipeline);
criterion_main!(benches);
