//! Graph builder API for FlowRaft
//!
//! Provides both type-safe and dynamic graph builders for defining workflows.

pub mod builder;
pub mod converter;
pub mod dynamic;
pub mod validation;

pub use builder::{
    EdgeSpec, FunctionWrapper, Graph, GraphBuilder, NodeFunction, NodeName, wrap_function,
};
pub use converter::{dynamic_graph_to_workflow, graph_to_workflow};
pub use dynamic::{DynamicGraph, DynamicGraphBuilder};
pub use validation::{GraphValidationError, validate_graph};
