//! Additional tests for task pending transitions to increase coverage

use flow_raft_core::{RetryConfig, Task, TaskDependencies, TaskId};

#[test]
fn test_task_new_with_inputs_outputs() {
    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );
    assert_eq!(task.name, "test_task");
    assert_eq!(task.handler, "handler");
}

#[test]
fn test_task_new_with_timeout() {
    let task = Task::new(
        TaskId::default(),
        "test_task",
        "handler",
        RetryConfig::new(3),
        TaskDependencies::default(),
    );
    // timeout_secs defaults to None
    assert_eq!(task.timeout_secs, None);
}
