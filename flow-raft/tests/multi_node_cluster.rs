//! Integration tests for multi-node cluster using launch_raft_cluster.

use std::time::Duration;

use flow_raft_core::{WorkflowId, WorkflowSnapshot, WorkflowState};
use flow_raft_raft::command::WorkflowCommandBuilder;
use flow_raft_raft::types::NodeId;
use flow_raft_server::raft_cluster::launch_raft_cluster;

fn test_workflow_snapshot(workflow_id: WorkflowId) -> WorkflowSnapshot {
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

#[tokio::test]
async fn test_single_node_cluster() {
    let handle = launch_raft_cluster(&[1]).await.expect("launch");
    assert_eq!(handle.node_count(), 1);
    let leader = handle.wait_for_leader(Duration::from_secs(5)).await;
    assert!(leader.is_some());
}

#[tokio::test]
async fn test_cluster_workflow_replication() {
    let node_ids: Vec<NodeId> = vec![1, 2, 3];
    let handle = launch_raft_cluster(&node_ids).await.expect("launch");
    let leader_app = handle
        .wait_for_leader(Duration::from_secs(10))
        .await
        .expect("leader elected");

    let workflow_id = WorkflowId::default();
    let request = WorkflowCommandBuilder::create_workflow(test_workflow_snapshot(workflow_id));
    leader_app
        .create_workflow(request)
        .await
        .expect("create_workflow");

    let apps = handle.node_apps();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let mut ok = true;
        for (_, app) in &apps {
            if app.get_workflow(&workflow_id).await.is_none() {
                ok = false;
                break;
            }
        }
        if ok {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    for (nid, app) in &apps {
        let w = app.get_workflow(&workflow_id).await;
        assert!(w.is_some(), "node {} should see workflow", nid);
    }
}
