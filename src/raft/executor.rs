//! Workflow executor
//!
//! Bridges Raft state machine with task execution, allowing any node to execute tasks.

use std::sync::Arc;

use openraft::Raft;
use openraft::RaftTypeConfig;
use openraft::error::ClientWriteError;
use openraft::error::RaftError;

use crate::core::{TaskExecution, TaskId, WorkflowId};
use crate::raft::command::WorkflowCommandBuilder;
use crate::raft::storage::StateMachineStore;
use crate::raft::types::TypeConfig;

/// Task execution handler trait
pub trait TaskHandler: Send + Sync {
    /// Execute a task and return the result
    fn execute(
        &self,
        task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
}

/// Workflow executor that can run on any node
pub struct WorkflowExecutor {
    raft: Arc<Raft<TypeConfig>>,
    state_machine: StateMachineStore<TypeConfig>,
    node_id: u64,
}

impl WorkflowExecutor {
    /// Create a new workflow executor
    pub fn new(
        raft: Arc<Raft<TypeConfig>>,
        state_machine: StateMachineStore<TypeConfig>,
        node_id: u64,
    ) -> Self {
        Self {
            raft,
            state_machine,
            node_id,
        }
    }

    /// Get the state machine store (for accessing workflow state)
    pub fn state_machine(&self) -> &StateMachineStore<TypeConfig> {
        &self.state_machine
    }

    /// Get the Raft instance
    pub fn raft(&self) -> &Arc<Raft<TypeConfig>> {
        &self.raft
    }

    /// Get ready tasks for a workflow (any node can read state)
    ///
    /// Returns tasks that:
    /// - Have all prerequisites completed
    /// - Are not already in a terminal state (completed, permanently failed, cancelled)
    /// - Are not currently running or scheduled
    pub async fn get_ready_tasks(&self, workflow_id: &WorkflowId) -> Vec<TaskId> {
        let Some(workflow) = self.state_machine.get_workflow(workflow_id).await else {
            return Vec::new();
        };

        // Only return ready tasks if workflow is running
        if !matches!(workflow.state, crate::core::WorkflowState::Running) {
            return Vec::new();
        }

        // Collect completed tasks (terminal states)
        use std::collections::HashSet;
        let completed: HashSet<TaskId> = workflow
            .executions
            .iter()
            .filter(|(_, exec)| exec.state.is_terminal())
            .map(|(id, _)| *id)
            .collect();

        // Collect tasks that are already running or scheduled (not ready)
        let in_progress: HashSet<TaskId> = workflow
            .executions
            .iter()
            .filter(|(_, exec)| {
                matches!(
                    exec.state,
                    crate::core::TaskState::Running | crate::core::TaskState::Scheduled
                )
            })
            .map(|(id, _)| *id)
            .collect();

        // Get all task IDs from definitions
        use indexmap::IndexMap;
        let tasks: IndexMap<TaskId, ()> = workflow
            .task_definitions
            .keys()
            .copied()
            .map(|id| (id, ()))
            .collect();

        // Use the ready_tasks function from dag utils
        // Note: ready_tasks is re-exported from core::dag
        use crate::core::ready_tasks;
        let mut ready = ready_tasks(&tasks, &workflow.dependencies, &completed);

        // Filter out tasks that are already in progress
        ready.retain(|task_id| !in_progress.contains(task_id));
        ready
    }

    /// Execute a task and update state via Raft
    ///
    /// This method:
    /// 1. Executes the task using the provided handler
    /// 2. Updates the task execution state via Raft consensus
    /// 3. Returns an error if the Raft write fails
    pub async fn execute_task(
        &self,
        workflow_id: WorkflowId,
        task_id: TaskId,
        handler: &dyn TaskHandler,
        inputs: serde_json::Value,
    ) -> Result<
        (),
        RaftError<
            <TypeConfig as RaftTypeConfig>::NodeId,
            ClientWriteError<
                <TypeConfig as RaftTypeConfig>::NodeId,
                <TypeConfig as RaftTypeConfig>::Node,
            >,
        >,
    > {
        let started_at = chrono::Utc::now();

        // Execute task locally
        let result = handler.execute(task_id, inputs.clone());

        let completed_at = chrono::Utc::now();

        // Update task execution state via Raft
        let (state, last_error, outputs) = match result {
            Ok(output) => (
                crate::core::TaskState::Completed,
                None,
                Some(output),
            ),
            Err(e) => (
                crate::core::TaskState::Failed {
                    error_message: Some(e.clone()),
                    failure_kind: crate::core::FailureKind::Retryable,
                },
                Some(e),
                None,
            ),
        };

        let execution = TaskExecution {
            task_id,
            state,
            attempts: 1,
            started_at: Some(started_at),
            completed_at: Some(completed_at),
            last_error,
            outputs,
        };

        let request =
            WorkflowCommandBuilder::update_task_execution(workflow_id, task_id, execution);

        self.raft.client_write(request).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{TaskDefinition, TaskDependencies, TaskState, WorkflowSnapshot, WorkflowState};
    use crate::raft::config::default_config;
    use crate::raft::network::MemoryNetworkFactory;
    use crate::raft::storage::{LogStore, StateMachineStore};
    use crate::raft::types::NodeId;
    use chrono::Utc;
    use indexmap::IndexMap;
    use openraft::Raft;
    use std::collections::{BTreeSet, HashSet};
    use std::sync::Arc;

    struct MockTaskHandler {
        should_succeed: bool,
        error_message: Option<String>,
    }

    impl MockTaskHandler {
        fn new() -> Self {
            Self {
                should_succeed: true,
                error_message: None,
            }
        }

        fn with_failure(error: String) -> Self {
            Self {
                should_succeed: false,
                error_message: Some(error),
            }
        }
    }

    impl TaskHandler for MockTaskHandler {
        fn execute(
            &self,
            task_id: TaskId,
            inputs: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            if self.should_succeed {
                Ok(serde_json::json!({
                    "task_id": task_id.to_string(),
                    "inputs": inputs,
                    "result": "success"
                }))
            } else {
                Err(self.error_message.clone().unwrap_or_else(|| "Task failed".to_string()))
            }
        }
    }

    async fn create_test_executor() -> WorkflowExecutor {
        let node_id = 1;
        let config = Arc::new(default_config().validate().unwrap());
        let network = MemoryNetworkFactory::new();
        let log_store = LogStore::default();
        let state_machine = StateMachineStore::default();

        let raft = Raft::new(node_id, config, network, log_store, state_machine.clone())
            .await
            .unwrap();

        WorkflowExecutor::new(Arc::new(raft), state_machine, node_id)
    }

    fn create_test_workflow_snapshot(
        workflow_id: WorkflowId,
        state: WorkflowState,
    ) -> WorkflowSnapshot {
        let started_at = if matches!(state, WorkflowState::Running) {
            Some(Utc::now())
        } else {
            None
        };
        WorkflowSnapshot {
            workflow_id,
            state,
            task_definitions: IndexMap::new(),
            executions: IndexMap::new(),
            dependencies: IndexMap::new(),
            retry_configs: IndexMap::new(),
            created_at: Utc::now(),
            started_at,
            completed_at: None,
            inputs: serde_json::json!({}),
            outputs: None,
            error_message: None,
        }
    }

    #[tokio::test]
    async fn test_executor_new() {
        let executor = create_test_executor().await;
        assert_eq!(executor.node_id, 1);
    }

    #[tokio::test]
    async fn test_get_ready_tasks_no_workflow() {
        let executor = create_test_executor().await;
        let workflow_id = WorkflowId::default();

        let ready = executor.get_ready_tasks(&workflow_id).await;
        assert!(ready.is_empty());
    }

    #[tokio::test]
    async fn test_get_ready_tasks_workflow_not_running() {
        let executor = create_test_executor().await;
        
        // Initialize single-node cluster
        let node_ids: BTreeSet<NodeId> = [1u64].into_iter().collect();
        executor.raft.initialize(node_ids).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let workflow_id = WorkflowId::default();
        let mut workflow = create_test_workflow_snapshot(workflow_id, WorkflowState::Draft);

        // Add a task
        let task_id = TaskId::default();
        workflow.task_definitions.insert(
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

        // Store workflow in state machine
        let request = crate::raft::types::Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        executor.raft.client_write(request).await.unwrap();

        // Wait a bit for state machine to apply
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let ready = executor.get_ready_tasks(&workflow_id).await;
        assert!(ready.is_empty(), "Should return empty for non-running workflow");
    }

    #[tokio::test]
    async fn test_get_ready_tasks_single_task_no_dependencies() {
        let executor = create_test_executor().await;
        
        // Initialize single-node cluster
        let node_ids: BTreeSet<NodeId> = [1u64].into_iter().collect();
        executor.raft.initialize(node_ids).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let workflow_id = WorkflowId::default();
        let mut workflow = create_test_workflow_snapshot(workflow_id, WorkflowState::Running);

        // Add a task with no dependencies
        let task_id = TaskId::default();
        workflow.task_definitions.insert(
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

        // Store workflow in state machine
        let request = crate::raft::types::Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        executor.raft.client_write(request).await.unwrap();

        // Wait a bit for state machine to apply
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let ready = executor.get_ready_tasks(&workflow_id).await;
        assert_eq!(ready.len(), 1);
        assert!(ready.contains(&task_id));
    }

    #[tokio::test]
    async fn test_get_ready_tasks_with_dependencies() {
        let executor = create_test_executor().await;
        
        // Initialize single-node cluster
        let node_ids: BTreeSet<NodeId> = [1u64].into_iter().collect();
        executor.raft.initialize(node_ids).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let workflow_id = WorkflowId::default();
        let mut workflow = create_test_workflow_snapshot(workflow_id, WorkflowState::Running);

        let task1_id = TaskId::default();
        let task2_id = TaskId::default();

        // Add task1 with no dependencies
        workflow.task_definitions.insert(
            task1_id,
            TaskDefinition {
                id: task1_id,
                name: "task1".to_string(),
                handler: "test_handler".to_string(),
                inputs: HashSet::new(),
                outputs: HashSet::new(),
                timeout_secs: None,
            },
        );

        // Add task2 that depends on task1
        let mut deps = TaskDependencies::default();
        deps.add_prerequisite(task1_id);
        workflow.task_definitions.insert(
            task2_id,
            TaskDefinition {
                id: task2_id,
                name: "task2".to_string(),
                handler: "test_handler".to_string(),
                inputs: HashSet::new(),
                outputs: HashSet::new(),
                timeout_secs: None,
            },
        );

        workflow.dependencies.insert(task1_id, TaskDependencies::default());
        workflow.dependencies.insert(task2_id, deps);

        // Store workflow in state machine
        let request = crate::raft::types::Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        executor.raft.client_write(request).await.unwrap();

        // Wait a bit for state machine to apply
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let ready = executor.get_ready_tasks(&workflow_id).await;
        assert_eq!(ready.len(), 1);
        assert!(ready.contains(&task1_id));
        assert!(!ready.contains(&task2_id), "task2 should not be ready until task1 completes");
    }

    #[tokio::test]
    async fn test_get_ready_tasks_with_completed_prerequisite() {
        let executor = create_test_executor().await;
        
        // Initialize single-node cluster
        let node_ids: BTreeSet<NodeId> = [1u64].into_iter().collect();
        executor.raft.initialize(node_ids).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let workflow_id = WorkflowId::default();
        let mut workflow = create_test_workflow_snapshot(workflow_id, WorkflowState::Running);

        let task1_id = TaskId::default();
        let task2_id = TaskId::default();

        // Add tasks
        workflow.task_definitions.insert(
            task1_id,
            TaskDefinition {
                id: task1_id,
                name: "task1".to_string(),
                handler: "test_handler".to_string(),
                inputs: HashSet::new(),
                outputs: HashSet::new(),
                timeout_secs: None,
            },
        );

        let mut deps = TaskDependencies::default();
        deps.add_prerequisite(task1_id);
        workflow.task_definitions.insert(
            task2_id,
            TaskDefinition {
                id: task2_id,
                name: "task2".to_string(),
                handler: "test_handler".to_string(),
                inputs: HashSet::new(),
                outputs: HashSet::new(),
                timeout_secs: None,
            },
        );

        workflow.dependencies.insert(task1_id, TaskDependencies::default());
        workflow.dependencies.insert(task2_id, deps);

        // Mark task1 as completed
        workflow.executions.insert(
            task1_id,
            TaskExecution {
                task_id: task1_id,
                state: TaskState::Completed,
                attempts: 1,
                started_at: Some(Utc::now()),
                completed_at: Some(Utc::now()),
                last_error: None,
                outputs: Some(serde_json::json!({"result": "done"})),
            },
        );

        // Store workflow in state machine
        let request = crate::raft::types::Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        executor.raft.client_write(request).await.unwrap();

        // Wait a bit for state machine to apply
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let ready = executor.get_ready_tasks(&workflow_id).await;
        assert_eq!(ready.len(), 1);
        assert!(ready.contains(&task2_id), "task2 should be ready after task1 completes");
    }

    #[tokio::test]
    async fn test_get_ready_tasks_excludes_running_tasks() {
        let executor = create_test_executor().await;
        
        // Initialize single-node cluster
        let node_ids: BTreeSet<NodeId> = [1u64].into_iter().collect();
        executor.raft.initialize(node_ids).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let workflow_id = WorkflowId::default();
        let mut workflow = create_test_workflow_snapshot(workflow_id, WorkflowState::Running);

        let task1_id = TaskId::default();
        let task2_id = TaskId::default();

        // Add tasks
        workflow.task_definitions.insert(
            task1_id,
            TaskDefinition {
                id: task1_id,
                name: "task1".to_string(),
                handler: "test_handler".to_string(),
                inputs: HashSet::new(),
                outputs: HashSet::new(),
                timeout_secs: None,
            },
        );

        workflow.task_definitions.insert(
            task2_id,
            TaskDefinition {
                id: task2_id,
                name: "task2".to_string(),
                handler: "test_handler".to_string(),
                inputs: HashSet::new(),
                outputs: HashSet::new(),
                timeout_secs: None,
            },
        );

        workflow.dependencies.insert(task1_id, TaskDependencies::default());
        workflow.dependencies.insert(task2_id, TaskDependencies::default());

        // Mark task1 as running
        workflow.executions.insert(
            task1_id,
            TaskExecution {
                task_id: task1_id,
                state: TaskState::Running,
                attempts: 1,
                started_at: Some(Utc::now()),
                completed_at: None,
                last_error: None,
                outputs: None,
            },
        );

        // Store workflow in state machine
        let request = crate::raft::types::Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        executor.raft.client_write(request).await.unwrap();

        // Wait a bit for state machine to apply
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let ready = executor.get_ready_tasks(&workflow_id).await;
        assert_eq!(ready.len(), 1);
        assert!(!ready.contains(&task1_id), "task1 should not be ready (it's running)");
        assert!(ready.contains(&task2_id), "task2 should be ready");
    }

    #[tokio::test]
    async fn test_execute_task_success() {
        let executor = create_test_executor().await;
        let workflow_id = WorkflowId::default();
        let task_id = TaskId::default();

        // Initialize single-node cluster
        use std::collections::BTreeSet;
        executor.raft.initialize([1u64].into_iter().collect::<BTreeSet<_>>()).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Create workflow first
        let workflow = create_test_workflow_snapshot(workflow_id, WorkflowState::Running);
        let request = crate::raft::types::Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        executor.raft.client_write(request).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let handler = MockTaskHandler::new();
        let inputs = serde_json::json!({"param": "value"});

        let result = executor
            .execute_task(workflow_id, task_id, &handler, inputs)
            .await;

        assert!(result.is_ok(), "Task execution should succeed");

        // Verify task state was updated
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let updated_workflow = executor.state_machine.get_workflow(&workflow_id).await;
        assert!(updated_workflow.is_some());
        let workflow = updated_workflow.unwrap();
        let execution = workflow.executions.get(&task_id);
        assert!(execution.is_some());
        assert!(matches!(execution.unwrap().state, TaskState::Completed));
    }

    #[tokio::test]
    async fn test_execute_task_failure() {
        let executor = create_test_executor().await;
        let workflow_id = WorkflowId::default();
        let task_id = TaskId::default();

        // Initialize single-node cluster
        use std::collections::BTreeSet;
        executor.raft.initialize([1u64].into_iter().collect::<BTreeSet<_>>()).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Create workflow first
        let workflow = create_test_workflow_snapshot(workflow_id, WorkflowState::Running);
        let request = crate::raft::types::Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        executor.raft.client_write(request).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let handler = MockTaskHandler::with_failure("Task execution failed".to_string());
        let inputs = serde_json::json!({"param": "value"});

        let result = executor
            .execute_task(workflow_id, task_id, &handler, inputs)
            .await;

        assert!(result.is_ok(), "Raft write should succeed even if task fails");

        // Verify task state was updated to Failed
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let updated_workflow = executor.state_machine.get_workflow(&workflow_id).await;
        assert!(updated_workflow.is_some());
        let workflow = updated_workflow.unwrap();
        let execution = workflow.executions.get(&task_id);
        assert!(execution.is_some());
        match &execution.unwrap().state {
            TaskState::Failed { error_message, failure_kind } => {
                assert_eq!(error_message.as_ref().unwrap(), "Task execution failed");
                assert_eq!(*failure_kind, crate::core::FailureKind::Retryable);
            }
            _ => panic!("Expected Failed state"),
        }
    }

    #[tokio::test]
    async fn test_execute_task_with_custom_inputs() {
        let executor = create_test_executor().await;
        let workflow_id = WorkflowId::default();
        let task_id = TaskId::default();

        // Initialize single-node cluster
        use std::collections::BTreeSet;
        executor.raft.initialize([1u64].into_iter().collect::<BTreeSet<_>>()).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Create workflow first
        let workflow = create_test_workflow_snapshot(workflow_id, WorkflowState::Running);
        let request = crate::raft::types::Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        executor.raft.client_write(request).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let handler = MockTaskHandler::new();
        let inputs = serde_json::json!({
            "param1": "value1",
            "param2": 42,
            "param3": true
        });

        let result = executor
            .execute_task(workflow_id, task_id, &handler, inputs.clone())
            .await;

        assert!(result.is_ok());

        // Verify outputs contain the inputs
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let updated_workflow = executor.state_machine.get_workflow(&workflow_id).await;
        let workflow = updated_workflow.unwrap();
        let execution = workflow.executions.get(&task_id).unwrap();
        assert!(execution.outputs.is_some());
        let outputs = execution.outputs.as_ref().unwrap();
        assert_eq!(outputs.get("inputs"), Some(&inputs));
    }
}
