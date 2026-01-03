//! Workflow module with type-driven state machine
//!
//! This module provides workflow definitions with compile-time state enforcement
//! using phantom types. Each state (Draft, Scheduled, Running, etc.) is a
//! distinct type parameter, preventing invalid operations at compile time.

mod error;
mod id;
mod snapshot;
mod state;
mod transitions;

pub use error::WorkflowError;
pub use id::WorkflowId;
pub use snapshot::{WorkflowSnapshot, WorkflowStatus};
pub use state::*;

use crate::{RetryConfig, TaskDefinition, TaskDependencies, TaskExecution, TaskId};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;

/// Workflow definition with type-driven state machine
///
/// The state is encoded as a phantom type parameter, ensuring that only
/// valid transitions can be performed at compile time.
#[derive(Debug, Clone)]
pub struct Workflow<State = WorkflowDraft> {
    /// Workflow identifier
    pub id: WorkflowId,
    /// Task definitions
    pub task_definitions: IndexMap<TaskId, TaskDefinition>,
    /// Task execution states
    pub executions: IndexMap<TaskId, TaskExecution>,
    /// Task dependencies
    pub dependencies: IndexMap<TaskId, TaskDependencies>,
    /// Retry configurations per task
    pub retry_configs: IndexMap<TaskId, RetryConfig>,
    /// Created at timestamp
    pub created_at: DateTime<Utc>,
    /// Started at timestamp
    pub started_at: Option<DateTime<Utc>>,
    /// Completed at timestamp
    pub completed_at: Option<DateTime<Utc>>,
    /// Workflow inputs
    pub inputs: serde_json::Value,
    /// Workflow outputs
    pub outputs: Option<serde_json::Value>,
    /// Error message (if failed or cancelled)
    pub error_message: Option<String>,
    /// Workflow state (phantom type parameter)
    pub state: State,
}
