//! Comprehensive tests for metrics

use flow_raft_core::{TaskId, WorkflowId};
use flow_raft_observability::metrics::*;

#[tokio::test]
async fn test_metrics_collector_default() {
    let collector = MetricsCollector::default();
    assert_eq!(collector.total_workflows(), 0);
    assert_eq!(collector.total_tasks(), 0);
}

#[tokio::test]
async fn test_get_all_workflow_metrics() {
    let collector = MetricsCollector::new();
    let workflow_id1 = WorkflowId::default();
    let workflow_id2 = WorkflowId::default();

    collector.record_workflow_start(workflow_id1).await;
    collector.record_workflow_start(workflow_id2).await;

    let all_metrics = collector.get_all_workflow_metrics().await;
    assert_eq!(all_metrics.len(), 2);
    assert!(all_metrics.contains_key(&workflow_id1));
    assert!(all_metrics.contains_key(&workflow_id2));
}

#[tokio::test]
async fn test_record_task_execution_failed() {
    let collector = MetricsCollector::new();
    let workflow_id = WorkflowId::default();
    let task_id = TaskId::default();

    collector.record_workflow_start(workflow_id).await;
    collector
        .record_task_execution(workflow_id, task_id, 200, 2, false)
        .await;

    let workflow_metrics = collector.get_workflow_metrics(&workflow_id).await;
    assert!(workflow_metrics.is_some());
    let workflow_metrics = workflow_metrics.unwrap();
    assert_eq!(workflow_metrics.tasks_failed, 1);
    assert_eq!(workflow_metrics.tasks_completed, 0);
}

#[tokio::test]
async fn test_record_task_execution_with_retries() {
    let collector = MetricsCollector::new();
    let workflow_id = WorkflowId::default();
    let task_id = TaskId::default();

    collector.record_workflow_start(workflow_id).await;
    collector
        .record_task_execution(workflow_id, task_id, 300, 3, true)
        .await;

    let task_metrics = collector.get_task_metrics(&workflow_id, &task_id).await;
    assert!(task_metrics.is_some());
    let task_metrics = task_metrics.unwrap();
    assert_eq!(task_metrics.attempts, 3);
    assert!(task_metrics.succeeded);
}

#[tokio::test]
async fn test_record_workflow_completion_with_duration() {
    let collector = MetricsCollector::new();
    let workflow_id = WorkflowId::default();

    collector.record_workflow_start(workflow_id).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    collector.record_workflow_completion(workflow_id).await;

    let metrics = collector.get_workflow_metrics(&workflow_id).await;
    assert!(metrics.is_some());
    let metrics = metrics.unwrap();
    assert!(metrics.completed_at.is_some());
    assert!(metrics.total_time_ms >= 50);
}

#[tokio::test]
async fn test_get_task_metrics_nonexistent() {
    let collector = MetricsCollector::new();
    let workflow_id = WorkflowId::default();
    let task_id = TaskId::default();

    let metrics = collector.get_task_metrics(&workflow_id, &task_id).await;
    assert!(metrics.is_none());
}

#[tokio::test]
async fn test_get_workflow_metrics_nonexistent() {
    let collector = MetricsCollector::new();
    let workflow_id = WorkflowId::default();

    let metrics = collector.get_workflow_metrics(&workflow_id).await;
    assert!(metrics.is_none());
}

#[test]
fn test_record_raft_operation() {
    let collector = MetricsCollector::new();
    collector.record_raft_operation("write", 0.1);
    collector.record_raft_operation("read", 0.05);
    // Operations recorded (metrics are global)
}

#[test]
fn test_update_cluster_nodes() {
    let collector = MetricsCollector::new();
    collector.update_cluster_nodes(3);
    collector.update_cluster_nodes(5);
    // Gauge updated (metrics are global)
}

#[test]
fn test_update_cluster_leader() {
    let collector = MetricsCollector::new();
    collector.update_cluster_leader(1);
    collector.update_cluster_leader(2);
    // Gauge updated (metrics are global)
}
