//! Execution history for FlowRaft
//!
//! Stores and retrieves execution history for workflows and tasks.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use flow_raft_core::{TaskId, WorkflowId};

/// Execution event type
#[derive(Debug, Clone)]
pub enum ExecutionEventType {
    /// Workflow state changed
    WorkflowStateChange,
    /// Task started
    TaskStarted,
    /// Task completed
    TaskCompleted,
    /// Task failed
    TaskFailed,
    /// Task cancelled
    TaskCancelled,
}

/// Execution event
#[derive(Debug, Clone)]
pub struct ExecutionEvent {
    /// Event type
    pub event_type: ExecutionEventType,
    /// Task ID (if applicable)
    pub task_id: Option<TaskId>,
    /// Event data (JSON string)
    pub data: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Execution history for a workflow
#[derive(Debug, Clone, Default)]
pub struct ExecutionHistory {
    /// Workflow ID
    pub workflow_id: WorkflowId,
    /// Events in chronological order
    pub events: Vec<ExecutionEvent>,
}

/// Execution history store
pub struct HistoryStore {
    /// Workflow execution histories
    histories: Arc<RwLock<HashMap<WorkflowId, ExecutionHistory>>>,
}

impl HistoryStore {
    /// Creates a new history store
    pub fn new() -> Self {
        Self {
            histories: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Records an execution event
    pub async fn record_event(&self, workflow_id: WorkflowId, event: ExecutionEvent) {
        let mut histories = self.histories.write().await;
        let history = histories
            .entry(workflow_id)
            .or_insert_with(|| ExecutionHistory {
                workflow_id,
                events: Vec::new(),
            });
        history.events.push(event);
    }

    /// Gets execution history for a workflow
    pub async fn get_history(
        &self,
        workflow_id: &WorkflowId,
        limit: Option<usize>,
    ) -> Option<ExecutionHistory> {
        let histories = self.histories.read().await;
        histories.get(workflow_id).map(|history| {
            let mut history = history.clone();
            if let Some(limit) = limit {
                // Return most recent events
                let start = history.events.len().saturating_sub(limit);
                history.events = history.events[start..].to_vec();
            }
            history
        })
    }

    /// Gets all execution histories
    pub async fn get_all_histories(&self) -> HashMap<WorkflowId, ExecutionHistory> {
        let histories = self.histories.read().await;
        histories.clone()
    }

    /// Clears history for a workflow
    pub async fn clear_history(&self, workflow_id: &WorkflowId) {
        let mut histories = self.histories.write().await;
        histories.remove(workflow_id);
    }
}

impl Default for HistoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_and_get_event() {
        let store = HistoryStore::new();
        let workflow_id = WorkflowId::default();

        let event = ExecutionEvent {
            event_type: ExecutionEventType::TaskStarted,
            task_id: Some(TaskId::default()),
            data: "{}".to_string(),
            timestamp: chrono::Utc::now(),
        };

        store.record_event(workflow_id, event.clone()).await;

        let history = store.get_history(&workflow_id, None).await;
        assert!(history.is_some());
        assert_eq!(history.unwrap().events.len(), 1);
    }

    #[tokio::test]
    async fn test_get_history_with_limit() {
        let store = HistoryStore::new();
        let workflow_id = WorkflowId::default();

        // Record multiple events
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
}
