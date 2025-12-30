//! Metrics collection for FlowRaft
//!
//! Collects execution metrics for workflows and tasks.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::core::{TaskId, WorkflowId};

/// Execution metrics for a workflow
#[derive(Debug, Clone)]
pub struct WorkflowMetrics {
    /// Workflow ID
    pub workflow_id: WorkflowId,
    /// Total execution time in milliseconds
    pub total_time_ms: u64,
    /// Number of tasks executed
    pub tasks_executed: u64,
    /// Number of tasks completed
    pub tasks_completed: u64,
    /// Number of tasks failed
    pub tasks_failed: u64,
    /// Timestamp when workflow started
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Timestamp when workflow completed
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Execution metrics for a task
#[derive(Debug, Clone)]
pub struct TaskMetrics {
    /// Task ID
    pub task_id: TaskId,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Number of attempts
    pub attempts: u32,
    /// Whether the task succeeded
    pub succeeded: bool,
}

/// Metrics collector
pub struct MetricsCollector {
    /// Workflow metrics
    workflow_metrics: Arc<RwLock<HashMap<WorkflowId, WorkflowMetrics>>>,
    /// Task metrics
    task_metrics: Arc<RwLock<HashMap<(WorkflowId, TaskId), TaskMetrics>>>,
    /// Total workflows executed
    total_workflows: AtomicU64,
    /// Total tasks executed
    total_tasks: AtomicU64,
}

impl MetricsCollector {
    /// Creates a new metrics collector
    pub fn new() -> Self {
        Self {
            workflow_metrics: Arc::new(RwLock::new(HashMap::new())),
            task_metrics: Arc::new(RwLock::new(HashMap::new())),
            total_workflows: AtomicU64::new(0),
            total_tasks: AtomicU64::new(0),
        }
    }

    /// Records workflow start
    pub async fn record_workflow_start(&self, workflow_id: WorkflowId) {
        let mut metrics = self.workflow_metrics.write().await;
        metrics.insert(
            workflow_id,
            WorkflowMetrics {
                workflow_id,
                total_time_ms: 0,
                tasks_executed: 0,
                tasks_completed: 0,
                tasks_failed: 0,
                started_at: Some(chrono::Utc::now()),
                completed_at: None,
            },
        );
        self.total_workflows.fetch_add(1, Ordering::Relaxed);
    }

    /// Records workflow completion
    pub async fn record_workflow_completion(&self, workflow_id: WorkflowId) {
        let mut metrics = self.workflow_metrics.write().await;
        if let Some(metric) = metrics.get_mut(&workflow_id) {
            metric.completed_at = Some(chrono::Utc::now());
            if let (Some(started), Some(completed)) = (metric.started_at, metric.completed_at) {
                let duration = completed.signed_duration_since(started);
                metric.total_time_ms = duration.num_milliseconds() as u64;
            }
        }
    }

    /// Records task execution
    pub async fn record_task_execution(
        &self,
        workflow_id: WorkflowId,
        task_id: TaskId,
        execution_time_ms: u64,
        attempts: u32,
        succeeded: bool,
    ) {
        let mut task_metrics = self.task_metrics.write().await;
        task_metrics.insert(
            (workflow_id, task_id),
            TaskMetrics {
                task_id,
                execution_time_ms,
                attempts,
                succeeded,
            },
        );

        let mut workflow_metrics = self.workflow_metrics.write().await;
        if let Some(metric) = workflow_metrics.get_mut(&workflow_id) {
            metric.tasks_executed += 1;
            if succeeded {
                metric.tasks_completed += 1;
            } else {
                metric.tasks_failed += 1;
            }
        }

        self.total_tasks.fetch_add(1, Ordering::Relaxed);
    }

    /// Gets workflow metrics
    pub async fn get_workflow_metrics(&self, workflow_id: &WorkflowId) -> Option<WorkflowMetrics> {
        let metrics = self.workflow_metrics.read().await;
        metrics.get(workflow_id).cloned()
    }

    /// Gets task metrics
    pub async fn get_task_metrics(
        &self,
        workflow_id: &WorkflowId,
        task_id: &TaskId,
    ) -> Option<TaskMetrics> {
        let metrics = self.task_metrics.read().await;
        metrics.get(&(*workflow_id, *task_id)).cloned()
    }

    /// Gets total workflows executed
    pub fn total_workflows(&self) -> u64 {
        self.total_workflows.load(Ordering::Relaxed)
    }

    /// Gets total tasks executed
    pub fn total_tasks(&self) -> u64 {
        self.total_tasks.load(Ordering::Relaxed)
    }

    /// Gets all workflow metrics
    pub async fn get_all_workflow_metrics(&self) -> HashMap<WorkflowId, WorkflowMetrics> {
        let metrics = self.workflow_metrics.read().await;
        metrics.clone()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_workflow_start() {
        let collector = MetricsCollector::new();
        let workflow_id = WorkflowId::default();

        collector.record_workflow_start(workflow_id).await;

        let metrics = collector.get_workflow_metrics(&workflow_id).await;
        assert!(metrics.is_some());
        assert!(metrics.unwrap().started_at.is_some());
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
        assert_eq!(task_metrics.unwrap().execution_time_ms, 100);
    }
}
