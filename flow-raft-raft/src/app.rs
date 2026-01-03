//! Application layer for FlowRaft
//!
//! Provides high-level workflow operations that interact with Raft.

use std::sync::Arc;

use openraft::Raft;
use openraft::RaftTypeConfig;
use openraft::error::ClientWriteError;
use openraft::error::RaftError;

use crate::storage::StateMachineStore;
use crate::types::{Request, Response, TypeConfig};
use flow_raft_core::WorkflowId;

pub mod builder;
pub use builder::{AppBuilderError, FlowRaftAppBuilder};

/// FlowRaft application layer
pub struct FlowRaftApp {
    raft: Arc<Raft<TypeConfig>>,
    state_machine: StateMachineStore<TypeConfig>,
}

impl FlowRaftApp {
    /// Create a new FlowRaft application
    pub fn new(raft: Arc<Raft<TypeConfig>>, state_machine: StateMachineStore<TypeConfig>) -> Self {
        Self {
            raft,
            state_machine,
        }
    }

    /// Create a new workflow
    #[tracing::instrument(level = "info", skip(self, request))]
    #[allow(clippy::type_complexity)]
    pub async fn create_workflow(
        &self,
        request: Request,
    ) -> Result<
        Response,
        RaftError<
            <TypeConfig as RaftTypeConfig>::NodeId,
            ClientWriteError<
                <TypeConfig as RaftTypeConfig>::NodeId,
                <TypeConfig as RaftTypeConfig>::Node,
            >,
        >,
    > {
        let result = self.raft.client_write(request).await?;
        Ok(result.data)
    }

    /// Get workflow by ID
    pub async fn get_workflow(
        &self,
        workflow_id: &WorkflowId,
    ) -> Option<flow_raft_core::WorkflowSnapshot> {
        self.state_machine.get_workflow(workflow_id).await
    }

    /// Get all workflows
    pub async fn get_all_workflows(
        &self,
    ) -> std::collections::BTreeMap<WorkflowId, flow_raft_core::WorkflowSnapshot> {
        self.state_machine.get_all_workflows().await
    }

    /// Get the Raft instance
    ///
    /// This is useful for creating executors or accessing Raft state.
    pub fn raft(&self) -> &Arc<Raft<TypeConfig>> {
        &self.raft
    }

    /// Get the state machine store
    ///
    /// This is useful for creating executors or directly accessing workflow state.
    pub fn state_machine(&self) -> &StateMachineStore<TypeConfig> {
        &self.state_machine
    }

    /// Register a workflow definition
    ///
    /// This stores the workflow definition for later execution.
    /// The workflow is converted to a WorkflowSnapshot and stored via Raft.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Graph conversion fails
    /// - Workflow state transitions fail
    /// - Raft write fails
    pub async fn register_workflow(
        &self,
        workflow_def: flow_raft_api::WorkflowDef,
    ) -> Result<WorkflowId, String> {
        use flow_raft_api::graph::converter::graph_to_workflow;
        use flow_raft_core::WorkflowSnapshot;

        // Convert graph to workflow snapshot
        let workflow_id = workflow_def.workflow_id;
        let draft_workflow = graph_to_workflow(
            workflow_def.graph,
            workflow_id,
            workflow_def.default_retry_config,
            serde_json::json!({}),
        )
        .map_err(|e| format!("Failed to convert graph to workflow: {}", e))?;

        let scheduled = draft_workflow
            .schedule()
            .map_err(|e| format!("Failed to schedule workflow: {:?}", e))?;

        let running = scheduled
            .start()
            .map_err(|e| format!("Failed to start workflow: {:?}", e))?;

        let snapshot = WorkflowSnapshot::from_workflow(&running);
        let request = Request::CreateWorkflow { workflow: snapshot };

        self.create_workflow(request)
            .await
            .map_err(|e| format!("Failed to create workflow via Raft: {:?}", e))?;
        Ok(workflow_id)
    }

    /// Register multiple workflows in batch
    pub async fn register_workflows(
        &self,
        workflows: Vec<flow_raft_api::WorkflowDef>,
    ) -> Result<Vec<WorkflowId>, String> {
        let mut workflow_ids = Vec::with_capacity(workflows.len());
        for workflow_def in workflows {
            let workflow_id = self.register_workflow(workflow_def).await?;
            workflow_ids.push(workflow_id);
        }
        Ok(workflow_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use flow_raft_core::{WorkflowSnapshot, WorkflowState};
    use indexmap::IndexMap;
    use rstest::*;

    #[fixture]
    fn test_workflow_snapshot() -> WorkflowSnapshot {
        WorkflowSnapshot {
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
        }
    }

    /// Helper to create a test app with Raft setup
    async fn create_test_app() -> (FlowRaftApp, crate::node::FlowRaftNode) {
        use crate::config::default_config;
        use crate::network::MemoryNetworkFactory;
        use crate::node::FlowRaftNode;
        use crate::storage::LogStore;
        use crate::types::NodeId;

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
    async fn test_create_workflow() {
        let (app, _node) = create_test_app().await;
        let snapshot = test_workflow_snapshot();
        let request = Request::CreateWorkflow {
            workflow: snapshot.clone(),
        };

        let result = app.create_workflow(request).await;
        assert!(result.is_ok());

        let retrieved = app.get_workflow(&snapshot.workflow_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().workflow_id, snapshot.workflow_id);
    }

    #[tokio::test]
    async fn test_get_workflow_not_found() {
        let (app, _node) = create_test_app().await;
        let workflow_id = WorkflowId::default();

        let retrieved = app.get_workflow(&workflow_id).await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_get_all_workflows() {
        let (app, _node) = create_test_app().await;

        // Initially empty
        let workflows = app.get_all_workflows().await;
        assert!(workflows.is_empty());

        // Create a workflow
        let snapshot = test_workflow_snapshot();
        let request = Request::CreateWorkflow {
            workflow: snapshot.clone(),
        };
        app.create_workflow(request).await.unwrap();

        // Now should have one workflow
        let workflows = app.get_all_workflows().await;
        assert_eq!(workflows.len(), 1);
        assert!(workflows.contains_key(&snapshot.workflow_id));
    }

    #[tokio::test]
    async fn test_register_workflow() {
        let (app, _node) = create_test_app().await;

        use flow_raft_api::WorkflowDef;
        use flow_raft_api::graph::GraphBuilder;
        use flow_raft_core::RetryConfig;

        let workflow = WorkflowDef::from_graph(
            "test",
            GraphBuilder::new("test")
                .add_node("task1", "handler1", vec![], vec![], None)
                .set_root("task1")
                .build()
                .unwrap(),
            RetryConfig::default(),
        );

        let result = app.register_workflow(workflow.clone()).await;
        assert!(result.is_ok());
        let workflow_id = result.unwrap();

        // Verify workflow was registered
        let retrieved = app.get_workflow(&workflow_id).await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_register_workflows() {
        let (app, _node) = create_test_app().await;

        use flow_raft_api::WorkflowDef;
        use flow_raft_api::graph::GraphBuilder;
        use flow_raft_core::RetryConfig;

        let workflow1 = WorkflowDef::from_graph(
            "test1",
            GraphBuilder::new("test1")
                .add_node("task1", "handler1", vec![], vec![], None)
                .set_root("task1")
                .build()
                .unwrap(),
            RetryConfig::default(),
        );

        let workflow2 = WorkflowDef::from_graph(
            "test2",
            GraphBuilder::new("test2")
                .add_node("task2", "handler2", vec![], vec![], None)
                .set_root("task2")
                .build()
                .unwrap(),
            RetryConfig::default(),
        );

        let result = app
            .register_workflows(vec![workflow1.clone(), workflow2.clone()])
            .await;
        assert!(result.is_ok());
        let workflow_ids = result.unwrap();
        assert_eq!(workflow_ids.len(), 2);

        // Verify both workflows were registered
        let workflows = app.get_all_workflows().await;
        assert_eq!(workflows.len(), 2);
    }
}
