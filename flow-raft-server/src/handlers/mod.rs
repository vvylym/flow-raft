//! Handler registry for FlowRaft
//!
//! Provides per-workflow handler registration and execution.

pub mod executor;
pub mod registry;

pub use executor::{HandlerExecutionError, HandlerExecutor};
pub use registry::HandlerRegistry;
