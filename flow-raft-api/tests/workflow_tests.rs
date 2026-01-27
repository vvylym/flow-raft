//! Tests for workflow definition (TypedGraphBuilder -> WorkflowDef)

use flow_raft_api::graph::{TypedGraphBuilder, node};
use flow_raft_core::RetryConfig;

fn nop(_: ()) -> Result<(), String> {
    Ok(())
}

#[test]
fn test_workflow_def_from_typed_graph() {
    let mut b = TypedGraphBuilder::new("test");
    b.add_node("task1", node(nop), None).set_root("task1");
    let tg = b.build().unwrap();
    let workflow_def = tg.workflow_def("test_workflow").unwrap();
    assert_eq!(workflow_def.name(), "test_workflow");
    assert_eq!(workflow_def.graph().nodes.len(), 1);
}

#[test]
fn test_workflow_def_retry_config() {
    let retry_config = RetryConfig::new(5);
    let mut b = TypedGraphBuilder::new("test").with_retry_config(retry_config.clone());
    b.add_node("task1", node(nop), None).set_root("task1");
    let workflow_def = b.build().unwrap().workflow_def("test").unwrap();
    assert_eq!(workflow_def.default_retry_config.max_attempts, 5);
}
