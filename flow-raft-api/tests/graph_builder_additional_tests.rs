//! Additional tests for GraphBuilder to increase coverage

use flow_raft_api::graph::builder::{GraphBuilder, NodeName};
use flow_raft_core::RetryConfig;

#[test]
fn test_node_name_new() {
    let name = NodeName::new("test_node");
    assert_eq!(name.as_ref(), "test_node");
}

#[test]
fn test_node_name_as_ref() {
    let name = NodeName::new("test");
    let s: &str = name.as_ref();
    assert_eq!(s, "test");
}

#[test]
fn test_graph_builder_with_timeout() {
    let mut builder = GraphBuilder::new("test");
    builder.add_node("task1", "handler1", vec![], vec![], Some(60));
    let graph = builder.build();
    assert!(graph.is_ok());
    let graph = graph.unwrap();
    let node = graph.nodes.get(&NodeName::new("task1")).unwrap();
    assert_eq!(node.timeout_secs, Some(60));
}

#[test]
fn test_graph_builder_with_default_retry_config() {
    let mut builder = GraphBuilder::new("test");
    let retry_config = RetryConfig::new(5);
    builder = builder.with_default_retry_config(retry_config);
    builder.add_node("task1", "handler1", vec![], vec![], None);
    let graph = builder.build();
    assert!(graph.is_ok());
}

#[test]
fn test_graph_builder_build_empty() {
    let builder = GraphBuilder::new("test");
    let graph = builder.build();
    // Empty graph should fail to build (needs at least one node)
    assert!(graph.is_err());
}

#[test]
fn test_graph_builder_build_without_root() {
    let mut builder = GraphBuilder::new("test");
    builder.add_node("task1", "handler1", vec![], vec![], None);
    // Build without setting root - first node should become root
    let graph = builder.build();
    assert!(graph.is_ok());
    let graph = graph.unwrap();
    assert_eq!(graph.root, Some(NodeName::new("task1")));
}
