//! Tests for gRPC types

use flow_raft_core::{TaskId, WorkflowId};
use flow_raft_server::grpc::types::{parse_inputs, parse_task_id, parse_workflow_id};

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
