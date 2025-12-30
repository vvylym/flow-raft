//! Application layer for FlowRaft
//!
//! Provides high-level workflow operations that interact with Raft.

use std::sync::Arc;

use openraft::Raft;
use openraft::RaftTypeConfig;
use openraft::error::ClientWriteError;
use openraft::error::RaftError;

use crate::core::WorkflowId;
use crate::raft::storage::StateMachineStore;
use crate::raft::types::{Request, Response, TypeConfig};

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
    ) -> Option<crate::core::WorkflowSnapshot> {
        self.state_machine.get_workflow(workflow_id).await
    }

    /// Get all workflows
    pub async fn get_all_workflows(&self) -> std::collections::BTreeMap<WorkflowId, crate::core::WorkflowSnapshot> {
        self.state_machine.get_all_workflows().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{WorkflowSnapshot, WorkflowState};
    use chrono::Utc;
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

    // Note: Full integration tests will be in integration test files
    // These are placeholder tests for the structure
}
