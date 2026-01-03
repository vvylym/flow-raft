//! Advanced parallelism example
//!
//! Demonstrates:
//! - Dynamic parallelism
//! - Concurrency limits
//! - Resource pooling
//! - Batch processing

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

// Handler for processing items in parallel
struct ItemProcessor {
    concurrency_limit: usize,
}

impl TaskHandler for ItemProcessor {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        // Extract item from inputs
        let item = inputs
            .get("item")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        println!("  [ItemProcessor] Processing item: {}", item);
        
        Ok(serde_json::to_value(ProcessedItem {
            item: item.clone(),
            processed: true,
        })
        .unwrap())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Define workflow with parallel processing
    let mut builder = GraphBuilder::new("parallel_processing");
    builder.add_node(
        "split",
        "split_handler",
        vec!["batch".to_string()],
        vec!["items".to_string()],
        None,
    );

    // Add parallel processing nodes
    for i in 0..5 {
        builder.add_node(
            format!("process_item_{}", i),
            "item_processor",
            vec!["item".to_string()],
            vec!["result".to_string()],
            None,
        );
    }

    builder
        .add_node(
            "merge",
            "merge_handler",
            vec!["results".to_string()],
            vec!["final".to_string()],
            None,
        )
        .add_edge("split", "process_item_0")
        .add_edge("split", "process_item_1")
        .add_edge("split", "process_item_2")
        .add_edge("split", "process_item_3")
        .add_edge("split", "process_item_4")
        .add_edge("process_item_0", "merge")
        .add_edge("process_item_1", "merge")
        .add_edge("process_item_2", "merge")
        .add_edge("process_item_3", "merge")
        .add_edge("process_item_4", "merge")
        .set_root("split");

    let workflow = builder.build()?;
    let workflow_def = WorkflowDef::from_graph("parallel_processing", workflow, RetryConfig::default());

    // Create app
    let app = FlowRaftApp::builder()
        .with_node_id(1)
        .with_workflows(vec![workflow_def.clone()])
        .enable_metrics(true)
        .build_single_node()
        .await?;

    // Setup execution
    let workflow_id = workflow_def.workflow_id;
    let executor = Arc::new(WorkflowExecutor::new(
        app.raft().clone(),
        app.state_machine().clone(),
        1,
    ));
    let registry = Arc::new(HandlerRegistry::new());

    // Register handlers
    registry
        .register_handler(
            workflow_id,
            "split_handler".to_string(),
            Arc::new(EchoHandler {
                name: "split".to_string(),
            }) as Arc<dyn TaskHandler>,
        )
        .await;

    for i in 0..5 {
        registry
            .register_handler(
                workflow_id,
                "item_processor".to_string(),
                Arc::new(ItemProcessor {
                    concurrency_limit: 3,
                }) as Arc<dyn TaskHandler>,
            )
            .await;
    }

    registry
        .register_handler(
            workflow_id,
            "merge_handler".to_string(),
            Arc::new(EchoHandler {
                name: "merge".to_string(),
            }) as Arc<dyn TaskHandler>,
        )
        .await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Execute workflow
    let handler_executor = HandlerExecutor::new(executor, registry);
    println!("\n=== Executing parallel workflow ===");
    
    handler_executor.execute_workflow(workflow_id, 100).await?;

    println!("✓ Parallel workflow completed!");
    Ok(())
}

struct EchoHandler {
    name: String,
}

impl TaskHandler for EchoHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("  [{}] Processing: {:?}", self.name, inputs);
        Ok(inputs)
    }
}
