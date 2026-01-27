//! FlowRaft Server
//!
//! Server implementation for FlowRaft workflow engine.
//! This crate provides node launcher, gRPC service, raft cluster, and handlers.

pub mod cli_handlers;
pub mod grpc;
pub mod handlers;
pub mod node;
pub mod raft_cluster;
pub mod serve;

// Re-export commonly used types
pub use node::*;
pub use raft_cluster::{RaftClusterHandle, launch_raft_cluster};
