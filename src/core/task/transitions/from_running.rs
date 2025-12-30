//! Transitions from TaskRunning state
//!
//! This module contains all state transitions that originate from the
//! TaskRunning state. Each transition is clearly documented and independently
//! testable.

use chrono::Utc;

use crate::core::{FailureKind, Task, TaskCancelled, TaskCompleted, TaskFailed, TaskRunning};

impl Task<TaskRunning> {
    /// Transitions from Running to Completed
    ///
    /// # Arguments
    /// * `outputs` - Optional task outputs
    pub fn complete(self, outputs: Option<serde_json::Value>) -> Task<TaskCompleted> {
        Task {
            id: self.id,
            name: self.name,
            handler: self.handler,
            inputs: self.inputs,
            outputs: self.outputs,
            dependencies: self.dependencies,
            retry_config: self.retry_config,
            timeout_secs: self.timeout_secs,
            state: TaskCompleted,
            started_at: self.started_at,
            completed_at: Some(Utc::now()),
            last_error: None,
            outputs_data: outputs,
        }
    }

    /// Transitions from Running to Failed
    ///
    /// # Arguments
    /// * `error_message` - Error message
    /// * `failure_kind` - Type of failure (retryable or terminal)
    pub fn fail(
        self,
        error_message: Option<String>,
        failure_kind: FailureKind,
    ) -> Task<TaskFailed> {
        let mut retry_config = self.retry_config;
        retry_config.last_failure_kind = Some(failure_kind);

        Task {
            id: self.id,
            name: self.name,
            handler: self.handler,
            inputs: self.inputs,
            outputs: self.outputs,
            dependencies: self.dependencies,
            retry_config,
            timeout_secs: self.timeout_secs,
            state: TaskFailed {
                error_message: error_message.clone(),
                failure_kind,
            },
            started_at: self.started_at,
            completed_at: Some(Utc::now()),
            last_error: error_message,
            outputs_data: None,
        }
    }

    /// Transitions from Running to Cancelled
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
            completed_at: Some(Utc::now()),
            last_error: self.last_error.or_else(|| Some("Cancelled".to_string())),
            outputs_data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{RetryConfig, TaskDependencies, TaskId, TaskPending, TaskScheduled};
    use std::collections::HashSet;

    fn test_task() -> Task<TaskPending> {
        Task::new(
            TaskId::default(),
            "test_task",
            "test_handler",
            RetryConfig::new(3),
            TaskDependencies::default(),
        )
    }

    fn test_running_task() -> Task<TaskRunning> {
        let task = test_task();
        let scheduled: Task<TaskScheduled> = task.schedule(&HashSet::new()).unwrap();
        scheduled.start()
    }

    #[test]
    fn test_task_complete() {
        let task = test_task();
        let scheduled = task.schedule(&HashSet::new()).unwrap();
        let running = scheduled.start();
        let outputs = serde_json::json!({"result": "success"});
        let completed = running.complete(Some(outputs.clone()));
        assert!(matches!(completed.state, TaskCompleted));
        assert!(completed.completed_at.is_some());
        assert_eq!(completed.outputs_data, Some(outputs));
    }

    #[test]
    fn test_task_fail_retryable() {
        let task = test_task();
        let scheduled = task.schedule(&HashSet::new()).unwrap();
        let running = scheduled.start();
        let failed = running.fail(Some("test error".to_string()), FailureKind::Retryable);
        assert!(matches!(failed.state, TaskFailed { .. }));
        assert!(failed.completed_at.is_some());
        assert_eq!(failed.last_error, Some("test error".to_string()));
        assert_eq!(
            failed.retry_config.last_failure_kind,
            Some(FailureKind::Retryable)
        );
    }

    #[test]
    fn test_task_fail_terminal() {
        let task = test_task();
        let scheduled = task.schedule(&HashSet::new()).unwrap();
        let running = scheduled.start();
        let failed = running.fail(Some("terminal error".to_string()), FailureKind::Terminal);
        assert!(matches!(failed.state, TaskFailed { .. }));
        assert_eq!(
            failed.retry_config.last_failure_kind,
            Some(FailureKind::Terminal)
        );
    }

    #[test]
    fn test_task_cancel_from_running() {
        let running = test_running_task();
        let cancelled = running.cancel();
        assert!(matches!(cancelled.state, TaskCancelled));
        assert!(cancelled.started_at.is_some());
        assert!(cancelled.completed_at.is_some());
        assert_eq!(cancelled.last_error, Some("Cancelled".to_string()));
    }

    #[test]
    fn test_task_complete_with_none_outputs() {
        let running = test_running_task();
        let completed = running.complete(None);
        assert!(matches!(completed.state, TaskCompleted));
        assert!(completed.completed_at.is_some());
        assert_eq!(completed.outputs_data, None);
    }

    #[test]
    fn test_task_fail_with_none_error_message() {
        let running = test_running_task();
        let failed = running.fail(None, FailureKind::Retryable);
        assert!(matches!(failed.state, TaskFailed { .. }));
        assert!(failed.completed_at.is_some());
        assert_eq!(failed.last_error, None);
        assert_eq!(
            failed.retry_config.last_failure_kind,
            Some(FailureKind::Retryable)
        );
    }

    #[test]
    fn test_task_failed_state_fields() {
        let running = test_running_task();
        let failed = running.fail(
            Some("test error message".to_string()),
            FailureKind::Retryable,
        );

        // Verify the state struct fields
        let TaskFailed {
            error_message,
            failure_kind,
        } = failed.state;

        assert_eq!(error_message, Some("test error message".to_string()));
        assert_eq!(failure_kind, FailureKind::Retryable);
    }

    #[test]
    fn test_task_started_at_preserved_on_failure() {
        let running = test_running_task();
        let started_at = running.started_at;

        let failed = running.fail(Some("error".to_string()), FailureKind::Retryable);
        assert_eq!(failed.started_at, started_at);
    }

    #[test]
    fn test_task_cancel_from_running_preserves_existing_error() {
        let mut running = test_running_task();
        running.last_error = Some("Previous error".to_string());
        let cancelled = running.cancel();
        assert!(matches!(cancelled.state, TaskCancelled));
        assert_eq!(cancelled.last_error, Some("Previous error".to_string()));
    }
}
