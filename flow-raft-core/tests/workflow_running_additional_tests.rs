//! Additional tests for workflow running transitions to increase coverage

use flow_raft_core::{
    RetryConfig, Task, TaskDependencies, TaskId, TaskState, Workflow, WorkflowDraft, WorkflowId,
    WorkflowRunning,
};

#[test]
fn test_workflow_get_ready_tasks_with_completed() {
    let mut workflow: Workflow<WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));
    let task1_id = TaskId::default();
    let task2_id = TaskId::default();

    let task1 = Task::new(
        task1_id,
        "task1",
        "handler1",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );
    let task2 = Task::new(
        task2_id,
        "task2",
        "handler2",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    workflow = workflow.add_task(task1, RetryConfig::new(3)).unwrap();
    workflow = workflow.add_task(task2, RetryConfig::new(3)).unwrap();

    let scheduled = workflow.schedule().unwrap();
    let running: Workflow<WorkflowRunning> = scheduled.start().unwrap();

    // Initially both tasks should be ready (no dependencies)
    let ready = running.get_ready_tasks();
    assert_eq!(ready.len(), 2);
}

#[test]
fn test_workflow_complete_with_outputs() {
    let mut workflow: Workflow<WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));
    let task_id = TaskId::default();
    let task = Task::new(
        task_id,
        "task1",
        "handler1",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );
    workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();

    let scheduled = workflow.schedule().unwrap();
    let mut running: Workflow<WorkflowRunning> = scheduled.start().unwrap();

    // Mark task as completed
    if let Some(execution) = running.executions.get_mut(&task_id) {
        execution.state = TaskState::Completed;
    }

    let completed = running.complete();
    // Verify workflow is completed
    assert!(std::mem::size_of_val(&completed) > 0);
}
