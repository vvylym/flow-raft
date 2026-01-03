//! Comprehensive tests for workflow state transitions

use flow_raft_core::{
    RetryConfig, Task, TaskDependencies, TaskId, Workflow, WorkflowDraft, WorkflowId,
};

#[test]
fn test_workflow_draft_to_scheduled() {
    let draft: Workflow<WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    // Draft workflow with no tasks cannot be scheduled
    let scheduled = draft.schedule();
    assert!(scheduled.is_err());
}

#[test]
fn test_workflow_scheduled_to_running() {
    let mut draft: Workflow<WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    // Add a task first
    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );
    draft = draft.add_task(task, RetryConfig::new(3)).unwrap();

    let scheduled = draft.schedule().unwrap();
    let running = scheduled.start();
    assert!(running.is_ok());
}

#[test]
fn test_workflow_state_variants() {
    use flow_raft_core::WorkflowState;

    // Test that all variants can be created
    let _draft = WorkflowState::Draft;
    let _scheduled = WorkflowState::Scheduled;
    let _running = WorkflowState::Running;
    let _paused = WorkflowState::Paused;
    let _completed = WorkflowState::Completed;
    let _failed = WorkflowState::Failed {
        error_message: None,
    };
    let _cancelled = WorkflowState::Cancelled;

    // Test pattern matching for terminal states
    assert!(matches!(WorkflowState::Completed, WorkflowState::Completed));
    let failed_state = WorkflowState::Failed {
        error_message: None,
    };
    assert!(matches!(failed_state, WorkflowState::Failed { .. }));
    assert!(matches!(WorkflowState::Cancelled, WorkflowState::Cancelled));
}
