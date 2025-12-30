//! Graph builder API for FlowRaft
//!
//! Provides both type-safe and dynamic graph builders for defining workflows.

pub mod builder;
pub mod converter;
pub mod dynamic;

pub use builder::{EdgeSpec, Graph, GraphBuilder, NodeName};
pub use converter::{graph_to_workflow, dynamic_graph_to_workflow};
pub use dynamic::{DynamicGraph, DynamicGraphBuilder};
