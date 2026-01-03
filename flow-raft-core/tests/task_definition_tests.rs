//! Tests for task definition

use flow_raft_core::{TaskDefinition, TaskId};
use std::collections::HashSet;

#[test]
fn test_task_definition_creation() {
    let task_id = TaskId::default();
    let definition = TaskDefinition {
        id: task_id,
        name: "test_task".to_string(),
        handler: "test_handler".to_string(),
        inputs: HashSet::from(["input1".to_string(), "input2".to_string()]),
        outputs: HashSet::from(["output1".to_string()]),
        timeout_secs: Some(60),
    };

    assert_eq!(definition.id, task_id);
    assert_eq!(definition.name, "test_task");
    assert_eq!(definition.handler, "test_handler");
    assert_eq!(definition.inputs.len(), 2);
    assert_eq!(definition.outputs.len(), 1);
    assert_eq!(definition.timeout_secs, Some(60));
}

#[test]
fn test_task_definition_serialization() {
    let task_id = TaskId::default();
    let definition = TaskDefinition {
        id: task_id,
        name: "test_task".to_string(),
        handler: "test_handler".to_string(),
        inputs: HashSet::from(["input1".to_string()]),
        outputs: HashSet::from(["output1".to_string()]),
        timeout_secs: None,
    };

    let json = serde_json::to_string(&definition).unwrap();
    let deserialized: TaskDefinition = serde_json::from_str(&json).unwrap();

    assert_eq!(definition.id, deserialized.id);
    assert_eq!(definition.name, deserialized.name);
    assert_eq!(definition.handler, deserialized.handler);
}
