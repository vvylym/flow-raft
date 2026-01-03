//! Tests for graph to workflow converter

use flow_raft_api::graph::builder::GraphBuilder;
use flow_raft_api::graph::converter::graph_to_workflow;
use flow_raft_core::RetryConfig;
use flow_raft_core::WorkflowId;

#[test]
fn test_simple_workflow_conversion() {
    let mut builder = GraphBuilder::new("simple_workflow");
    builder
        .add_node("task1", "handler1", vec![], vec![], None)
        .set_root("task1");

    let graph = builder.build().unwrap();
    let workflow = graph_to_workflow(
        graph,
        WorkflowId::default(),
        RetryConfig::default(),
        serde_json::json!({}),
    );

    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    assert_eq!(workflow.task_definitions.len(), 1);
}

#[test]
fn test_workflow_with_dependencies() {
    let mut builder = GraphBuilder::new("workflow_with_deps");
    builder
        .add_node("task1", "handler1", vec![], vec![], None)
        .add_node("task2", "handler2", vec![], vec![], None)
        .add_simple_edge("task1", "task2")
        .set_root("task1");

    let graph = builder.build().unwrap();
    let workflow = graph_to_workflow(
        graph,
        WorkflowId::default(),
        RetryConfig::default(),
        serde_json::json!({}),
    );

    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    assert_eq!(workflow.task_definitions.len(), 2);
    // task2 should depend on task1
    let task2_id = workflow.task_definitions.keys().nth(1).copied().unwrap();
    assert!(workflow.dependencies.contains_key(&task2_id));
}
