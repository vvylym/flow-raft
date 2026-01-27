//! CLI handlers for flowraft: workflow and cluster operations via gRPC.
//!
//! Used by the flowraft binary and, for define/trigger, by node::cli.

use std::path::PathBuf;

use flow_raft_proto::proto::flow_raft_service_client::FlowRaftServiceClient;
use flow_raft_proto::proto::*;
use tonic::Request;

/// Workflow define: read JSON from file, call DefineWorkflow RPC.
pub async fn handle_workflow_define(
    server: &str,
    file: PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content = std::fs::read_to_string(&file)
        .map_err(|e| format!("Failed to read file {:?}: {}", file, e))?;
    serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|e| format!("Invalid JSON in workflow definition: {}", e))?;

    let workflow_name = serde_json::from_str::<serde_json::Value>(&content)
        .ok()
        .and_then(|v| v.get("name").and_then(|n| n.as_str().map(String::from)))
        .unwrap_or_else(|| {
            file.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("workflow")
                .to_string()
        });

    let mut client = FlowRaftServiceClient::connect(server.to_string())
        .await
        .map_err(|e| format!("Failed to connect to {}: {}", server, e))?;

    let req = DefineWorkflowRequest {
        name: workflow_name,
        definition: content,
    };
    let res = client
        .define_workflow(Request::new(req))
        .await
        .map_err(|e| format!("gRPC: {}", e))?
        .into_inner();

    println!("✓ Workflow defined");
    println!("  Workflow ID: {}", res.workflow_id);
    println!("  Name: {}", res.name);
    println!("  Status: {}", res.status);
    Ok(())
}

/// Workflow trigger: optional input JSON or file, call TriggerWorkflow RPC.
pub async fn handle_workflow_trigger(
    server: &str,
    workflow_id: String,
    input: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let inputs_json = match input {
        Some(s) if s.starts_with('{') || s.starts_with('[') => s,
        Some(path) => std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read input {:?}: {}", path, e))?,
        None => "{}".to_string(),
    };
    serde_json::from_str::<serde_json::Value>(&inputs_json)
        .map_err(|e| format!("Invalid JSON in inputs: {}", e))?;

    let mut client = FlowRaftServiceClient::connect(server.to_string())
        .await
        .map_err(|e| format!("Failed to connect to {}: {}", server, e))?;

    let req = TriggerWorkflowRequest {
        workflow_id: workflow_id.clone(),
        inputs: Some(inputs_json),
    };
    let res = client
        .trigger_workflow(Request::new(req))
        .await
        .map_err(|e| format!("gRPC: {}", e))?
        .into_inner();

    println!("✓ Workflow triggered");
    println!("  Workflow ID: {}", res.workflow_id);
    println!("  Execution ID: {}", res.execution_id);
    println!("  Status: {}", res.status);
    if let Some(e) = res.error {
        println!("  Error: {}", e);
    }
    Ok(())
}

/// Workflow get: call GetWorkflow RPC and print status.
pub async fn handle_workflow_get(
    server: &str,
    workflow_id: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut client = FlowRaftServiceClient::connect(server.to_string())
        .await
        .map_err(|e| format!("Failed to connect to {}: {}", server, e))?;

    let req = GetWorkflowRequest {
        workflow_id: workflow_id.clone(),
    };
    let res = client
        .get_workflow(Request::new(req))
        .await
        .map_err(|e| format!("gRPC: {}", e))?
        .into_inner();

    println!("Workflow ID: {}", res.workflow_id);
    println!("State: {}", res.state);
    if let Some(e) = res.error_message {
        println!("Error: {}", e);
    }
    if let Some(o) = res.outputs {
        println!("Outputs: {}", o);
    }
    println!("Created: {}", res.created_at);
    if let Some(s) = res.started_at {
        println!("Started: {}", s);
    }
    if let Some(c) = res.completed_at {
        println!("Completed: {}", c);
    }
    for t in res.tasks {
        println!(
            "  Task {}: {} (attempts: {})",
            t.task_id, t.state, t.attempts
        );
    }
    Ok(())
}

/// Workflow list: call ListWorkflows RPC and print table.
pub async fn handle_workflow_list(
    server: &str,
    limit: i32,
    offset: i32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut client = FlowRaftServiceClient::connect(server.to_string())
        .await
        .map_err(|e| format!("Failed to connect to {}: {}", server, e))?;

    let req = ListWorkflowsRequest {
        filter: None,
        limit,
        offset,
    };
    let res = client
        .list_workflows(Request::new(req))
        .await
        .map_err(|e| format!("gRPC: {}", e))?
        .into_inner();

    println!("Total: {}", res.total);
    for w in res.workflows {
        println!(
            "  {}  {}  tasks {}/{}  failed {}  created {}",
            w.workflow_id, w.state, w.completed_tasks, w.total_tasks, w.failed_tasks, w.created_at
        );
    }
    Ok(())
}

/// Workflow cancel: call CancelWorkflow RPC.
pub async fn handle_workflow_cancel(
    server: &str,
    workflow_id: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut client = FlowRaftServiceClient::connect(server.to_string())
        .await
        .map_err(|e| format!("Failed to connect to {}: {}", server, e))?;

    let req = CancelWorkflowRequest {
        workflow_id: workflow_id.clone(),
    };
    let res = client
        .cancel_workflow(Request::new(req))
        .await
        .map_err(|e| format!("gRPC: {}", e))?
        .into_inner();

    println!(
        "✓ Workflow cancelled: {}  state: {}",
        res.workflow_id, res.state
    );
    Ok(())
}

/// Cluster status: call GetNodeStatus RPC (node_id=1 as default).
pub async fn handle_cluster_status(
    server: &str,
    node_id: u64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut client = FlowRaftServiceClient::connect(server.to_string())
        .await
        .map_err(|e| format!("Failed to connect to {}: {}", server, e))?;

    let req = GetNodeStatusRequest { node_id };
    let res = client
        .get_node_status(Request::new(req))
        .await
        .map_err(|e| format!("gRPC: {}", e))?
        .into_inner();

    println!("Node ID: {}", res.node_id);
    println!("Mode: {}", res.mode);
    println!("Leader: {}", res.is_leader);
    println!("Cluster nodes: {:?}", res.cluster_nodes);
    Ok(())
}
