//! Comprehensive tests for workflow snapshot

use chrono::Utc;
use flow_raft_core::{
    TaskExecution, TaskId, TaskState, Workflow, WorkflowDraft, WorkflowId, WorkflowSnapshot,
};
use indexmap::IndexMap;

#[test]
fn test_workflow_snapshot_from_workflow() {
    let workflow_id = WorkflowId::default();
    let workflow: Workflow<WorkflowDraft> = Workflow::new(workflow_id, serde_json::json!({}));

    let snapshot = WorkflowSnapshot::from_workflow(&workflow);
    assert_eq!(snapshot.workflow_id, workflow_id);
    assert!(matches!(
        snapshot.state,
        flow_raft_core::WorkflowState::Draft
    ));
    assert_eq!(snapshot.task_definitions.len(), 0);
    assert_eq!(snapshot.executions.len(), 0);
}

#[test]
fn test_workflow_snapshot_status_empty() {
    let snapshot = WorkflowSnapshot {
        workflow_id: WorkflowId::default(),
        state: flow_raft_core::WorkflowState::Draft,
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

    let status = snapshot.status();
    assert_eq!(status.total_tasks, 0);
    assert_eq!(status.completed, 0);
    assert_eq!(status.failed, 0);
    assert_eq!(status.running, 0);
    assert_eq!(status.pending, 0);
}

#[test]
fn test_workflow_snapshot_status_with_tasks() {
    let task1 = TaskId::default();
    let task2 = TaskId::default();

    let mut executions = IndexMap::new();
    executions.insert(
        task1,
        TaskExecution {
            task_id: task1,
            state: TaskState::Completed,
            attempts: 1,
            started_at: None,
            completed_at: None,
            last_error: None,
            outputs: None,
        },
    );
    executions.insert(
        task2,
        TaskExecution {
            task_id: task2,
            state: TaskState::Running,
            attempts: 1,
            started_at: None,
            completed_at: None,
            last_error: None,
            outputs: None,
        },
    );

    let snapshot = WorkflowSnapshot {
        workflow_id: WorkflowId::default(),
        state: flow_raft_core::WorkflowState::Running,
        task_definitions: IndexMap::new(),
        executions,
        dependencies: IndexMap::new(),
        retry_configs: IndexMap::new(),
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        inputs: serde_json::json!({}),
        outputs: None,
        error_message: None,
    };

    let status = snapshot.status();
    assert_eq!(status.total_tasks, 2);
    assert_eq!(status.completed, 1);
    assert_eq!(status.running, 1);
    assert_eq!(status.failed, 0);
}

#[test]
fn test_workflow_snapshot_status_with_failed_tasks() {
    let task1 = TaskId::default();
    let mut executions = IndexMap::new();
    executions.insert(
        task1,
        TaskExecution {
            task_id: task1,
            state: TaskState::Failed {
                error_message: Some("error".to_string()),
                failure_kind: flow_raft_core::FailureKind::Retryable,
            },
            attempts: 2,
            started_at: None,
            completed_at: None,
            last_error: Some("error".to_string()),
            outputs: None,
        },
    );

    let snapshot = WorkflowSnapshot {
        workflow_id: WorkflowId::default(),
        state: flow_raft_core::WorkflowState::Running,
        task_definitions: IndexMap::new(),
        executions,
        dependencies: IndexMap::new(),
        retry_configs: IndexMap::new(),
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        inputs: serde_json::json!({}),
        outputs: None,
        error_message: None,
    };

    let status = snapshot.status();
    assert_eq!(status.total_tasks, 1);
    assert_eq!(status.failed, 1);
    assert_eq!(status.completed, 0);
}

#[test]
fn test_workflow_snapshot_serialization() {
    let snapshot = WorkflowSnapshot {
        workflow_id: WorkflowId::default(),
        state: flow_raft_core::WorkflowState::Completed,
        task_definitions: IndexMap::new(),
        executions: IndexMap::new(),
        dependencies: IndexMap::new(),
        retry_configs: IndexMap::new(),
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: Some(Utc::now()),
        inputs: serde_json::json!({"key": "value"}),
        outputs: Some(serde_json::json!({"result": "success"})),
        error_message: None,
    };

    let json = serde_json::to_string(&snapshot).unwrap();
    let deserialized: WorkflowSnapshot = serde_json::from_str(&json).unwrap();

    assert_eq!(snapshot.workflow_id, deserialized.workflow_id);
    // Check that the state is Completed (may be serialized differently)
    match deserialized.state {
        flow_raft_core::WorkflowState::Completed => {}
        _ => panic!("Expected Completed state, got {:?}", deserialized.state),
    }
    assert!(deserialized.outputs.is_some());
}
