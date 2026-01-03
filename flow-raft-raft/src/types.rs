//! Raft type configuration for FlowRaft
//!
//! Defines the OpenRaft type configuration including Request/Response types
//! for workflow operations.

use serde::{Deserialize, Serialize};
use std::fmt;

use flow_raft_core::{TaskExecution, TaskId, WorkflowId, WorkflowSnapshot};

/// Node identifier type
pub type NodeId = u64;

/// Request types for workflow operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    /// Create a new workflow
    CreateWorkflow {
        /// Workflow snapshot containing initial state
        workflow: WorkflowSnapshot,
    },
    /// Transition workflow to a new state
    TransitionWorkflow {
        /// Workflow ID
        workflow_id: WorkflowId,
        /// New workflow snapshot after transition
        workflow: WorkflowSnapshot,
    },
    /// Update task execution state
    UpdateTaskExecution {
        /// Workflow ID
        workflow_id: WorkflowId,
        /// Task ID
        task_id: TaskId,
        /// Updated task execution state
        execution: TaskExecution,
    },
    /// Cancel a workflow
    CancelWorkflow {
        /// Workflow ID
        workflow_id: WorkflowId,
        /// Final workflow snapshot after cancellation
        workflow: WorkflowSnapshot,
    },
}

/// Response types for workflow operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    /// Workflow created successfully
    WorkflowCreated {
        /// Workflow ID
        workflow_id: WorkflowId,
    },
    /// Workflow transitioned successfully
    WorkflowTransitioned {
        /// Workflow ID
        workflow_id: WorkflowId,
    },
    /// Task execution updated successfully
    TaskExecutionUpdated {
        /// Workflow ID
        workflow_id: WorkflowId,
        /// Task ID
        task_id: TaskId,
    },
    /// Workflow cancelled successfully
    WorkflowCancelled {
        /// Workflow ID
        workflow_id: WorkflowId,
    },
    /// Empty response (for operations that don't need a response)
    None,
}

impl Response {
    /// Creates a None response
    pub fn none() -> Self {
        Response::None
    }
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Request::CreateWorkflow { workflow } => {
                write!(
                    f,
                    "CreateWorkflow {{ workflow_id: {} }}",
                    workflow.workflow_id
                )
            }
            Request::TransitionWorkflow { workflow_id, .. } => {
                write!(f, "TransitionWorkflow {{ workflow_id: {} }}", workflow_id)
            }
            Request::UpdateTaskExecution {
                workflow_id,
                task_id,
                ..
            } => {
                write!(
                    f,
                    "UpdateTaskExecution {{ workflow_id: {}, task_id: {} }}",
                    workflow_id, task_id
                )
            }
            Request::CancelWorkflow { workflow_id, .. } => {
                write!(f, "CancelWorkflow {{ workflow_id: {} }}", workflow_id)
            }
        }
    }
}

// Declare OpenRaft type configuration
openraft::declare_raft_types!(
    /// Type configuration for FlowRaft
    pub TypeConfig:
        D = Request,
        R = Response,
        NodeId = NodeId,
        Node = openraft::BasicNode,
        SnapshotData = std::io::Cursor<Vec<u8>>,
        AsyncRuntime = openraft::TokioRuntime,
);

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use flow_raft_core::{TaskState, WorkflowState};
    use indexmap::IndexMap;
    use rstest::*;

    #[fixture]
    fn test_workflow_snapshot() -> WorkflowSnapshot {
        WorkflowSnapshot {
            workflow_id: WorkflowId::default(),
            state: WorkflowState::Draft,
            task_definitions: IndexMap::new(),
            executions: IndexMap::new(),
            dependencies: IndexMap::new(),
            retry_configs: IndexMap::new(),
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            inputs: serde_json::json!({}),
            outputs: None,
            error_message: None,
        }
    }

    #[test]
    fn test_request_create_workflow() {
        let snapshot = test_workflow_snapshot();
        let request = Request::CreateWorkflow {
            workflow: snapshot.clone(),
        };

        match request {
            Request::CreateWorkflow { workflow } => {
                assert_eq!(workflow.workflow_id, snapshot.workflow_id);
            }
            _ => panic!("Expected CreateWorkflow request"),
        }
    }

    #[test]
    fn test_request_transition_workflow() {
        let workflow_id = WorkflowId::default();
        let mut snapshot = test_workflow_snapshot();
        snapshot.state = WorkflowState::Running;

        let request = Request::TransitionWorkflow {
            workflow_id,
            workflow: snapshot.clone(),
        };

        match request {
            Request::TransitionWorkflow {
                workflow_id: id,
                workflow,
            } => {
                assert_eq!(id, workflow_id);
                assert!(matches!(workflow.state, WorkflowState::Running));
            }
            _ => panic!("Expected TransitionWorkflow request"),
        }
    }

    #[test]
    fn test_request_update_task_execution() {
        let workflow_id = WorkflowId::default();
        let task_id = TaskId::default();
        let execution = TaskExecution {
            task_id,
            state: TaskState::Running,
            attempts: 1,
            started_at: Some(Utc::now()),
            completed_at: None,
            last_error: None,
            outputs: None,
        };

        let request = Request::UpdateTaskExecution {
            workflow_id,
            task_id,
            execution: execution.clone(),
        };

        match request {
            Request::UpdateTaskExecution {
                workflow_id: wf_id,
                task_id: t_id,
                execution: exec,
            } => {
                assert_eq!(wf_id, workflow_id);
                assert_eq!(t_id, task_id);
                assert_eq!(exec.state, execution.state);
            }
            _ => panic!("Expected UpdateTaskExecution request"),
        }
    }

    #[test]
    fn test_request_cancel_workflow() {
        let workflow_id = WorkflowId::default();
        let mut snapshot = test_workflow_snapshot();
        snapshot.state = WorkflowState::Cancelled;

        let request = Request::CancelWorkflow {
            workflow_id,
            workflow: snapshot.clone(),
        };

        match request {
            Request::CancelWorkflow {
                workflow_id: id,
                workflow,
            } => {
                assert_eq!(id, workflow_id);
                assert!(matches!(workflow.state, WorkflowState::Cancelled));
            }
            _ => panic!("Expected CancelWorkflow request"),
        }
    }

    #[rstest]
    #[case::workflow_created(Response::WorkflowCreated { workflow_id: WorkflowId::default() })]
    #[case::workflow_transitioned(Response::WorkflowTransitioned { workflow_id: WorkflowId::default() })]
    #[case::task_updated(Response::TaskExecutionUpdated { workflow_id: WorkflowId::default(), task_id: TaskId::default() })]
    #[case::workflow_cancelled(Response::WorkflowCancelled { workflow_id: WorkflowId::default() })]
    #[case::none(Response::none())]
    fn test_response_variants(#[case] response: Response) {
        // Test that all response variants can be created and matched
        match response {
            Response::WorkflowCreated { .. } => {}
            Response::WorkflowTransitioned { .. } => {}
            Response::TaskExecutionUpdated { .. } => {}
            Response::WorkflowCancelled { .. } => {}
            Response::None => {}
        }
    }

    #[test]
    fn test_response_none() {
        let response = Response::none();
        assert!(matches!(response, Response::None));
    }

    #[test]
    fn test_request_serialization() {
        let snapshot = test_workflow_snapshot();
        let request = Request::CreateWorkflow {
            workflow: snapshot.clone(),
        };

        let serialized = serde_json::to_string(&request).expect("Failed to serialize");
        let deserialized: Request =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        match (request, deserialized) {
            (
                Request::CreateWorkflow { workflow: w1 },
                Request::CreateWorkflow { workflow: w2 },
            ) => {
                assert_eq!(w1.workflow_id, w2.workflow_id);
            }
            _ => panic!("Serialization roundtrip failed"),
        }
    }

    #[test]
    fn test_response_serialization() {
        let workflow_id = WorkflowId::default();
        let response = Response::WorkflowCreated { workflow_id };

        let serialized = serde_json::to_string(&response).expect("Failed to serialize");
        let deserialized: Response =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        match (response, deserialized) {
            (
                Response::WorkflowCreated { workflow_id: id1 },
                Response::WorkflowCreated { workflow_id: id2 },
            ) => {
                assert_eq!(id1, id2);
            }
            _ => panic!("Serialization roundtrip failed"),
        }
    }
}
