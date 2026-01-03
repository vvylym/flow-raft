//! Task module with type-driven state machine
//!
//! This module provides task definitions with compile-time state enforcement
//! using phantom types. Each state (Pending, Scheduled, Running, etc.) is a
//! distinct type parameter, preventing invalid operations at compile time.

mod definition;
mod error;
mod execution;
mod id;
mod state;
mod transitions;

pub use definition::TaskDefinition;
pub use error::TaskError;
pub use execution::TaskExecution;
pub use id::TaskId;
pub use state::*;

use crate::{RetryConfig, TaskDependencies};
use chrono::{DateTime, Utc};
use std::collections::HashSet;

/// Task definition with type-driven state machine
///
/// The state is encoded as a phantom type parameter, ensuring that only
/// valid transitions can be performed at compile time.
#[derive(Debug, Clone)]
pub struct Task<State = TaskPending> {
    /// Task identifier
    pub id: TaskId,
    /// Task name
    pub name: String,
    /// Execution handler identifier
    pub handler: String,
    /// Input parameter names
    pub inputs: HashSet<String>,
    /// Output parameter names
    pub outputs: HashSet<String>,
    /// Task dependencies
    pub dependencies: TaskDependencies,
    /// Retry configuration
    pub retry_config: RetryConfig,
    /// Optional timeout in seconds
    pub timeout_secs: Option<u64>,
    /// Task state (phantom type parameter)
    pub state: State,
    /// Timestamp when task started
    pub started_at: Option<DateTime<Utc>>,
    /// Timestamp when task completed
    pub completed_at: Option<DateTime<Utc>>,
    /// Last error message
    pub last_error: Option<String>,
    /// Task outputs (JSON value)
    pub outputs_data: Option<serde_json::Value>,
}
