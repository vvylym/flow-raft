//! Task definition (immutable task metadata)
//!
//! This module provides the `TaskDefinition` struct which represents the
//! immutable metadata for a task. This is used by workflows to store task
//! definitions separately from their execution state.

use crate::core::TaskId;
use std::collections::HashSet;

/// Task definition (simplified for workflow storage)
///
/// This represents the immutable metadata of a task, separate from its
/// execution state. Used by workflows to track task definitions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskDefinition {
    /// Task ID
    pub id: TaskId,
    /// Task name
    pub name: String,
    /// Execution handler identifier
    pub handler: String,
    /// Input parameter names
    pub inputs: HashSet<String>,
    /// Output parameter names
    pub outputs: HashSet<String>,
    /// Optional timeout in seconds
    pub timeout_secs: Option<u64>,
}
