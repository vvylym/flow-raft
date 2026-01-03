//! Tests for handler registry

use flow_raft_core::{TaskId, WorkflowId};
use flow_raft_raft::executor::TaskHandler;
use flow_raft_server::handlers::registry::HandlerRegistry;
use std::collections::HashMap;
use std::sync::Arc;

struct TestHandler {
    result: serde_json::Value,
}

impl TaskHandler for TestHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        _inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Ok(self.result.clone())
    }
}

#[tokio::test]
async fn test_handler_registry_new() {
    let registry = HandlerRegistry::new();
    let workflow_id = WorkflowId::default();
    let handlers = registry.get_workflow_handlers(&workflow_id).await;
    assert!(handlers.is_empty());
}

#[tokio::test]
async fn test_handler_registry_default() {
    let registry = HandlerRegistry::default();
    let workflow_id = WorkflowId::default();
    let handlers = registry.get_workflow_handlers(&workflow_id).await;
    assert!(handlers.is_empty());
}

#[tokio::test]
async fn test_register_and_get_handler() {
    let registry = HandlerRegistry::new();
    let workflow_id = WorkflowId::default();
    let handler_name = "test_handler".to_string();
    let handler = Arc::new(TestHandler {
        result: serde_json::json!({"result": "success"}),
    });

    registry
        .register_handler(workflow_id, handler_name.clone(), handler.clone())
        .await;

    let retrieved = registry.get_handler(&workflow_id, &handler_name).await;
    assert!(retrieved.is_some());
}

#[tokio::test]
async fn test_register_multiple_handlers() {
    let registry = HandlerRegistry::new();
    let workflow_id = WorkflowId::default();

    let mut handlers = HashMap::new();
    handlers.insert(
        "handler1".to_string(),
        Arc::new(TestHandler {
            result: serde_json::json!({"result": "handler1"}),
        }) as Arc<dyn TaskHandler>,
    );
    handlers.insert(
        "handler2".to_string(),
        Arc::new(TestHandler {
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
    let handler = Arc::new(TestHandler {
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
    let handler = Arc::new(TestHandler {
        result: serde_json::json!({"result": "success"}),
    });

    assert!(!registry.has_handler(&workflow_id, &handler_name).await);

    registry
        .register_handler(workflow_id, handler_name.clone(), handler)
        .await;

    assert!(registry.has_handler(&workflow_id, &handler_name).await);
}
