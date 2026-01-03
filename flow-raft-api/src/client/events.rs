//! Execution events for workflow tracking

use flow_raft_core::TaskId;
use serde_json::Value;

/// Execution event for workflow tracking
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionEvent {
    /// Task started execution
    TaskStarted {
        /// Task ID
        task_id: TaskId,
        /// Task inputs
        inputs: Value,
    },
    /// Task completed successfully
    TaskCompleted {
        /// Task ID
        task_id: TaskId,
        /// Task outputs
        outputs: Value,
    },
    /// Task failed
    TaskFailed {
        /// Task ID
        task_id: TaskId,
        /// Error message
        error: String,
    },
    /// Workflow completed successfully
    WorkflowCompleted {
        /// Workflow outputs
        outputs: Value,
    },
    /// Workflow failed
    WorkflowFailed {
        /// Error message
        error: String,
    },
}
