//! Node management for FlowRaft
//!
//! Provides node launcher, configuration, and CLI interface.

pub mod cli;
pub mod config;
pub mod launcher;

#[cfg(feature = "cli")]
pub use cli::{Cli, Commands, run};
pub use config::{NetworkConfig, NodeConfig, NodeMode};
pub use launcher::{
    NodeLaunchError, init_tracing, join_cluster, launch_cluster_node, launch_follower,
    launch_leader, launch_single_node, start_metrics_server,
};
