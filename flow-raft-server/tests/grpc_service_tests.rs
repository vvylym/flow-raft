//! Comprehensive tests for gRPC service

use flow_raft_proto::proto::*;
use flow_raft_raft::app::FlowRaftApp;
use flow_raft_raft::config::default_config;
use flow_raft_raft::executor::WorkflowExecutor;
use flow_raft_raft::network::MemoryNetworkFactory;
use flow_raft_raft::node::FlowRaftNode;
use flow_raft_raft::storage::{LogStore, StateMachineStore};
use flow_raft_raft::types::NodeId;
use flow_raft_server::grpc::FlowRaftService;
use flow_raft_server::grpc::service::FlowRaftServiceImpl;
use flow_raft_server::handlers::HandlerRegistry;
use std::sync::Arc;

async fn create_test_service() -> FlowRaftServiceImpl {
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

    FlowRaftServiceImpl::new(app, executor, registry)
}

#[tokio::test]
async fn test_grpc_service_new() {
    let service = create_test_service().await;
    // Verify service is created
    assert!(std::mem::size_of_val(&service) > 0);
}

#[tokio::test]
async fn test_grpc_service_get_node_status() {
    let service = create_test_service().await;
    let request = tonic::Request::new(GetNodeStatusRequest {
        node_id: 0, // 0 means current node
    });
    let response = service.get_node_status(request).await;
    assert!(response.is_ok());
    let status = response.unwrap().into_inner();
    assert_eq!(status.node_id, 1);
    assert_eq!(status.mode, "leader");
    assert!(status.is_leader);
}

#[tokio::test]
async fn test_grpc_service_define_workflow() {
    let service = create_test_service().await;
    let definition = serde_json::json!({
        "name": "test_workflow",
        "graph": {
            "name": "test_workflow",
            "nodes": [{
                "name": "task1",
                "task_id": "00000000-0000-0000-0000-000000000001",
                "handler": "handler1",
                "inputs": [],
                "outputs": [],
                "timeout_secs": null
            }],
            "edges": [],
            "root": "task1"
        },
        "default_retry_config": { "max_attempts": 3 }
    })
    .to_string();
    let request = tonic::Request::new(DefineWorkflowRequest {
        name: "test_workflow".to_string(),
        definition,
    });
    let response = service.define_workflow(request).await;
    assert!(response.is_ok(), "define_workflow should succeed");
    let def = response.unwrap().into_inner();
    assert!(!def.workflow_id.is_empty());
    assert_eq!(def.name, "test_workflow");
    assert_eq!(def.status, "draft");

    // Workflow should be persisted: get_workflow finds it
    let get_req = tonic::Request::new(GetWorkflowRequest {
        workflow_id: def.workflow_id.clone(),
    });
    let get_resp = service.get_workflow(get_req).await;
    assert!(
        get_resp.is_ok(),
        "get_workflow should find the defined workflow"
    );
}

#[tokio::test]
async fn test_grpc_service_list_workflows() {
    let service = create_test_service().await;
    let request = tonic::Request::new(ListWorkflowsRequest {
        filter: None,
        limit: 100,
        offset: 0,
    });
    let response = service.list_workflows(request).await;
    assert!(response.is_ok());
    let list = response.unwrap().into_inner();
    assert!(list.workflows.is_empty()); // Initially empty
}

#[tokio::test]
async fn test_grpc_service_get_workflow_not_found() {
    let service = create_test_service().await;
    // Use just the UUID part, not "workflow:uuid"
    let workflow_id = flow_raft_core::WorkflowId::default();
    let workflow_id_str = workflow_id.as_ref().to_string(); // Get just the UUID part
    let request = tonic::Request::new(GetWorkflowRequest {
        workflow_id: workflow_id_str,
    });
    let response = service.get_workflow(request).await;
    // Should return not found
    assert!(response.is_err());
    assert!(response.unwrap_err().code() == tonic::Code::NotFound);
}

#[tokio::test]
async fn test_grpc_service_watch_workflow() {
    let mut service = create_test_service().await;
    // Set up a watcher for the service
    let watcher = Arc::new(flow_raft_observability::WorkflowWatcher::new());
    service.set_watcher(watcher);

    // Use a valid UUID format
    let workflow_id_str = "00000000-0000-0000-0000-000000000001".to_string();
    let request = tonic::Request::new(WatchWorkflowRequest {
        workflow_id: workflow_id_str,
    });
    let stream = service.watch_workflow(request).await;
    // Stream should be created (even if empty)
    assert!(stream.is_ok());
    let _ = stream.unwrap();
}

#[tokio::test]
async fn test_grpc_service_get_execution_history() {
    let service = create_test_service().await;
    // Use just the UUID part, not "workflow:uuid"
    let workflow_id = flow_raft_core::WorkflowId::default();
    let workflow_id_str = workflow_id.as_ref().to_string(); // Get just the UUID part
    let request = tonic::Request::new(GetExecutionHistoryRequest {
        workflow_id: workflow_id_str,
        limit: 10,
    });
    let response = service.get_execution_history(request).await;
    // Should succeed even if workflow doesn't exist (returns empty history)
    // But if UUID parsing fails, it returns InvalidArgument
    if let Err(e) = &response {
        // If it fails, it should be InvalidArgument (UUID parsing error)
        assert_eq!(
            e.code(),
            tonic::Code::InvalidArgument,
            "Expected InvalidArgument for parsing error, got {:?}",
            e.code()
        );
    } else {
        let history = response.unwrap().into_inner();
        assert!(history.events.is_empty()); // Initially empty
    }
}

#[tokio::test]
async fn test_grpc_service_get_task_results_not_found() {
    let service = create_test_service().await;
    // Use just the UUID parts, not "workflow:uuid" or "task:uuid"
    let workflow_id = flow_raft_core::WorkflowId::default();
    let task_id = flow_raft_core::TaskId::default();
    let workflow_id_str = workflow_id.as_ref().to_string(); // Get just the UUID part
    let task_id_str = task_id.as_ref().to_string(); // Get just the UUID part
    let request = tonic::Request::new(GetTaskResultsRequest {
        workflow_id: workflow_id_str,
        task_id: task_id_str,
    });
    let response = service.get_task_results(request).await;
    // Should return not found
    assert!(response.is_err());
    let err = response.unwrap_err();
    // The service should return NotFound for non-existent workflows/tasks
    assert_eq!(err.code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_grpc_service_get_execution_history_with_store() {
    use flow_raft_observability::HistoryStore;
    use std::sync::Arc as StdArc;

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
    let history_store = StdArc::new(HistoryStore::new());

    let service =
        FlowRaftServiceImpl::with_history_store(app, executor, registry, history_store.clone());

    // Use a valid UUID format
    let workflow_id = flow_raft_core::WorkflowId::default();
    let workflow_id_str = workflow_id.as_ref().to_string();

    // Record an event
    use flow_raft_observability::history::{ExecutionEvent, ExecutionEventType};
    history_store
        .record_event(
            workflow_id,
            ExecutionEvent {
                event_type: ExecutionEventType::TaskStarted,
                task_id: None,
                data: "{}".to_string(),
                timestamp: chrono::Utc::now(),
            },
        )
        .await;

    let request = tonic::Request::new(GetExecutionHistoryRequest {
        workflow_id: workflow_id_str,
        limit: 10,
    });
    let response = service.get_execution_history(request).await;
    assert!(response.is_ok());
    let history = response.unwrap().into_inner();
    assert_eq!(history.events.len(), 1);
    assert_eq!(history.events[0].event_type, "task_started");
}

#[tokio::test]
async fn test_grpc_service_pause_workflow_not_found() {
    let service = create_test_service().await;
    let workflow_id = flow_raft_core::WorkflowId::default();
    let workflow_id_str = workflow_id.as_ref().to_string();
    let request = tonic::Request::new(PauseWorkflowRequest {
        workflow_id: workflow_id_str,
    });
    let response = service.pause_workflow(request).await;
    // Should return not found
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_grpc_service_resume_workflow_not_found() {
    let service = create_test_service().await;
    let workflow_id = flow_raft_core::WorkflowId::default();
    let workflow_id_str = workflow_id.as_ref().to_string();
    let request = tonic::Request::new(ResumeWorkflowRequest {
        workflow_id: workflow_id_str,
    });
    let response = service.resume_workflow(request).await;
    // Should return not found
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_grpc_service_cancel_workflow_not_found() {
    let service = create_test_service().await;
    let workflow_id = flow_raft_core::WorkflowId::default();
    let workflow_id_str = workflow_id.as_ref().to_string();
    let request = tonic::Request::new(CancelWorkflowRequest {
        workflow_id: workflow_id_str,
    });
    let response = service.cancel_workflow(request).await;
    // Should return not found
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}
