//! Transitions from WorkflowScheduled state
//!
//! This module contains all state transitions that originate from the
//! WorkflowScheduled state. Each transition is clearly documented and independently
//! testable.

use chrono::Utc;

use crate::core::{Workflow, WorkflowCancelled, WorkflowError, WorkflowRunning, WorkflowScheduled};

impl Workflow<WorkflowScheduled> {
    /// Transitions from Scheduled to Running
    ///
    /// # Errors
    /// Returns `WorkflowError::NoTasksFound` if workflow has no tasks.
    pub fn start(self) -> Result<Workflow<WorkflowRunning>, WorkflowError> {
        if self.task_definitions.is_empty() {
            return Err(WorkflowError::NoTasksFound);
        }

        Ok(Workflow {
            id: self.id,
            task_definitions: self.task_definitions,
            executions: self.executions,
            dependencies: self.dependencies,
            retry_configs: self.retry_configs,
            created_at: self.created_at,
            started_at: Some(Utc::now()),
            completed_at: None,
            inputs: self.inputs,
            outputs: None,
            error_message: self.error_message,
            state: WorkflowRunning,
        })
    }

    /// Transitions from Scheduled to Cancelled
    ///
    /// Preserves existing error messages if present, otherwise sets a cancellation message.
    pub fn cancel(self) -> Workflow<WorkflowCancelled> {
        Workflow {
            id: self.id,
            task_definitions: self.task_definitions,
            executions: self.executions,
            dependencies: self.dependencies,
            retry_configs: self.retry_configs,
            created_at: self.created_at,
            started_at: None,
            completed_at: Some(Utc::now()),
            inputs: self.inputs,
            outputs: None,
            error_message: self
                .error_message
                .or_else(|| Some("Cancelled before start".to_string())),
            state: WorkflowCancelled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{RetryConfig, TaskDependencies, TaskId, TaskPending, WorkflowDraft};
    use rstest::*;

    #[fixture]
    fn test_workflow() -> Workflow<WorkflowDraft> {
        Workflow::new(crate::core::WorkflowId::default(), serde_json::json!({}))
    }

    #[fixture]
    fn test_task(#[default(TaskId::default())] id: TaskId) -> crate::core::Task<TaskPending> {
        crate::core::Task::new(
            id,
            "test_task",
            "test_handler",
            RetryConfig::new(3),
            TaskDependencies::default(),
        )
    }

    #[test]
    fn test_workflow_start() {
        let mut workflow = test_workflow();
        let task_id = TaskId::default();
        let task = test_task(task_id);
        workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();
        let scheduled = workflow.schedule().unwrap();

        let running = scheduled.start().unwrap();
        assert!(matches!(running.state, WorkflowRunning));
        assert!(running.started_at.is_some());
    }

    #[rstest]
    #[case::no_error_message(None, Some("Cancelled before start".to_string()))]
    #[case::with_error_message(Some("Previous error".to_string()), Some("Previous error".to_string()))]
    fn test_workflow_cancel_from_scheduled(
        #[case] initial_error: Option<String>,
        #[case] expected_error: Option<String>,
    ) {
        let mut workflow = test_workflow();
        let task_id = TaskId::default();
        let task = test_task(task_id);
        workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();
        let mut scheduled = workflow.schedule().unwrap();
        scheduled.error_message = initial_error;

        let cancelled = scheduled.cancel();
        assert!(matches!(cancelled.state, WorkflowCancelled));
        assert!(cancelled.completed_at.is_some());
        assert_eq!(cancelled.error_message, expected_error);
    }
}
