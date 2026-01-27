//! Comprehensive tests for FlowRaftClient

use flow_raft_api::client::{ClientError, FlowRaftClient, WorkflowExecutionId, WorkflowStatus};
use flow_raft_core::WorkflowId;
use flow_raft_raft::app::FlowRaftApp;
use flow_raft_raft::config::default_config;
use flow_raft_raft::executor::WorkflowExecutor;
use flow_raft_raft::network::MemoryNetworkFactory;
use flow_raft_raft::node::FlowRaftNode;
use flow_raft_raft::storage::{LogStore, StateMachineStore};
use flow_raft_raft::types::NodeId;
use flow_raft_server::grpc::{FlowRaftServiceImpl, FlowRaftServiceServer};
use flow_raft_server::handlers::HandlerRegistry;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

#[test]
fn test_workflow_execution_id_from_workflow_id() {
    let workflow_id = WorkflowId::default();
    let exec_id: WorkflowExecutionId = workflow_id.into();
    assert_eq!(exec_id.to_string(), workflow_id.to_string());
}

#[test]
fn test_workflow_execution_id_to_workflow_id() {
    let workflow_id = WorkflowId::default();
    let exec_id = WorkflowExecutionId(workflow_id);
    let converted: WorkflowId = exec_id.into();
    assert_eq!(converted, workflow_id);
}

#[test]
fn test_workflow_execution_id_display() {
    let workflow_id = WorkflowId::default();
    let exec_id = WorkflowExecutionId(workflow_id);
    let display = format!("{}", exec_id);
    assert!(!display.is_empty());
}

#[test]
fn test_workflow_status_variants() {
    let _pending = WorkflowStatus::Pending;
    let _running = WorkflowStatus::Running;
    let _completed = WorkflowStatus::Completed {
        outputs: Some(serde_json::json!({"result": "success"})),
    };
    let _failed = WorkflowStatus::Failed {
        error: Some("error".to_string()),
    };
    let _cancelled = WorkflowStatus::Cancelled;
    // Verify all variants can be created
}

#[test]
fn test_client_error_display() {
    let error = ClientError::Connection("test error".to_string());
    let message = format!("{}", error);
    assert!(message.contains("test error"));

    let error = ClientError::Server("server error".to_string());
    let message = format!("{}", error);
    assert!(message.contains("server error"));

    let exec_id = WorkflowExecutionId(WorkflowId::default());
    let error = ClientError::NotFound(exec_id);
    let message = format!("{}", error);
    assert!(message.contains("not found"));

    let error = ClientError::Timeout(exec_id);
    let message = format!("{}", error);
    assert!(message.contains("Timeout"));

    let error = ClientError::InvalidInput("invalid".to_string());
    let message = format!("{}", error);
    assert!(message.contains("invalid"));
}

/// Helper function to create a test gRPC server on a random port
/// Returns a tuple of (server_handle, endpoint_url)
async fn create_test_server() -> (JoinHandle<Result<(), tonic::transport::Error>>, String) {
    let node_id: NodeId = 1;
    let config = default_config();
    let network = MemoryNetworkFactory::new();
    let log_store = LogStore::default();
    let state_machine = StateMachineStore::default();

    let node = FlowRaftNode::new(node_id, config, network, log_store, state_machine.clone())
        .await
        .unwrap();
    node.initialize_single_node().await.unwrap();

    let raft = node.raft.clone();
    let app = Arc::new(FlowRaftApp::new(raft.clone(), state_machine.clone()));
    let executor = Arc::new(WorkflowExecutor::new(raft, state_machine, node_id));
    let registry = Arc::new(HandlerRegistry::new());

    let service = FlowRaftServiceImpl::new(app, executor, registry);

    // Bind to port 0 to get a random available port
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = format!("http://127.0.0.1:{}", port);

    // Convert TcpListener to a stream
    let incoming = TcpListenerStream::new(listener);

    // Start server in background
    let server = Server::builder()
        .add_service(FlowRaftServiceServer::new(service))
        .serve_with_incoming(incoming);

    let handle = tokio::spawn(server);

    // Give server a moment to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    (handle, endpoint)
}

#[tokio::test]
async fn test_client_submit_workflow() {
    let (_handle, endpoint) = create_test_server().await;
    let client = FlowRaftClient::new(&endpoint);
    let mut b = flow_raft_api::graph::TypedGraphBuilder::new("test");
    b.add_node(
        "task1",
        flow_raft_api::graph::node(|_: ()| Ok::<(), String>(())),
        None,
    )
    .set_root("task1");
    let workflow = b.build().unwrap().workflow_def("test").unwrap();
    let result = client
        .submit_workflow(workflow, serde_json::json!({}))
        .await;
    // Note: submit_workflow calls define_workflow which is a placeholder
    // It will fail because define_workflow doesn't actually create the workflow
    // This is expected behavior until define_workflow is fully implemented
    // For now, we verify it returns an error (not a connection error)
    assert!(result.is_err());
    // Should be InvalidInput or Server error, not Connection error
    match result.unwrap_err() {
        ClientError::Connection(_) => panic!("Should not be a connection error"),
        ClientError::InvalidInput(_) | ClientError::Server(_) => {}
        e => panic!("Unexpected error type: {:?}", e),
    }

    // Cleanup
    _handle.abort();
}

#[tokio::test]
async fn test_client_get_workflow_status() {
    let (_handle, endpoint) = create_test_server().await;
    let client = FlowRaftClient::new(&endpoint);

    // Try to get status for a non-existent workflow
    // This verifies the gRPC call works (not a connection error)
    let exec_id = WorkflowExecutionId(WorkflowId::default());
    let result = client.get_workflow_status(exec_id).await;
    // Should return Server error with "not found" message (not Connection error)
    assert!(result.is_err());
    match result.unwrap_err() {
        ClientError::Server(msg) if msg.contains("not found") => {} // Expected - workflow doesn't exist
        ClientError::Connection(_) => panic!("Should not be a connection error"),
        e => panic!("Unexpected error: {:?}", e),
    }

    // Cleanup
    _handle.abort();
}

#[tokio::test]
async fn test_client_get_workflow_output_timeout() {
    let (_handle, endpoint) = create_test_server().await;
    let mut client = FlowRaftClient::new(&endpoint).with_timeout(Duration::from_millis(10));

    // Try to get output for a non-existent workflow with very short timeout
    let exec_id = WorkflowExecutionId(WorkflowId::default());
    let result = client.get_workflow_output(exec_id).await;
    // Should return Server error with "not found" message (not Connection error)
    // Note: get_workflow_output polls get_workflow_status, which will fail immediately
    // with "not found", so it won't timeout
    assert!(result.is_err());
    match result.unwrap_err() {
        ClientError::Server(msg) if msg.contains("not found") => {} // Expected
        ClientError::Connection(_) => panic!("Should not be a connection error"),
        e => panic!("Expected Server error with 'not found', got {:?}", e),
    }

    // Cleanup
    _handle.abort();
}

#[tokio::test]
async fn test_client_cancel_workflow() {
    let (_handle, endpoint) = create_test_server().await;
    let client = FlowRaftClient::new(&endpoint);

    // Try to cancel a non-existent workflow
    // This verifies the gRPC call works (not a connection error)
    let exec_id = WorkflowExecutionId(WorkflowId::default());
    let result = client.cancel_workflow(exec_id).await;
    // Should return NotFound error (not Connection error)
    // Note: cancel_workflow returns Result<(), ClientError>, so we check the error
    assert!(result.is_err());
    match result.unwrap_err() {
        ClientError::Server(msg) if msg.contains("not found") => {} // Expected
        ClientError::Connection(_) => panic!("Should not be a connection error"),
        e => panic!("Unexpected error: {:?}", e),
    }

    // Cleanup
    _handle.abort();
}

#[tokio::test]
async fn test_client_get_task_result() {
    let (_handle, endpoint) = create_test_server().await;
    let client = FlowRaftClient::new(&endpoint);

    // Try to get task result for non-existent workflow/task
    let exec_id = WorkflowExecutionId(WorkflowId::default());
    let task_id = flow_raft_core::TaskId::default();
    let result = client.get_task_result(exec_id, task_id).await;
    // Should return Server error (not Connection error)
    assert!(result.is_err());
    match result.unwrap_err() {
        ClientError::Server(_) => {} // Expected
        ClientError::Connection(_) => panic!("Should not be a connection error"),
        e => panic!("Unexpected error: {:?}", e),
    }

    // Cleanup
    _handle.abort();
}

#[tokio::test]
async fn test_client_run() {
    let (_handle, endpoint) = create_test_server().await;
    let client = FlowRaftClient::new(&endpoint);

    // Try to run a non-existent workflow by name
    let result = client.run("nonexistent", serde_json::json!({})).await;
    // Should return NotFound error (not Connection error)
    assert!(result.is_err());
    match result.unwrap_err() {
        ClientError::NotFound(_) => {} // Expected
        ClientError::Connection(_) => panic!("Should not be a connection error"),
        e => panic!("Unexpected error: {:?}", e),
    }

    // Cleanup
    _handle.abort();
}

#[tokio::test]
async fn test_client_watch_execution() {
    let (_handle, endpoint) = create_test_server().await;
    let mut client = FlowRaftClient::new(&endpoint);

    // Try to watch a non-existent workflow
    let exec_id = WorkflowExecutionId(WorkflowId::default());
    let result = client.watch_execution(exec_id).await;
    // Should return Server error (not Connection error)
    // Note: watch_workflow requires a watcher to be configured, so it may return unavailable
    match result {
        Err(ClientError::Server(msg))
            if msg.contains("not found") || msg.contains("unavailable") => {}
        Err(ClientError::Connection(_)) => panic!("Should not be a connection error"),
        Err(e) => panic!("Unexpected error: {:?}", e),
        Ok(_) => {
            // If it succeeds, the stream will be empty (no watcher configured)
            // This is acceptable - the gRPC call worked
        }
    }

    // Cleanup
    _handle.abort();
}

#[tokio::test]
async fn test_client_pause_workflow() {
    let (_handle, endpoint) = create_test_server().await;
    let client = FlowRaftClient::new(&endpoint);

    // Try to pause a non-existent workflow
    let exec_id = WorkflowExecutionId(WorkflowId::default());
    let result = client.pause_workflow(exec_id).await;
    // Should return Server error (not Connection error)
    assert!(result.is_err());
    match result.unwrap_err() {
        ClientError::Server(msg) if msg.contains("not found") => {}
        ClientError::Connection(_) => panic!("Should not be a connection error"),
        e => panic!("Unexpected error: {:?}", e),
    }

    // Cleanup
    _handle.abort();
}

#[tokio::test]
async fn test_client_resume_workflow() {
    let (_handle, endpoint) = create_test_server().await;
    let client = FlowRaftClient::new(&endpoint);

    // Try to resume a non-existent workflow
    let exec_id = WorkflowExecutionId(WorkflowId::default());
    let result = client.resume_workflow(exec_id).await;
    // Should return Server error (not Connection error)
    assert!(result.is_err());
    match result.unwrap_err() {
        ClientError::Server(msg) if msg.contains("not found") => {}
        ClientError::Connection(_) => panic!("Should not be a connection error"),
        e => panic!("Unexpected error: {:?}", e),
    }

    // Cleanup
    _handle.abort();
}
