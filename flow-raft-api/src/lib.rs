//! FlowRaft API
//!
//! Public API for FlowRaft workflow definition and client.
//! This crate provides the graph builder API and workflow definition interfaces.

pub mod client;
pub mod graph;
pub mod workflow;

// Re-export commonly used types
pub use client::{ExecutionEvent, FlowRaftClient, FlowRaftClientBuilder, WorkflowExecutionId};
pub use graph::{DynamicGraph, DynamicGraphBuilder, GraphBuilder, NodeName, wrap_function};
pub use workflow::*;
