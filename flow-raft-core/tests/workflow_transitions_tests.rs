//! Comprehensive tests for workflow transitions

use flow_raft_core::{RetryConfig, Task, TaskDependencies, TaskId, Workflow, WorkflowId};

#[test]
fn test_workflow_draft_add_task() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    let result = workflow.add_task(task, RetryConfig::new(3));
    assert!(result.is_ok());
}

#[test]
fn test_workflow_draft_add_task_with_dependency() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    let task1 = Task::new(
        TaskId::default(),
        "task1",
        "handler1",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    let workflow = workflow.add_task(task1, RetryConfig::new(3)).unwrap();

    let task1_id = workflow.task_definitions.keys().next().unwrap();
    let mut deps = TaskDependencies::default();
    deps.add_prerequisite(*task1_id);

    let task2 = Task::new(
        TaskId::default(),
        "task2",
        "handler2",
        RetryConfig::new(3),
        deps,
    );

    let result = workflow.add_task(task2, RetryConfig::new(3));
    assert!(result.is_ok());
}

#[test]
fn test_workflow_draft_add_task_missing_dependency() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    let missing_id = TaskId::default();
    let mut deps = TaskDependencies::default();
    deps.add_prerequisite(missing_id);

    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        deps,
    );

    let result = workflow.add_task(task, RetryConfig::new(3));
    assert!(result.is_err());
}

#[test]
fn test_workflow_draft_to_scheduled() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    let workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();
    let scheduled = workflow.schedule();
    assert!(scheduled.is_ok());
}

#[test]
fn test_workflow_draft_to_scheduled_no_tasks() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    let scheduled = workflow.schedule();
    assert!(scheduled.is_err());
}

// Note: WorkflowDraft doesn't have a cancel method - cancellation happens from Scheduled/Running/Paused states
// This test is removed as it's not a valid transition

#[test]
fn test_workflow_scheduled_to_running() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    let workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();
    let scheduled = workflow.schedule().unwrap();
    let running = scheduled.start();
    assert!(running.is_ok());
    let running = running.unwrap();
    assert!(matches!(running.state, flow_raft_core::WorkflowRunning));
    assert!(running.started_at.is_some());
}

#[test]
fn test_workflow_scheduled_to_running_no_tasks() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    // Create scheduled workflow directly (bypassing validation)
    let scheduled: Workflow<flow_raft_core::WorkflowScheduled> = Workflow {
        id: workflow.id,
        task_definitions: workflow.task_definitions,
        executions: workflow.executions,
        dependencies: workflow.dependencies,
        retry_configs: workflow.retry_configs,
        created_at: workflow.created_at,
        started_at: None,
        completed_at: None,
        inputs: workflow.inputs,
        outputs: None,
        error_message: None,
        state: flow_raft_core::WorkflowScheduled,
    };

    let running = scheduled.start();
    assert!(running.is_err());
}

#[test]
fn test_workflow_scheduled_to_cancelled() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    let workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();
    let scheduled = workflow.schedule().unwrap();
    let cancelled = scheduled.cancel();

    assert!(matches!(cancelled.state, flow_raft_core::WorkflowCancelled));
    assert!(cancelled.completed_at.is_some());
}

#[test]
fn test_workflow_running_get_ready_tasks() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    let task1 = Task::new(
        TaskId::default(),
        "task1",
        "handler1",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    let task1_id = task1.id;
    let workflow = workflow.add_task(task1, RetryConfig::new(3)).unwrap();
    let scheduled = workflow.schedule().unwrap();
    let running = scheduled.start().unwrap();

    let ready = running.get_ready_tasks();
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&task1_id));
}

#[test]
fn test_workflow_running_to_completed() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    let task_id = task.id;
    let workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();
    let scheduled = workflow.schedule().unwrap();
    let running = scheduled.start().unwrap();

    // Mark task as completed
    let mut running = running;
    if let Some(execution) = running.executions.get_mut(&task_id) {
        execution.state = flow_raft_core::TaskState::Completed;
        execution.completed_at = Some(chrono::Utc::now());
    }

    let completed = running.complete();
    assert!(matches!(completed.state, flow_raft_core::WorkflowCompleted));
    assert!(completed.completed_at.is_some());
}

#[test]
fn test_workflow_running_to_paused() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    let workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();
    let scheduled = workflow.schedule().unwrap();
    let running = scheduled.start().unwrap();
    let paused = running.pause();

    assert!(matches!(paused.state, flow_raft_core::WorkflowPaused));
}

#[test]
fn test_workflow_running_to_failed() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    let workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();
    let scheduled = workflow.schedule().unwrap();
    let running = scheduled.start().unwrap();
    let failed = running.fail(Some("workflow error".to_string()));

    assert!(matches!(
        failed.state,
        flow_raft_core::WorkflowFailed { .. }
    ));
}

#[test]
fn test_workflow_running_to_cancelled() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    let workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();
    let scheduled = workflow.schedule().unwrap();
    let running = scheduled.start().unwrap();
    let cancelled = running.cancel();

    assert!(matches!(cancelled.state, flow_raft_core::WorkflowCancelled));
    assert!(cancelled.completed_at.is_some());
}

#[test]
fn test_workflow_paused_to_running() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    let workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();
    let scheduled = workflow.schedule().unwrap();
    let running = scheduled.start().unwrap();
    let paused = running.pause();
    let resumed = paused.resume();

    assert!(matches!(resumed.state, flow_raft_core::WorkflowRunning));
}

#[test]
fn test_workflow_paused_to_cancelled() {
    let workflow: Workflow<flow_raft_core::WorkflowDraft> =
        Workflow::new(WorkflowId::default(), serde_json::json!({}));

    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    let workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();
    let scheduled = workflow.schedule().unwrap();
    let running = scheduled.start().unwrap();
    let paused = running.pause();
    let cancelled = paused.cancel();

    assert!(matches!(cancelled.state, flow_raft_core::WorkflowCancelled));
    assert!(cancelled.completed_at.is_some());
}
