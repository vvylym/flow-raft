//! Tests for typed graph handler registration

use flow_raft::WorkflowId;
use flow_raft::graph::{TypedGraphBuilder, node};
use flow_raft::handlers::HandlerRegistry;
use flow_raft::register_typed_graph_handlers;

#[tokio::test]
async fn test_register_typed_graph_handlers() {
    let registry = HandlerRegistry::new();
    let workflow_id = WorkflowId::default();
    let mut b = TypedGraphBuilder::new("test_workflow");
    b.add_node("n1", node(|_: ()| Ok::<(), String>(())), None)
        .set_root("n1");
    let graph = b.build().expect("build should succeed");

    register_typed_graph_handlers(&registry, workflow_id, &graph).await;

    assert!(registry.has_handler(&workflow_id, "fn_n1").await);
}

#[tokio::test]
async fn test_register_typed_graph_handlers_multiple_nodes() {
    let registry = HandlerRegistry::new();
    let workflow_id = WorkflowId::default();
    let mut b = TypedGraphBuilder::new("multi");
    b.add_node("a", node(|_: ()| Ok::<i32, String>(1)), None)
        .add_node("b", node(|x: i32| Ok::<i32, String>(x + 1)), None)
        .add_simple_edge("a", "b")
        .set_root("a");
    let graph = b.build().expect("build should succeed");

    register_typed_graph_handlers(&registry, workflow_id, &graph).await;

    assert!(registry.has_handler(&workflow_id, "fn_a").await);
    assert!(registry.has_handler(&workflow_id, "fn_b").await);
}
