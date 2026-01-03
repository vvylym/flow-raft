//! Tests for task execution

use flow_raft_core::{FailureKind, TaskExecution, TaskId, TaskState};

#[test]
fn test_task_execution_creation() {
    let task_id = TaskId::default();
    let execution = TaskExecution {
        task_id,
        state: TaskState::Pending,
        attempts: 0,
        started_at: None,
        completed_at: None,
        last_error: None,
        outputs: None,
    };

    assert_eq!(execution.task_id, task_id);
    assert!(matches!(execution.state, TaskState::Pending));
    assert_eq!(execution.attempts, 0);
}

#[test]
fn test_task_execution_with_state() {
    let task_id = TaskId::default();
    let execution = TaskExecution {
        task_id,
        state: TaskState::Failed {
            error_message: Some("test error".to_string()),
            failure_kind: FailureKind::Retryable,
        },
        attempts: 2,
        started_at: Some(chrono::Utc::now()),
        completed_at: None,
        last_error: Some("test error".to_string()),
        outputs: None,
    };

    assert_eq!(execution.attempts, 2);
    assert!(execution.started_at.is_some());
    assert!(execution.last_error.is_some());
}

#[test]
fn test_task_execution_serialization() {
    let task_id = TaskId::default();
    let execution = TaskExecution {
        task_id,
        state: TaskState::Completed,
        attempts: 1,
        started_at: Some(chrono::Utc::now()),
        completed_at: Some(chrono::Utc::now()),
        last_error: None,
        outputs: Some(serde_json::json!({"result": "success"})),
    };

    let json = serde_json::to_string(&execution).unwrap();
    let deserialized: TaskExecution = serde_json::from_str(&json).unwrap();

    assert_eq!(execution.task_id, deserialized.task_id);
    // Check that the state is Completed (may be serialized differently)
    match deserialized.state {
        TaskState::Completed => {}
        _ => panic!("Expected Completed state, got {:?}", deserialized.state),
    }
    assert_eq!(execution.attempts, deserialized.attempts);
}
