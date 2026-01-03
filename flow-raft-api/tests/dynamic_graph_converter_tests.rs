//! Tests for dynamic graph converter

use flow_raft_api::graph::converter::dynamic_graph_to_workflow;
use flow_raft_api::graph::dynamic::DynamicGraphBuilder;
use flow_raft_core::RetryConfig;
use flow_raft_core::WorkflowId;

#[test]
fn test_dynamic_graph_to_workflow() {
    let mut builder = DynamicGraphBuilder::new("test");
    builder
        .add_node("task1", "handler1", vec![], vec![], None)
        .set_root("task1");

    let graph = builder.build().unwrap();
    let workflow_id = WorkflowId::default();
    let retry_config = RetryConfig::default();
    let workflow =
        dynamic_graph_to_workflow(graph, workflow_id, retry_config, serde_json::json!({}));

    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    assert_eq!(workflow.task_definitions.len(), 1);
}

#[test]
fn test_dynamic_graph_to_workflow_with_edges() {
    let mut builder = DynamicGraphBuilder::new("test");
    builder
        .add_node("task1", "handler1", vec![], vec![], None)
        .add_node("task2", "handler2", vec![], vec![], None)
        .add_simple_edge("task1", "task2")
        .set_root("task1");

    let graph = builder.build().unwrap();
    let workflow_id = WorkflowId::default();
    let retry_config = RetryConfig::default();
    let workflow =
        dynamic_graph_to_workflow(graph, workflow_id, retry_config, serde_json::json!({}));

    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    assert_eq!(workflow.task_definitions.len(), 2);
}
