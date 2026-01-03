//! Cluster coordination for FlowRaft
//!
//! Provides cluster management with leader/follower roles and workflow distribution.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use flow_raft_api::WorkflowDef;
use flow_raft_api::graph::converter::graph_to_workflow;
use flow_raft_core::{WorkflowId, WorkflowSnapshot};
#[allow(unused_imports)]
use flow_raft_raft::config::RaftConfig; // Used in function signatures: new_leader, join_cluster
use flow_raft_raft::network::MemoryNetworkFactory;
use flow_raft_raft::storage::{LogStore, StateMachineStore};
use flow_raft_raft::types::NodeId;
use flow_raft_raft::{FlowRaftApp, FlowRaftNode};
use openraft::StoredMembership;
use openraft::storage::RaftStateMachine;
use serde_json::Value;
use thiserror::Error;
use tokio::sync::RwLock;

/// Node role in the cluster
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    /// Leader node coordinates the cluster
    Leader,
    /// Follower node executes workflows
    Follower,
}

/// Cluster status
#[derive(Debug, Clone)]
pub struct ClusterStatus {
    /// Current leader node ID
    pub leader: Option<NodeId>,
    /// All node IDs in the cluster
    pub nodes: BTreeSet<NodeId>,
    /// This node's role
    pub role: NodeRole,
    /// This node's ID
    pub node_id: NodeId,
}

/// Cluster node errors
#[derive(Debug, Error)]
pub enum ClusterError {
    /// Raft error
    #[error("Raft error: {0}")]
    Raft(String),
    /// Workflow not found
    #[error("Workflow not found: {0}")]
    WorkflowNotFound(WorkflowId),
    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Cluster node that can coordinate workflows
pub struct ClusterNode {
    node_id: NodeId,
    role: NodeRole,
    node: FlowRaftNode,
    app: Arc<FlowRaftApp>,
    workflows: Arc<RwLock<HashMap<WorkflowId, WorkflowDef>>>,
    shutdown_flag: Arc<AtomicBool>,
    active_workflows: Arc<RwLock<HashSet<WorkflowId>>>,
}

impl ClusterNode {
    /// Initialize as leader (single node cluster)
    pub async fn new_leader(node_id: NodeId, config: RaftConfig) -> Result<Self, ClusterError> {
        let network = MemoryNetworkFactory::new();
        let log_store = LogStore::default();
        let state_machine = StateMachineStore::default();

        let node = FlowRaftNode::new(node_id, config, network, log_store, state_machine.clone())
            .await
            .map_err(|e| ClusterError::Raft(format!("Failed to create node: {:?}", e)))?;

        node.initialize_single_node()
            .await
            .map_err(|e| ClusterError::Raft(format!("Failed to initialize: {:?}", e)))?;

        let raft = node.raft.clone();
        let app = Arc::new(FlowRaftApp::new(raft, state_machine.clone()));

        Ok(Self {
            node_id,
            role: NodeRole::Leader,
            node,
            app,
            workflows: Arc::new(RwLock::new(HashMap::new())),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            active_workflows: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    /// Join an existing cluster as follower
    pub async fn join_cluster(
        node_id: NodeId,
        config: RaftConfig,
        _leader_address: String,
    ) -> Result<Self, ClusterError> {
        // Note: Currently uses in-memory network for testing. In production,
        // this would establish actual network connections to the leader node
        // using the provided leader_address. The MemoryNetworkFactory is used
        // here for the MVP implementation.
        let network = MemoryNetworkFactory::new();
        let log_store = LogStore::default();
        let state_machine = StateMachineStore::default();

        let node = FlowRaftNode::new(node_id, config, network, log_store, state_machine.clone())
            .await
            .map_err(|e| ClusterError::Raft(format!("Failed to create node: {:?}", e)))?;

        let raft = node.raft.clone();
        let app = Arc::new(FlowRaftApp::new(raft, state_machine.clone()));

        Ok(Self {
            node_id,
            role: NodeRole::Follower,
            node,
            app,
            workflows: Arc::new(RwLock::new(HashMap::new())),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            active_workflows: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    /// Get cluster status
    pub async fn cluster_status(&mut self) -> ClusterStatus {
        // Query Raft for actual cluster membership and leader information
        let metrics = self.node.raft.metrics().borrow().clone();

        // Extract leader ID from metrics
        let leader = metrics.current_leader;

        // Extract cluster membership from state machine
        // The state machine stores the last applied membership
        let nodes: BTreeSet<NodeId> = match self.node.state_machine.applied_state().await {
            Ok((_, membership)) => {
                // Type annotation to help inference
                let membership: StoredMembership<NodeId, openraft::BasicNode> = membership;
                membership
                    .membership()
                    .nodes()
                    .map(|(node_id, _)| *node_id)
                    .collect()
            }
            Err(_) => {
                // Fallback: if we can't get membership, use current node only
                [self.node_id].into_iter().collect()
            }
        };

        // Determine role: check if this node is the leader
        let role = if leader == Some(self.node_id) {
            NodeRole::Leader
        } else {
            NodeRole::Follower
        };

        ClusterStatus {
            leader,
            nodes,
            role,
            node_id: self.node_id,
        }
    }

    /// Register a workflow on this node
    pub async fn register_workflow(
        &self,
        workflow: WorkflowDef,
    ) -> Result<WorkflowId, ClusterError> {
        let workflow_id = workflow.workflow_id;
        let mut workflows = self.workflows.write().await;
        workflows.insert(workflow_id, workflow);
        Ok(workflow_id)
    }

    /// Get list of registered workflows
    pub async fn list_workflows(&self) -> Vec<WorkflowId> {
        let workflows: tokio::sync::RwLockReadGuard<'_, HashMap<WorkflowId, WorkflowDef>> =
            self.workflows.read().await;
        workflows.keys().copied().collect()
    }

    /// Remove a workflow
    pub async fn unregister_workflow(&self, workflow_id: WorkflowId) -> Result<(), ClusterError> {
        let mut workflows: tokio::sync::RwLockWriteGuard<'_, HashMap<WorkflowId, WorkflowDef>> =
            self.workflows.write().await;
        workflows.remove(&workflow_id);
        Ok(())
    }

    /// Submit workflow to cluster (routed to appropriate node)
    pub async fn submit_workflow(
        &self,
        workflow: WorkflowDef,
        input: Value,
    ) -> Result<WorkflowId, ClusterError> {
        // Extract data before moving workflow
        let workflow_id = workflow.workflow_id;
        let graph = workflow.graph.clone();
        let retry_config = workflow.default_retry_config.clone();

        // Register workflow on this node
        self.register_workflow(workflow).await?;

        // Convert workflow to WorkflowSnapshot and create via Raft

        let draft_workflow = graph_to_workflow(graph, workflow_id, retry_config, input)
            .map_err(|e| ClusterError::InvalidConfig(format!("Failed to convert graph: {}", e)))?;

        let scheduled = draft_workflow
            .schedule()
            .map_err(|e| ClusterError::InvalidConfig(format!("Failed to schedule: {}", e)))?;
        let running = scheduled
            .start()
            .map_err(|e| ClusterError::InvalidConfig(format!("Failed to start: {}", e)))?;

        let snapshot = WorkflowSnapshot::from_workflow(&running);
        let request = flow_raft_raft::types::Request::CreateWorkflow { workflow: snapshot };

        // Check if shutdown is in progress
        if self.shutdown_flag.load(Ordering::SeqCst) {
            return Err(ClusterError::InvalidConfig(
                "Node is shutting down".to_string(),
            ));
        }

        // Track active workflow
        self.active_workflows.write().await.insert(workflow_id);

        self.app
            .create_workflow(request)
            .await
            .map_err(|e| ClusterError::Raft(format!("Failed to create workflow: {:?}", e)))?;

        Ok(workflow_id)
    }

    /// Get the Raft app
    pub fn app(&self) -> &Arc<FlowRaftApp> {
        &self.app
    }

    /// Get the node
    pub fn node(&self) -> &FlowRaftNode {
        &self.node
    }

    /// Get node ID
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Get node role
    pub fn role(&self) -> NodeRole {
        self.role
    }

    /// Gracefully shutdown the cluster node
    ///
    /// Records metrics and cleans up resources.
    pub async fn shutdown(&self) -> Result<(), ClusterError> {
        // Set shutdown flag to prevent new operations
        self.shutdown_flag.store(true, Ordering::SeqCst);

        // Wait for active workflows with timeout (30 seconds)
        let timeout = Duration::from_secs(30);
        let start = Instant::now();

        while start.elapsed() < timeout {
            let active_count = self.active_workflows.read().await.len();
            if active_count == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Check if any workflows are still active
        let active_count = self.active_workflows.read().await.len();
        if active_count > 0 {
            return Err(ClusterError::InvalidConfig(format!(
                "Timeout waiting for {} workflows to complete",
                active_count
            )));
        }

        // Shutdown Raft node
        // Note: openraft doesn't have a direct shutdown method, but we can
        // drop the Raft instance which will clean up resources
        // For now, we just mark as shutdown - actual cleanup happens on drop

        Ok(())
    }
}
