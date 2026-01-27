//! FlowRaft API
//!
//! Public API for FlowRaft workflow definition and client.
//! This crate provides the graph builder API and workflow definition interfaces.

pub mod client;
pub mod graph;
pub mod workflow;

// Re-export commonly used types (single GraphBuilder, builder pattern only; no macros for stateless graphs)
pub use client::{ExecutionEvent, FlowRaftClient, FlowRaftClientBuilder, WorkflowExecutionId};
pub use graph::{
    GraphBuilder, NodeName, TypedCondition, TypedGraph, TypedGraphBuilder, TypedMerge,
    TypedNodeFunction, TypedSplit, TypedSwitch, condition, graph_to_workflow, merge, node,
    node_async, node_async_ok, node_ok, split, switch,
};
pub use workflow::*;
