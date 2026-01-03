//! Workflow state types
//!
//! Defines marker types for compile-time state enforcement and enum for serialization.

/// Workflow state enum for serialization
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(untagged, rename = "camelCase")]
pub enum WorkflowState {
    /// Workflow is in draft (being constructed)
    #[serde(rename = "draft")]
    Draft,
    /// Workflow is scheduled (ready to start)
    #[serde(rename = "scheduled")]
    Scheduled,
    /// Workflow is running
    #[serde(rename = "running")]
    Running,
    /// Workflow is paused
    #[serde(rename = "paused")]
    Paused,
    /// Workflow completed successfully
    #[serde(rename = "completed")]
    Completed,
    /// Workflow failed
    #[serde(rename = "failed")]
    Failed {
        /// Error message
        error_message: Option<String>,
    },
    /// Workflow was cancelled
    #[serde(rename = "cancelled")]
    Cancelled,
}

// Generate marker types and From implementations using the macro
crate::define_state_types! {
    WorkflowState {
        Draft => WorkflowDraft,
        Scheduled => WorkflowScheduled,
        Running => WorkflowRunning,
        Paused => WorkflowPaused,
        Completed => WorkflowCompleted,
        Failed {
            error_message: Option<String>
        } => WorkflowFailed,
        Cancelled => WorkflowCancelled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::draft(|| WorkflowState::from(&WorkflowDraft), WorkflowState::Draft)]
    #[case::scheduled(|| WorkflowState::from(&WorkflowScheduled), WorkflowState::Scheduled)]
    #[case::running(|| WorkflowState::from(&WorkflowRunning), WorkflowState::Running)]
    #[case::paused(|| WorkflowState::from(&WorkflowPaused), WorkflowState::Paused)]
    #[case::completed(|| WorkflowState::from(&WorkflowCompleted), WorkflowState::Completed)]
    #[case::cancelled(|| WorkflowState::from(&WorkflowCancelled), WorkflowState::Cancelled)]
    fn test_from_simple_state_marker(
        #[case] marker_fn: impl FnOnce() -> WorkflowState,
        #[case] expected: WorkflowState,
    ) {
        let state = marker_fn();
        assert_eq!(state, expected);
    }

    #[rstest]
    #[case::with_error(Some("test error".to_string()), Some("test error".to_string()))]
    #[case::no_error(None, None)]
    fn test_from_workflow_failed(
        #[case] error_message: Option<String>,
        #[case] expected_error: Option<String>,
    ) {
        let marker = WorkflowFailed {
            error_message: error_message.clone(),
        };
        let state: WorkflowState = (&marker).into();
        match state {
            WorkflowState::Failed {
                error_message: actual,
            } => {
                assert_eq!(actual, expected_error);
            }
            _ => panic!("Expected Failed state"),
        }
    }
}
