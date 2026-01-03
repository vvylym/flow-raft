//! Additional tests for executor module

use chrono::Utc;
use flow_raft_core::{
    TaskDefinition, TaskDependencies, TaskId, TaskState, WorkflowId, WorkflowSnapshot,
    WorkflowState,
};
use flow_raft_raft::app::FlowRaftApp;
use flow_raft_raft::config::default_config;
use flow_raft_raft::executor::{TaskHandler, WorkflowExecutor};
use flow_raft_raft::network::MemoryNetworkFactory;
use flow_raft_raft::node::FlowRaftNode;
use flow_raft_raft::storage::{LogStore, StateMachineStore};
use flow_raft_raft::types::NodeId;
use indexmap::IndexMap;
use std::collections::HashSet;
use std::sync::Arc;

struct TestHandler;

impl TaskHandler for TestHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({
            "result": inputs.get("value").cloned().unwrap_or(serde_json::json!(0))
        }))
    }
}

struct FailingHandler {
    attempt_count: std::sync::Arc<std::sync::atomic::AtomicU32>,
    max_attempts: u32,
}

impl TaskHandler for FailingHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        _inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let count = self
            .attempt_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if count < self.max_attempts {
            Err(format!("Task failed on attempt {}", count + 1))
        } else {
            Ok(serde_json::json!({"result": "success"}))
        }
    }
}

/// Helper function to create a test executor with Raft setup
async fn create_test_executor_with_app() -> (WorkflowExecutor, Arc<FlowRaftApp>, FlowRaftNode) {
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
    let executor = WorkflowExecutor::new(raft, state_machine, node_id);

    (executor, app, node)
}

#[test]
fn test_task_handler_trait() {
    let handler = TestHandler;
    let result = handler.execute(TaskId::default(), serde_json::json!({"value": 42}));
    assert!(result.is_ok());
    assert_eq!(result.unwrap().get("result"), Some(&serde_json::json!(42)));
}

#[tokio::test]
async fn test_executor_execute_workflow() {
    let (executor, app, _node) = create_test_executor_with_app().await;

    // Create a simple workflow
    let workflow_id = WorkflowId::default();
    let task_id = TaskId::default();

    let workflow = WorkflowSnapshot {
        workflow_id,
        state: WorkflowState::Running,
        task_definitions: {
            let mut map = IndexMap::new();
            map.insert(
                task_id,
                TaskDefinition {
                    id: task_id,
                    name: "test_task".to_string(),
                    handler: "test_handler".to_string(),
                    inputs: HashSet::new(),
                    outputs: HashSet::new(),
                    timeout_secs: None,
                },
            );
            map
        },
        executions: IndexMap::new(),
        dependencies: {
            let mut map = IndexMap::new();
            map.insert(task_id, TaskDependencies::default());
            map
        },
        retry_configs: IndexMap::new(),
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        inputs: serde_json::json!({}),
        outputs: None,
        error_message: None,
    };

    // Register workflow via Raft
    let request = flow_raft_raft::types::Request::CreateWorkflow {
        workflow: workflow.clone(),
    };
    app.create_workflow(request).await.unwrap();

    // Wait for state machine to apply
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Execute the task
    let handler = TestHandler;
    let inputs = serde_json::json!({"value": 42});
    let result = executor
        .execute_task(workflow_id, task_id, &handler, inputs)
        .await;

    assert!(result.is_ok(), "Task execution should succeed");

    // Verify task state was updated
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let updated_workflow = executor.state_machine().get_workflow(&workflow_id).await;
    assert!(updated_workflow.is_some());
    let workflow = updated_workflow.unwrap();
    let execution = workflow.executions.get(&task_id);
    assert!(execution.is_some());
    assert!(matches!(execution.unwrap().state, TaskState::Completed));
}

#[tokio::test]
async fn test_executor_task_dependencies() {
    let (executor, app, _node) = create_test_executor_with_app().await;

    let workflow_id = WorkflowId::default();
    let task1_id = TaskId::default();
    let task2_id = TaskId::default();

    // Create workflow with task2 depending on task1
    let mut deps = TaskDependencies::default();
    deps.add_prerequisite(task1_id);

    let workflow = WorkflowSnapshot {
        workflow_id,
        state: WorkflowState::Running,
        task_definitions: {
            let mut map = IndexMap::new();
            map.insert(
                task1_id,
                TaskDefinition {
                    id: task1_id,
                    name: "task1".to_string(),
                    handler: "handler1".to_string(),
                    inputs: HashSet::new(),
                    outputs: HashSet::new(),
                    timeout_secs: None,
                },
            );
            map.insert(
                task2_id,
                TaskDefinition {
                    id: task2_id,
                    name: "task2".to_string(),
                    handler: "handler2".to_string(),
                    inputs: HashSet::new(),
                    outputs: HashSet::new(),
                    timeout_secs: None,
                },
            );
            map
        },
        executions: IndexMap::new(),
        dependencies: {
            let mut map = IndexMap::new();
            map.insert(task1_id, TaskDependencies::default());
            map.insert(task2_id, deps);
            map
        },
        retry_configs: IndexMap::new(),
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        inputs: serde_json::json!({}),
        outputs: None,
        error_message: None,
    };

    // Register workflow
    let request = flow_raft_raft::types::Request::CreateWorkflow {
        workflow: workflow.clone(),
    };
    app.create_workflow(request).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Initially, only task1 should be ready
    let ready = executor.get_ready_tasks(&workflow_id).await;
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&task1_id));
    assert!(!ready.contains(&task2_id));

    // Execute task1
    let handler = TestHandler;
    executor
        .execute_task(workflow_id, task1_id, &handler, serde_json::json!({}))
        .await
        .unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Now task2 should be ready
    let ready = executor.get_ready_tasks(&workflow_id).await;
    assert!(
        ready.contains(&task2_id),
        "task2 should be ready after task1 completes"
    );
}

#[tokio::test]
async fn test_executor_task_failure() {
    let (executor, app, _node) = create_test_executor_with_app().await;

    let workflow_id = WorkflowId::default();
    let task_id = TaskId::default();

    let workflow = WorkflowSnapshot {
        workflow_id,
        state: WorkflowState::Running,
        task_definitions: {
            let mut map = IndexMap::new();
            map.insert(
                task_id,
                TaskDefinition {
                    id: task_id,
                    name: "test_task".to_string(),
                    handler: "test_handler".to_string(),
                    inputs: HashSet::new(),
                    outputs: HashSet::new(),
                    timeout_secs: None,
                },
            );
            map
        },
        executions: IndexMap::new(),
        dependencies: {
            let mut map = IndexMap::new();
            map.insert(task_id, TaskDependencies::default());
            map
        },
        retry_configs: IndexMap::new(),
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        inputs: serde_json::json!({}),
        outputs: None,
        error_message: None,
    };

    // Register workflow
    let request = flow_raft_raft::types::Request::CreateWorkflow {
        workflow: workflow.clone(),
    };
    app.create_workflow(request).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Execute task with failing handler
    struct SimpleFailingHandler;
    impl TaskHandler for SimpleFailingHandler {
        fn execute(
            &self,
            _task_id: TaskId,
            _inputs: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            Err("Task failed".to_string())
        }
    }

    let handler = SimpleFailingHandler;
    let result = executor
        .execute_task(workflow_id, task_id, &handler, serde_json::json!({}))
        .await;
    assert!(
        result.is_ok(),
        "execute_task should succeed even if handler fails"
    );

    // Verify task state shows failure
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let updated_workflow = executor.state_machine().get_workflow(&workflow_id).await;
    assert!(updated_workflow.is_some());
    let workflow = updated_workflow.unwrap();
    let execution = workflow.executions.get(&task_id);
    assert!(execution.is_some());
    assert!(matches!(execution.unwrap().state, TaskState::Failed { .. }));
}

#[tokio::test]
async fn test_executor_task_retry() {
    let (executor, app, _node) = create_test_executor_with_app().await;

    let workflow_id = WorkflowId::default();
    let task_id = TaskId::default();

    let workflow = WorkflowSnapshot {
        workflow_id,
        state: WorkflowState::Running,
        task_definitions: {
            let mut map = IndexMap::new();
            map.insert(
                task_id,
                TaskDefinition {
                    id: task_id,
                    name: "test_task".to_string(),
                    handler: "test_handler".to_string(),
                    inputs: HashSet::new(),
                    outputs: HashSet::new(),
                    timeout_secs: None,
                },
            );
            map
        },
        executions: IndexMap::new(),
        dependencies: {
            let mut map = IndexMap::new();
            map.insert(task_id, TaskDependencies::default());
            map
        },
        retry_configs: IndexMap::new(),
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        inputs: serde_json::json!({}),
        outputs: None,
        error_message: None,
    };

    // Register workflow
    let request = flow_raft_raft::types::Request::CreateWorkflow {
        workflow: workflow.clone(),
    };
    app.create_workflow(request).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Create handler that fails twice then succeeds
    let attempt_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let handler = FailingHandler {
        attempt_count: attempt_count.clone(),
        max_attempts: 2,
    };

    // First execution should fail
    let result = executor
        .execute_task(workflow_id, task_id, &handler, serde_json::json!({}))
        .await;
    assert!(result.is_ok());
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify task is in Failed state (retryable)
    let workflow = executor
        .state_machine()
        .get_workflow(&workflow_id)
        .await
        .unwrap();
    let execution = workflow.executions.get(&task_id);
    assert!(execution.is_some());
    assert!(matches!(execution.unwrap().state, TaskState::Failed { .. }));

    // Second execution should also fail
    let result = executor
        .execute_task(workflow_id, task_id, &handler, serde_json::json!({}))
        .await;
    assert!(result.is_ok());
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Third execution should succeed
    let result = executor
        .execute_task(workflow_id, task_id, &handler, serde_json::json!({}))
        .await;
    assert!(result.is_ok());
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify task is now completed
    let workflow = executor
        .state_machine()
        .get_workflow(&workflow_id)
        .await
        .unwrap();
    let execution = workflow.executions.get(&task_id);
    assert!(execution.is_some());
    assert!(matches!(execution.unwrap().state, TaskState::Completed));
}

#[tokio::test]
async fn test_executor_workflow_cancellation() {
    let (executor, app, _node) = create_test_executor_with_app().await;

    let workflow_id = WorkflowId::default();
    let task_id = TaskId::default();

    let workflow = WorkflowSnapshot {
        workflow_id,
        state: WorkflowState::Running,
        task_definitions: {
            let mut map = IndexMap::new();
            map.insert(
                task_id,
                TaskDefinition {
                    id: task_id,
                    name: "test_task".to_string(),
                    handler: "test_handler".to_string(),
                    inputs: HashSet::new(),
                    outputs: HashSet::new(),
                    timeout_secs: None,
                },
            );
            map
        },
        executions: IndexMap::new(),
        dependencies: {
            let mut map = IndexMap::new();
            map.insert(task_id, TaskDependencies::default());
            map
        },
        retry_configs: IndexMap::new(),
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        inputs: serde_json::json!({}),
        outputs: None,
        error_message: None,
    };

    // Register workflow
    let request = flow_raft_raft::types::Request::CreateWorkflow {
        workflow: workflow.clone(),
    };
    app.create_workflow(request).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Cancel workflow
    let mut cancelled_workflow = workflow.clone();
    cancelled_workflow.state = WorkflowState::Cancelled;
    cancelled_workflow.completed_at = Some(Utc::now());
    let cancel_request = flow_raft_raft::types::Request::CancelWorkflow {
        workflow_id,
        workflow: cancelled_workflow,
    };
    app.create_workflow(cancel_request).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify no tasks are ready (workflow is cancelled)
    let ready = executor.get_ready_tasks(&workflow_id).await;
    assert!(
        ready.is_empty(),
        "No tasks should be ready when workflow is cancelled"
    );
}
