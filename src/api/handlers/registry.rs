//! Handler registry for FlowRaft
//!
//! Provides per-workflow handler registration and lookup.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::core::WorkflowId;
use crate::raft::executor::TaskHandler;

/// Per-workflow handler collection
#[derive(Clone, Default)]
struct WorkflowHandlers {
    /// Map from handler name to handler implementation
    handlers: HashMap<String, Arc<dyn TaskHandler>>,
}

/// Handler registry that stores handlers per workflow
#[derive(Clone)]
pub struct HandlerRegistry {
    /// Map from workflow ID to its handlers
    workflows: Arc<RwLock<HashMap<WorkflowId, WorkflowHandlers>>>,
}

impl HandlerRegistry {
    /// Creates a new handler registry
    pub fn new() -> Self {
        Self {
            workflows: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a handler for a specific workflow
    ///
    /// # Arguments
    /// * `workflow_id` - The workflow ID
    /// * `handler_name` - The handler name (must match the handler identifier in task definition)
    /// * `handler` - The handler implementation
    pub async fn register_handler(
        &self,
        workflow_id: WorkflowId,
        handler_name: String,
        handler: Arc<dyn TaskHandler>,
    ) {
        let mut workflows = self.workflows.write().await;
        let workflow_handlers = workflows.entry(workflow_id).or_insert_with(WorkflowHandlers::default);
        workflow_handlers.handlers.insert(handler_name, handler);
    }

    /// Registers multiple handlers for a workflow at once
    ///
    /// # Arguments
    /// * `workflow_id` - The workflow ID
    /// * `handlers` - Map from handler name to handler implementation
    pub async fn register_handlers(
        &self,
        workflow_id: WorkflowId,
        handlers: HashMap<String, Arc<dyn TaskHandler>>,
    ) {
        let mut workflows = self.workflows.write().await;
        let workflow_handlers = workflows.entry(workflow_id).or_insert_with(WorkflowHandlers::default);
        for (name, handler) in handlers {
            workflow_handlers.handlers.insert(name, handler);
        }
    }

    /// Gets a handler for a specific workflow and handler name
    ///
    /// # Arguments
    /// * `workflow_id` - The workflow ID
    /// * `handler_name` - The handler name
    ///
    /// # Returns
    /// The handler if found, None otherwise
    pub async fn get_handler(
        &self,
        workflow_id: &WorkflowId,
        handler_name: &str,
    ) -> Option<Arc<dyn TaskHandler>> {
        let workflows = self.workflows.read().await;
        workflows
            .get(workflow_id)
            .and_then(|workflow_handlers| workflow_handlers.handlers.get(handler_name))
            .cloned()
    }

    /// Gets all handlers for a workflow
    ///
    /// # Arguments
    /// * `workflow_id` - The workflow ID
    ///
    /// # Returns
    /// Map from handler name to handler implementation
    pub async fn get_workflow_handlers(
        &self,
        workflow_id: &WorkflowId,
    ) -> HashMap<String, Arc<dyn TaskHandler>> {
        let workflows = self.workflows.read().await;
        workflows
            .get(workflow_id)
            .map(|workflow_handlers| workflow_handlers.handlers.clone())
            .unwrap_or_default()
    }

    /// Removes all handlers for a workflow
    ///
    /// # Arguments
    /// * `workflow_id` - The workflow ID
    pub async fn remove_workflow(&self, workflow_id: &WorkflowId) {
        let mut workflows = self.workflows.write().await;
        workflows.remove(workflow_id);
    }

    /// Checks if a handler exists for a workflow
    ///
    /// # Arguments
    /// * `workflow_id` - The workflow ID
    /// * `handler_name` - The handler name
    ///
    /// # Returns
    /// True if the handler exists, false otherwise
    pub async fn has_handler(&self, workflow_id: &WorkflowId, handler_name: &str) -> bool {
        let workflows = self.workflows.read().await;
        workflows
            .get(workflow_id)
            .map(|workflow_handlers| workflow_handlers.handlers.contains_key(handler_name))
            .unwrap_or(false)
    }
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for HandlerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandlerRegistry")
            .field("workflows", &"<locked>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::TaskId;
    use crate::raft::executor::TaskHandler;

    struct MockHandler {
        result: serde_json::Value,
    }

    impl TaskHandler for MockHandler {
        fn execute(
            &self,
            _task_id: TaskId,
            _inputs: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            Ok(self.result.clone())
        }
    }

    #[tokio::test]
    async fn test_register_and_get_handler() {
        let registry = HandlerRegistry::new();
        let workflow_id = WorkflowId::default();
        let handler_name = "test_handler".to_string();
        let handler = Arc::new(MockHandler {
            result: serde_json::json!({"result": "success"}),
        });

        registry
            .register_handler(workflow_id, handler_name.clone(), handler.clone())
            .await;

        let retrieved = registry.get_handler(&workflow_id, &handler_name).await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_get_nonexistent_handler() {
        let registry = HandlerRegistry::new();
        let workflow_id = WorkflowId::default();

        let retrieved = registry.get_handler(&workflow_id, "nonexistent").await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_register_multiple_handlers() {
        let registry = HandlerRegistry::new();
        let workflow_id = WorkflowId::default();

        let mut handlers = HashMap::new();
        handlers.insert(
            "handler1".to_string(),
            Arc::new(MockHandler {
                result: serde_json::json!({"result": "handler1"}),
            }) as Arc<dyn TaskHandler>,
        );
        handlers.insert(
            "handler2".to_string(),
            Arc::new(MockHandler {
                result: serde_json::json!({"result": "handler2"}),
            }) as Arc<dyn TaskHandler>,
        );

        registry.register_handlers(workflow_id, handlers).await;

        let all_handlers = registry.get_workflow_handlers(&workflow_id).await;
        assert_eq!(all_handlers.len(), 2);
    }

    #[tokio::test]
    async fn test_remove_workflow() {
        let registry = HandlerRegistry::new();
        let workflow_id = WorkflowId::default();
        let handler_name = "test_handler".to_string();
        let handler = Arc::new(MockHandler {
            result: serde_json::json!({"result": "success"}),
        });

        registry
            .register_handler(workflow_id, handler_name.clone(), handler)
            .await;

        registry.remove_workflow(&workflow_id).await;

        let retrieved = registry.get_handler(&workflow_id, &handler_name).await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_has_handler() {
        let registry = HandlerRegistry::new();
        let workflow_id = WorkflowId::default();
        let handler_name = "test_handler".to_string();
        let handler = Arc::new(MockHandler {
            result: serde_json::json!({"result": "success"}),
        });

        assert!(!registry.has_handler(&workflow_id, &handler_name).await);

        registry
            .register_handler(workflow_id, handler_name.clone(), handler)
            .await;

        assert!(registry.has_handler(&workflow_id, &handler_name).await);
    }
}
