//! FlowRaft Raft
//!
//! Raft integration for FlowRaft workflow engine.
//! This crate provides Raft consensus integration for distributed workflow execution.

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
pub use app::{AppBuilderError, FlowRaftAppBuilder};
pub use config::RaftConfig;
pub use node::FlowRaftNode;
pub use types::TypeConfig;
