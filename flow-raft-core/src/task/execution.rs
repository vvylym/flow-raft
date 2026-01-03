//! Task execution state (runtime state, not type-driven)
//!
//! This module provides the `TaskExecution` struct which represents the
//! runtime execution state of a task. This is used by workflows to track
//! the current state of task execution, separate from the type-driven
//! `Task<State>` structure.

use crate::{TaskId, TaskState};
use chrono::{DateTime, Utc};

/// Task execution state (runtime state, not type-driven)
///
/// This represents the runtime execution state of a task within a workflow.
/// Unlike `Task<State>` which uses phantom types for compile-time safety,
/// this uses an enum for serialization and runtime state tracking.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskExecution {
    /// Task ID
    pub task_id: TaskId,
    /// Current state
    pub state: TaskState,
    /// Attempt number
    pub attempts: u32,
    /// Started at timestamp
    pub started_at: Option<DateTime<Utc>>,
    /// Completed at timestamp
    pub completed_at: Option<DateTime<Utc>>,
    /// Last error message
    pub last_error: Option<String>,
    /// Task outputs
    pub outputs: Option<serde_json::Value>,
}
