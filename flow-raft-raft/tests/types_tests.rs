//! Comprehensive tests for Raft types

use chrono::Utc;
use flow_raft_core::{
    TaskExecution, TaskId, TaskState, WorkflowId, WorkflowSnapshot, WorkflowState,
};
use flow_raft_raft::types::{Request, Response};
use indexmap::IndexMap;

#[test]
fn test_request_create_workflow() {
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
    let request = Request::CreateWorkflow {
        workflow: snapshot.clone(),
    };
    match request {
        Request::CreateWorkflow { workflow } => {
            assert_eq!(workflow.workflow_id, snapshot.workflow_id);
        }
        _ => panic!("Expected CreateWorkflow"),
    }
}

#[test]
fn test_request_transition_workflow() {
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
    let request = Request::TransitionWorkflow {
        workflow_id,
        workflow: snapshot,
    };
    match request {
        Request::TransitionWorkflow {
            workflow_id: id,
            workflow: _,
        } => {
            assert_eq!(id, workflow_id);
        }
        _ => panic!("Expected TransitionWorkflow"),
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
        _ => panic!("Expected UpdateTaskExecution"),
    }
}

#[test]
fn test_request_cancel_workflow() {
    let workflow_id = WorkflowId::default();
    let snapshot = WorkflowSnapshot {
        workflow_id,
        state: WorkflowState::Cancelled,
        task_definitions: IndexMap::new(),
        executions: IndexMap::new(),
        dependencies: IndexMap::new(),
        retry_configs: IndexMap::new(),
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: Some(Utc::now()),
        inputs: serde_json::json!({}),
        outputs: None,
        error_message: Some("cancelled".to_string()),
    };
    let request = Request::CancelWorkflow {
        workflow_id,
        workflow: snapshot,
    };
    match request {
        Request::CancelWorkflow {
            workflow_id: id,
            workflow: _,
        } => {
            assert_eq!(id, workflow_id);
        }
        _ => panic!("Expected CancelWorkflow"),
    }
}

#[test]
fn test_response_none() {
    let response = Response::none();
    match response {
        Response::None => {}
        _ => panic!("Expected None response"),
    }
}

#[test]
fn test_response_serialization() {
    let response = Response::WorkflowCreated {
        workflow_id: WorkflowId::default(),
    };
    let json = serde_json::to_string(&response).unwrap();
    let deserialized: Response = serde_json::from_str(&json).unwrap();
    match deserialized {
        Response::WorkflowCreated { workflow_id: _ } => {}
        _ => panic!("Expected WorkflowCreated"),
    }
}

#[test]
fn test_request_serialization() {
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
    let request = Request::CreateWorkflow { workflow: snapshot };
    let json = serde_json::to_string(&request).unwrap();
    let deserialized: Request = serde_json::from_str(&json).unwrap();
    match deserialized {
        Request::CreateWorkflow { workflow: _ } => {}
        _ => panic!("Expected CreateWorkflow"),
    }
}

#[test]
fn test_request_display() {
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
    let request = Request::CreateWorkflow { workflow: snapshot };
    let display = format!("{}", request);
    assert!(display.contains("CreateWorkflow"));
}

#[test]
fn test_response_display() {
    let response = Response::WorkflowCreated {
        workflow_id: WorkflowId::default(),
    };
    let display = format!("{:?}", response);
    assert!(display.contains("WorkflowCreated"));
}
