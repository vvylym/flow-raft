//! Test helpers and tests for Raft cluster.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::time::Duration;

    use flow_raft_core::{WorkflowId, WorkflowSnapshot, WorkflowState};
    use flow_raft_raft::command::WorkflowCommandBuilder;
    use flow_raft_raft::types::NodeId;
    use tokio::time::sleep;

    use crate::node::launcher::NodeLaunchError;

    use super::super::launch_raft_cluster;

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
    async fn launch_raft_cluster_three_nodes_forms_cluster() {
        let node_ids: Vec<NodeId> = vec![1, 2, 3];
        let handle = launch_raft_cluster(&node_ids).await.unwrap();
        assert_eq!(handle.node_count(), 3);

        let leader = handle.wait_for_leader(Duration::from_secs(10)).await;
        assert!(leader.is_some(), "a leader should be elected");
    }

    #[tokio::test]
    async fn launch_raft_cluster_replication_leader_sees_workflow() {
        let node_ids: Vec<NodeId> = vec![1, 2, 3];
        let handle = launch_raft_cluster(&node_ids).await.unwrap();

        let leader_app = handle
            .wait_for_leader(Duration::from_secs(10))
            .await
            .expect("leader should be elected");

        let workflow_id = WorkflowId::default();
        let snapshot = test_workflow_snapshot(workflow_id);
        let request = WorkflowCommandBuilder::create_workflow(snapshot);
        leader_app
            .create_workflow(request)
            .await
            .expect("create_workflow should succeed");

        sleep(Duration::from_millis(200)).await;

        let w = leader_app.get_workflow(&workflow_id).await;
        assert!(w.is_some(), "leader should see the workflow after create");
        assert_eq!(w.unwrap().workflow_id, workflow_id);
    }

    #[tokio::test]
    async fn launch_raft_cluster_replication_visible_on_all_nodes() {
        let node_ids: Vec<NodeId> = vec![1, 2, 3];
        let handle = launch_raft_cluster(&node_ids).await.unwrap();
        let leader_app = handle
            .wait_for_leader(Duration::from_secs(10))
            .await
            .expect("leader elected");
        let workflow_id = WorkflowId::default();
        let request = WorkflowCommandBuilder::create_workflow(test_workflow_snapshot(workflow_id));
        leader_app.create_workflow(request).await.unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let apps = handle.node_apps();
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
            sleep(Duration::from_millis(100)).await;
        }
        for (nid, app) in &apps {
            let w = app.get_workflow(&workflow_id).await;
            assert!(w.is_some(), "node {} should see workflow", nid);
        }
    }

    #[tokio::test]
    async fn launch_raft_cluster_empty_nodes_err() {
        let result = launch_raft_cluster(&[]).await;
        assert!(matches!(result, Err(NodeLaunchError::Config(_))));
    }

    #[tokio::test]
    async fn launch_raft_cluster_wait_for_leader_timeout_returns_none() {
        let node_ids: Vec<NodeId> = vec![1, 2, 3];
        let handle = launch_raft_cluster(&node_ids).await.unwrap();
        let _ = handle.wait_for_leader(Duration::ZERO).await;
        assert_eq!(handle.node_count(), 3);
    }

    #[tokio::test]
    async fn launch_raft_cluster_leader_app_returns_none_when_no_leader_yet() {
        let node_ids: Vec<NodeId> = vec![1, 2];
        let handle = launch_raft_cluster(&node_ids).await.unwrap();
        let _ = handle.leader_app().await;
        let apps = handle.node_apps();
        assert_eq!(apps.len(), 2);
    }
}
