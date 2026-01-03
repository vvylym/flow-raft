//! Error types for the workflow module

use thiserror::Error;

/// Errors that can occur during workflow operations
#[derive(Debug, Error)]
pub enum WorkflowError {
    /// Cycle detected in workflow DAG
    #[error("cycle detected in workflow DAG")]
    CycleDetected,

    /// Dependency not found
    #[error("dependency {0} not found")]
    DependencyNotFound(String),

    /// No tasks found in workflow
    #[error("no tasks found in workflow")]
    NoTasksFound,
}
