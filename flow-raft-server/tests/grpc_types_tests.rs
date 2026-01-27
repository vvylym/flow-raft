//! Tests for gRPC types

use chrono::Utc;
use flow_raft_core::{
    TaskExecution, TaskId, TaskState, WorkflowId, WorkflowSnapshot, WorkflowState,
};
use flow_raft_server::grpc::types::{
    parse_inputs, parse_task_id, parse_workflow_id, task_execution_to_status,
    workflow_snapshot_to_status,
};
use indexmap::IndexMap;

#[test]
fn test_parse_workflow_id() {
    let workflow_id = WorkflowId::default();
    // parse_workflow_id expects just the UUID part, not "workflow:uuid"
    let id_str = workflow_id.as_ref().to_string();
    let parsed = parse_workflow_id(&id_str);
    assert!(parsed.is_ok());
    assert_eq!(parsed.unwrap(), workflow_id);
}

#[test]
fn test_parse_workflow_id_invalid() {
    let parsed = parse_workflow_id("invalid-uuid");
    assert!(parsed.is_err());
}

#[test]
fn test_parse_task_id() {
    let task_id = TaskId::default();
    // parse_task_id expects just the UUID part, not "task:uuid"
    let id_str = task_id.as_ref().to_string();
    let parsed = parse_task_id(&id_str);
    assert!(parsed.is_ok());
    assert_eq!(parsed.unwrap(), task_id);
}

#[test]
fn test_parse_task_id_invalid() {
    let parsed = parse_task_id("invalid-uuid");
    assert!(parsed.is_err());
}

#[test]
fn test_parse_inputs() {
    let json = r#"{"key": "value", "number": 42}"#;
    let parsed = parse_inputs(Some(json.to_string()));
    assert!(parsed.is_ok());
    let value = parsed.unwrap();
    assert_eq!(value.get("key").and_then(|v| v.as_str()), Some("value"));
    assert_eq!(value.get("number").and_then(|v| v.as_u64()), Some(42));
}

#[test]
fn test_parse_inputs_invalid_json() {
    let parsed = parse_inputs(Some("invalid json".to_string()));
    assert!(parsed.is_err());
}

#[test]
fn test_parse_inputs_empty() {
    let parsed = parse_inputs(Some("{}".to_string()));
    assert!(parsed.is_ok());
    assert!(parsed.unwrap().as_object().unwrap().is_empty());
}

#[test]
fn test_parse_inputs_none() {
    let parsed = parse_inputs(None);
    assert!(parsed.is_ok());
    assert_eq!(parsed.unwrap(), serde_json::json!({}));
}

#[test]
fn test_parse_workflow_id_with_prefix() {
    let w = WorkflowId::default();
    let s = format!("workflow:{}", w.as_ref());
    let parsed = parse_workflow_id(&s);
    assert!(parsed.is_ok());
    assert_eq!(parsed.unwrap(), w);
}

#[test]
fn test_task_execution_to_status() {
    let task_id = TaskId::default();
    let exec = TaskExecution {
        task_id,
        state: TaskState::Completed,
        attempts: 1,
        last_error: None,
        outputs: Some(serde_json::json!({"x": 1})),
        started_at: Some(Utc::now()),
        completed_at: Some(Utc::now()),
    };
    let status = task_execution_to_status(task_id, &exec);
    assert_eq!(status.task_id, task_id.to_string());
    assert_eq!(status.state, "completed");
    assert_eq!(status.attempts, 1);
}

#[test]
fn test_workflow_snapshot_to_status() {
    let snapshot = WorkflowSnapshot {
        workflow_id: WorkflowId::default(),
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
    let status = workflow_snapshot_to_status(&snapshot);
    assert_eq!(status.state, "running");
}
