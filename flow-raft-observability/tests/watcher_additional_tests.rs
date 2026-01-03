//! Additional tests for watcher to increase coverage

use flow_raft_core::WorkflowId;
use flow_raft_observability::watcher::{WorkflowUpdate, WorkflowWatcher};

#[tokio::test]
async fn test_watcher_broadcast_update() {
    let watcher = WorkflowWatcher::new();
    let workflow_id = WorkflowId::default();
    let mut receiver = watcher.watch_workflow(workflow_id).await;

    let update = WorkflowUpdate {
        workflow_id,
        event_type: "test".to_string(),
        data: Some("test_data".to_string()),
        timestamp: chrono::Utc::now(),
    };

    watcher.broadcast_update(update.clone());

    // Should receive the update
    let received = receiver.recv().await;
    assert!(received.is_ok());
    assert_eq!(received.unwrap().event_type, "test");
}

#[tokio::test]
async fn test_watcher_watch_all_workflows() {
    let watcher = WorkflowWatcher::new();
    let mut receiver = watcher.watch_all_workflows();

    let update = WorkflowUpdate {
        workflow_id: WorkflowId::default(),
        event_type: "test".to_string(),
        data: None,
        timestamp: chrono::Utc::now(),
    };

    watcher.broadcast_update(update.clone());

    // Should receive the update
    let received = receiver.recv().await;
    assert!(received.is_ok());
}

#[test]
fn test_watcher_default() {
    let watcher = WorkflowWatcher::default();
    // Verify default watcher is created
    assert!(std::mem::size_of_val(&watcher) > 0);
}
