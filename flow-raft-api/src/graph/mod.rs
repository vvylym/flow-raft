//! Graph builder API for FlowRaft
//!
//! Provides a single [GraphBuilder] (builder pattern, no macros) for type-safe workflow graphs.
//! Use [node], [condition], [split], [merge], [switch] to wrap functions, then add nodes and edges.
//! For shared state across nodes, use [stateful::StatefulGraphBuilder] and the [stateful] module.

pub mod builder;
pub mod converter;
pub(crate) mod dynamic;
pub mod stateful;
pub mod typed;
pub mod validation;

pub use builder::{EdgeSpec, Graph, NodeFunction, NodeName};
pub use converter::graph_to_workflow;
pub use typed::{
    TypedCondition, TypedGraph, TypedGraphBuilder, TypedMerge, TypedNodeFunction, TypedSplit,
    TypedSwitch, condition, merge, node, node_async, node_async_ok, node_ok, split, switch,
};
pub use validation::{GraphValidationError, validate_graph};

/// Single graph builder: type-safe workflow graph via builder pattern. Use [node], [condition], [split], [merge], [switch] with it.
pub type GraphBuilder = TypedGraphBuilder;
