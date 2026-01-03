//! Transitions from TaskFailed state
//!
//! This module contains all state transitions that originate from the
//! TaskFailed state. Each transition is clearly documented and independently
//! testable.

use crate::{
    Task, TaskCancelled, TaskError, TaskFailed, TaskPermanentlyFailed, TaskScheduled,
};

impl Task<TaskFailed> {
    /// Transitions from Failed to Scheduled (retry)
    ///
    /// Validates that retry is possible before allowing the transition.
    ///
    /// # Errors
    /// Returns `TaskError::MaxRetriesExceeded` if retry limit is reached.
    pub fn retry(mut self) -> Result<Task<TaskScheduled>, TaskError> {
        // Increment retry counter
        self.retry_config
            .increment()
            .map_err(|_| TaskError::MaxRetriesExceeded {
                task_id: self.id.to_string(),
                max_attempts: self.retry_config.max_attempts,
                current_attempts: self.retry_config.current_attempt,
            })?;

        Ok(Task {
            id: self.id,
            name: self.name,
            handler: self.handler,
            inputs: self.inputs,
            outputs: self.outputs,
            dependencies: self.dependencies,
            retry_config: self.retry_config,
            timeout_secs: self.timeout_secs,
            state: TaskScheduled,
            started_at: None,
            completed_at: None,
            last_error: None,
            outputs_data: None,
        })
    }

    /// Transitions from Failed to PermanentlyFailed
    pub fn permanent_fail(self) -> Task<TaskPermanentlyFailed> {
        Task {
            id: self.id,
            name: self.name,
            handler: self.handler,
            inputs: self.inputs,
            outputs: self.outputs,
            dependencies: self.dependencies,
            retry_config: self.retry_config,
            timeout_secs: self.timeout_secs,
            state: TaskPermanentlyFailed {
                error_message: self.last_error.clone(),
            },
            started_at: self.started_at,
            completed_at: Some(chrono::Utc::now()),
            last_error: self.last_error,
            outputs_data: None,
        }
    }

    /// Transitions from Failed to Cancelled
    ///
    /// Preserves existing error messages if present, otherwise sets a cancellation message.
    pub fn cancel(self) -> Task<TaskCancelled> {
        Task {
            id: self.id,
            name: self.name,
            handler: self.handler,
            inputs: self.inputs,
            outputs: self.outputs,
            dependencies: self.dependencies,
            retry_config: self.retry_config,
            timeout_secs: self.timeout_secs,
            state: TaskCancelled,
            started_at: self.started_at,
            completed_at: Some(chrono::Utc::now()),
            last_error: self
                .last_error
                .or_else(|| Some("Cancelled after failure".to_string())),
            outputs_data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FailureKind, RetryConfig, TaskDependencies, TaskId, TaskPending, TaskRunning,
    };
    use rstest::*;
    use std::collections::HashSet;

    #[fixture]
    fn test_task() -> Task<TaskPending> {
        Task::new(
            TaskId::default(),
            "test_task",
            "test_handler",
            RetryConfig::new(3),
            TaskDependencies::default(),
        )
    }

    #[fixture]
    fn test_failed_task() -> Task<TaskFailed> {
        let task = test_task();
        let scheduled = task.schedule(&HashSet::new()).unwrap();
        let running: Task<TaskRunning> = scheduled.start();
        running.fail(Some("error".to_string()), FailureKind::Retryable)
    }

    #[test]
    fn test_task_retry_success() {
        let failed = test_failed_task();
        let retried = failed.retry().unwrap();
        assert!(matches!(retried.state, TaskScheduled));
        assert_eq!(retried.retry_config.current_attempt, 1);
        // Verify all fields are properly reset
        assert!(retried.started_at.is_none());
        assert!(retried.completed_at.is_none());
        assert!(retried.last_error.is_none());
        assert!(retried.outputs_data.is_none());
    }

    #[rstest]
    #[case::with_error(Some("error".to_string()), Some("error".to_string()))]
    #[case::no_error(None, None)]
    fn test_task_permanent_fail(
        #[case] error_message: Option<String>,
        #[case] expected_error: Option<String>,
    ) {
        let task = test_task();
        let scheduled = task.schedule(&HashSet::new()).unwrap();
        let running: Task<TaskRunning> = scheduled.start();
        let failed = running.fail(error_message.clone(), FailureKind::Retryable);
        let permanent = failed.permanent_fail();
        assert!(matches!(permanent.state, TaskPermanentlyFailed { .. }));
        assert!(permanent.completed_at.is_some());
        assert_eq!(permanent.last_error, expected_error);
        assert_eq!(permanent.state.error_message, expected_error);
    }

    #[rstest]
    #[case::with_error(Some("error".to_string()))]
    #[case::no_error(None)]
    fn test_task_cancel_from_failed(#[case] error_message: Option<String>) {
        let task = test_task();
        let scheduled = task.schedule(&HashSet::new()).unwrap();
        let running: Task<TaskRunning> = scheduled.start();
        let failed = running.fail(error_message.clone(), FailureKind::Retryable);
        let expected_error = error_message.or_else(|| Some("Cancelled after failure".to_string()));
        let cancelled = failed.cancel();
        assert!(matches!(cancelled.state, TaskCancelled));
        assert!(cancelled.completed_at.is_some());
        assert_eq!(cancelled.last_error, expected_error);
    }

    #[rstest]
    #[case::max_attempts_exceeded(1, 1, None)]
    #[case::terminal_failure(3, 0, Some(FailureKind::Terminal))]
    #[case::boundary_max_attempts(3, 3, None)]
    fn test_task_retry_error_cases(
        #[case] max_attempts: u8,
        #[case] current_attempt: u8,
        #[case] last_failure_kind: Option<FailureKind>,
    ) {
        let mut retry_config = RetryConfig::new(max_attempts);
        retry_config.current_attempt = current_attempt;
        retry_config.last_failure_kind = last_failure_kind;

        let task = Task::new(
            TaskId::default(),
            "test_task",
            "test_handler",
            retry_config,
            TaskDependencies::default(),
        );
        let scheduled = task.schedule(&HashSet::new()).unwrap();
        let running: Task<TaskRunning> = scheduled.start();
        let failure_kind = last_failure_kind.unwrap_or(FailureKind::Retryable);
        let failed = running.fail(Some("error".to_string()), failure_kind);
        let result = failed.retry();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TaskError::MaxRetriesExceeded { .. }
        ));
    }
}
