//! Transitions from WorkflowRunning state
//!
//! This module contains all state transitions that originate from the
//! WorkflowRunning state. Each transition is clearly documented and independently
//! testable.

use chrono::Utc;
use indexmap::IndexMap;
use rayon::prelude::*;
use std::collections::HashSet;

use crate::core::dag::ready_tasks;
use crate::core::{
    TaskId, TaskState, Workflow, WorkflowCancelled, WorkflowCompleted, WorkflowFailed,
    WorkflowPaused, WorkflowRunning,
};

impl Workflow<WorkflowRunning> {
    /// Gets tasks that are ready to execute (all prerequisites completed)
    ///
    /// Uses parallel processing for improved performance.
    ///
    /// # Returns
    /// Vector of task IDs ready to execute
    pub fn get_ready_tasks(&self) -> Vec<TaskId> {
        // Collect completed tasks using parallel processing
        let completed: HashSet<TaskId> = self
            .executions
            .par_iter()
            .filter(|(_, exec)| exec.state.is_terminal())
            .map(|(_, exec)| exec.task_id)
            .collect();

        // Create task map using parallel processing
        let tasks: IndexMap<TaskId, ()> = self
            .task_definitions
            .keys()
            .copied()
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|id| (id, ()))
            .collect();

        ready_tasks(&tasks, &self.dependencies, &completed)
    }

    /// Transitions from Running to Completed
    ///
    /// Validates that all tasks are in terminal states.
    pub fn complete(self) -> Workflow<WorkflowCompleted> {
        // Collect outputs from completed tasks using parallel processing
        let output_pairs: Vec<(String, serde_json::Value)> = self
            .executions
            .par_iter()
            .flat_map(|(_, execution)| {
                if let (Some(task_def), Some(outputs_data)) = (
                    self.task_definitions.get(&execution.task_id),
                    &execution.outputs,
                ) {
                    task_def
                        .outputs
                        .iter()
                        .filter_map(|output_name| {
                            outputs_data
                                .get(output_name)
                                .map(|value| (output_name.clone(), value.clone()))
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                }
            })
            .collect();

        let mut outputs = serde_json::Map::new();
        for (key, value) in output_pairs {
            outputs.insert(key, value);
        }

        Workflow {
            id: self.id,
            task_definitions: self.task_definitions,
            executions: self.executions,
            dependencies: self.dependencies,
            retry_configs: self.retry_configs,
            created_at: self.created_at,
            started_at: self.started_at,
            completed_at: Some(Utc::now()),
            inputs: self.inputs,
            outputs: if outputs.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(outputs))
            },
            error_message: None,
            state: WorkflowCompleted,
        }
    }

    /// Transitions from Running to Failed
    ///
    /// # Arguments
    /// * `error_message` - Error message describing the failure
    pub fn fail(self, error_message: Option<String>) -> Workflow<WorkflowFailed> {
        Workflow {
            id: self.id,
            task_definitions: self.task_definitions,
            executions: self.executions,
            dependencies: self.dependencies,
            retry_configs: self.retry_configs,
            created_at: self.created_at,
            started_at: self.started_at,
            completed_at: Some(Utc::now()),
            inputs: self.inputs,
            outputs: None,
            error_message: error_message.clone(),
            state: WorkflowFailed { error_message },
        }
    }

    /// Transitions from Running to Paused
    pub fn pause(self) -> Workflow<WorkflowPaused> {
        Workflow {
            id: self.id,
            task_definitions: self.task_definitions,
            executions: self.executions,
            dependencies: self.dependencies,
            retry_configs: self.retry_configs,
            created_at: self.created_at,
            started_at: self.started_at,
            completed_at: None,
            inputs: self.inputs,
            outputs: None,
            error_message: None,
            state: WorkflowPaused,
        }
    }

    /// Transitions from Running to Cancelled
    pub fn cancel(mut self) -> Workflow<WorkflowCancelled> {
        // Mark all non-terminal tasks as cancelled using parallel processing
        // Collect task IDs that need to be cancelled
        let tasks_to_cancel: Vec<TaskId> = self
            .executions
            .par_iter()
            .filter(|(_, exec)| !exec.state.is_terminal())
            .map(|(task_id, _)| *task_id)
            .collect();

        // Update executions (sequential for mutable access)
        // Preserve existing error messages if present, otherwise set cancellation message
        let now = Utc::now();
        for task_id in tasks_to_cancel {
            if let Some(execution) = self.executions.get_mut(&task_id) {
                execution.state = TaskState::Cancelled;
                execution.completed_at = Some(now);
                // Preserve existing error message if present, otherwise set cancellation message
                if execution.last_error.is_none() {
                    execution.last_error = Some("Cancelled".to_string());
                }
            }
        }

        Workflow {
            id: self.id,
            task_definitions: self.task_definitions,
            executions: self.executions,
            dependencies: self.dependencies,
            retry_configs: self.retry_configs,
            created_at: self.created_at,
            started_at: self.started_at,
            completed_at: Some(Utc::now()),
            inputs: self.inputs,
            outputs: None,
            error_message: self.error_message.or_else(|| Some("Cancelled".to_string())),
            state: WorkflowCancelled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        RetryConfig, Task, TaskDependencies, TaskId, TaskPending, TaskState, WorkflowDraft,
        WorkflowScheduled,
    };
    use rstest::*;
    use std::collections::HashSet;

    #[fixture]
    fn test_workflow() -> Workflow<WorkflowDraft> {
        Workflow::new(crate::core::WorkflowId::default(), serde_json::json!({}))
    }

    #[fixture]
    fn test_task(#[default(TaskId::default())] id: TaskId) -> Task<TaskPending> {
        Task::new(
            id,
            "test_task",
            "test_handler",
            RetryConfig::new(3),
            TaskDependencies::default(),
        )
    }

    fn test_running_workflow() -> Workflow<WorkflowRunning> {
        let mut workflow = test_workflow();
        let task_id = TaskId::default();
        let task = test_task(task_id);
        workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();
        let scheduled: Workflow<WorkflowScheduled> = workflow.schedule().unwrap();
        scheduled.start().unwrap()
    }

    #[test]
    fn test_workflow_get_ready_tasks() {
        let mut workflow = test_workflow();
        let task1_id = TaskId::default();
        let task2_id = TaskId::default();

        let task1 = test_task(task1_id);
        workflow = workflow.add_task(task1, RetryConfig::new(3)).unwrap();

        let mut deps = TaskDependencies::default();
        deps.add_prerequisite(task1_id);
        let task2 = Task::new(task2_id, "task2", "handler2", RetryConfig::new(3), deps);
        workflow = workflow.add_task(task2, RetryConfig::new(3)).unwrap();

        let scheduled = workflow.schedule().unwrap();
        let running = scheduled.start().unwrap();

        let ready = running.get_ready_tasks();
        assert!(ready.contains(&task1_id));
        assert!(!ready.contains(&task2_id)); // task2 depends on task1
    }

    #[rstest]
    #[case::with_error(Some("test error".to_string()), Some("test error".to_string()))]
    #[case::no_error(None, None)]
    fn test_workflow_fail(
        #[case] error_message: Option<String>,
        #[case] expected_error: Option<String>,
    ) {
        let running = test_running_workflow();
        let running_id = running.id;
        let running_task_defs_len = running.task_definitions.len();
        let running_executions_len = running.executions.len();
        let failed = running.fail(error_message);
        assert!(matches!(failed.state, WorkflowFailed { .. }));
        assert!(failed.completed_at.is_some());
        assert_eq!(failed.error_message, expected_error);
        // Verify state also has the error_message
        assert_eq!(failed.state.error_message, expected_error);
        // Verify all fields are preserved
        assert_eq!(failed.id, running_id);
        assert_eq!(failed.task_definitions.len(), running_task_defs_len);
        assert_eq!(failed.executions.len(), running_executions_len);
    }

    #[test]
    fn test_workflow_pause() {
        let running = test_running_workflow();
        let running_id = running.id;
        let running_started_at = running.started_at;
        let running_task_defs_len = running.task_definitions.len();
        let running_executions_len = running.executions.len();

        let paused = running.pause();
        assert!(matches!(paused.state, WorkflowPaused));
        // Verify all fields are preserved
        assert_eq!(paused.id, running_id);
        assert_eq!(paused.started_at, running_started_at);
        assert_eq!(paused.task_definitions.len(), running_task_defs_len);
        assert_eq!(paused.executions.len(), running_executions_len);
        // Verify pause-specific fields
        assert!(paused.completed_at.is_none());
        assert!(paused.outputs.is_none());
        assert!(paused.error_message.is_none());
    }

    #[rstest]
    #[case::no_error_message(None, Some("Cancelled".to_string()))]
    #[case::with_error_message(Some("Previous error".to_string()), Some("Previous error".to_string()))]
    fn test_workflow_cancel_from_running(
        #[case] initial_error: Option<String>,
        #[case] expected_error: Option<String>,
    ) {
        let mut workflow = test_workflow();
        let task_id = TaskId::default();
        let task = test_task(task_id);
        workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();

        let scheduled = workflow.schedule().unwrap();
        let mut running = scheduled.start().unwrap();
        running.error_message = initial_error;

        let cancelled = running.cancel();
        assert!(matches!(cancelled.state, WorkflowCancelled));
        assert!(cancelled.completed_at.is_some());
        assert_eq!(cancelled.error_message, expected_error);

        // Check that task execution was marked as cancelled
        if let Some(execution) = cancelled.executions.get(&task_id) {
            assert!(matches!(execution.state, TaskState::Cancelled));
        }
    }

    /// Test workflow complete with multiple tasks and output collection
    #[test]
    fn test_workflow_complete_with_outputs() {
        let mut workflow = test_workflow();
        let task1_id = TaskId::default();
        let task2_id = TaskId::default();

        let mut task1 = test_task(task1_id);
        task1.outputs = HashSet::from_iter(["result1".to_string(), "result2".to_string()]);
        workflow = workflow.add_task(task1, RetryConfig::new(3)).unwrap();

        let mut task2 = test_task(task2_id);
        task2.outputs = HashSet::from_iter(["result3".to_string()]);
        workflow = workflow.add_task(task2, RetryConfig::new(3)).unwrap();

        let scheduled = workflow.schedule().unwrap();
        let mut running = scheduled.start().unwrap();

        // Mark tasks as completed with outputs matching their output names
        if let Some(exec1) = running.executions.get_mut(&task1_id) {
            exec1.state = TaskState::Completed;
            exec1.outputs = Some(serde_json::json!({
                "result1": "value1",
                "result2": "value2"
            }));
        }
        if let Some(exec2) = running.executions.get_mut(&task2_id) {
            exec2.state = TaskState::Completed;
            exec2.outputs = Some(serde_json::json!({
                "result3": "value3"
            }));
        }

        let completed = running.complete();
        assert!(matches!(completed.state, WorkflowCompleted));
        assert!(completed.outputs.is_some());

        // Verify outputs were collected
        let outputs = completed.outputs.unwrap();
        assert_eq!(outputs.get("result1"), Some(&serde_json::json!("value1")));
        assert_eq!(outputs.get("result2"), Some(&serde_json::json!("value2")));
        assert_eq!(outputs.get("result3"), Some(&serde_json::json!("value3")));
    }
}
