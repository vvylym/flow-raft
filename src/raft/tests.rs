//! Integration tests for Raft cluster setup
//!
//! Tests single-node and multi-node cluster scenarios.

use std::collections::BTreeSet;
use std::time::Duration;

use openraft::Config;
use tokio::time::sleep;

use crate::core::{WorkflowId, WorkflowSnapshot, WorkflowState};
use crate::raft::app::FlowRaftApp;
use crate::raft::command::WorkflowCommandBuilder;
use crate::raft::config::RaftConfig;
use crate::raft::network::MemoryNetworkFactory;
use crate::raft::node::FlowRaftNode;
use crate::raft::storage::{LogStore, StateMachineStore};
use crate::raft::types::NodeId;

/// Helper to create a test Raft configuration with fast election
fn test_config() -> RaftConfig {
    Config {
        heartbeat_interval: 100,      // Fast heartbeats for testing
        election_timeout_min: 300,     // Fast election for testing
        election_timeout_max: 500,     // Fast election for testing
        ..Default::default()
    }
}

/// Helper to create a single node
async fn create_node(
    node_id: NodeId,
    config: RaftConfig,
    network: MemoryNetworkFactory,
) -> Result<FlowRaftNode, openraft::error::RaftError<NodeId, openraft::error::Infallible>> {
    let log_store = LogStore::default();
    let state_machine = StateMachineStore::default();
    FlowRaftNode::new(node_id, config, network, log_store, state_machine).await
}

/// Helper to wait for a node to become leader
async fn wait_for_leader(
    node: &FlowRaftNode,
    timeout: Duration,
) -> Result<(), String> {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        // is_leader() returns Result<(), Error> - Ok(()) means it's the leader
        if node.raft.ensure_linearizable().await.is_ok() {
            return Ok(());
        }
        sleep(Duration::from_millis(50)).await;
    }
    Err("Timeout waiting for leader".to_string())
}

/// Helper to wait for a node to have a leader (either itself or another node)
async fn wait_for_leader_any(
    node: &FlowRaftNode,
    timeout: Duration,
) -> Result<NodeId, String> {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        // Check if this node is the leader
        if node.raft.ensure_linearizable().await.is_ok() {
            return Ok(node.node_id);
        }
        // Check metrics for current leader
        let metrics = node.raft.metrics().borrow().clone();
        if let Some(leader_id) = metrics.current_leader {
            return Ok(leader_id);
        }
        sleep(Duration::from_millis(50)).await;
    }
    Err("Timeout waiting for any leader".to_string())
}

/// Helper to create a test workflow snapshot
fn create_test_workflow(workflow_id: WorkflowId) -> WorkflowSnapshot {
    WorkflowSnapshot {
        workflow_id,
        state: WorkflowState::Draft,
        task_definitions: indexmap::IndexMap::new(),
        executions: indexmap::IndexMap::new(),
        dependencies: indexmap::IndexMap::new(),
        retry_configs: indexmap::IndexMap::new(),
        created_at: chrono::Utc::now(),
        started_at: None,
        completed_at: None,
        inputs: serde_json::json!({}),
        outputs: None,
        error_message: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_single_node_setup() {
        let node_id = 1;
        let config = test_config();
        let network = MemoryNetworkFactory::new();

        // Create a single node
        let node = create_node(node_id, config, network).await;
        assert!(node.is_ok(), "Failed to create node");

        let node = node.unwrap();
        assert_eq!(node.node_id, node_id);

        // Initialize as single-node cluster
        let init_result = node.initialize_single_node().await;
        assert!(init_result.is_ok(), "Failed to initialize single-node cluster");

        // Wait for the node to become leader (single node cluster)
        let result = wait_for_leader(&node, Duration::from_secs(5)).await;
        assert!(result.is_ok(), "Node should become leader in single-node cluster");
    }

    #[tokio::test]
    async fn test_single_node_create_workflow() {
        let node_id = 1;
        let config = test_config();
        let network = MemoryNetworkFactory::new();

        let node = create_node(node_id, config, network).await.unwrap();

        // Initialize as single-node cluster
        node.initialize_single_node().await.unwrap();

        // Wait for leader
        wait_for_leader(&node, Duration::from_secs(5)).await.unwrap();

        // Create application layer
        let app = FlowRaftApp::new(node.raft.clone(), node.state_machine.clone());

        // Create a workflow
        let workflow_id = WorkflowId::default();
        let workflow = create_test_workflow(workflow_id);
        let request = WorkflowCommandBuilder::create_workflow(workflow);

        let response = app.create_workflow(request).await;
        assert!(response.is_ok(), "Should be able to create workflow");

        // Verify workflow was created
        let retrieved = app.get_workflow(&workflow_id).await;
        assert!(retrieved.is_some(), "Workflow should exist");
        assert_eq!(retrieved.unwrap().workflow_id, workflow_id);
    }

    #[tokio::test]
    async fn test_cluster_setup_4_nodes() {
        let config = test_config();
        
        // Create 4 nodes with shared network
        let network = MemoryNetworkFactory::new();
        let mut nodes = Vec::new();

        for node_id in 1..=4 {
            let node = create_node(node_id, config.clone(), network.clone()).await;
            assert!(node.is_ok(), "Failed to create node {}", node_id);
            nodes.push(node.unwrap());
        }

        // Initialize cluster with all 4 nodes (on the first node)
        let node_ids: BTreeSet<NodeId> = (1..=4).collect();
        let init_result = nodes[0].initialize_cluster(node_ids).await;
        assert!(init_result.is_ok(), "Failed to initialize cluster");

        // Wait for one of them to become leader
        let mut leader_found = false;
        for node in &nodes {
            if let Ok(_) = wait_for_leader_any(node, Duration::from_secs(10)).await {
                leader_found = true;
                break;
            }
        }
        assert!(leader_found, "A leader should be elected in 4-node cluster");
    }

    #[tokio::test]
    async fn test_cluster_replication() {
        let config = test_config();
        let network = MemoryNetworkFactory::new();
        let mut nodes = Vec::new();

        // Create 4 nodes
        for node_id in 1..=4 {
            let node = create_node(node_id, config.clone(), network.clone()).await.unwrap();
            nodes.push(node);
        }

        // Initialize cluster with all 4 nodes
        let node_ids: BTreeSet<NodeId> = (1..=4).collect();
        nodes[0].initialize_cluster(node_ids).await.unwrap();

        // Wait for leader election
        let mut leader_node: Option<&FlowRaftNode> = None;
        for node in &nodes {
            if let Ok(_leader_id) = wait_for_leader_any(node, Duration::from_secs(10)).await {
                // Check if this node is the leader
                if node.raft.ensure_linearizable().await.is_ok() {
                    leader_node = Some(node);
                    break;
                }
            }
        }

        assert!(leader_node.is_some(), "A leader should be elected");

        let leader = leader_node.unwrap();
        let app = FlowRaftApp::new(leader.raft.clone(), leader.state_machine.clone());

        // Create a workflow on the leader
        let workflow_id = WorkflowId::default();
        let workflow = create_test_workflow(workflow_id);
        let request = WorkflowCommandBuilder::create_workflow(workflow);

        let response = app.create_workflow(request).await;
        assert!(response.is_ok(), "Leader should be able to create workflow");

        // Wait a bit for replication
        sleep(Duration::from_millis(500)).await;

        // Verify all nodes have the workflow (eventually consistent)
        // Note: In a real implementation, we'd wait for committed entries
        // For now, we check that the leader has it
        let retrieved = app.get_workflow(&workflow_id).await;
        assert!(retrieved.is_some(), "Leader should have the workflow");
        assert_eq!(retrieved.unwrap().workflow_id, workflow_id);
    }

    #[tokio::test]
    async fn test_cluster_multiple_workflows() {
        let config = test_config();
        let network = MemoryNetworkFactory::new();
        let mut nodes = Vec::new();

        // Create 4 nodes
        for node_id in 1..=4 {
            let node = create_node(node_id, config.clone(), network.clone()).await.unwrap();
            nodes.push(node);
        }

        // Initialize cluster with all 4 nodes
        let node_ids: BTreeSet<NodeId> = (1..=4).collect();
        nodes[0].initialize_cluster(node_ids).await.unwrap();

        // Wait for leader
        let mut leader_node: Option<&FlowRaftNode> = None;
        for node in &nodes {
            if node.raft.ensure_linearizable().await.is_ok() {
                leader_node = Some(node);
                break;
            }
        }

        // If no leader yet, wait for one
        if leader_node.is_none() {
            for node in &nodes {
                if let Ok(_) = wait_for_leader_any(node, Duration::from_secs(10)).await {
                    if node.raft.ensure_linearizable().await.is_ok() {
                        leader_node = Some(node);
                        break;
                    }
                }
            }
        }

        assert!(leader_node.is_some(), "A leader should be elected");

        let leader = leader_node.unwrap();
        let app = FlowRaftApp::new(leader.raft.clone(), leader.state_machine.clone());

        // Create multiple workflows
        let workflow_ids: Vec<WorkflowId> = (0..5).map(|_| WorkflowId::default()).collect();

        for workflow_id in &workflow_ids {
            let workflow = create_test_workflow(*workflow_id);
            let request = WorkflowCommandBuilder::create_workflow(workflow);
            let response = app.create_workflow(request).await;
            assert!(response.is_ok(), "Should be able to create workflow");
        }

        // Wait for replication
        sleep(Duration::from_millis(500)).await;

        // Verify all workflows exist on leader
        for workflow_id in &workflow_ids {
            let retrieved = app.get_workflow(workflow_id).await;
            assert!(retrieved.is_some(), "Workflow {} should exist", workflow_id);
        }
    }
}
