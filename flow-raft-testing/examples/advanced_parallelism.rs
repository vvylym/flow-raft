//! Advanced parallelism example
//!
//! Typed API: split/merge with typed node functions.
//! - split receives Batch and returns which process_item nodes run
//! - process_item_0..4 take Batch (per split output) and return ProcessedItem
//! - merge combines Vec<ProcessedItem> into a final result

use flow_raft::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Batch {
    id: String,
    items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProcessedItem {
    item: String,
    processed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BatchResult {
    batch_id: String,
    processed_count: usize,
}

fn split_batch(batch: Batch) -> Result<Batch, String> {
    println!(
        "Splitting batch: {} into {} items",
        batch.id,
        batch.items.len()
    );
    Ok(batch)
}

fn process_item_0(batch: Batch) -> Result<ProcessedItem, String> {
    let item = batch.items.first().cloned().unwrap_or_else(|| "?".into());
    println!("  [process_item_0] Processing: {}", item);
    Ok(ProcessedItem {
        item: item.clone(),
        processed: true,
    })
}
fn process_item_1(batch: Batch) -> Result<ProcessedItem, String> {
    let item = batch.items.get(1).cloned().unwrap_or_else(|| "?".into());
    println!("  [process_item_1] Processing: {}", item);
    Ok(ProcessedItem {
        item: item.clone(),
        processed: true,
    })
}
fn process_item_2(batch: Batch) -> Result<ProcessedItem, String> {
    let item = batch.items.get(2).cloned().unwrap_or_else(|| "?".into());
    println!("  [process_item_2] Processing: {}", item);
    Ok(ProcessedItem {
        item: item.clone(),
        processed: true,
    })
}
fn process_item_3(batch: Batch) -> Result<ProcessedItem, String> {
    let item = batch.items.get(3).cloned().unwrap_or_else(|| "?".into());
    println!("  [process_item_3] Processing: {}", item);
    Ok(ProcessedItem {
        item: item.clone(),
        processed: true,
    })
}
fn process_item_4(batch: Batch) -> Result<ProcessedItem, String> {
    let item = batch.items.get(4).cloned().unwrap_or_else(|| "?".into());
    println!("  [process_item_4] Processing: {}", item);
    Ok(ProcessedItem {
        item: item.clone(),
        processed: true,
    })
}

fn finalize_batch(r: BatchResult) -> Result<BatchResult, String> {
    println!("  [merge] {} items processed", r.processed_count);
    Ok(r)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let mut builder = TypedGraphBuilder::new("parallel_processing");
    builder
        .add_node("split", node(split_batch), None)
        .add_node("process_item_0", node(process_item_0), None)
        .add_node("process_item_1", node(process_item_1), None)
        .add_node("process_item_2", node(process_item_2), None)
        .add_node("process_item_3", node(process_item_3), None)
        .add_node("process_item_4", node(process_item_4), None)
        .add_node("merge", node(finalize_batch), None)
        .add_split_edge(
            "split",
            split(|_b: Batch| {
                Ok(vec![
                    "process_item_0".into(),
                    "process_item_1".into(),
                    "process_item_2".into(),
                    "process_item_3".into(),
                    "process_item_4".into(),
                ])
            }),
            vec![
                "process_item_0",
                "process_item_1",
                "process_item_2",
                "process_item_3",
                "process_item_4",
            ],
        )
        .add_merge_edge(
            vec![
                "process_item_0",
                "process_item_1",
                "process_item_2",
                "process_item_3",
                "process_item_4",
            ],
            merge(|inputs: Vec<ProcessedItem>| {
                Ok::<BatchResult, String>(BatchResult {
                    batch_id: "batch_1".into(),
                    processed_count: inputs.iter().filter(|r| r.processed).count(),
                })
            }),
            "merge",
        )
        .set_root("split");

    let typed_graph = builder.build()?;
    let workflow_def = typed_graph.workflow_def("parallel_processing")?;

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

    if let Some(mut workflow) = app.get_workflow(&workflow_id).await {
        workflow.inputs = serde_json::to_value(Batch {
            id: "batch_100".to_string(),
            items: vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
        })
        .unwrap();
        use flow_raft_raft::command::WorkflowCommandBuilder;
        let request = WorkflowCommandBuilder::transition_workflow(workflow_id, workflow);
        app.create_workflow(request)
            .await
            .map_err(|e| format!("Failed to update workflow: {:?}", e))?;
    }

    let handler_executor = HandlerExecutor::new(executor, registry);
    println!("\n=== Executing parallel workflow ===");
    handler_executor.execute_workflow(workflow_id, 100).await?;

    let s = app.get_workflow(&workflow_id).await.expect("workflow");
    let tid = s
        .task_definitions
        .iter()
        .find(|(_, d)| d.name == "merge")
        .map(|(id, _)| *id)
        .expect("merge task");
    let out = s
        .executions
        .get(&tid)
        .and_then(|e| e.outputs.as_ref())
        .cloned()
        .expect("merge output");
    let out: BatchResult = serde_json::from_value(out).expect("BatchResult");
    assert_eq!(out.batch_id, "batch_1");
    assert_eq!(out.processed_count, 5, "all 5 process_item nodes must run and return processed:true");

    println!("✓ Parallel workflow completed (processed_count={})", out.processed_count);
    Ok(())
}
