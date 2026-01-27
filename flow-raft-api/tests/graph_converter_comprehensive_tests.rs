//! Comprehensive tests for graph converter (using TypedGraphBuilder)

use flow_raft_api::graph::converter::graph_to_workflow;
use flow_raft_api::graph::{TypedGraphBuilder, condition, merge, node, split};
use flow_raft_core::RetryConfig;
use flow_raft_core::WorkflowId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WithValue {
    value: bool,
}

fn nop(_: ()) -> Result<(), String> {
    Ok(())
}

#[test]
fn test_graph_to_workflow_conditional_edge() {
    fn start(_: ()) -> Result<WithValue, String> {
        Ok(WithValue { value: true })
    }
    fn then_node(_: WithValue) -> Result<(), String> {
        Ok(())
    }
    fn else_node(_: WithValue) -> Result<(), String> {
        Ok(())
    }

    let mut b = TypedGraphBuilder::new("test");
    b.add_node("start", node(start), None)
        .add_node("then_node", node(then_node), None)
        .add_node("else_node", node(else_node), None)
        .add_conditional_edge(
            "start",
            condition(|x: WithValue| x.value),
            "then_node",
            "else_node",
        )
        .set_root("start");

    let graph = b.build().unwrap().graph().clone();
    let workflow = graph_to_workflow(
        graph,
        WorkflowId::default(),
        RetryConfig::default(),
        serde_json::json!({}),
    );

    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    assert_eq!(workflow.task_definitions.len(), 3);
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
    fn start(_: ()) -> Result<(), String> {
        Ok(())
    }
    let mut b = TypedGraphBuilder::new("test");
    b.add_node("start", node(start), None)
        .add_node("split_1", node(nop), None)
        .add_node("split_2", node(nop), None)
        .add_split_edge(
            "start",
            split(|_: ()| Ok(vec!["split_1".into(), "split_2".into()])),
            vec!["split_1", "split_2"],
        )
        .set_root("start");

    let graph = b.build().unwrap().graph().clone();
    let workflow = graph_to_workflow(
        graph,
        WorkflowId::default(),
        RetryConfig::default(),
        serde_json::json!({}),
    );

    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    assert_eq!(workflow.task_definitions.len(), 3);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Merged {
    merged: bool,
    count: usize,
}

#[test]
fn test_graph_to_workflow_merge_edge() {
    fn start(_: ()) -> Result<(), String> {
        Ok(())
    }
    fn merge_target(_: Merged) -> Result<(), String> {
        Ok(())
    }

    let mut b = TypedGraphBuilder::new("test");
    b.add_node("start", node(start), None)
        .add_node("source1", node(nop), None)
        .add_node("source2", node(nop), None)
        .add_node("merge_target", node(merge_target), None)
        .add_simple_edge("start", "source1")
        .add_simple_edge("start", "source2")
        .add_merge_edge(
            vec!["source1", "source2"],
            merge(|inputs: Vec<()>| {
                Ok::<Merged, String>(Merged {
                    merged: true,
                    count: inputs.len(),
                })
            }),
            "merge_target",
        )
        .set_root("start");

    let graph = b.build().unwrap().graph().clone();
    let workflow = graph_to_workflow(
        graph,
        WorkflowId::default(),
        RetryConfig::default(),
        serde_json::json!({}),
    );

    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    assert_eq!(workflow.task_definitions.len(), 4);
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
    let mut b = TypedGraphBuilder::new("test");
    b.add_node("task1", node(nop), None);
    b.add_simple_edge("task1", "nonexistent");
}
