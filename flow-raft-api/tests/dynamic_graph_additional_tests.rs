//! Additional tests for DynamicGraphBuilder to increase coverage

use flow_raft_api::graph::builder::{ConditionObject, MergeObject, SplitObject};
use flow_raft_api::graph::dynamic::DynamicGraphBuilder;
use std::sync::Arc;

#[derive(Debug)]
struct TestCondition;
impl ConditionObject for TestCondition {
    fn evaluate(
        &self,
        _input: serde_json::Value,
    ) -> Result<flow_raft_api::graph::builder::NodeName, String> {
        Ok(flow_raft_api::graph::builder::NodeName::new("then"))
    }
}

#[derive(Debug)]
struct TestSplit;
impl SplitObject for TestSplit {
    fn evaluate(
        &self,
        _input: serde_json::Value,
    ) -> Result<Vec<flow_raft_api::graph::builder::NodeName>, String> {
        Ok(vec![flow_raft_api::graph::builder::NodeName::new("split1")])
    }
}

#[derive(Debug)]
struct TestMerge;
impl MergeObject for TestMerge {
    fn merge(&self, _inputs: Vec<serde_json::Value>) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({}))
    }
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
        .add_split_edge("start", Arc::new(TestSplit), vec!["split1"])
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
        .add_node("merge_target", "handler3", vec![], vec![], None)
        .add_merge_edge(
            vec!["source1", "source2"],
            Arc::new(TestMerge),
            "merge_target",
        )
        .set_root("source1");
    let graph = builder.build();
    assert!(graph.is_ok());
}
