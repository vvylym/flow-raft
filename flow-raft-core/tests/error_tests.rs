//! Tests for error types

use flow_raft_core::{MaxRetriesExceededError, TaskError, WorkflowError};

#[test]
fn test_max_retries_exceeded_error() {
    let error = MaxRetriesExceededError::new(3, 4);
    let message = format!("{}", error);
    assert!(message.contains("max retries exceeded"));
    assert!(message.contains("4/3"));
}

#[test]
fn test_task_error_dependency_not_satisfied() {
    let error = TaskError::DependencyNotSatisfied {
        task_id: "task1".to_string(),
        dependency_id: "task2".to_string(),
    };
    let message = format!("{}", error);
    assert!(message.contains("task1"));
    assert!(message.contains("task2"));
    assert!(message.contains("dependency not satisfied"));
}

#[test]
fn test_task_error_max_retries_exceeded() {
    let error = TaskError::MaxRetriesExceeded {
        task_id: "task1".to_string(),
        max_attempts: 3,
        current_attempts: 4,
    };
    let message = format!("{}", error);
    assert!(message.contains("task1"));
    assert!(message.contains("4/3"));
    assert!(message.contains("max retries exceeded"));
}

#[test]
fn test_workflow_error_cycle_detected() {
    let error = WorkflowError::CycleDetected;
    let message = format!("{}", error);
    assert!(message.contains("cycle detected"));
}

#[test]
fn test_workflow_error_dependency_not_found() {
    let error = WorkflowError::DependencyNotFound("task2".to_string());
    let message = format!("{}", error);
    assert!(message.contains("task2"));
    assert!(message.contains("dependency"));
}

#[test]
fn test_workflow_error_no_tasks_found() {
    let error = WorkflowError::NoTasksFound;
    let message = format!("{}", error);
    assert!(message.contains("no tasks found"));
}
