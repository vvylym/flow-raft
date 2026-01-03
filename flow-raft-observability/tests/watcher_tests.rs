//! Tests for workflow watcher

use flow_raft_core::WorkflowId;
use flow_raft_observability::watcher::*;

#[tokio::test]
async fn test_watcher_new() {
    let watcher = WorkflowWatcher::new();
    // Watcher should be created successfully
    let _receiver = watcher.watch_all_workflows();
}

#[tokio::test]
async fn test_broadcast_and_receive_update() {
    let watcher = WorkflowWatcher::new();
    let workflow_id = WorkflowId::default();

    let mut receiver = watcher.watch_workflow(workflow_id).await;

    let update = WorkflowUpdate {
        workflow_id,
        event_type: "test_event".to_string(),
        data: Some("test_data".to_string()),
        timestamp: chrono::Utc::now(),
    };

    watcher.broadcast_update(update.clone());

    let received = receiver.recv().await.unwrap();
    assert_eq!(received.workflow_id, workflow_id);
    assert_eq!(received.event_type, "test_event");
}

#[tokio::test]
async fn test_watch_all_workflows() {
    let watcher = WorkflowWatcher::new();
    let workflow_id = WorkflowId::default();

    let mut receiver = watcher.watch_all_workflows();

    let update = WorkflowUpdate {
        workflow_id,
        event_type: "test".to_string(),
        data: None,
        timestamp: chrono::Utc::now(),
    };

    watcher.broadcast_update(update);

    let received = receiver.recv().await.unwrap();
    assert_eq!(received.workflow_id, workflow_id);
}
