//! Workflow execution benchmarks
//!
//! Benchmarks workflow execution performance for various scenarios.

#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use flow_raft::api::graph::GraphBuilder;
use flow_raft::api::graph::converter::graph_to_workflow;
use flow_raft::core::{RetryConfig, WorkflowId};
use flow_raft::raft::config::default_config;
use flow_raft::raft::executor::WorkflowExecutor;
use flow_raft::raft::network::MemoryNetworkFactory;
use flow_raft::raft::storage::{LogStore, StateMachineStore};
use flow_raft::raft::types::Request;
use flow_raft::raft::app::FlowRaftApp;
use flow_raft::api::handlers::HandlerRegistry;
use std::sync::Arc;
use std::collections::BTreeSet;

async fn setup_single_node() -> (Arc<FlowRaftApp>, Arc<WorkflowExecutor>, Arc<HandlerRegistry>) {
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
    let executor = Arc::new(WorkflowExecutor::new(raft, state_machine.clone(), node_id));
    let registry = Arc::new(HandlerRegistry::new());

    (app, executor, registry)
}

fn create_simple_workflow(num_tasks: usize) -> flow_raft::core::Workflow<flow_raft::core::WorkflowDraft> {
    let mut builder = GraphBuilder::new("benchmark_workflow");
    
    // Create chain of tasks
    for i in 0..num_tasks {
        let task_name = format!("task{}", i);
        builder.add_node(
            &task_name,
            &format!("handler{}", i),
            vec![],
            vec![],
            None,
        );
        
        if i > 0 {
            builder.add_simple_edge(&format!("task{}", i - 1), &task_name);
        }
    }
    
    builder.set_root("task0");
    let graph = builder.build().unwrap();
    
    let workflow_id = WorkflowId::default();
    let retry_config = RetryConfig::default();
    graph_to_workflow(graph, workflow_id, retry_config, serde_json::json!({})).unwrap()
}

fn benchmark_workflow_creation(c: &mut Criterion) {
    c.bench_function("create_workflow_10_tasks", |b| {
        b.iter(|| {
            let workflow = create_simple_workflow(black_box(10));
            let scheduled = workflow.schedule().unwrap();
            let running = scheduled.start().unwrap();
            black_box(running)
        })
    });

    c.bench_function("create_workflow_100_tasks", |b| {
        b.iter(|| {
            let workflow = create_simple_workflow(black_box(100));
            let scheduled = workflow.schedule().unwrap();
            let running = scheduled.start().unwrap();
            black_box(running)
        })
    });
}

fn benchmark_workflow_scheduling(c: &mut Criterion) {
    c.bench_function("schedule_workflow_10_tasks", |b| {
        b.iter(|| {
            let workflow = create_simple_workflow(10);
            let scheduled = workflow.schedule().unwrap();
            let running = scheduled.start().unwrap();
            black_box(running)
        })
    });
}

fn benchmark_workflow_storage(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("store_workflow_10_tasks", |b| {
        b.iter(|| {
            rt.block_on(async {
                let (app, _, _) = setup_single_node().await;
                let workflow = create_simple_workflow(10);
                let scheduled = workflow.schedule().unwrap();
                let running = scheduled.start().unwrap();
                let snapshot = flow_raft::core::WorkflowSnapshot::from_workflow(&running);
                let request = Request::CreateWorkflow {
                    workflow: snapshot.clone(),
                };
                let _ = app.create_workflow(request).await;
                black_box(snapshot)
            })
        })
    });
}

criterion_group!(
    benches,
    benchmark_workflow_creation,
    benchmark_workflow_scheduling,
    benchmark_workflow_storage
);
criterion_main!(benches);
