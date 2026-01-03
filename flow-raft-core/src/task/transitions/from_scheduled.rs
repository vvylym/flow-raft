//! Transitions from TaskScheduled state
//!
//! This module contains all state transitions that originate from the
//! TaskScheduled state. Each transition is clearly documented and independently
//! testable.

use chrono::Utc;

use crate::{Task, TaskCancelled, TaskRunning, TaskScheduled};

impl Task<TaskScheduled> {
    /// Transitions from Scheduled to Running
    pub fn start(self) -> Task<TaskRunning> {
        Task {
            id: self.id,
            name: self.name,
            handler: self.handler,
            inputs: self.inputs,
            outputs: self.outputs,
            dependencies: self.dependencies,
            retry_config: self.retry_config,
            timeout_secs: self.timeout_secs,
            state: TaskRunning,
            started_at: Some(Utc::now()),
            completed_at: None,
            last_error: None,
            outputs_data: None,
        }
    }

    /// Transitions from Scheduled to Cancelled
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
            started_at: None,
            completed_at: Some(Utc::now()),
            last_error: self.last_error.or_else(|| Some("Cancelled".to_string())),
            outputs_data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RetryConfig, TaskDependencies, TaskId, TaskPending};
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

    #[test]
    fn test_task_start() {
        let task = test_task();
        let scheduled = task.schedule(&HashSet::new()).unwrap();
        let running = scheduled.start();
        assert!(matches!(running.state, TaskRunning));
        assert!(running.started_at.is_some());
    }

    #[test]
    fn test_task_cancel_from_scheduled() {
        let task = test_task();
        let scheduled = task.schedule(&HashSet::new()).unwrap();
        let cancelled = scheduled.cancel();
        assert!(matches!(cancelled.state, TaskCancelled));
        assert!(cancelled.completed_at.is_some());
        assert_eq!(cancelled.last_error, Some("Cancelled".to_string()));
    }

    #[test]
    fn test_task_cancel_from_scheduled_preserves_existing_error() {
        let task = test_task();
        let mut scheduled = task.schedule(&HashSet::new()).unwrap();
        // Manually set an error (simulating a scenario where error was set before cancellation)
        scheduled.last_error = Some("Previous error".to_string());
        let cancelled = scheduled.cancel();
        assert!(matches!(cancelled.state, TaskCancelled));
        assert_eq!(cancelled.last_error, Some("Previous error".to_string()));
    }
}
