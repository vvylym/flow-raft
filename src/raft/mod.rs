//! Raft module for distributed workflow execution
//!
//! This module provides Raft consensus integration for FlowRaft, enabling
//! distributed workflow execution with state replication across nodes.

pub mod app;
pub mod command;
pub mod config;
pub mod executor;
pub mod network;
pub mod node;
pub mod storage;
pub mod types;

#[cfg(test)]
mod tests;

pub use app::FlowRaftApp;
pub use config::RaftConfig;
pub use node::FlowRaftNode;
pub use types::TypeConfig;
