//! Transitions from WorkflowPaused state
//!
//! This module contains all state transitions that originate from the
//! WorkflowPaused state. Each transition is clearly documented and independently
//! testable.

use crate::core::{Workflow, WorkflowCancelled, WorkflowPaused, WorkflowRunning};

impl Workflow<WorkflowPaused> {
    /// Transitions from Paused to Running
    pub fn resume(self) -> Workflow<WorkflowRunning> {
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
            state: WorkflowRunning,
        }
    }

    /// Transitions from Paused to Cancelled
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
            started_at: self.started_at,
            completed_at: Some(chrono::Utc::now()),
            inputs: self.inputs,
            outputs: None,
            error_message: self
                .error_message
                .or_else(|| Some("Cancelled while paused".to_string())),
            state: WorkflowCancelled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        RetryConfig, Task, TaskDependencies, TaskId, TaskPending, WorkflowDraft, WorkflowRunning,
        WorkflowScheduled,
    };
    use rstest::*;

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

    fn test_paused_workflow() -> Workflow<WorkflowPaused> {
        let mut workflow = test_workflow();
        let task_id = TaskId::default();
        let task = test_task(task_id);
        workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();
        let scheduled: Workflow<WorkflowScheduled> = workflow.schedule().unwrap();
        let running: Workflow<WorkflowRunning> = scheduled.start().unwrap();
        running.pause()
    }

    #[test]
    fn test_workflow_resume() {
        let paused = test_paused_workflow();
        let started_at = paused.started_at;

        let resumed = paused.resume();
        assert!(matches!(resumed.state, WorkflowRunning));
        assert_eq!(resumed.started_at, started_at);
    }

    /// Comprehensive lifecycle test for workflow pause/resume
    #[test]
    fn test_workflow_pause_resume_lifecycle() {
        let mut workflow = test_workflow();
        let task1_id = TaskId::default();
        let task2_id = TaskId::default();

        let task1 = test_task(task1_id);
        workflow = workflow.add_task(task1, RetryConfig::new(3)).unwrap();

        let mut deps = TaskDependencies::default();
        deps.add_prerequisite(task1_id);
        let task2 = Task::new(task2_id, "task2", "handler2", RetryConfig::new(3), deps);
        workflow = workflow.add_task(task2, RetryConfig::new(3)).unwrap();

        // Draft -> Scheduled -> Running
        let scheduled = workflow.schedule().unwrap();
        let running = scheduled.start().unwrap();

        // Running -> Paused
        let started_at = running.started_at;
        let paused = running.pause();
        assert!(matches!(paused.state, WorkflowPaused));
        assert_eq!(paused.started_at, started_at);

        // Paused -> Running (resume)
        let resumed = paused.resume();
        assert!(matches!(resumed.state, WorkflowRunning));
        assert_eq!(resumed.started_at, started_at);

        // Verify ready tasks still work after resume
        let ready = resumed.get_ready_tasks();
        assert!(ready.contains(&task1_id));
    }

    #[test]
    fn test_workflow_cancel_from_paused() {
        let paused = test_paused_workflow();

        let cancelled = paused.cancel();
        assert!(matches!(cancelled.state, WorkflowCancelled));
        assert_eq!(
            cancelled.error_message,
            Some("Cancelled while paused".to_string())
        );
    }

    #[test]
    fn test_workflow_cancel_from_paused_preserves_existing_error() {
        let mut paused = test_paused_workflow();
        paused.error_message = Some("Previous error".to_string());
        let cancelled = paused.cancel();
        assert!(matches!(cancelled.state, WorkflowCancelled));
        assert_eq!(cancelled.error_message, Some("Previous error".to_string()));
    }
}
