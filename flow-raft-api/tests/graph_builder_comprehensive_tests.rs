//! Comprehensive tests for graph builder

use flow_raft_api::graph::builder::*;
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
    fn evaluate(&self, _input: serde_json::Value) -> Result<Vec<NodeName>, String> {
        Ok(vec![
            NodeName::new("split_node_1"),
            NodeName::new("split_node_2"),
        ])
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
fn test_graph_builder_with_timeout() {
    let mut builder = GraphBuilder::new("test");
    builder
        .add_node("task1", "handler1", vec![], vec![], Some(30))
        .set_root("task1");

    let graph = builder.build();
    assert!(graph.is_ok());
    let graph = graph.unwrap();
    let node = graph.nodes.get(&NodeName::new("task1")).unwrap();
    assert_eq!(node.timeout_secs, Some(30));
}

#[test]
fn test_graph_builder_add_conditional_edge() {
    let mut builder = GraphBuilder::new("test");
    builder
        .add_node("start", "handler1", vec![], vec![], None)
        .add_node("then_node", "handler2", vec![], vec![], None)
        .add_node("else_node", "handler3", vec![], vec![], None)
        .add_conditional_edge("start", Arc::new(TestCondition), "then_node", "else_node")
        .set_root("start");

    let graph = builder.build();
    assert!(graph.is_ok());
}

#[test]
fn test_graph_builder_add_split_edge() {
    let mut builder = GraphBuilder::new("test");
    builder
        .add_node("start", "handler1", vec![], vec![], None)
        .add_node("split_node_1", "handler2", vec![], vec![], None)
        .add_node("split_node_2", "handler3", vec![], vec![], None)
        .add_split_edge(
            "start",
            Arc::new(TestSplit),
            vec!["split_node_1", "split_node_2"],
        )
        .set_root("start");

    let graph = builder.build();
    assert!(graph.is_ok());
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
}

#[test]
fn test_graph_builder_complex_workflow() {
    let mut builder = GraphBuilder::new("complex");
    builder
        .add_node("start", "handler1", vec![], vec![], None)
        .add_node("middle", "handler2", vec![], vec![], None)
        .add_node("end", "handler3", vec![], vec![], None)
        .add_simple_edge("start", "middle")
        .add_simple_edge("middle", "end")
        .set_root("start");

    let graph = builder.build().unwrap();
    let workflow_id = WorkflowId::default();
    let retry_config = RetryConfig::default();
    let workflow = graph_to_workflow(graph, workflow_id, retry_config, serde_json::json!({}));

    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    assert_eq!(workflow.task_definitions.len(), 3);
}

#[test]
fn test_graph_builder_with_default_retry_config() {
    let retry_config = RetryConfig::new(5);
    let mut builder = GraphBuilder::new("test").with_default_retry_config(retry_config.clone());
    builder
        .add_node("task1", "handler1", vec![], vec![], None)
        .set_root("task1");

    let graph = builder.build().unwrap();
    // Verify retry config is used when converting to workflow
    let workflow_id = WorkflowId::default();
    let workflow = graph_to_workflow(graph, workflow_id, retry_config, serde_json::json!({}));
    assert!(workflow.is_ok());
}

#[test]
fn test_node_name() {
    let name1 = NodeName::new("test");
    let name2 = NodeName::new("test");
    assert_eq!(name1, name2);
    assert_eq!(name1.as_ref(), "test");
}

#[test]
fn test_edge_spec_debug() {
    let simple = EdgeSpec::Simple(NodeName::new("target"));
    let debug_str = format!("{:?}", simple);
    assert!(debug_str.contains("Simple"));
    assert!(debug_str.contains("target"));
}
