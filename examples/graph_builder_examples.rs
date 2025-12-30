//! Graph builder examples
//!
//! Demonstrates both type-safe and dynamic graph builders.

use flow_raft::api::graph::{DynamicGraphBuilder, GraphBuilder};
use flow_raft::api::graph::builder::{ConditionObject, MergeObject, SplitObject};
use flow_raft::api::graph::converter::{dynamic_graph_to_workflow, graph_to_workflow};
use flow_raft::core::{RetryConfig, WorkflowId};
use std::sync::Arc;
use futures::future::BoxFuture;

// Example condition object
#[derive(Debug)]
struct EvenCondition;

impl ConditionObject for EvenCondition {
    fn evaluate(&self, input: serde_json::Value) -> BoxFuture<'static, Result<flow_raft::api::graph::NodeName, String>> {
        Box::pin(async move {
            let value = input
                .get("value")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| "Missing or invalid value".to_string())?;
            
            if value % 2 == 0 {
                Ok(flow_raft::api::graph::NodeName::new("even_task"))
            } else {
                Ok(flow_raft::api::graph::NodeName::new("odd_task"))
            }
        })
    }

    fn input_typeid(&self) -> std::any::TypeId {
        std::any::TypeId::of::<serde_json::Value>()
    }
}

// Example split object
#[derive(Debug)]
struct ExampleSplit;

impl SplitObject for ExampleSplit {
    fn split(&self, _input: serde_json::Value) -> BoxFuture<'static, Result<Vec<flow_raft::api::graph::NodeName>, String>> {
        Box::pin(async move {
            Ok(vec![
                flow_raft::api::graph::NodeName::new("branch1"),
                flow_raft::api::graph::NodeName::new("branch2"),
            ])
        })
    }

    fn input_typeid(&self) -> std::any::TypeId {
        std::any::TypeId::of::<serde_json::Value>()
    }
}

// Example merge object
#[derive(Debug)]
struct ExampleMerge;

impl MergeObject for ExampleMerge {
    fn merge(&self, inputs: Vec<serde_json::Value>) -> BoxFuture<'static, Result<serde_json::Value, String>> {
        Box::pin(async move {
            let mut result = serde_json::json!({});
            for (i, input) in inputs.iter().enumerate() {
                result[format!("branch{}", i + 1)] = input.clone();
            }
            Ok(result)
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Graph Builder Examples");

    // Example 1: Type-safe graph builder
    println!("\n1. Type-safe Graph Builder:");
    let mut builder = GraphBuilder::new("type_safe_workflow");
    builder
        .add_node("start", "start_handler", vec![], vec![], None)
        .add_node("process", "process_handler", vec![], vec![], None)
        .add_node("end", "end_handler", vec![], vec![], None)
        .add_simple_edge("start", "process")
        .add_simple_edge("process", "end")
        .set_root("start");

    let graph = builder.build()?;
    println!("   Built graph with {} nodes", graph.nodes.len());

    // Example 2: Dynamic graph builder
    println!("\n2. Dynamic Graph Builder:");
    let mut dyn_builder = DynamicGraphBuilder::new("dynamic_workflow");
    dyn_builder
        .add_node("node1", "handler1", vec![], vec![], None)
        .add_node("node2", "handler2", vec![], vec![], None)
        .add_simple_edge("node1", "node2");

    let dyn_graph = dyn_builder.build()?;
    println!("   Built dynamic graph with {} nodes", dyn_graph.nodes.len());

    // Example 3: Graph with conditional edge
    println!("\n3. Graph with Conditional Edge:");
    let mut cond_builder = GraphBuilder::new("conditional_workflow");
    cond_builder
        .add_node("input", "input_handler", vec![], vec![], None)
        .add_node("even_task", "even_handler", vec![], vec![], None)
        .add_node("odd_task", "odd_handler", vec![], vec![], None)
        .add_conditional_edge(
            "input",
            Arc::new(EvenCondition) as Arc<dyn ConditionObject>,
            "even_task",
            "odd_task",
        );

    let cond_graph = cond_builder.build()?;
    println!("   Built conditional graph with {} nodes", cond_graph.nodes.len());

    // Example 4: Graph with split and merge
    println!("\n4. Graph with Split and Merge:");
    let mut split_builder = GraphBuilder::new("split_merge_workflow");
    split_builder
        .add_node("start", "start_handler", vec![], vec![], None)
        .add_node("branch1", "branch1_handler", vec![], vec![], None)
        .add_node("branch2", "branch2_handler", vec![], vec![], None)
        .add_node("merge", "merge_handler", vec![], vec![], None)
        .add_split_edge("start", Arc::new(ExampleSplit) as Arc<dyn SplitObject>, vec!["branch1", "branch2"])
        .add_merge_edge(
            vec!["branch1", "branch2"],
            Arc::new(ExampleMerge) as Arc<dyn MergeObject>,
            "merge",
        );

    let split_graph = split_builder.build()?;
    println!("   Built split/merge graph with {} nodes", split_graph.nodes.len());

    // Convert to workflows
    let workflow_id = WorkflowId::default();
    let retry_config = RetryConfig::default();

    let workflow1 = graph_to_workflow(graph, workflow_id, retry_config.clone(), serde_json::json!({}))?;
    println!("\n   Converted type-safe graph to workflow with {} tasks", workflow1.task_definitions.len());

    let workflow2 = dynamic_graph_to_workflow(dyn_graph, workflow_id, retry_config.clone(), serde_json::json!({}))?;
    println!("   Converted dynamic graph to workflow with {} tasks", workflow2.task_definitions.len());

    println!("\nAll examples completed successfully!");
    Ok(())
}
