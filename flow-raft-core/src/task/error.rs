//! Error types for the task module

use thiserror::Error;

/// Errors that can occur during task operations
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum TaskError {
    /// Dependency not satisfied
    #[error("dependency not satisfied for task {task_id}: dependency {dependency_id}")]
    DependencyNotSatisfied {
        /// Task ID
        task_id: String,
        /// Dependency task ID
        dependency_id: String,
    },

    /// Maximum retries exceeded
    #[error("max retries exceeded for task {task_id}: {current_attempts}/{max_attempts}")]
    MaxRetriesExceeded {
        /// Task ID
        task_id: String,
        /// Maximum attempts allowed
        max_attempts: u8,
        /// Current attempt number
        current_attempts: u8,
    },
}
