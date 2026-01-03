//! Tests for command builder

use chrono::Utc;
use flow_raft_core::{
    TaskExecution, TaskId, TaskState, WorkflowId, WorkflowSnapshot, WorkflowState,
};
use flow_raft_raft::command::WorkflowCommandBuilder;
use indexmap::IndexMap;

#[test]
fn test_create_workflow_command() {
    let snapshot = WorkflowSnapshot {
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
    };

    let command = WorkflowCommandBuilder::create_workflow(snapshot.clone());
    match command {
        flow_raft_raft::types::Request::CreateWorkflow { workflow } => {
            assert_eq!(workflow.workflow_id, snapshot.workflow_id);
        }
        _ => panic!("Expected CreateWorkflow command"),
    }
}

#[test]
fn test_transition_workflow_command() {
    let workflow_id = WorkflowId::default();
    let snapshot = WorkflowSnapshot {
        workflow_id,
        state: WorkflowState::Running,
        task_definitions: IndexMap::new(),
        executions: IndexMap::new(),
        dependencies: IndexMap::new(),
        retry_configs: IndexMap::new(),
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        inputs: serde_json::json!({}),
        outputs: None,
        error_message: None,
    };

    let command = WorkflowCommandBuilder::transition_workflow(workflow_id, snapshot);
    match command {
        flow_raft_raft::types::Request::TransitionWorkflow {
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
        state: TaskState::Completed,
        attempts: 1,
        started_at: Some(Utc::now()),
        completed_at: Some(Utc::now()),
        last_error: None,
        outputs: Some(serde_json::json!({"result": "success"})),
    };

    let command =
        WorkflowCommandBuilder::update_task_execution(workflow_id, task_id, execution.clone());
    match command {
        flow_raft_raft::types::Request::UpdateTaskExecution {
            workflow_id: wf_id,
            task_id: t_id,
            execution: exec,
        } => {
            assert_eq!(wf_id, workflow_id);
            assert_eq!(t_id, task_id);
            assert!(matches!(exec.state, TaskState::Completed));
        }
        _ => panic!("Expected UpdateTaskExecution command"),
    }
}

#[test]
fn test_cancel_workflow_command() {
    let workflow_id = WorkflowId::default();
    let snapshot = WorkflowSnapshot {
        workflow_id,
        state: WorkflowState::Cancelled,
        task_definitions: IndexMap::new(),
        executions: IndexMap::new(),
        dependencies: IndexMap::new(),
        retry_configs: IndexMap::new(),
        created_at: Utc::now(),
        started_at: None,
        completed_at: Some(Utc::now()),
        inputs: serde_json::json!({}),
        outputs: None,
        error_message: Some("Cancelled".to_string()),
    };

    let command = WorkflowCommandBuilder::cancel_workflow(workflow_id, snapshot);
    match command {
        flow_raft_raft::types::Request::CancelWorkflow {
            workflow_id: id,
            workflow: _,
        } => {
            assert_eq!(id, workflow_id);
        }
        _ => panic!("Expected CancelWorkflow command"),
    }
}
