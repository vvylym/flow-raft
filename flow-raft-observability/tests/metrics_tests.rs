//! Comprehensive tests for metrics

use flow_raft_core::{TaskId, WorkflowId};
use flow_raft_observability::metrics::*;

#[tokio::test]
async fn test_metrics_collector_new() {
    let collector = MetricsCollector::new();
    assert_eq!(collector.total_workflows(), 0);
    assert_eq!(collector.total_tasks(), 0);
}

#[tokio::test]
async fn test_record_workflow_start() {
    let collector = MetricsCollector::new();
    let workflow_id = WorkflowId::default();

    collector.record_workflow_start(workflow_id).await;

    let metrics = collector.get_workflow_metrics(&workflow_id).await;
    assert!(metrics.is_some());
    let metrics = metrics.unwrap();
    assert!(metrics.started_at.is_some());
    assert_eq!(metrics.tasks_executed, 0);
}

#[tokio::test]
async fn test_record_workflow_completion() {
    let collector = MetricsCollector::new();
    let workflow_id = WorkflowId::default();

    collector.record_workflow_start(workflow_id).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    collector.record_workflow_completion(workflow_id).await;

    let metrics = collector.get_workflow_metrics(&workflow_id).await;
    assert!(metrics.is_some());
    let metrics = metrics.unwrap();
    assert!(metrics.completed_at.is_some());
    assert!(metrics.total_time_ms > 0);
}

#[tokio::test]
async fn test_record_task_execution() {
    let collector = MetricsCollector::new();
    let workflow_id = WorkflowId::default();
    let task_id = TaskId::default();

    collector.record_workflow_start(workflow_id).await;
    collector
        .record_task_execution(workflow_id, task_id, 100, 1, true)
        .await;

    let task_metrics = collector.get_task_metrics(&workflow_id, &task_id).await;
    assert!(task_metrics.is_some());
    let task_metrics = task_metrics.unwrap();
    assert_eq!(task_metrics.execution_time_ms, 100);
    assert!(task_metrics.succeeded);
}

#[tokio::test]
async fn test_record_task_with_retries() {
    let collector = MetricsCollector::new();
    let workflow_id = WorkflowId::default();
    let task_id = TaskId::default();

    collector.record_workflow_start(workflow_id).await;
    collector
        .record_task_execution(workflow_id, task_id, 200, 3, true)
        .await;

    let task_metrics = collector.get_task_metrics(&workflow_id, &task_id).await;
    assert!(task_metrics.is_some());
    let task_metrics = task_metrics.unwrap();
    assert_eq!(task_metrics.attempts, 3);
}

#[test]
fn test_record_raft_operation() {
    let collector = MetricsCollector::new();
    collector.record_raft_operation("write", 0.1);
    // Operation recorded (no way to verify without accessing internal state)
}

#[test]
fn test_update_cluster_nodes() {
    let collector = MetricsCollector::new();
    collector.update_cluster_nodes(3);
    // Gauge updated (no way to verify without accessing internal state)
}

#[test]
fn test_update_cluster_leader() {
    let collector = MetricsCollector::new();
    collector.update_cluster_leader(1);
    // Gauge updated (no way to verify without accessing internal state)
}
