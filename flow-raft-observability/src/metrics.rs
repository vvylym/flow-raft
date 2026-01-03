//! Metrics collection for FlowRaft
//!
//! Collects execution metrics for workflows and tasks.
//! Integrates with the metrics crate for Prometheus export.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use metrics::{counter, gauge, histogram};
use tokio::sync::RwLock;

use flow_raft_core::{TaskId, WorkflowId};

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
        // Metrics will be registered on first use via the macros
        Self {
            workflow_metrics: Arc::new(RwLock::new(HashMap::new())),
            task_metrics: Arc::new(RwLock::new(HashMap::new())),
            total_workflows: AtomicU64::new(0),
            total_tasks: AtomicU64::new(0),
        }
    }

    /// Records workflow start
    pub async fn record_workflow_start(&self, workflow_id: WorkflowId) {
        counter!("flowraft_workflows_created_total").increment(1);

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
        counter!("flowraft_workflows_completed_total").increment(1);

        let mut metrics = self.workflow_metrics.write().await;
        if let Some(metric) = metrics.get_mut(&workflow_id) {
            metric.completed_at = Some(chrono::Utc::now());
            if let (Some(started), Some(completed)) = (metric.started_at, metric.completed_at) {
                let duration = completed.signed_duration_since(started);
                let duration_secs = duration.num_milliseconds() as f64 / 1000.0;
                metric.total_time_ms = duration.num_milliseconds() as u64;

                // Record duration histogram
                histogram!("flowraft_workflow_duration_seconds").record(duration_secs);
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
        let execution_time_secs = execution_time_ms as f64 / 1000.0;

        // Record Prometheus metrics (labels would require custom recorder, using simple metrics for now)
        counter!("flowraft_tasks_executed_total").increment(1);
        histogram!("flowraft_task_duration_seconds").record(execution_time_secs);

        if attempts > 1 {
            let retry_count = (attempts - 1) as u64;
            for _ in 0..retry_count {
                counter!("flowraft_tasks_retries_total").increment(1);
            }
        }

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
                counter!("flowraft_workflows_failed_total").increment(1);
            }
        }

        self.total_tasks.fetch_add(1, Ordering::Relaxed);
    }

    /// Records Raft operation
    pub fn record_raft_operation(&self, _operation_type: &str, duration_secs: f64) {
        counter!("flowraft_raft_operations_total").increment(1);
        histogram!("flowraft_raft_operation_duration_seconds").record(duration_secs);
    }

    /// Records state replication metrics
    pub fn record_state_replication(
        &self,
        bytes_replicated: u64,
        duration_secs: f64,
        success: bool,
    ) {
        counter!("flowraft_raft_replication_total").increment(1);
        if success {
            counter!("flowraft_raft_replication_success_total").increment(1);
            histogram!("flowraft_raft_replication_bytes").record(bytes_replicated as f64);
        } else {
            counter!("flowraft_raft_replication_failure_total").increment(1);
        }
        histogram!("flowraft_raft_replication_duration_seconds").record(duration_secs);
    }

    /// Records leader election metrics
    pub fn record_leader_election(
        &self,
        old_leader: Option<u64>,
        new_leader: u64,
        election_duration_secs: f64,
    ) {
        counter!("flowraft_raft_elections_total").increment(1);
        histogram!("flowraft_raft_election_duration_seconds").record(election_duration_secs);
        gauge!("flowraft_cluster_leader").set(new_leader as f64);

        if let Some(old) = old_leader
            && old != new_leader
        {
            counter!("flowraft_raft_leader_changes_total").increment(1);
        }
    }

    /// Records append entries metrics
    pub fn record_append_entries(&self, entries_count: u64, duration_secs: f64, success: bool) {
        counter!("flowraft_raft_append_entries_total").increment(1);
        if success {
            counter!("flowraft_raft_append_entries_success_total").increment(1);
            histogram!("flowraft_raft_append_entries_count").record(entries_count as f64);
        } else {
            counter!("flowraft_raft_append_entries_failure_total").increment(1);
        }
        histogram!("flowraft_raft_append_entries_duration_seconds").record(duration_secs);
    }

    /// Records vote request metrics
    pub fn record_vote_request(&self, _candidate_id: u64, granted: bool, duration_secs: f64) {
        counter!("flowraft_raft_vote_requests_total").increment(1);
        if granted {
            counter!("flowraft_raft_votes_granted_total").increment(1);
        } else {
            counter!("flowraft_raft_votes_denied_total").increment(1);
        }
        histogram!("flowraft_raft_vote_duration_seconds").record(duration_secs);
    }

    /// Records snapshot metrics
    pub fn record_snapshot(&self, snapshot_size_bytes: u64, duration_secs: f64, success: bool) {
        counter!("flowraft_raft_snapshots_total").increment(1);
        if success {
            counter!("flowraft_raft_snapshots_success_total").increment(1);
            histogram!("flowraft_raft_snapshot_size_bytes").record(snapshot_size_bytes as f64);
        } else {
            counter!("flowraft_raft_snapshots_failure_total").increment(1);
        }
        histogram!("flowraft_raft_snapshot_duration_seconds").record(duration_secs);
    }

    /// Records node state change
    pub fn record_node_state_change(&self, node_id: u64, _old_state: &str, new_state: &str) {
        counter!("flowraft_raft_node_state_changes_total").increment(1);
        // Note: gauge with labels requires custom recorder setup
        // For now, we'll use a simple counter per state
        counter!(
            "flowraft_raft_node_state",
            &[
                ("node_id", node_id.to_string()),
                ("state", new_state.to_string())
            ]
        )
        .increment(1);
    }

    /// Records replication lag
    pub fn record_replication_lag(&self, follower_id: u64, lag_entries: u64) {
        gauge!(
            "flowraft_raft_replication_lag_entries",
            &[("follower_id", follower_id.to_string())]
        )
        .set(lag_entries as f64);
    }

    /// Gets cluster metrics summary
    pub async fn get_cluster_metrics(&self) -> ClusterMetrics {
        ClusterMetrics {
            total_workflows: self.total_workflows.load(Ordering::Relaxed),
            total_tasks: self.total_tasks.load(Ordering::Relaxed),
        }
    }

    /// Gets complete metrics summary
    pub async fn get_metrics_summary(&self) -> MetricsSummary {
        MetricsSummary {
            workflows: self.get_all_workflow_metrics().await,
            cluster: self.get_cluster_metrics().await,
        }
    }

    /// Updates cluster node count
    pub fn update_cluster_nodes(&self, count: u64) {
        gauge!("flowraft_cluster_nodes").set(count as f64);
    }

    /// Updates cluster leader node ID
    pub fn update_cluster_leader(&self, leader_id: u64) {
        gauge!("flowraft_cluster_leader").set(leader_id as f64);
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

/// Cluster metrics summary
#[derive(Debug, Clone)]
pub struct ClusterMetrics {
    /// Total workflows executed
    pub total_workflows: u64,
    /// Total tasks executed
    pub total_tasks: u64,
}

/// Complete metrics summary
#[derive(Debug, Clone)]
pub struct MetricsSummary {
    /// Workflow metrics by workflow ID
    pub workflows: HashMap<WorkflowId, WorkflowMetrics>,
    /// Cluster-level metrics
    pub cluster: ClusterMetrics,
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
