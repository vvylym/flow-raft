//! Additional tests for metrics to increase coverage

use flow_raft_core::{TaskId, WorkflowId};
use flow_raft_observability::metrics::MetricsCollector;

#[tokio::test]
async fn test_metrics_collector_record_workflow_completion_with_duration() {
    let collector = MetricsCollector::new();
    let workflow_id = WorkflowId::default();

    collector.record_workflow_start(workflow_id).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    collector.record_workflow_completion(workflow_id).await;

    let metrics = collector.get_workflow_metrics(&workflow_id).await;
    assert!(metrics.is_some());
    let metrics = metrics.unwrap();
    assert!(metrics.total_time_ms > 0);
}

#[tokio::test]
async fn test_metrics_collector_record_task_execution_failed() {
    let collector = MetricsCollector::new();
    let workflow_id = WorkflowId::default();
    let task_id = TaskId::default();

    collector
        .record_task_execution(workflow_id, task_id, 100, 1, false)
        .await;

    let metrics = collector.get_task_metrics(&workflow_id, &task_id).await;
    assert!(metrics.is_some());
    let metrics = metrics.unwrap();
    assert!(!metrics.succeeded);
}

#[test]
fn test_metrics_collector_update_cluster_nodes() {
    let collector = MetricsCollector::new();
    collector.update_cluster_nodes(5);
    // Verify no panic
}

#[test]
fn test_metrics_collector_update_cluster_leader() {
    let collector = MetricsCollector::new();
    let leader_id: u64 = 1;
    collector.update_cluster_leader(leader_id);
    // Verify no panic
}

#[test]
fn test_metrics_collector_record_raft_operation() {
    let collector = MetricsCollector::new();
    collector.record_raft_operation("append_entries", 0.1);
    // Verify no panic
}
