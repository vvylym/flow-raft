//! Workflow watcher for real-time updates
//!
//! Provides real-time workflow status updates using channels.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{RwLock, broadcast};

use flow_raft_core::WorkflowId;

/// Workflow update event
#[derive(Debug, Clone)]
pub struct WorkflowUpdate {
    /// Workflow ID
    pub workflow_id: WorkflowId,
    /// Event type (e.g., "state_change", "task_completed", "task_failed")
    pub event_type: String,
    /// Event data (JSON string)
    pub data: Option<String>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Workflow watcher that broadcasts updates
pub struct WorkflowWatcher {
    /// Broadcast sender for all workflow updates
    all_updates: broadcast::Sender<WorkflowUpdate>,
    /// Per-workflow broadcast senders
    workflow_senders: Arc<RwLock<HashMap<WorkflowId, broadcast::Sender<WorkflowUpdate>>>>,
}

impl WorkflowWatcher {
    /// Creates a new workflow watcher
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            all_updates: tx,
            workflow_senders: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Broadcasts an update for a workflow
    pub fn broadcast_update(&self, update: WorkflowUpdate) {
        // Broadcast to all subscribers
        let _ = self.all_updates.send(update.clone());

        // Broadcast to workflow-specific subscribers
        // Use try_read to avoid blocking in async context
        if let Ok(workflow_senders) = self.workflow_senders.try_read()
            && let Some(sender) = workflow_senders.get(&update.workflow_id)
        {
            let _ = sender.send(update);
        }
    }

    /// Subscribes to updates for a specific workflow
    pub async fn watch_workflow(
        &self,
        workflow_id: WorkflowId,
    ) -> broadcast::Receiver<WorkflowUpdate> {
        let mut senders = self.workflow_senders.write().await;

        // Get or create sender for this workflow
        let sender = senders
            .entry(workflow_id)
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(1024);
                tx
            })
            .clone();

        sender.subscribe()
    }

    /// Subscribes to updates for all workflows
    pub fn watch_all_workflows(&self) -> broadcast::Receiver<WorkflowUpdate> {
        self.all_updates.subscribe()
    }
}

impl Default for WorkflowWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_watch_workflow() {
        let watcher = WorkflowWatcher::new();
        let workflow_id = WorkflowId::default();

        let mut receiver = watcher.watch_workflow(workflow_id).await;

        let update = WorkflowUpdate {
            workflow_id,
            event_type: "test".to_string(),
            data: Some("test data".to_string()),
            timestamp: chrono::Utc::now(),
        };

        watcher.broadcast_update(update.clone());

        let received = receiver.recv().await.unwrap();
        assert_eq!(received.workflow_id, workflow_id);
        assert_eq!(received.event_type, "test");
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

        watcher.broadcast_update(update.clone());

        let received = receiver.recv().await.unwrap();
        assert_eq!(received.workflow_id, workflow_id);
    }
}
