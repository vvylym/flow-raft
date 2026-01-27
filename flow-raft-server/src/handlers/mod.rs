//! Handler registry for FlowRaft
//!
//! Provides per-workflow handler registration and execution.

pub mod executor;
pub mod registry;
pub mod task_router;

pub use executor::{HandlerExecutionError, HandlerExecutor};
pub use registry::HandlerRegistry;
pub use task_router::{
    ClientRunTaskCaller, LocalOnlyTaskRouter, MapTaskRouter, RunTaskCaller, TaskRouter,
};
