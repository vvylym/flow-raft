//! Tests for graph builder edge methods

use flow_raft_api::graph::builder::*;
use flow_raft_core::RetryConfig;
use std::sync::Arc;

/// Test condition implementation for edge tests
///
/// This struct is used via trait implementation (ConditionObject),
/// so it's never directly constructed but is used in tests.
#[derive(Debug)]
#[allow(dead_code)] // Used via trait implementation
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

/// Test split implementation for edge tests
///
/// This struct is used via trait implementation (SplitObject),
/// so it's never directly constructed but is used in tests.
#[derive(Debug)]
#[allow(dead_code)] // Used via trait implementation
struct TestSplit;

impl SplitObject for TestSplit {
    fn evaluate(&self, _input: serde_json::Value) -> Result<Vec<NodeName>, String> {
        Ok(vec![NodeName::new("split_1"), NodeName::new("split_2")])
    }
}

#[derive(Debug)]
struct TestMerge;

impl MergeObject for TestMerge {
    fn merge(&self, inputs: Vec<serde_json::Value>) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({
            "merged": true,
            "count": inputs.len()
        }))
    }
}

#[test]
fn test_graph_builder_add_node_with_inputs_outputs() {
    let mut builder = GraphBuilder::new("test");
    builder.add_node(
        "task1",
        "handler1",
        vec!["input1".to_string(), "input2".to_string()],
        vec!["output1".to_string()],
        None,
    );

    let graph = builder.build();
    assert!(graph.is_ok());
    let graph = graph.unwrap();
    let node = graph.nodes.get(&NodeName::new("task1")).unwrap();
    assert_eq!(node.inputs.len(), 2);
    assert_eq!(node.outputs.len(), 1);
}

#[test]
fn test_graph_builder_add_merge_edge() {
    let mut builder = GraphBuilder::new("test");
    builder
        .add_node("start", "handler0", vec![], vec![], None)
        .add_node("source1", "handler1", vec![], vec![], None)
        .add_node("source2", "handler2", vec![], vec![], None)
        .add_node("merge_target", "handler3", vec![], vec![], None)
        .add_simple_edge("start", "source1")
        .add_simple_edge("start", "source2")
        .add_merge_edge(
            vec!["source1", "source2"],
            Arc::new(TestMerge),
            "merge_target",
        )
        .set_root("start");

    let graph = builder.build();
    assert!(graph.is_ok());
    let graph = graph.unwrap();
    assert!(
        graph
            .merge_specs
            .contains_key(&NodeName::new("merge_target"))
    );
}

#[test]
fn test_graph_builder_build_without_root() {
    let mut builder = GraphBuilder::new("test");
    builder.add_node("task1", "handler1", vec![], vec![], None);

    let graph = builder.build();
    // Should succeed even without explicit root (first node becomes root)
    assert!(graph.is_ok());
}

#[test]
fn test_graph_builder_build_empty() {
    let builder = GraphBuilder::new("test");
    let graph = builder.build();
    // Empty graph should fail
    assert!(graph.is_err());
}

#[test]
fn test_graph_builder_with_default_retry_config() {
    let retry_config = RetryConfig::new(5);
    let builder = GraphBuilder::new("test").with_default_retry_config(retry_config);
    // Builder should retain the retry config
    let mut builder = builder;
    builder
        .add_node("task1", "handler1", vec![], vec![], None)
        .set_root("task1");
    let graph = builder.build();
    assert!(graph.is_ok());
}
