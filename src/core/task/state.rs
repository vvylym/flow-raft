//! Task state types
//!
//! Defines marker types for compile-time state enforcement and enum for serialization.

use crate::core::FailureKind;

/// Task state enum for serialization
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(untagged, rename = "camelCase")]
pub enum TaskState {
    /// Task is pending (waiting for dependencies)
    #[serde(rename = "pending")]
    Pending,
    /// Task is scheduled (ready to execute)
    #[serde(rename = "scheduled")]
    Scheduled,
    /// Task is currently running
    #[serde(rename = "running")]
    Running,
    /// Task completed successfully
    #[serde(rename = "completed")]
    Completed,
    /// Task failed (may be retried)
    #[serde(rename = "failed")]
    Failed {
        /// Error message
        error_message: Option<String>,
        /// Failure kind (retryable or terminal)
        failure_kind: FailureKind,
    },
    /// Task failed permanently (exhausted retries)
    #[serde(rename = "permanently-failed")]
    PermanentlyFailed {
        /// Error message
        error_message: Option<String>,
    },
    /// Task was cancelled
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl TaskState {
    /// Returns true if the state is terminal (cannot transition from this state)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::PermanentlyFailed { .. } | TaskState::Cancelled
        )
    }
}

// Generate marker types and From implementations using the macro
crate::define_state_types! {
    TaskState {
        Pending => TaskPending,
        Scheduled => TaskScheduled,
        Running => TaskRunning,
        Completed => TaskCompleted,
        Failed {
            error_message: Option<String>,
            failure_kind: FailureKind
        } => TaskFailed,
        PermanentlyFailed {
            error_message: Option<String>
        } => TaskPermanentlyFailed,
        Cancelled => TaskCancelled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(TaskState::Pending, false)]
    #[case(TaskState::Scheduled, false)]
    #[case(TaskState::Running, false)]
    #[case(TaskState::Completed, true)]
    #[case(TaskState::Failed { error_message: None, failure_kind: FailureKind::Retryable }, false)]
    #[case(TaskState::Failed { error_message: None, failure_kind: FailureKind::Terminal }, false)]
    #[case(TaskState::PermanentlyFailed { error_message: None }, true)]
    #[case(TaskState::Cancelled, true)]
    fn test_state_is_terminal(#[case] state: TaskState, #[case] expected: bool) {
        assert_eq!(state.is_terminal(), expected);
    }

    #[rstest]
    #[case::pending(|| TaskState::from(&TaskPending), TaskState::Pending)]
    #[case::scheduled(|| TaskState::from(&TaskScheduled), TaskState::Scheduled)]
    #[case::running(|| TaskState::from(&TaskRunning), TaskState::Running)]
    #[case::completed(|| TaskState::from(&TaskCompleted), TaskState::Completed)]
    #[case::cancelled(|| TaskState::from(&TaskCancelled), TaskState::Cancelled)]
    fn test_from_simple_state_marker(
        #[case] marker_fn: impl FnOnce() -> TaskState,
        #[case] expected: TaskState,
    ) {
        let state = marker_fn();
        assert_eq!(state, expected);
    }

    #[rstest]
    #[case::retryable_with_error(Some("test error".to_string()), FailureKind::Retryable, Some("test error".to_string()), FailureKind::Retryable)]
    #[case::retryable_no_error(None, FailureKind::Retryable, None, FailureKind::Retryable)]
    #[case::terminal_with_error(Some("terminal error".to_string()), FailureKind::Terminal, Some("terminal error".to_string()), FailureKind::Terminal)]
    #[case::terminal_no_error(None, FailureKind::Terminal, None, FailureKind::Terminal)]
    fn test_from_task_failed(
        #[case] error_message: Option<String>,
        #[case] failure_kind: FailureKind,
        #[case] expected_error: Option<String>,
        #[case] expected_kind: FailureKind,
    ) {
        let marker = TaskFailed {
            error_message: error_message.clone(),
            failure_kind,
        };
        let state: TaskState = (&marker).into();
        match state {
            TaskState::Failed {
                error_message: actual_error,
                failure_kind: actual_kind,
            } => {
                assert_eq!(actual_error, expected_error);
                assert_eq!(actual_kind, expected_kind);
            }
            _ => panic!("Expected Failed state"),
        }
    }

    #[rstest]
    #[case::with_error(Some("permanent error".to_string()), Some("permanent error".to_string()))]
    #[case::no_error(None, None)]
    fn test_from_task_permanently_failed(
        #[case] error_message: Option<String>,
        #[case] expected_error: Option<String>,
    ) {
        let marker = TaskPermanentlyFailed {
            error_message: error_message.clone(),
        };
        let state: TaskState = (&marker).into();
        match state {
            TaskState::PermanentlyFailed {
                error_message: actual,
            } => {
                assert_eq!(actual, expected_error);
            }
            _ => panic!("Expected PermanentlyFailed state"),
        }
    }
}
