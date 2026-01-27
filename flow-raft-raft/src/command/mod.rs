//! Command builders for workflow operations
//!
//! Converts workflow transitions to Raft commands.

use crate::types::Request;
use flow_raft_core::{TaskExecution, TaskId, WorkflowId, WorkflowSnapshot};

/// Command builder for workflow operations
pub struct WorkflowCommandBuilder;

impl WorkflowCommandBuilder {
    /// Create a command to create a new workflow
    pub fn create_workflow(workflow: WorkflowSnapshot) -> Request {
        Request::CreateWorkflow { workflow }
    }

    /// Create a command to transition a workflow
    pub fn transition_workflow(workflow_id: WorkflowId, workflow: WorkflowSnapshot) -> Request {
        Request::TransitionWorkflow {
            workflow_id,
            workflow,
        }
    }

    /// Create a command to update task execution
    pub fn update_task_execution(
        workflow_id: WorkflowId,
        task_id: TaskId,
        execution: TaskExecution,
    ) -> Request {
        Request::UpdateTaskExecution {
            workflow_id,
            task_id,
            execution,
        }
    }

    /// Create a command to cancel a workflow
    pub fn cancel_workflow(workflow_id: WorkflowId, workflow: WorkflowSnapshot) -> Request {
        Request::CancelWorkflow {
            workflow_id,
            workflow,
        }
    }
}

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
    fn test_create_workflow_command() {
        let snapshot = test_workflow_snapshot();
        let command = WorkflowCommandBuilder::create_workflow(snapshot.clone());
        match command {
            Request::CreateWorkflow { workflow } => {
                assert_eq!(workflow.workflow_id, snapshot.workflow_id);
            }
            _ => panic!("Expected CreateWorkflow command"),
        }
    }

    #[test]
    fn test_transition_workflow_command() {
        let workflow_id = WorkflowId::default();
        let snapshot = test_workflow_snapshot();
        let command = WorkflowCommandBuilder::transition_workflow(workflow_id, snapshot.clone());
        match command {
            Request::TransitionWorkflow {
                workflow_id: id,
                workflow: _,
            } => {
                assert_eq!(id, workflow_id);
            }
            _ => panic!("Expected TransitionWorkflow command"),
        }
    }

    #[test]
    fn test_update_task_execution_command() {
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

        let command =
            WorkflowCommandBuilder::update_task_execution(workflow_id, task_id, execution.clone());
        match command {
            Request::UpdateTaskExecution {
                workflow_id: wf_id,
                task_id: t_id,
                execution: exec,
            } => {
                assert_eq!(wf_id, workflow_id);
                assert_eq!(t_id, task_id);
                assert_eq!(exec.state, execution.state);
            }
            _ => panic!("Expected UpdateTaskExecution command"),
        }
    }
}
