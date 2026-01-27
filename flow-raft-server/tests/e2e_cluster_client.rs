//! E2E test: 3-node Raft cluster, gRPC with executor+watcher, client trigger → watch → result.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use flow_raft_api::WorkflowDef;
use flow_raft_api::client::FlowRaftClient;
use flow_raft_api::graph::{TypedGraphBuilder, node};
use flow_raft_core::TaskId;
use flow_raft_raft::executor::TaskHandler;
use flow_raft_server::grpc::run_grpc_on_cluster;
use flow_raft_server::handlers::HandlerRegistry;
use flow_raft_server::launch_raft_cluster;
use serde_json::json;

struct NopHandler;

impl TaskHandler for NopHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        _inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Ok(json!({"ok": true}))
    }
}

#[tokio::test]
async fn e2e_three_node_grpc_client_trigger_watch_result() {
    let node_ids = [1u64, 2, 3];
    let handle = launch_raft_cluster(&node_ids)
        .await
        .expect("launch cluster");

    let mut builder = TypedGraphBuilder::new("e2e_workflow");
    let nop = |_: ()| Ok::<_, String>(());
    builder.add_node("n1", node(nop), None).set_root("n1");
    let typed = builder.build().expect("build graph");
    let workflow_def: WorkflowDef = typed.workflow_def("e2e").expect("workflow_def");
    let workflow_id = workflow_def.workflow_id;

    let handler_name = workflow_def
        .graph
        .nodes
        .values()
        .next()
        .map(|n| n.handler.clone())
        .unwrap_or_else(|| "fn_0".to_string());

    let registry = Arc::new(HandlerRegistry::new());
    registry
        .register_handler(
            workflow_id,
            handler_name,
            Arc::new(NopHandler) as Arc<dyn TaskHandler>,
        )
        .await;

    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (join, bound_addr) = run_grpc_on_cluster(&handle, registry, vec![workflow_def], addr)
        .await
        .expect("run gRPC");

    let endpoint = format!("http://{}", bound_addr);
    let client = FlowRaftClient::new(endpoint).with_timeout(Duration::from_secs(30));

    let exec_id = client
        .trigger_workflow_by_id(workflow_id, json!({}))
        .await
        .expect("trigger_workflow_by_id");

    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        if std::time::Instant::now() > deadline {
            join.abort();
            let _ = join.await;
            panic!("timeout waiting for workflow completion");
        }
        match client.get_workflow_status(exec_id).await {
            Ok(flow_raft_api::client::WorkflowStatus::Completed { outputs: _ }) => break,
            Ok(flow_raft_api::client::WorkflowStatus::Failed { error }) => {
                join.abort();
                let _ = join.await;
                panic!("workflow failed: {:?}", error);
            }
            Ok(flow_raft_api::client::WorkflowStatus::Cancelled) => {
                join.abort();
                let _ = join.await;
                panic!("workflow cancelled");
            }
            Ok(flow_raft_api::client::WorkflowStatus::Pending)
            | Ok(flow_raft_api::client::WorkflowStatus::Running) => {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Err(e) => {
                join.abort();
                let _ = join.await;
                panic!("get_workflow_status failed: {:?}", e);
            }
        }
    }

    join.abort();
    let _ = join.await;
}
