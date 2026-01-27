//! Node management for FlowRaft
//!
//! Provides node launcher (init_tracing, start_metrics_server, NodeLaunchError)
//! and configuration.

pub mod config;
pub mod launcher;

pub use config::{NetworkConfig, NodeConfig, NodeMode};
pub use launcher::{NodeLaunchError, init_tracing, start_metrics_server};
