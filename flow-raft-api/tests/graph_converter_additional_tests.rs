//! Additional tests for graph converter to increase coverage

use flow_raft_api::graph::builder::GraphBuilder;
use flow_raft_api::graph::converter::graph_to_workflow;
use flow_raft_core::{RetryConfig, WorkflowId};

#[test]
fn test_graph_to_workflow_empty_graph() {
    let graph_result = GraphBuilder::new("test").build();
    // Empty graph should fail to build
    assert!(graph_result.is_err());
    // So we can't test conversion of empty graph
}

#[test]
fn test_graph_to_workflow_with_merge_specs() {
    use flow_raft_api::graph::builder::MergeObject;
    use std::sync::Arc;

    #[derive(Debug)]
    struct TestMerge;
    impl MergeObject for TestMerge {
        fn merge(&self, _inputs: Vec<serde_json::Value>) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({}))
        }
    }

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
            Arc::new(TestMerge) as Arc<dyn MergeObject>,
            "merge_target",
        )
        .set_root("start");

    let graph = builder.build().unwrap();
    let workflow = graph_to_workflow(
        graph,
        WorkflowId::default(),
        RetryConfig::default(),
        serde_json::json!({}),
    );
    assert!(workflow.is_ok());
    let workflow = workflow.unwrap();
    // merge_target should depend on both source1 and source2
    // Now we have: start, source1, source2, merge_target = 4 nodes
    assert_eq!(workflow.task_definitions.len(), 4);
}
