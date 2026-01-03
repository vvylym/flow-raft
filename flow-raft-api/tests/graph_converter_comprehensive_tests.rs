//! Comprehensive tests for graph converter

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
fn test_graph_to_workflow_conditional_edge() {
    let mut builder = GraphBuilder::new("test");
    builder
        .add_node("start", "handler1", vec![], vec![], None)
        .add_node("then_node", "handler2", vec![], vec![], None)
        .add_node("else_node", "handler3", vec![], vec![], None)
        .add_conditional_edge("start", Arc::new(TestCondition), "then_node", "else_node")
        .set_root("start");

    let graph = builder.build().unwrap();
    let workflow_id = WorkflowId::default();
    let retry_config = RetryConfig::default();
    let workflow = graph_to_workflow(graph, workflow_id, retry_config, serde_json::json!({}));

    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    assert_eq!(workflow.task_definitions.len(), 3);
    // Both branches should depend on start
    let then_task_id = workflow
        .task_definitions
        .iter()
        .find(|(_, def)| def.name == "then_node")
        .map(|(id, _)| *id)
        .unwrap();
    let else_task_id = workflow
        .task_definitions
        .iter()
        .find(|(_, def)| def.name == "else_node")
        .map(|(id, _)| *id)
        .unwrap();
    let start_task_id = workflow
        .task_definitions
        .iter()
        .find(|(_, def)| def.name == "start")
        .map(|(id, _)| *id)
        .unwrap();

    assert!(
        workflow
            .dependencies
            .get(&then_task_id)
            .unwrap()
            .prerequisites
            .contains(&start_task_id)
    );
    assert!(
        workflow
            .dependencies
            .get(&else_task_id)
            .unwrap()
            .prerequisites
            .contains(&start_task_id)
    );
}

#[test]
fn test_graph_to_workflow_split_edge() {
    let mut builder = GraphBuilder::new("test");
    builder
        .add_node("start", "handler1", vec![], vec![], None)
        .add_node("split_1", "handler2", vec![], vec![], None)
        .add_node("split_2", "handler3", vec![], vec![], None)
        .add_split_edge("start", Arc::new(TestSplit), vec!["split_1", "split_2"])
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
fn test_graph_to_workflow_merge_edge() {
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

    let graph = builder.build().unwrap();
    let workflow_id = WorkflowId::default();
    let retry_config = RetryConfig::default();
    let workflow = graph_to_workflow(graph, workflow_id, retry_config, serde_json::json!({}));

    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    // Now we have: start, source1, source2, merge_target = 4 nodes
    assert_eq!(workflow.task_definitions.len(), 4);

    // Merge target should depend on both sources
    let merge_task_id = workflow
        .task_definitions
        .iter()
        .find(|(_, def)| def.name == "merge_target")
        .map(|(id, _)| *id)
        .unwrap();
    let source1_task_id = workflow
        .task_definitions
        .iter()
        .find(|(_, def)| def.name == "source1")
        .map(|(id, _)| *id)
        .unwrap();
    let source2_task_id = workflow
        .task_definitions
        .iter()
        .find(|(_, def)| def.name == "source2")
        .map(|(id, _)| *id)
        .unwrap();

    let deps = workflow.dependencies.get(&merge_task_id).unwrap();
    assert!(deps.prerequisites.contains(&source1_task_id));
    assert!(deps.prerequisites.contains(&source2_task_id));
}

#[test]
#[should_panic(expected = "not found")]
fn test_graph_to_workflow_missing_node_error() {
    // This test verifies that adding an edge to a non-existent node panics
    let mut builder = GraphBuilder::new("test");
    builder.add_node("task1", "handler1", vec![], vec![], None);
    // This should panic when trying to add an edge to a non-existent node
    builder.add_simple_edge("task1", "nonexistent");
}
