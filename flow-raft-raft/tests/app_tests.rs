//! Comprehensive tests for FlowRaftApp

use chrono::Utc;
use flow_raft_core::{WorkflowId, WorkflowSnapshot, WorkflowState};
use flow_raft_raft::app::FlowRaftApp;
use flow_raft_raft::config::default_config;
use flow_raft_raft::network::MemoryNetworkFactory;
use flow_raft_raft::node::FlowRaftNode;
use flow_raft_raft::storage::{LogStore, StateMachineStore};
use flow_raft_raft::types::{NodeId, Request, Response};
use indexmap::IndexMap;

async fn create_test_app() -> (FlowRaftApp, FlowRaftNode) {
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
    let app = FlowRaftApp::new(raft, state_machine);
    (app, node)
}

#[tokio::test]
async fn test_app_new() {
    let (app, _node) = create_test_app().await;
    // Verify app is created
    assert!(std::mem::size_of_val(&app) > 0);
}

#[tokio::test]
async fn test_app_get_workflow_nonexistent() {
    let (app, _node) = create_test_app().await;
    let workflow_id = WorkflowId::default();
    let workflow = app.get_workflow(&workflow_id).await;
    assert!(workflow.is_none());
}

#[tokio::test]
async fn test_app_get_all_workflows_empty() {
    let (app, _node) = create_test_app().await;
    let workflows = app.get_all_workflows().await;
    assert!(workflows.is_empty());
}

#[tokio::test]
async fn test_app_create_workflow() {
    let (app, _node) = create_test_app().await;
    let snapshot = WorkflowSnapshot {
        workflow_id: WorkflowId::default(),
        state: WorkflowState::Draft,
        task_definitions: IndexMap::new(),
        executions: IndexMap::new(),
        dependencies: IndexMap::new(),
        retry_configs: IndexMap::new(),
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        inputs: serde_json::json!({}),
        outputs: None,
        error_message: None,
    };
    let request = Request::CreateWorkflow { workflow: snapshot };
    let response = app.create_workflow(request).await;
    assert!(response.is_ok());
    match response.unwrap() {
        Response::WorkflowCreated { workflow_id: _ } => {}
        _ => panic!("Expected WorkflowCreated response"),
    }
}

#[tokio::test]
async fn test_app_get_workflow_after_create() {
    let (app, _node) = create_test_app().await;
    let workflow_id = WorkflowId::default();
    let snapshot = WorkflowSnapshot {
        workflow_id,
        state: WorkflowState::Draft,
        task_definitions: IndexMap::new(),
        executions: IndexMap::new(),
        dependencies: IndexMap::new(),
        retry_configs: IndexMap::new(),
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        inputs: serde_json::json!({}),
        outputs: None,
        error_message: None,
    };
    let request = Request::CreateWorkflow {
        workflow: snapshot.clone(),
    };
    app.create_workflow(request).await.unwrap();

    let retrieved = app.get_workflow(&workflow_id).await;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().workflow_id, workflow_id);
}

#[tokio::test]
async fn test_app_get_all_workflows_after_create() {
    let (app, _node) = create_test_app().await;
    let workflow_id = WorkflowId::default();
    let snapshot = WorkflowSnapshot {
        workflow_id,
        state: WorkflowState::Draft,
        task_definitions: IndexMap::new(),
        executions: IndexMap::new(),
        dependencies: IndexMap::new(),
        retry_configs: IndexMap::new(),
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        inputs: serde_json::json!({}),
        outputs: None,
        error_message: None,
    };
    let request = Request::CreateWorkflow { workflow: snapshot };
    app.create_workflow(request).await.unwrap();

    let workflows = app.get_all_workflows().await;
    assert_eq!(workflows.len(), 1);
    assert!(workflows.contains_key(&workflow_id));
}
