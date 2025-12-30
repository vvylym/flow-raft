//! Node management for FlowRaft
//!
//! Provides node launcher, configuration, and CLI interface.

pub mod cli;
pub mod config;
pub mod launcher;

#[cfg(feature = "cli")]
pub use cli::{run, Cli, Commands};
pub use config::{NetworkConfig, NodeConfig, NodeMode};
pub use launcher::{join_cluster, launch_follower, launch_leader, NodeLaunchError};
