//! Comparison benchmarks with Temporal
//!
//! Note: This is a placeholder for comparison benchmarks.
//! In a real implementation, you would need to set up Temporal
//! and run equivalent workflows to compare performance.

#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use flow_raft::api::graph::GraphBuilder;
use flow_raft::api::graph::converter::graph_to_workflow;
use flow_raft::core::{RetryConfig, WorkflowId};
use flow_raft::raft::config::default_config;
use flow_raft::raft::executor::{TaskHandler, WorkflowExecutor};
use flow_raft::raft::network::MemoryNetworkFactory;
use flow_raft::raft::storage::{LogStore, StateMachineStore};
use flow_raft::raft::app::FlowRaftApp;
use std::sync::Arc;
use std::collections::BTreeSet;
use std::time::Instant;

struct SimpleTaskHandler;

impl TaskHandler for SimpleTaskHandler {
    fn execute(
        &self,
        _task_id: flow_raft::core::TaskId,
        _inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        // Simulate minimal work
        Ok(serde_json::json!({"result": "success"}))
    }
}

async fn benchmark_flowraft_simple_workflow() -> u64 {
    let start = Instant::now();
    
    // Setup
    let node_id = 1;
    let config = Arc::new(default_config().validate().unwrap());
    let network = MemoryNetworkFactory::new();
    let log_store = LogStore::default();
    let state_machine = StateMachineStore::default();

    let raft = openraft::Raft::new(node_id, config, network, log_store, state_machine.clone())
        .await
        .unwrap();
    let raft = Arc::new(raft);

    raft.initialize([1u64].into_iter().collect::<BTreeSet<_>>())
        .await
        .unwrap();

    let app = Arc::new(FlowRaftApp::new(raft.clone(), state_machine.clone()));
    
    // Create workflow
    let mut builder = GraphBuilder::new("simple");
    builder
        .add_node("task1", "handler1", vec![], vec![], None)
        .add_node("task2", "handler2", vec![], vec![], None)
        .add_node("task3", "handler3", vec![], vec![], None)
        .add_simple_edge("task1", "task2")
        .add_simple_edge("task2", "task3")
        .set_root("task1");

    let graph = builder.build().unwrap();
    let workflow_id = WorkflowId::default();
    let retry_config = RetryConfig::default();
    let workflow = graph_to_workflow(graph, workflow_id, retry_config, serde_json::json!({})).unwrap();
    let scheduled = workflow.schedule().unwrap();
    let running = scheduled.start().unwrap();
    
    // Store workflow
    let snapshot = flow_raft::core::WorkflowSnapshot::from_workflow(&running);
    let request = flow_raft::raft::types::Request::CreateWorkflow {
        workflow: snapshot,
    };
    app.create_workflow(request).await.unwrap();
    
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

// Note: Temporal comparison benchmarks would require:
// 1. Temporal server setup
// 2. Temporal client SDK
// 3. Equivalent workflow definitions
// This is a placeholder structure

criterion_group!(
    benches,
    benchmark_flowraft_latency,
    benchmark_flowraft_throughput
);
criterion_main!(benches);
