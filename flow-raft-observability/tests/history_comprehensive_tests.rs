//! Comprehensive tests for history store

use flow_raft_core::{TaskId, WorkflowId};
use flow_raft_observability::history::*;

#[tokio::test]
async fn test_history_store_new() {
    let store = HistoryStore::new();
    let histories = store.get_all_histories().await;
    assert!(histories.is_empty());
}

#[tokio::test]
async fn test_history_store_default() {
    let store = HistoryStore::default();
    let histories = store.get_all_histories().await;
    assert!(histories.is_empty());
}

#[tokio::test]
async fn test_record_multiple_events() {
    let store = HistoryStore::new();
    let workflow_id = WorkflowId::default();

    for i in 0..5 {
        let event = ExecutionEvent {
            event_type: ExecutionEventType::TaskStarted,
            task_id: Some(TaskId::default()),
            data: format!("{{\"index\": {}}}", i),
            timestamp: chrono::Utc::now(),
        };
        store.record_event(workflow_id, event).await;
    }

    let history = store.get_history(&workflow_id, None).await;
    assert!(history.is_some());
    assert_eq!(history.unwrap().events.len(), 5);
}

#[tokio::test]
async fn test_get_history_with_limit_exact() {
    let store = HistoryStore::new();
    let workflow_id = WorkflowId::default();

    for _ in 0..10 {
        let event = ExecutionEvent {
            event_type: ExecutionEventType::TaskStarted,
            task_id: None,
            data: "{}".to_string(),
            timestamp: chrono::Utc::now(),
        };
        store.record_event(workflow_id, event).await;
    }

    let history = store.get_history(&workflow_id, Some(5)).await;
    assert!(history.is_some());
    assert_eq!(history.unwrap().events.len(), 5);
}

#[tokio::test]
async fn test_get_history_with_limit_more_than_available() {
    let store = HistoryStore::new();
    let workflow_id = WorkflowId::default();

    for _ in 0..3 {
        let event = ExecutionEvent {
            event_type: ExecutionEventType::TaskStarted,
            task_id: None,
            data: "{}".to_string(),
            timestamp: chrono::Utc::now(),
        };
        store.record_event(workflow_id, event).await;
    }

    let history = store.get_history(&workflow_id, Some(10)).await;
    assert!(history.is_some());
    assert_eq!(history.unwrap().events.len(), 3);
}

#[tokio::test]
async fn test_get_history_nonexistent_workflow() {
    let store = HistoryStore::new();
    let workflow_id = WorkflowId::default();

    let history = store.get_history(&workflow_id, None).await;
    assert!(history.is_none());
}

#[tokio::test]
async fn test_clear_history() {
    let store = HistoryStore::new();
    let workflow_id = WorkflowId::default();

    let event = ExecutionEvent {
        event_type: ExecutionEventType::TaskStarted,
        task_id: None,
        data: "{}".to_string(),
        timestamp: chrono::Utc::now(),
    };
    store.record_event(workflow_id, event).await;

    store.clear_history(&workflow_id).await;

    let history = store.get_history(&workflow_id, None).await;
    assert!(history.is_none());
}

#[tokio::test]
async fn test_get_all_histories_multiple_workflows() {
    let store = HistoryStore::new();
    let workflow_id1 = WorkflowId::default();
    let workflow_id2 = WorkflowId::default();

    store
        .record_event(
            workflow_id1,
            ExecutionEvent {
                event_type: ExecutionEventType::TaskStarted,
                task_id: None,
                data: "{}".to_string(),
                timestamp: chrono::Utc::now(),
            },
        )
        .await;
    store
        .record_event(
            workflow_id2,
            ExecutionEvent {
                event_type: ExecutionEventType::TaskCompleted,
                task_id: None,
                data: "{}".to_string(),
                timestamp: chrono::Utc::now(),
            },
        )
        .await;

    let histories = store.get_all_histories().await;
    assert_eq!(histories.len(), 2);
}

#[test]
fn test_execution_event_type_variants() {
    let types = vec![
        ExecutionEventType::WorkflowStateChange,
        ExecutionEventType::TaskStarted,
        ExecutionEventType::TaskCompleted,
        ExecutionEventType::TaskFailed,
        ExecutionEventType::TaskCancelled,
    ];

    for event_type in types {
        let event = ExecutionEvent {
            event_type,
            task_id: Some(TaskId::default()),
            data: "{}".to_string(),
            timestamp: chrono::Utc::now(),
        };
        // Just verify it can be created and cloned
        let _cloned = event.clone();
    }
}
