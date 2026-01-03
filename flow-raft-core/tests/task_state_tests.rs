//! Comprehensive tests for task state

use flow_raft_core::{FailureKind, TaskState};

#[test]
fn test_task_state_is_terminal() {
    assert!(!TaskState::Pending.is_terminal());
    assert!(!TaskState::Scheduled.is_terminal());
    assert!(!TaskState::Running.is_terminal());
    assert!(TaskState::Completed.is_terminal());
    // Failed with Retryable is NOT terminal (can be retried)
    assert!(
        !TaskState::Failed {
            error_message: None,
            failure_kind: FailureKind::Retryable,
        }
        .is_terminal()
    );
    assert!(
        TaskState::PermanentlyFailed {
            error_message: None
        }
        .is_terminal()
    );
    assert!(TaskState::Cancelled.is_terminal());
}
