//! Raft state machine implementation
//!
//! Provides state machine for workflow state replication.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures::lock::Mutex;
use openraft::EntryPayload;
use openraft::ErrorSubject;
use openraft::ErrorVerb;
use openraft::LogId;
use openraft::OptionalSend;
use openraft::RaftLogId;
use openraft::RaftSnapshotBuilder;
use openraft::RaftTypeConfig;
use openraft::SnapshotMeta;
use openraft::StorageError;
use openraft::StorageIOError;
use openraft::StoredMembership;
use openraft::storage::RaftStateMachine;
use openraft::storage::Snapshot;
use serde::{Deserialize, Serialize};

use crate::types::{Request, Response, TypeConfig};
use flow_raft_core::{WorkflowId, WorkflowSnapshot};

/// Stored snapshot data
#[derive(Debug)]
pub struct StoredSnapshot<C: RaftTypeConfig> {
    /// Snapshot metadata
    pub meta: SnapshotMeta<C::NodeId, C::Node>,
    /// Serialized snapshot data
    pub data: Vec<u8>,
}

/// Data contained in the Raft state machine
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct StateMachineData {
    /// Workflow state indexed by workflow ID
    pub workflows: BTreeMap<WorkflowId, WorkflowSnapshot>,
}

/// Inner storage for the state machine
#[derive(Debug)]
struct StateMachineStoreInner<C: RaftTypeConfig> {
    last_applied_log: Option<LogId<C::NodeId>>,
    last_membership: StoredMembership<C::NodeId, C::Node>,
    state_machine: StateMachineData,
    snapshot_idx: AtomicU64,
    current_snapshot: Option<StoredSnapshot<C>>,
}

impl<C: RaftTypeConfig> Default for StateMachineStoreInner<C> {
    fn default() -> Self {
        Self {
            last_applied_log: None,
            last_membership: StoredMembership::default(),
            state_machine: StateMachineData::default(),
            snapshot_idx: AtomicU64::new(0),
            current_snapshot: None,
        }
    }
}

impl<C: RaftTypeConfig> StateMachineStoreInner<C> {
    fn next_snapshot_idx(&self) -> u64 {
        self.snapshot_idx.fetch_add(1, Ordering::Relaxed) + 1
    }
}

/// State machine store for workflow state
#[derive(Debug)]
pub struct StateMachineStore<C: RaftTypeConfig>(Arc<Mutex<StateMachineStoreInner<C>>>);

impl<C: RaftTypeConfig> Default for StateMachineStore<C> {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(StateMachineStoreInner::default())))
    }
}

impl<C: RaftTypeConfig> Clone for StateMachineStore<C> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<C: RaftTypeConfig> StateMachineStore<C> {
    /// Get workflow by ID
    pub async fn get_workflow(&self, workflow_id: &WorkflowId) -> Option<WorkflowSnapshot> {
        let inner = self.0.lock().await;
        inner.state_machine.workflows.get(workflow_id).cloned()
    }

    /// Get all workflows
    pub async fn get_all_workflows(&self) -> BTreeMap<WorkflowId, WorkflowSnapshot> {
        let inner = self.0.lock().await;
        inner.state_machine.workflows.clone()
    }
}

impl RaftSnapshotBuilder<TypeConfig> for StateMachineStore<TypeConfig> {
    #[tracing::instrument(level = "trace", skip(self))]
    async fn build_snapshot(
        &mut self,
    ) -> Result<Snapshot<TypeConfig>, StorageError<<TypeConfig as RaftTypeConfig>::NodeId>> {
        let mut inner = self.0.lock().await;

        let data = serde_json::to_vec(&inner.state_machine.workflows).map_err(
            |e| -> StorageError<<TypeConfig as RaftTypeConfig>::NodeId> {
                StorageIOError::new(
                    ErrorSubject::StateMachine,
                    ErrorVerb::Write,
                    openraft::AnyError::error(e.to_string()),
                )
                .into()
            },
        )?;

        let snapshot_idx = inner.next_snapshot_idx();
        let snapshot_id = if let Some(last) = inner.last_applied_log {
            format!("{}-{}-{}", last.leader_id, last.index, snapshot_idx)
        } else {
            format!("--{}", snapshot_idx)
        };

        let meta = SnapshotMeta::<
            <TypeConfig as RaftTypeConfig>::NodeId,
            <TypeConfig as RaftTypeConfig>::Node,
        > {
            last_log_id: inner.last_applied_log,
            last_membership: inner.last_membership.clone(),
            snapshot_id,
        };

        let snapshot = StoredSnapshot {
            meta: meta.clone(),
            data: data.clone(),
        };

        inner.current_snapshot = Some(snapshot);

        Ok(Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(data)),
        })
    }
}

impl RaftStateMachine<TypeConfig> for StateMachineStore<TypeConfig> {
    type SnapshotBuilder = Self;

    async fn applied_state(
        &mut self,
    ) -> Result<
        (
            Option<LogId<<TypeConfig as RaftTypeConfig>::NodeId>>,
            StoredMembership<
                <TypeConfig as RaftTypeConfig>::NodeId,
                <TypeConfig as RaftTypeConfig>::Node,
            >,
        ),
        StorageError<<TypeConfig as RaftTypeConfig>::NodeId>,
    > {
        let inner = self.0.lock().await;
        #[allow(clippy::needless_borrows_for_generic_args)]
        Ok((inner.last_applied_log, inner.last_membership.clone()))
    }

    #[tracing::instrument(level = "trace", skip(self, entries))]
    async fn apply<I>(
        &mut self,
        entries: I,
    ) -> Result<Vec<Response>, StorageError<<TypeConfig as RaftTypeConfig>::NodeId>>
    where
        I: IntoIterator<Item = <TypeConfig as RaftTypeConfig>::Entry> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        let mut inner = self.0.lock().await;
        let mut responses = Vec::new();

        for entry in entries {
            // TypeConfig::Entry is openraft::Entry<TypeConfig> which has public fields
            // Use RaftLogId trait to get log_id since we can't access fields directly on generic type
            let log_id = *<<TypeConfig as RaftTypeConfig>::Entry as RaftLogId<
                <TypeConfig as RaftTypeConfig>::NodeId,
            >>::get_log_id(&entry);
            tracing::debug!(log_id = ?log_id, "applying entry to state machine");

            inner.last_applied_log = Some(log_id);

            // Access payload field directly - Entry has public payload field
            let response = match &entry.payload {
                EntryPayload::Blank => Response::none(),
                EntryPayload::Normal(req) => match req {
                    Request::CreateWorkflow { workflow } => {
                        inner
                            .state_machine
                            .workflows
                            .insert(workflow.workflow_id, workflow.clone());
                        Response::WorkflowCreated {
                            workflow_id: workflow.workflow_id,
                        }
                    }
                    Request::TransitionWorkflow {
                        workflow_id,
                        workflow,
                    } => {
                        inner
                            .state_machine
                            .workflows
                            .insert(*workflow_id, workflow.clone());
                        Response::WorkflowTransitioned {
                            workflow_id: *workflow_id,
                        }
                    }
                    Request::UpdateTaskExecution {
                        workflow_id,
                        task_id,
                        execution,
                    } => {
                        if let Some(workflow) = inner.state_machine.workflows.get_mut(workflow_id) {
                            workflow.executions.insert(*task_id, execution.clone());
                        }
                        Response::TaskExecutionUpdated {
                            workflow_id: *workflow_id,
                            task_id: *task_id,
                        }
                    }
                    Request::CancelWorkflow {
                        workflow_id,
                        workflow,
                    } => {
                        inner
                            .state_machine
                            .workflows
                            .insert(*workflow_id, workflow.clone());
                        Response::WorkflowCancelled {
                            workflow_id: *workflow_id,
                        }
                    }
                },
                EntryPayload::Membership(mem) => {
                    inner.last_membership = StoredMembership::new(Some(log_id), mem.clone());
                    Response::none()
                }
            };

            responses.push(response);
        }
        Ok(responses)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<Box<Cursor<Vec<u8>>>, StorageError<<TypeConfig as RaftTypeConfig>::NodeId>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    #[tracing::instrument(level = "trace", skip(self, snapshot))]
    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<
            <TypeConfig as RaftTypeConfig>::NodeId,
            <TypeConfig as RaftTypeConfig>::Node,
        >,
        snapshot: Box<Cursor<Vec<u8>>>,
    ) -> Result<(), StorageError<<TypeConfig as RaftTypeConfig>::NodeId>> {
        tracing::info!(
            { snapshot_size = snapshot.get_ref().len() },
            "installing snapshot"
        );

        #[allow(clippy::needless_borrows_for_generic_args)]
        let new_snapshot = StoredSnapshot {
            meta: meta.clone(),
            data: snapshot.clone().into_inner(),
        };

        #[allow(clippy::needless_borrows_for_generic_args)]
        let workflows: BTreeMap<WorkflowId, WorkflowSnapshot> =
            serde_json::from_slice(&new_snapshot.data).map_err(
                |e| -> StorageError<<TypeConfig as RaftTypeConfig>::NodeId> {
                    StorageIOError::new(
                        ErrorSubject::StateMachine,
                        ErrorVerb::Read,
                        openraft::AnyError::error(e.to_string()),
                    )
                    .into()
                },
            )?;

        let mut inner = self.0.lock().await;
        #[allow(clippy::needless_borrows_for_generic_args)]
        {
            inner.last_applied_log = meta.last_log_id;
            inner.last_membership = meta.last_membership.clone();
        }
        inner.state_machine = StateMachineData { workflows };
        inner.current_snapshot = Some(new_snapshot);

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<TypeConfig>>, StorageError<<TypeConfig as RaftTypeConfig>::NodeId>>
    {
        let inner = self.0.lock().await;
        match &inner.current_snapshot {
            Some(snapshot) => {
                let data = snapshot.data.clone();
                Ok(Some(Snapshot {
                    meta: snapshot.meta.clone(),
                    snapshot: Box::new(Cursor::new(data)),
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TypeConfig;
    use chrono::Utc;
    use flow_raft_core::{TaskExecution, TaskId, TaskState, WorkflowState};
    use indexmap::IndexMap;
    use openraft::Entry;
    use openraft::EntryPayload;
    use openraft::LeaderId;
    use openraft::LogId;
    use rstest::*;

    #[fixture]
    fn test_state_machine() -> StateMachineStore<TypeConfig> {
        StateMachineStore::default()
    }

    fn create_test_workflow_snapshot(id: WorkflowId) -> WorkflowSnapshot {
        WorkflowSnapshot {
            workflow_id: id,
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

    fn create_test_entry(index: u64, payload: EntryPayload<TypeConfig>) -> Entry<TypeConfig> {
        Entry {
            log_id: LogId::new(LeaderId::new(1, 1), index),
            payload,
        }
    }

    #[tokio::test]
    async fn test_state_machine_default() {
        let mut sm = test_state_machine();
        let (log_id, membership) = sm.applied_state().await.unwrap();
        assert!(log_id.is_none());
        assert!(membership.membership().nodes().next().is_none());
    }

    #[tokio::test]
    async fn test_create_workflow() {
        let mut sm = test_state_machine();
        let workflow_id = WorkflowId::default();
        let workflow = create_test_workflow_snapshot(workflow_id);

        let request = Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        let entry = create_test_entry(1, EntryPayload::Normal(request));
        let entries = vec![entry];
        sm.apply(entries).await.unwrap();

        let retrieved = sm.get_workflow(&workflow_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().workflow_id, workflow_id);
    }

    #[tokio::test]
    async fn test_transition_workflow() {
        let mut sm = test_state_machine();
        let workflow_id = WorkflowId::default();
        let mut workflow = create_test_workflow_snapshot(workflow_id);
        workflow.state = WorkflowState::Draft;

        // Create workflow first
        let create_request = Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        let entry1 = create_test_entry(1, EntryPayload::Normal(create_request));
        let entries1 = vec![entry1];
        sm.apply(entries1).await.unwrap();

        // Transition workflow
        workflow.state = WorkflowState::Running;
        let transition_request = Request::TransitionWorkflow {
            workflow_id,
            workflow: workflow.clone(),
        };
        let entry2 = create_test_entry(2, EntryPayload::Normal(transition_request));
        let entries2 = vec![entry2];
        sm.apply(entries2).await.unwrap();

        let retrieved = sm.get_workflow(&workflow_id).await.unwrap();
        assert!(matches!(retrieved.state, WorkflowState::Running));
    }

    #[tokio::test]
    async fn test_update_task_execution() {
        let mut sm = test_state_machine();
        let workflow_id = WorkflowId::default();
        let task_id = TaskId::default();
        let workflow = create_test_workflow_snapshot(workflow_id);

        // Create workflow first
        let create_request = Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        let entry1 = create_test_entry(1, EntryPayload::Normal(create_request));
        let entries1 = vec![entry1];
        sm.apply(entries1).await.unwrap();

        // Update task execution
        let execution = TaskExecution {
            task_id,
            state: TaskState::Running,
            attempts: 1,
            started_at: Some(Utc::now()),
            completed_at: None,
            last_error: None,
            outputs: None,
        };

        let update_request = Request::UpdateTaskExecution {
            workflow_id,
            task_id,
            execution: execution.clone(),
        };
        let entry2 = create_test_entry(2, EntryPayload::Normal(update_request));
        let entries2 = vec![entry2];
        sm.apply(entries2).await.unwrap();

        let retrieved = sm.get_workflow(&workflow_id).await.unwrap();
        assert!(retrieved.executions.contains_key(&task_id));
        assert_eq!(
            retrieved.executions.get(&task_id).unwrap().state,
            execution.state
        );
    }

    #[tokio::test]
    async fn test_cancel_workflow() {
        let mut sm = test_state_machine();
        let workflow_id = WorkflowId::default();
        let mut workflow = create_test_workflow_snapshot(workflow_id);
        workflow.state = WorkflowState::Running;

        // Create workflow first
        let create_request = Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        let entry1 = create_test_entry(1, EntryPayload::Normal(create_request));
        let entries1 = vec![entry1];
        sm.apply(entries1).await.unwrap();

        // Cancel workflow
        workflow.state = WorkflowState::Cancelled;
        let cancel_request = Request::CancelWorkflow {
            workflow_id,
            workflow: workflow.clone(),
        };
        let entry2 = create_test_entry(2, EntryPayload::Normal(cancel_request));
        let entries2 = vec![entry2];
        sm.apply(entries2).await.unwrap();

        let retrieved = sm.get_workflow(&workflow_id).await.unwrap();
        assert!(matches!(retrieved.state, WorkflowState::Cancelled));
    }

    #[tokio::test]
    async fn test_apply_blank_entry() {
        let mut sm = test_state_machine();
        let entry = create_test_entry(1, EntryPayload::Blank);
        let entries = vec![entry];
        sm.apply(entries).await.unwrap();

        let (log_id, _) = sm.applied_state().await.unwrap();
        assert_eq!(log_id.map(|id| id.index), Some(1));
    }

    #[tokio::test]
    async fn test_apply_multiple_entries() {
        let mut sm = test_state_machine();
        let workflow_id1 = WorkflowId::default();
        let workflow_id2 = WorkflowId::default();
        let workflow1 = create_test_workflow_snapshot(workflow_id1);
        let workflow2 = create_test_workflow_snapshot(workflow_id2);

        let request1 = Request::CreateWorkflow {
            workflow: workflow1.clone(),
        };
        let request2 = Request::CreateWorkflow {
            workflow: workflow2.clone(),
        };

        let entry1 = create_test_entry(1, EntryPayload::Normal(request1));
        let entry2 = create_test_entry(2, EntryPayload::Normal(request2));
        let entries = vec![entry1, entry2];
        sm.apply(entries).await.unwrap();

        let all_workflows = sm.get_all_workflows().await;
        assert_eq!(all_workflows.len(), 2);
        assert!(all_workflows.contains_key(&workflow_id1));
        assert!(all_workflows.contains_key(&workflow_id2));
    }

    #[tokio::test]
    async fn test_build_snapshot() {
        let mut sm = test_state_machine();
        let workflow_id = WorkflowId::default();
        let workflow = create_test_workflow_snapshot(workflow_id);

        // Create workflow
        let request = Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        let entry = create_test_entry(1, EntryPayload::Normal(request));
        let entries = vec![entry];
        sm.apply(entries).await.unwrap();

        // Build snapshot
        let snapshot = sm.build_snapshot().await.unwrap();
        assert_eq!(snapshot.meta.last_log_id.map(|id| id.index), Some(1));

        // Verify snapshot data can be deserialized
        let workflows: BTreeMap<WorkflowId, WorkflowSnapshot> =
            serde_json::from_slice(snapshot.snapshot.get_ref()).unwrap();
        assert_eq!(workflows.len(), 1);
        assert!(workflows.contains_key(&workflow_id));
    }

    #[tokio::test]
    async fn test_install_snapshot() {
        let mut sm = test_state_machine();
        let workflow_id = WorkflowId::default();
        let workflow = create_test_workflow_snapshot(workflow_id);

        // Create snapshot data
        let mut workflows = BTreeMap::new();
        workflows.insert(workflow_id, workflow.clone());
        let snapshot_data = serde_json::to_vec(&workflows).unwrap();

        let log_id = LogId::new(LeaderId::new(1, 1), 10);
        let meta = SnapshotMeta {
            last_log_id: Some(log_id),
            last_membership: StoredMembership::default(),
            snapshot_id: "test-snapshot".to_string(),
        };

        sm.install_snapshot(&meta, Box::new(Cursor::new(snapshot_data)))
            .await
            .unwrap();

        let (applied_log_id, _) = sm.applied_state().await.unwrap();
        assert_eq!(applied_log_id, Some(log_id));

        let retrieved = sm.get_workflow(&workflow_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().workflow_id, workflow_id);
    }

    #[tokio::test]
    async fn test_get_current_snapshot_none() {
        let mut sm = test_state_machine();
        let snapshot = sm.get_current_snapshot().await.unwrap();
        assert!(snapshot.is_none());
    }

    #[tokio::test]
    async fn test_get_current_snapshot_after_build() {
        let mut sm = test_state_machine();
        let workflow_id = WorkflowId::default();
        let workflow = create_test_workflow_snapshot(workflow_id);

        let request = Request::CreateWorkflow {
            workflow: workflow.clone(),
        };
        let entry = create_test_entry(1, EntryPayload::Normal(request));
        let entries = vec![entry];
        sm.apply(entries).await.unwrap();

        sm.build_snapshot().await.unwrap();
        let snapshot = sm.get_current_snapshot().await.unwrap();
        assert!(snapshot.is_some());
        assert_eq!(
            snapshot.unwrap().meta.last_log_id.map(|id| id.index),
            Some(1)
        );
    }
}
