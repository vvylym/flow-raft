//! Graph builder examples
//!
//! Demonstrates both type-safe and dynamic graph builders using the simplified API.

use flow_raft::prelude::*;
use std::sync::Arc;

// Example condition object
#[derive(Debug)]
struct EvenCondition;

impl ConditionObject for EvenCondition {
    fn evaluate(&self, input: serde_json::Value) -> Result<NodeName, String> {
        let value = input
            .get("value")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "Missing or invalid value".to_string())?;
        
        if value % 2 == 0 {
            Ok(NodeName::new("even_task"))
        } else {
            Ok(NodeName::new("odd_task"))
        }
    }
}

// Example split object
#[derive(Debug)]
struct ExampleSplit;

impl SplitObject for ExampleSplit {
    fn evaluate(&self, _input: serde_json::Value) -> Result<Vec<NodeName>, String> {
        Ok(vec![
            NodeName::new("branch1"),
            NodeName::new("branch2"),
        ])
    }
}

// Example merge object
#[derive(Debug)]
struct ExampleMerge;

impl MergeObject for ExampleMerge {
    fn merge(&self, inputs: Vec<serde_json::Value>) -> Result<serde_json::Value, String> {
        let mut result = serde_json::json!({});
        for (i, input) in inputs.iter().enumerate() {
            result[format!("branch{}", i + 1)] = input.clone();
        }
        Ok(result)
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

    // Example 2: Simple graph with function-based nodes
    println!("\n2. Graph with Function-Based Nodes:");
    fn simple_task(_input: serde_json::Value) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({"result": "success"}))
    }
    
    let func_graph = GraphBuilder::new("function_workflow")
        .add_node_fn("node1", wrap_function(simple_task), None)
        .add_node_fn("node2", wrap_function(simple_task), None)
        .add_simple_edge("node1", "node2")
        .set_root("node1")
        .build()?;
    println!("   Built function-based graph with {} nodes", func_graph.nodes.len());

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

    // Convert to workflow definitions
    let retry_config = RetryConfig::default();

    let workflow1_def = WorkflowDef::from_graph("type_safe_workflow", graph, retry_config.clone());
    println!("\n   Converted type-safe graph to workflow with {} tasks", workflow1_def.graph.nodes.len());

    // Note: Dynamic graph conversion would be similar
    // let workflow2_def = WorkflowDef::from_dynamic_graph("dynamic_workflow", dyn_graph, retry_config.clone());
    // println!("   Converted dynamic graph to workflow with {} tasks", workflow2_def.graph.nodes.len());

    println!("\nAll examples completed successfully!");
    Ok(())
}
