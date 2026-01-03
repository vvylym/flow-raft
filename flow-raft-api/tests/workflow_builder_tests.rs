//! Comprehensive tests for WorkflowBuilder

use flow_raft_api::graph::builder::{ConditionObject, MergeObject, SplitObject};
use flow_raft_api::workflow::{WorkflowBuilder, WorkflowDef};
use flow_raft_core::RetryConfig;

#[derive(Debug)]
struct TestCondition;

impl ConditionObject for TestCondition {
    fn evaluate(
        &self,
        _input: serde_json::Value,
    ) -> Result<flow_raft_api::graph::builder::NodeName, String> {
        Ok(flow_raft_api::graph::builder::NodeName::new("then_node"))
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
fn test_workflow_builder_with_retry_config() {
    let mut builder = WorkflowBuilder::new("test");
    let retry_config = RetryConfig::new(5);
    builder = builder.with_retry_config(retry_config.clone());
    builder.add_task("task1", "handler1", vec![], vec![], None);
    let workflow = builder.build();
    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    assert_eq!(workflow.default_retry_config.max_attempts, 5);
}

#[test]
fn test_workflow_builder_add_edge() {
    let mut builder = WorkflowBuilder::new("test");
    builder
        .add_task("task1", "handler1", vec![], vec![], None)
        .add_task("task2", "handler2", vec![], vec![], None)
        .add_edge("task1", "task2")
        .set_root("task1");
    let workflow = builder.build();
    assert!(workflow.is_ok());
}

#[test]
fn test_workflow_builder_add_conditional_edge() {
    let mut builder = WorkflowBuilder::new("test");
    builder
        .add_task("start", "handler1", vec![], vec![], None)
        .add_task("then_node", "handler2", vec![], vec![], None)
        .add_task("else_node", "handler3", vec![], vec![], None)
        .add_conditional_edge("start", TestCondition, "then_node", "else_node")
        .set_root("start");
    let workflow = builder.build();
    assert!(workflow.is_ok());
}

#[test]
fn test_workflow_builder_add_split_edge() {
    let mut builder = WorkflowBuilder::new("test");
    builder
        .add_task("start", "handler1", vec![], vec![], None)
        .add_task("split1", "handler2", vec![], vec![], None)
        .add_split_edge("start", TestSplit, vec!["split1"])
        .set_root("start");
    let workflow = builder.build();
    assert!(workflow.is_ok());
}

#[test]
fn test_workflow_builder_add_merge_edge() {
    let mut builder = WorkflowBuilder::new("test");
    builder
        .add_task("start", "handler0", vec![], vec![], None)
        .add_task("source1", "handler1", vec![], vec![], None)
        .add_task("source2", "handler2", vec![], vec![], None)
        .add_task("merge_target", "handler3", vec![], vec![], None)
        .add_edge("start", "source1")
        .add_edge("start", "source2")
        .add_merge_edge(vec!["source1", "source2"], TestMerge, "merge_target")
        .set_root("start");
    let workflow = builder.build();
    assert!(workflow.is_ok());
}

#[test]
fn test_workflow_def_name() {
    let workflow = WorkflowDef::from_graph(
        "test_workflow",
        flow_raft_api::graph::GraphBuilder::new("test")
            .add_node("task1", "handler1", vec![], vec![], None)
            .set_root("task1")
            .build()
            .unwrap(),
        RetryConfig::default(),
    );
    assert_eq!(workflow.name(), "test_workflow");
}

#[test]
fn test_workflow_def_workflow_id() {
    let workflow = WorkflowDef::from_graph(
        "test_workflow",
        flow_raft_api::graph::GraphBuilder::new("test")
            .add_node("task1", "handler1", vec![], vec![], None)
            .set_root("task1")
            .build()
            .unwrap(),
        RetryConfig::default(),
    );
    let workflow_id = workflow.workflow_id();
    assert!(std::mem::size_of_val(workflow_id) > 0);
}

#[test]
fn test_workflow_def_graph() {
    let graph = flow_raft_api::graph::GraphBuilder::new("test")
        .add_node("task1", "handler1", vec![], vec![], None)
        .set_root("task1")
        .build()
        .unwrap();
    let workflow = WorkflowDef::from_graph("test_workflow", graph.clone(), RetryConfig::default());
    assert_eq!(workflow.graph().nodes.len(), graph.nodes.len());
}
