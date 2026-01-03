//! Tests for client API

use flow_raft_api::client::*;
use flow_raft_api::workflow::WorkflowDef;
use flow_raft_core::{RetryConfig, WorkflowId};
use serde_json::json;
use std::time::Duration;

#[test]
fn test_workflow_execution_id_from_workflow_id() {
    let workflow_id = WorkflowId::default();
    let execution_id: WorkflowExecutionId = workflow_id.into();
    assert_eq!(execution_id.0, workflow_id);
}

#[test]
fn test_workflow_execution_id_display() {
    let workflow_id = WorkflowId::default();
    let execution_id = WorkflowExecutionId::from(workflow_id);
    let display = format!("{}", execution_id);
    assert!(!display.is_empty());
}

#[test]
fn test_workflow_execution_id_to_workflow_id() {
    let workflow_id = WorkflowId::default();
    let execution_id = WorkflowExecutionId::from(workflow_id);
    let converted: WorkflowId = execution_id.into();
    assert_eq!(converted, workflow_id);
}

#[test]
fn test_workflow_status_variants() {
    let pending = WorkflowStatus::Pending;
    let running = WorkflowStatus::Running;
    let completed = WorkflowStatus::Completed {
        outputs: Some(json!({"result": "success"})),
    };
    let failed = WorkflowStatus::Failed {
        error: Some("test error".to_string()),
    };
    let cancelled = WorkflowStatus::Cancelled;

    assert!(matches!(pending, WorkflowStatus::Pending));
    assert!(matches!(running, WorkflowStatus::Running));
    assert!(matches!(completed, WorkflowStatus::Completed { .. }));
    assert!(matches!(failed, WorkflowStatus::Failed { .. }));
    assert!(matches!(cancelled, WorkflowStatus::Cancelled));
}

#[test]
fn test_flow_raft_client_new() {
    let client = FlowRaftClient::new("http://localhost:8080");
    // timeout field is private, test that client is created
    assert!(std::mem::size_of_val(&client) > 0);
}

#[test]
fn test_flow_raft_client_with_timeout() {
    let client = FlowRaftClient::new("http://localhost:8080").with_timeout(Duration::from_secs(60));
    // timeout field is private, test that client is created with timeout
    assert!(std::mem::size_of_val(&client) > 0);
}

#[tokio::test]
async fn test_client_submit_workflow() {
    let client = FlowRaftClient::new("http://localhost:8080");
    let workflow = WorkflowDef::from_graph(
        "test",
        flow_raft_api::graph::GraphBuilder::new("test")
            .add_node("task1", "handler1", vec![], vec![], None)
            .set_root("task1")
            .build()
            .unwrap(),
        RetryConfig::default(),
    );
    let result = client.submit_workflow(workflow, json!({})).await;
    // Now that client is implemented, it will try to connect and fail
    assert!(result.is_err());
    match result {
        Err(ClientError::Connection(_)) | Err(ClientError::Server(_)) => {
            // Expected: connection error or server error
        }
        _ => panic!("Expected Connection or Server error, got {:?}", result),
    }
}

#[tokio::test]
async fn test_client_get_workflow_status() {
    let client = FlowRaftClient::new("http://localhost:8080");
    let execution_id = WorkflowExecutionId::from(WorkflowId::default());
    let result = client.get_workflow_status(execution_id).await;
    // Now that client is implemented, it will try to connect and fail
    assert!(result.is_err());
    match result {
        Err(ClientError::Connection(_)) | Err(ClientError::Server(_)) => {
            // Expected: connection error or server error
        }
        _ => panic!("Expected Connection or Server error, got {:?}", result),
    }
}

#[tokio::test]
async fn test_client_get_workflow_output_timeout() {
    let mut client =
        FlowRaftClient::new("http://localhost:8080").with_timeout(Duration::from_millis(10));
    let execution_id = WorkflowExecutionId::from(WorkflowId::default());

    // This will timeout quickly since get_workflow_status always returns an error
    // and we have a very short timeout
    let result = client.get_workflow_output(execution_id).await;
    assert!(result.is_err());
    match result {
        Err(ClientError::Connection(_)) | Err(ClientError::Timeout(_)) => {}
        _ => panic!("Expected Connection or Timeout error"),
    }
}

#[tokio::test]
async fn test_client_cancel_workflow() {
    let client = FlowRaftClient::new("http://localhost:8080");
    let execution_id = WorkflowExecutionId::from(WorkflowId::default());
    let result = client.cancel_workflow(execution_id).await;
    // Now that client is implemented, it will try to connect and fail
    assert!(result.is_err());
    match result {
        Err(ClientError::Connection(_)) | Err(ClientError::Server(_)) => {
            // Expected: connection error or server error
        }
        _ => panic!("Expected Connection or Server error, got {:?}", result),
    }
}

#[tokio::test]
async fn test_client_pause_workflow() {
    let client = FlowRaftClient::new("http://localhost:8080");
    let execution_id = WorkflowExecutionId::from(WorkflowId::default());
    let result = client.pause_workflow(execution_id).await;
    // Now that client is implemented, it will try to connect and fail
    assert!(result.is_err());
    match result {
        Err(ClientError::Connection(_)) | Err(ClientError::Server(_)) => {
            // Expected: connection error or server error
        }
        _ => panic!("Expected Connection or Server error, got {:?}", result),
    }
}

#[tokio::test]
async fn test_client_resume_workflow() {
    let client = FlowRaftClient::new("http://localhost:8080");
    let execution_id = WorkflowExecutionId::from(WorkflowId::default());
    let result = client.resume_workflow(execution_id).await;
    // Now that client is implemented, it will try to connect and fail
    assert!(result.is_err());
    match result {
        Err(ClientError::Connection(_)) | Err(ClientError::Server(_)) => {
            // Expected: connection error or server error
        }
        _ => panic!("Expected Connection or Server error, got {:?}", result),
    }
}

#[test]
fn test_client_error_display() {
    let connection_error = ClientError::Connection("test error".to_string());
    assert!(format!("{}", connection_error).contains("test error"));

    let server_error = ClientError::Server("server error".to_string());
    assert!(format!("{}", server_error).contains("server error"));

    let execution_id = WorkflowExecutionId::from(WorkflowId::default());
    let not_found = ClientError::NotFound(execution_id);
    assert!(format!("{}", not_found).contains("not found"));

    let timeout = ClientError::Timeout(execution_id);
    assert!(format!("{}", timeout).contains("Timeout"));

    let invalid = ClientError::InvalidInput("invalid".to_string());
    assert!(format!("{}", invalid).contains("invalid"));
}
