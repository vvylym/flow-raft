//! Tests for FlowRaftAppBuilder

use flow_raft_api::graph::{TypedGraphBuilder, node};
use flow_raft_raft::types::NodeId;
use flow_raft_raft::{FlowRaftAppBuilder, config::default_config};

fn minimal_workflow_def() -> flow_raft_api::WorkflowDef {
    let mut b = TypedGraphBuilder::new("w");
    b.add_node("n", node(|_: ()| Ok::<(), String>(())), None)
        .set_root("n");
    b.build().unwrap().workflow_def("w").unwrap()
}

#[test]
fn test_app_builder_new_and_default() {
    let b = FlowRaftAppBuilder::new();
    assert!(std::mem::size_of_val(&b) > 0);
    let b2 = FlowRaftAppBuilder::default();
    assert!(std::mem::size_of_val(&b2) > 0);
}

#[test]
fn test_app_builder_fluent_with_node_id_and_config() {
    let node_id: NodeId = 1;
    let config = default_config();
    let b = FlowRaftAppBuilder::new()
        .with_node_id(node_id)
        .with_config(config);
    assert!(std::mem::size_of_val(&b) > 0);
}

#[test]
fn test_app_builder_with_workflows_and_add_workflow() {
    let w = minimal_workflow_def();
    let b = FlowRaftAppBuilder::new()
        .with_node_id(1)
        .add_workflow(w.clone());
    assert!(std::mem::size_of_val(&b) > 0);
    let b2 = FlowRaftAppBuilder::new()
        .with_node_id(1)
        .with_workflows(vec![w]);
    assert!(std::mem::size_of_val(&b2) > 0);
}

#[test]
fn test_app_builder_with_storage_and_metrics_port() {
    let b = FlowRaftAppBuilder::new()
        .with_node_id(1)
        .with_storage("/tmp/flow-raft-test".to_string())
        .with_metrics_port(9090);
    assert!(std::mem::size_of_val(&b) > 0);
}

#[test]
fn test_flow_raft_app_builder_from_app() {
    let b = FlowRaftAppBuilder::new();
    assert!(std::mem::size_of_val(&b) > 0);
}

#[tokio::test]
async fn test_app_builder_build_single_node() {
    let app = FlowRaftAppBuilder::new()
        .with_node_id(1)
        .build_single_node()
        .await
        .expect("build_single_node should succeed");
    assert!(std::mem::size_of_val(&app) > 0);
}

#[tokio::test]
async fn test_app_builder_build_single_node_missing_node_id() {
    let res = FlowRaftAppBuilder::new().build_single_node().await;
    match res {
        Err(e) => assert!(e.to_string().contains("node_id")),
        Ok(_) => panic!("expected build_single_node to fail without node_id"),
    }
}
