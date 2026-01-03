//! Transitions from TaskPending state
//!
//! This module contains all state transitions that originate from the
//! TaskPending state. Each transition is clearly documented and independently
//! testable.

use chrono::Utc;
use std::collections::HashSet;

use crate::{
    RetryConfig, Task, TaskCancelled, TaskDependencies, TaskError, TaskId, TaskPending,
    TaskScheduled,
};

impl Task<TaskPending> {
    /// Creates a new pending task
    ///
    /// # Arguments
    /// * `id` - Task identifier
    /// * `name` - Task name
    /// * `handler` - Execution handler identifier
    /// * `retry_config` - Retry configuration
    /// * `dependencies` - Task dependencies
    pub fn new(
        id: TaskId,
        name: impl Into<String>,
        handler: impl Into<String>,
        retry_config: RetryConfig,
        dependencies: TaskDependencies,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            handler: handler.into(),
            inputs: HashSet::new(),
            outputs: HashSet::new(),
            dependencies,
            retry_config,
            timeout_secs: None,
            state: TaskPending,
            started_at: None,
            completed_at: None,
            last_error: None,
            outputs_data: None,
        }
    }

    /// Transitions from Pending to Scheduled
    ///
    /// Validates that all prerequisites are completed before allowing the transition.
    ///
    /// # Arguments
    /// * `completed` - Set of completed task IDs
    ///
    /// # Errors
    /// Returns `TaskError::DependencyNotSatisfied` if prerequisites are not met.
    pub fn schedule(self, completed: &HashSet<TaskId>) -> Result<Task<TaskScheduled>, TaskError> {
        // Validate all prerequisites are completed
        if !self.dependencies.has_all_prerequisites_completed(completed) {
            // Find which prerequisite is not completed
            for &prereq in &self.dependencies.prerequisites {
                if !completed.contains(&prereq) {
                    return Err(TaskError::DependencyNotSatisfied {
                        task_id: self.id.to_string(),
                        dependency_id: prereq.to_string(),
                    });
                }
            }
        }

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

    /// Transitions from Pending to Cancelled
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
    use crate::TaskDependencies;
    use rstest::*;

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

    #[rstest]
    fn test_task_new(test_task: Task<TaskPending>) {
        let task = test_task;
        assert_eq!(task.name, "test_task");
        assert_eq!(task.handler, "test_handler");
        assert!(task.inputs.is_empty());
        assert!(task.outputs.is_empty());
    }

    #[test]
    fn test_task_schedule_no_dependencies() {
        let task = test_task();
        let completed = HashSet::new();
        let scheduled = task.schedule(&completed).unwrap();
        assert!(matches!(scheduled.state, TaskScheduled));
    }

    #[test]
    fn test_task_schedule_with_dependencies_met() {
        let prereq_id = TaskId::default();
        let mut deps = TaskDependencies::default();
        deps.add_prerequisite(prereq_id);

        let task = Task::new(
            TaskId::default(),
            "test_task",
            "test_handler",
            RetryConfig::new(3),
            deps,
        );

        let mut completed = HashSet::new();
        completed.insert(prereq_id);

        let scheduled = task.schedule(&completed).unwrap();
        assert!(matches!(scheduled.state, TaskScheduled));
    }

    #[test]
    fn test_task_schedule_with_dependencies_not_met() {
        let prereq_id = TaskId::default();
        let mut deps = TaskDependencies::default();
        deps.add_prerequisite(prereq_id);

        let task = Task::new(
            TaskId::default(),
            "test_task",
            "test_handler",
            RetryConfig::new(3),
            deps,
        );

        let completed = HashSet::new();
        let result = task.schedule(&completed);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TaskError::DependencyNotSatisfied { .. }
        ));
    }

    #[rstest]
    #[case::no_error(None, Some("Cancelled".to_string()))]
    #[case::with_error(Some("Previous error".to_string()), Some("Previous error".to_string()))]
    fn test_task_cancel_from_pending(
        test_task: Task<TaskPending>,
        #[case] initial_error: Option<String>,
        #[case] expected_error: Option<String>,
    ) {
        let mut task = test_task;
        task.last_error = initial_error;
        let cancelled = task.cancel();
        assert!(matches!(cancelled.state, TaskCancelled));
        assert!(cancelled.completed_at.is_some());
        assert_eq!(cancelled.last_error, expected_error);
    }
}
