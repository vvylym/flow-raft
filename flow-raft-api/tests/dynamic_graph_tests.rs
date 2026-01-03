//! Tests for dynamic graph builder

use flow_raft_api::graph::builder::{ConditionObject, MergeObject, NodeName, SplitObject};
use flow_raft_api::graph::dynamic::DynamicGraphBuilder;
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
            Ok(NodeName::new("then"))
        } else {
            Ok(NodeName::new("else"))
        }
    }
}

#[derive(Debug)]
struct TestSplit;

impl SplitObject for TestSplit {
    fn evaluate(&self, _input: serde_json::Value) -> Result<Vec<NodeName>, String> {
        Ok(vec![NodeName::new("split1"), NodeName::new("split2")])
    }
}

#[derive(Debug)]
struct TestMerge;

impl MergeObject for TestMerge {
    fn merge(&self, inputs: Vec<serde_json::Value>) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({"merged": inputs.len()}))
    }
}

#[test]
fn test_dynamic_graph_builder_new() {
    let mut builder = DynamicGraphBuilder::new("test_graph");
    builder.add_node("node1", "handler1", vec![], vec![], None);
    let graph = builder.build();
    assert!(graph.is_ok());
}

#[test]
fn test_dynamic_graph_builder_add_node() {
    let mut builder = DynamicGraphBuilder::new("test");
    builder.add_node(
        "node1",
        "handler1",
        vec!["input1".to_string()],
        vec!["output1".to_string()],
        Some(60),
    );

    let graph = builder.build();
    assert!(graph.is_ok());
    let graph = graph.unwrap();
    assert_eq!(graph.nodes.len(), 1);
    assert!(graph.nodes.contains_key(&NodeName::new("node1")));
}

#[test]
fn test_dynamic_graph_builder_add_simple_edge() {
    let mut builder = DynamicGraphBuilder::new("test");
    builder
        .add_node("node1", "handler1", vec![], vec![], None)
        .add_node("node2", "handler2", vec![], vec![], None)
        .add_simple_edge("node1", "node2")
        .set_root("node1");

    let graph = builder.build();
    assert!(graph.is_ok());
}

#[test]
fn test_dynamic_graph_builder_add_conditional_edge() {
    let mut builder = DynamicGraphBuilder::new("test");
    builder
        .add_node("start", "handler1", vec![], vec![], None)
        .add_node("then", "handler2", vec![], vec![], None)
        .add_node("else", "handler3", vec![], vec![], None)
        .add_conditional_edge("start", Arc::new(TestCondition), "then", "else")
        .set_root("start");

    let graph = builder.build();
    assert!(graph.is_ok());
}

#[test]
fn test_dynamic_graph_builder_add_split_edge() {
    let mut builder = DynamicGraphBuilder::new("test");
    builder
        .add_node("start", "handler1", vec![], vec![], None)
        .add_node("split1", "handler2", vec![], vec![], None)
        .add_node("split2", "handler3", vec![], vec![], None)
        .add_split_edge("start", Arc::new(TestSplit), vec!["split1", "split2"])
        .set_root("start");

    let graph = builder.build();
    assert!(graph.is_ok());
}

#[test]
fn test_dynamic_graph_builder_add_merge_edge() {
    let mut builder = DynamicGraphBuilder::new("test");
    builder
        .add_node("source1", "handler1", vec![], vec![], None)
        .add_node("source2", "handler2", vec![], vec![], None)
        .add_node("merge", "handler3", vec![], vec![], None)
        .add_merge_edge(vec!["source1", "source2"], Arc::new(TestMerge), "merge")
        .set_root("source1");

    let graph = builder.build();
    assert!(graph.is_ok());
}

#[test]
fn test_dynamic_graph_builder_set_root() {
    let mut builder = DynamicGraphBuilder::new("test");
    builder.add_node("node1", "handler1", vec![], vec![], None);
    builder.set_root("node1");

    let graph = builder.build();
    assert!(graph.is_ok());
    let graph = graph.unwrap();
    assert_eq!(graph.root, Some(NodeName::new("node1")));
}
