//! Additional tests for graph converter (typed API)

use flow_raft_api::graph::converter::graph_to_workflow;
use flow_raft_api::graph::{TypedGraphBuilder, merge, node};
use flow_raft_core::{RetryConfig, WorkflowId};

fn nop(_: ()) -> Result<(), String> {
    Ok(())
}

#[test]
fn test_graph_to_workflow_empty_graph() {
    let graph_result = TypedGraphBuilder::new("test").build();
    assert!(graph_result.is_err());
}

#[test]
fn test_graph_to_workflow_with_merge_specs() {
    let mut builder = TypedGraphBuilder::new("test");
    builder
        .add_node("start", node(nop), None)
        .add_node("source1", node(nop), None)
        .add_node("source2", node(nop), None)
        .add_node("merge_target", node(nop), None)
        .add_simple_edge("start", "source1")
        .add_simple_edge("start", "source2")
        .add_merge_edge(
            vec!["source1", "source2"],
            merge(|_inputs: Vec<()>| Ok::<(), String>(())),
            "merge_target",
        )
        .set_root("start");

    let typed = builder.build().unwrap();
    let graph = typed.graph().clone();
    let workflow = graph_to_workflow(
        graph,
        WorkflowId::default(),
        RetryConfig::default(),
        serde_json::json!({}),
    );
    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    assert_eq!(workflow.task_definitions.len(), 4);
}
