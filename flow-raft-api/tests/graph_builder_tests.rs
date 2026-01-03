//! Comprehensive tests for graph builder

use flow_raft_api::graph::builder::{
    ConditionObject, GraphBuilder, MergeObject, NodeName, SplitObject,
};
use flow_raft_api::graph::converter::graph_to_workflow;
use flow_raft_core::RetryConfig;
use flow_raft_core::WorkflowId;
use std::sync::Arc;

#[derive(Debug)]
struct TestCondition;

impl ConditionObject for TestCondition {
    fn evaluate(&self, input: serde_json::Value) -> Result<NodeName, String> {
        let value = input
            .get("value")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if value {
            Ok(NodeName::new("then_node"))
        } else {
            Ok(NodeName::new("else_node"))
        }
    }
}

#[derive(Debug)]
struct TestSplit;

impl SplitObject for TestSplit {
    fn evaluate(&self, input: serde_json::Value) -> Result<Vec<NodeName>, String> {
        let count = input.get("count").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
        Ok((0..count)
            .map(|i| NodeName::new(format!("split_node_{}", i)))
            .collect())
    }
}

#[derive(Debug)]
struct TestMerge;

impl MergeObject for TestMerge {
    fn merge(&self, inputs: Vec<serde_json::Value>) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({"merged": true, "count": inputs.len()}))
    }
}

#[test]
fn test_graph_builder_simple() {
    let mut builder = GraphBuilder::new("test_graph");
    builder
        .add_node("node1", "handler1", vec![], vec![], None)
        .add_node("node2", "handler2", vec![], vec![], None)
        .add_simple_edge("node1", "node2")
        .set_root("node1");

    let graph = builder.build();
    assert!(graph.is_ok());
    let graph = graph.unwrap();
    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.edges.len(), 1);
}

#[test]
fn test_graph_builder_conditional_edge() {
    let mut builder = GraphBuilder::new("test_graph");
    builder
        .add_node("start", "handler1", vec![], vec![], None)
        .add_node("then_node", "handler2", vec![], vec![], None)
        .add_node("else_node", "handler3", vec![], vec![], None)
        .add_conditional_edge(
            "start",
            Arc::new(TestCondition) as Arc<dyn ConditionObject>,
            "then_node",
            "else_node",
        )
        .set_root("start");

    let graph = builder.build();
    assert!(graph.is_ok());
}

#[test]
fn test_graph_builder_split_edge() {
    let mut builder = GraphBuilder::new("test_graph");
    builder
        .add_node("start", "handler1", vec![], vec![], None)
        .add_node("split_node_0", "handler2", vec![], vec![], None)
        .add_node("split_node_1", "handler3", vec![], vec![], None)
        .add_split_edge(
            "start",
            Arc::new(TestSplit) as Arc<dyn SplitObject>,
            vec!["split_node_0", "split_node_1"],
        )
        .set_root("start");

    let graph = builder.build();
    assert!(graph.is_ok());
}

#[test]
fn test_graph_builder_merge_edge() {
    let mut builder = GraphBuilder::new("test_graph");
    builder
        .add_node("start", "handler0", vec![], vec![], None)
        .add_node("split_node_0", "handler1", vec![], vec![], None)
        .add_node("split_node_1", "handler2", vec![], vec![], None)
        .add_node("merge_target", "handler3", vec![], vec![], None)
        .add_simple_edge("start", "split_node_0")
        .add_simple_edge("start", "split_node_1")
        .add_merge_edge(
            vec!["split_node_0", "split_node_1"],
            Arc::new(TestMerge) as Arc<dyn MergeObject>,
            "merge_target",
        )
        .set_root("start");

    let graph = builder.build();
    assert!(graph.is_ok());
}

#[test]
fn test_graph_to_workflow_conversion() {
    let mut builder = GraphBuilder::new("test_workflow");
    builder
        .add_node("task1", "handler1", vec![], vec![], None)
        .add_node("task2", "handler2", vec![], vec![], None)
        .add_simple_edge("task1", "task2")
        .set_root("task1");

    let graph = builder.build().unwrap();
    let workflow_id = WorkflowId::default();
    let retry_config = RetryConfig::default();
    let inputs = serde_json::json!({});

    let workflow = graph_to_workflow(graph, workflow_id, retry_config, inputs);
    assert!(workflow.is_ok());
}

#[test]
fn test_graph_builder_error_cases() {
    // Test building graph without root
    let mut builder = GraphBuilder::new("test_graph");
    builder.add_node("node1", "handler1", vec![], vec![], None);
    let graph = builder.build();
    // Should succeed even without explicit root (first node becomes root)
    assert!(graph.is_ok());
}

#[test]
#[should_panic(expected = "not found")]
fn test_graph_builder_error_cases_panic() {
    // Test edge to non-existent node should panic
    let mut builder = GraphBuilder::new("test_graph");
    builder
        .add_node("node1", "handler1", vec![], vec![], None)
        .add_simple_edge("node1", "nonexistent");
    let _graph = builder.build();
}
