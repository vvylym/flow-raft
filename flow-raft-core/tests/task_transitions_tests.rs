//! Comprehensive tests for task transitions

use flow_raft_core::{FailureKind, RetryConfig, Task, TaskDependencies, TaskId};

#[test]
fn test_task_pending_to_scheduled() {
    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    let completed = std::collections::HashSet::new();
    let scheduled = task.schedule(&completed);
    assert!(scheduled.is_ok());
}

#[test]
fn test_task_pending_to_scheduled_with_dependencies() {
    let dep_id = TaskId::default();
    let task_id = TaskId::default();

    let mut deps = TaskDependencies::default();
    deps.add_prerequisite(dep_id);

    let task = Task::new(task_id, "test_task", "handler", RetryConfig::new(3), deps);

    let mut completed = std::collections::HashSet::new();
    // Without dependency completed, should fail
    let scheduled = task.schedule(&completed);
    assert!(scheduled.is_err());

    // With dependency completed, should succeed
    completed.insert(dep_id);
    let task2 = Task::new(
        task_id,
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::with_prerequisites(vec![dep_id]),
    );
    let scheduled = task2.schedule(&completed);
    assert!(scheduled.is_ok());
}

#[test]
fn test_task_pending_to_cancelled() {
    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );

    let cancelled = task.cancel();
    assert!(matches!(cancelled.state, flow_raft_core::TaskCancelled));
    assert!(cancelled.completed_at.is_some());
}

#[test]
fn test_task_scheduled_to_running() {
    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );
    let completed = std::collections::HashSet::new();
    let scheduled = task.schedule(&completed).unwrap();
    let running = scheduled.start();

    assert!(matches!(running.state, flow_raft_core::TaskRunning));
    assert!(running.started_at.is_some());
}

#[test]
fn test_task_scheduled_to_cancelled() {
    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );
    let completed = std::collections::HashSet::new();
    let scheduled = task.schedule(&completed).unwrap();
    let cancelled = scheduled.cancel();

    assert!(matches!(cancelled.state, flow_raft_core::TaskCancelled));
    assert!(cancelled.completed_at.is_some());
}

#[test]
fn test_task_running_to_completed() {
    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );
    let completed = std::collections::HashSet::new();
    let scheduled = task.schedule(&completed).unwrap();
    let running = scheduled.start();
    let outputs = Some(serde_json::json!({"result": "success"}));
    let completed_task = running.complete(outputs.clone());

    assert!(matches!(
        completed_task.state,
        flow_raft_core::TaskCompleted
    ));
    assert!(completed_task.completed_at.is_some());
    assert_eq!(completed_task.outputs_data, outputs);
}

#[test]
fn test_task_running_to_failed_retryable() {
    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );
    let completed = std::collections::HashSet::new();
    let scheduled = task.schedule(&completed).unwrap();
    let running = scheduled.start();
    let failed = running.fail(Some("test error".to_string()), FailureKind::Retryable);

    assert!(matches!(failed.state, flow_raft_core::TaskFailed { .. }));
    assert_eq!(
        failed.retry_config.last_failure_kind,
        Some(FailureKind::Retryable)
    );
}

#[test]
fn test_task_running_to_failed_terminal() {
    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );
    let completed = std::collections::HashSet::new();
    let scheduled = task.schedule(&completed).unwrap();
    let running = scheduled.start();
    let failed = running.fail(Some("terminal error".to_string()), FailureKind::Terminal);

    assert!(matches!(failed.state, flow_raft_core::TaskFailed { .. }));
    assert_eq!(
        failed.retry_config.last_failure_kind,
        Some(FailureKind::Terminal)
    );
}

#[test]
fn test_task_running_to_cancelled() {
    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );
    let completed = std::collections::HashSet::new();
    let scheduled = task.schedule(&completed).unwrap();
    let running = scheduled.start();
    let cancelled = running.cancel();

    assert!(matches!(cancelled.state, flow_raft_core::TaskCancelled));
    assert!(cancelled.completed_at.is_some());
}

#[test]
fn test_task_failed_to_scheduled_retry() {
    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );
    let completed = std::collections::HashSet::new();
    let scheduled = task.schedule(&completed).unwrap();
    let running = scheduled.start();
    let failed = running.fail(Some("error".to_string()), FailureKind::Retryable);
    let retried = failed.retry();

    assert!(retried.is_ok());
    let retried = retried.unwrap();
    assert!(matches!(retried.state, flow_raft_core::TaskScheduled));
    assert_eq!(retried.retry_config.current_attempt, 1);
}

#[test]
fn test_task_failed_to_scheduled_retry_max_exceeded() {
    let mut retry_config = RetryConfig::new(1);
    retry_config.current_attempt = 1; // Already at max

    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        retry_config,
        TaskDependencies::default(),
    );
    let completed = std::collections::HashSet::new();
    let scheduled = task.schedule(&completed).unwrap();
    let running = scheduled.start();
    let failed = running.fail(Some("error".to_string()), FailureKind::Retryable);
    let retried = failed.retry();

    assert!(retried.is_err());
}

#[test]
fn test_task_failed_to_permanent_fail() {
    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );
    let completed = std::collections::HashSet::new();
    let scheduled = task.schedule(&completed).unwrap();
    let running = scheduled.start();
    let failed = running.fail(Some("error".to_string()), FailureKind::Retryable);
    let permanent = failed.permanent_fail();

    assert!(matches!(
        permanent.state,
        flow_raft_core::TaskPermanentlyFailed { .. }
    ));
}

#[test]
fn test_task_failed_to_cancelled() {
    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );
    let completed = std::collections::HashSet::new();
    let scheduled = task.schedule(&completed).unwrap();
    let running = scheduled.start();
    let failed = running.fail(Some("error".to_string()), FailureKind::Retryable);
    let cancelled = failed.cancel();

    assert!(matches!(cancelled.state, flow_raft_core::TaskCancelled));
}
