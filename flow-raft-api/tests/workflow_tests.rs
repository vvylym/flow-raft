//! Tests for workflow builder API

use flow_raft_api::graph::GraphBuilder;
use flow_raft_api::workflow::{WorkflowBuilder, WorkflowDef};
use flow_raft_core::RetryConfig;

#[test]
fn test_workflow_builder_new() {
    let mut builder = WorkflowBuilder::new("test_workflow");
    builder
        .add_task("task1", "handler1", vec![], vec![], None)
        .set_root("task1");
    let workflow = builder.build();
    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    assert_eq!(workflow.name(), "test_workflow");
}

#[test]
fn test_workflow_builder_with_retry_config() {
    let retry_config = RetryConfig::new(5);
    let mut builder = WorkflowBuilder::new("test").with_retry_config(retry_config.clone());
    builder
        .add_task("task1", "handler1", vec![], vec![], None)
        .set_root("task1");
    let workflow = builder.build();
    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    assert_eq!(workflow.default_retry_config.max_attempts, 5);
}

#[test]
fn test_workflow_builder_add_task() {
    let mut builder = WorkflowBuilder::new("test");
    builder.add_task(
        "task1",
        "handler1",
        vec!["input1".to_string()],
        vec!["output1".to_string()],
        Some(60),
    );

    let workflow = builder.build();
    assert!(workflow.is_ok());
}

#[test]
fn test_workflow_def_from_graph() {
    let mut graph_builder = GraphBuilder::new("test");
    graph_builder.add_node("task1", "handler1", vec![], vec![], None);
    graph_builder.set_root("task1");

    let graph = graph_builder.build().unwrap();
    let retry_config = RetryConfig::default();

    let workflow_def = WorkflowDef::from_graph("test_workflow", graph, retry_config);
    assert_eq!(workflow_def.name(), "test_workflow");
    assert_eq!(workflow_def.graph().nodes.len(), 1);
}
