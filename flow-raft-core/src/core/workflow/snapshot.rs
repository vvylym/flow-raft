//! Workflow snapshot for serialization
//!
//! Provides serializable representation of workflow state for persistence
//! and recovery.

use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    RetryConfig, TaskDefinition, TaskDependencies, TaskExecution, TaskId, TaskState, WorkflowId,
    WorkflowState,
};

/// Serializable workflow snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSnapshot {
    /// Workflow ID
    pub workflow_id: WorkflowId,
    /// Workflow state
    pub state: WorkflowState,
    /// Task definitions
    pub task_definitions: IndexMap<TaskId, TaskDefinition>,
    /// Task execution states
    pub executions: IndexMap<TaskId, TaskExecution>,
    /// Task dependencies
    pub dependencies: IndexMap<TaskId, TaskDependencies>,
    /// Retry configurations
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
    /// Error message (if failed)
    pub error_message: Option<String>,
}

/// Workflow status summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStatus {
    /// Total number of tasks
    pub total_tasks: usize,
    /// Number of completed tasks
    pub completed: usize,
    /// Number of failed tasks
    pub failed: usize,
    /// Number of running tasks
    pub running: usize,
    /// Number of pending tasks
    pub pending: usize,
}

impl WorkflowSnapshot {
    /// Creates a workflow snapshot from a Workflow
    pub fn from_workflow<State>(workflow: &crate::Workflow<State>) -> Self
    where
        for<'a> crate::WorkflowState: From<&'a State>,
    {
        Self {
            workflow_id: workflow.id,
            state: crate::WorkflowState::from(&workflow.state),
            task_definitions: workflow.task_definitions.clone(),
            executions: workflow.executions.clone(),
            dependencies: workflow.dependencies.clone(),
            retry_configs: workflow.retry_configs.clone(),
            created_at: workflow.created_at,
            started_at: workflow.started_at,
            completed_at: workflow.completed_at,
            inputs: workflow.inputs.clone(),
            outputs: workflow.outputs.clone(),
            error_message: workflow.error_message.clone(),
        }
    }

    /// Creates a workflow status summary using parallel processing
    pub fn status(&self) -> WorkflowStatus {
        let counts = self
            .executions
            .par_iter()
            .map(|(_, execution)| match &execution.state {
                TaskState::Completed => (1, 0, 0, 0),
                TaskState::Failed { .. } | TaskState::PermanentlyFailed { .. } => (0, 1, 0, 0),
                TaskState::Running => (0, 0, 1, 0),
                TaskState::Pending | TaskState::Scheduled => (0, 0, 0, 1),
                _ => (0, 0, 0, 0),
            })
            .reduce(
                || (0, 0, 0, 0),
                |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2, a.3 + b.3),
            );

        WorkflowStatus {
            total_tasks: self.executions.len(),
            completed: counts.0,
            failed: counts.1,
            running: counts.2,
            pending: counts.3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FailureKind;
    use chrono::Utc;

    fn create_test_snapshot() -> WorkflowSnapshot {
        WorkflowSnapshot {
            workflow_id: WorkflowId::default(),
            state: WorkflowState::Running,
            task_definitions: IndexMap::new(),
            executions: IndexMap::new(),
            dependencies: IndexMap::new(),
            retry_configs: IndexMap::new(),
            created_at: Utc::now(),
            started_at: Some(Utc::now()),
            completed_at: None,
            inputs: serde_json::json!({}),
            outputs: None,
            error_message: None,
        }
    }

    #[test]
    fn test_workflow_status_empty() {
        let snapshot = create_test_snapshot();
        let status = snapshot.status();
        assert_eq!(status.total_tasks, 0);
        assert_eq!(status.completed, 0);
        assert_eq!(status.failed, 0);
        assert_eq!(status.running, 0);
        assert_eq!(status.pending, 0);
    }

    #[test]
    fn test_workflow_status_with_tasks() {
        let mut snapshot = create_test_snapshot();
        let task_id1 = TaskId::default();
        let task_id2 = TaskId::default();
        let task_id3 = TaskId::default();

        snapshot.executions.insert(
            task_id1,
            TaskExecution {
                task_id: task_id1,
                state: TaskState::Completed,
                attempts: 1,
                started_at: Some(Utc::now()),
                completed_at: Some(Utc::now()),
                last_error: None,
                outputs: None,
            },
        );

        snapshot.executions.insert(
            task_id2,
            TaskExecution {
                task_id: task_id2,
                state: TaskState::Running,
                attempts: 1,
                started_at: Some(Utc::now()),
                completed_at: None,
                last_error: None,
                outputs: None,
            },
        );

        snapshot.executions.insert(
            task_id3,
            TaskExecution {
                task_id: task_id3,
                state: TaskState::Pending,
                attempts: 0,
                started_at: None,
                completed_at: None,
                last_error: None,
                outputs: None,
            },
        );

        let status = snapshot.status();
        assert_eq!(status.total_tasks, 3);
        assert_eq!(status.completed, 1);
        assert_eq!(status.running, 1);
        assert_eq!(status.pending, 1);
        assert_eq!(status.failed, 0);
    }

    #[test]
    fn test_workflow_status_with_failed_tasks() {
        let mut snapshot = create_test_snapshot();
        let task_id1 = TaskId::default();
        let task_id2 = TaskId::default();

        snapshot.executions.insert(
            task_id1,
            TaskExecution {
                task_id: task_id1,
                state: TaskState::Failed {
                    error_message: Some("error".to_string()),
                    failure_kind: FailureKind::Retryable,
                },
                attempts: 1,
                started_at: Some(Utc::now()),
                completed_at: Some(Utc::now()),
                last_error: Some("error".to_string()),
                outputs: None,
            },
        );

        snapshot.executions.insert(
            task_id2,
            TaskExecution {
                task_id: task_id2,
                state: TaskState::PermanentlyFailed {
                    error_message: Some("permanent error".to_string()),
                },
                attempts: 3,
                started_at: Some(Utc::now()),
                completed_at: Some(Utc::now()),
                last_error: Some("permanent error".to_string()),
                outputs: None,
            },
        );

        let status = snapshot.status();
        assert_eq!(status.total_tasks, 2);
        assert_eq!(status.failed, 2);
        assert_eq!(status.completed, 0);
    }

    #[test]
    fn test_workflow_status_with_scheduled_tasks() {
        let mut snapshot = create_test_snapshot();
        let task_id = TaskId::default();

        snapshot.executions.insert(
            task_id,
            TaskExecution {
                task_id,
                state: TaskState::Scheduled,
                attempts: 0,
                started_at: None,
                completed_at: None,
                last_error: None,
                outputs: None,
            },
        );

        let status = snapshot.status();
        assert_eq!(status.total_tasks, 1);
        assert_eq!(status.pending, 1);
    }

    #[test]
    fn test_workflow_status_with_cancelled_tasks() {
        let mut snapshot = create_test_snapshot();
        let task_id = TaskId::default();

        snapshot.executions.insert(
            task_id,
            TaskExecution {
                task_id,
                state: TaskState::Cancelled,
                attempts: 0,
                started_at: None,
                completed_at: Some(Utc::now()),
                last_error: Some("Cancelled".to_string()),
                outputs: None,
            },
        );

        let status = snapshot.status();
        assert_eq!(status.total_tasks, 1);
        // Cancelled tasks should not be counted in pending/running/completed/failed
        assert_eq!(status.pending, 0);
        assert_eq!(status.running, 0);
        assert_eq!(status.completed, 0);
        assert_eq!(status.failed, 0);
    }
}
