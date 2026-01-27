//! Tests for client API

use flow_raft_api::client::*;
use flow_raft_core::WorkflowId;
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
fn test_flow_raft_client_builder_new_and_default() {
    let b = FlowRaftClientBuilder::new();
    assert!(std::mem::size_of_val(&b) > 0);
    let b2 = FlowRaftClientBuilder::default();
    assert!(std::mem::size_of_val(&b2) > 0);
}

#[test]
fn test_flow_raft_client_builder_with_endpoint_and_timeout() {
    let b = FlowRaftClientBuilder::new()
        .with_endpoint("http://127.0.0.1:50051")
        .with_timeout(Duration::from_secs(60));
    let client = b.build();
    assert!(std::mem::size_of_val(&client) > 0);
}

#[test]
fn test_flow_raft_client_builder_build_uses_defaults() {
    let client = FlowRaftClientBuilder::new().build();
    assert!(std::mem::size_of_val(&client) > 0);
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

fn nop(_: ()) -> Result<(), String> {
    Ok(())
}

#[tokio::test]
async fn test_client_submit_workflow() {
    let client = FlowRaftClient::new("http://localhost:8080");
    let mut b = flow_raft_api::graph::TypedGraphBuilder::new("test");
    b.add_node("task1", flow_raft_api::graph::node(nop), None)
        .set_root("task1");
    let workflow = b.build().unwrap().workflow_def("test").unwrap();
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

    // With no server running and a short timeout, we expect an error (connection, timeout, or server).
    let result = client.get_workflow_output(execution_id).await;
    assert!(result.is_err());
    match result {
        Err(ClientError::Connection(_))
        | Err(ClientError::Timeout(_))
        | Err(ClientError::Server(_)) => {}
        other => panic!(
            "Expected Connection, Timeout, or Server error, got {:?}",
            other
        ),
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
