//! CLI interface for FlowRaft
//!
//! Provides command-line interface for launching nodes and managing workflows.

// Note: clap is in dev-dependencies, so this module is only available when building binaries
// For library usage, use the launcher functions directly
#[cfg(feature = "cli")]
use clap::{Parser, Subcommand};
use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::node::config::{NodeConfig, NodeMode};
use crate::node::launcher::{launch_follower, launch_leader};
use flow_raft_raft::network::MemoryNetworkFactory;
use flow_raft_raft::types::NodeId;

/// FlowRaft CLI
#[cfg(feature = "cli")]
#[derive(Parser, Debug)]
#[command(name = "flowraft")]
#[command(about = "FlowRaft - A distributed workflow engine", long_about = None)]
pub struct Cli {
    /// Command to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// CLI commands
#[cfg(feature = "cli")]
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Node management commands
    Node {
        /// Node command to execute
        #[command(subcommand)]
        command: NodeCommands,
    },
    /// Workflow management commands
    Workflow {
        /// Workflow command to execute
        #[command(subcommand)]
        command: WorkflowCommands,
    },
}

/// Node management commands
#[cfg(feature = "cli")]
#[derive(Subcommand, Debug)]
pub enum NodeCommands {
    /// Start a node
    Start {
        /// Node ID
        #[arg(long)]
        node_id: u64,
        /// Node mode (leader or follower)
        #[arg(long, default_value = "auto")]
        mode: String,
        /// Storage path (optional, uses in-memory if not specified)
        #[arg(long)]
        storage_path: Option<PathBuf>,
    },
    /// Join a node to a cluster
    Join {
        /// Node ID
        #[arg(long)]
        node_id: u64,
        /// Cluster node IDs (comma-separated)
        #[arg(long)]
        cluster: String,
    },
}

/// Workflow management commands
#[cfg(feature = "cli")]
#[derive(Subcommand, Debug)]
pub enum WorkflowCommands {
    /// Define a workflow from a file
    Define {
        /// Workflow definition file path
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
    /// Trigger a workflow execution
    Trigger {
        /// Workflow ID
        #[arg(long)]
        workflow_id: String,
        /// Input JSON file or JSON string
        #[arg(long)]
        input: Option<String>,
    },
}

/// Parses node mode from string
fn parse_node_mode(mode: &str) -> Result<NodeMode, String> {
    match mode.to_lowercase().as_str() {
        "leader" => Ok(NodeMode::Leader),
        "follower" => Ok(NodeMode::Follower),
        "auto" => Ok(NodeMode::Auto),
        _ => Err(format!(
            "Invalid node mode: {}. Must be 'leader', 'follower', or 'auto'",
            mode
        )),
    }
}

/// Parses cluster nodes from comma-separated string
fn parse_cluster_nodes(cluster: &str) -> Result<BTreeSet<NodeId>, String> {
    cluster
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<NodeId>()
                .map_err(|e| format!("Invalid node ID '{}': {}", s, e))
        })
        .collect()
}

/// Handles node start command
pub async fn handle_node_start(
    node_id: u64,
    mode: String,
    storage_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let node_mode = parse_node_mode(&mode)?;
    let config = NodeConfig::new(node_id, node_mode);
    let config = if let Some(path) = storage_path {
        config.with_storage_path(path)
    } else {
        config
    };

    let network = MemoryNetworkFactory::new();
    let _node = launch_leader(config, network).await?;

    println!("Node {} started in {:?} mode", node_id, node_mode);
    println!("Press Ctrl+C to stop");

    // Keep the node running
    tokio::signal::ctrl_c().await?;
    println!("\nShutting down node...");

    Ok(())
}

/// Handles node join command
pub async fn handle_node_join(
    node_id: u64,
    cluster: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let cluster_nodes = parse_cluster_nodes(&cluster)?;
    let config = NodeConfig::new(node_id, NodeMode::Follower);
    let network = MemoryNetworkFactory::new();
    let _node = launch_follower(config, network, cluster_nodes.clone()).await?;

    println!(
        "Node {} joined cluster with nodes: {:?}",
        node_id, cluster_nodes
    );
    println!("Press Ctrl+C to stop");

    // Keep the node running
    tokio::signal::ctrl_c().await?;
    println!("\nShutting down node...");

    Ok(())
}

/// Handles workflow define command
///
/// Reads a workflow definition from a JSON file and registers it via gRPC.
/// The JSON file should contain a workflow definition that can be parsed by the gRPC service.
pub async fn handle_workflow_define(file: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    use flow_raft_proto::proto::flow_raft_service_client::FlowRaftServiceClient;
    use flow_raft_proto::proto::*;
    use std::fs;
    use tonic::Request;

    // Read workflow definition from file
    let content =
        fs::read_to_string(&file).map_err(|e| format!("Failed to read file {:?}: {}", file, e))?;

    // Validate JSON (but don't parse into WorkflowDef since it doesn't implement Deserialize)
    serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|e| format!("Invalid JSON in workflow definition: {}", e))?;

    // Extract workflow name from JSON (if present)
    let workflow_name = serde_json::from_str::<serde_json::Value>(&content)
        .ok()
        .and_then(|v| {
            v.get("name")
                .and_then(|n| n.as_str().map(|s| s.to_string()))
        })
        .unwrap_or_else(|| {
            file.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("workflow")
                .to_string()
        });

    // Connect to gRPC server (default to localhost:50051)
    let endpoint =
        std::env::var("FLOWRAFT_ENDPOINT").unwrap_or_else(|_| "http://localhost:50051".to_string());

    let mut client = FlowRaftServiceClient::connect(endpoint.clone())
        .await
        .map_err(|e| format!("Failed to connect to server at {}: {}", endpoint, e))?;

    // Call define_workflow RPC with the JSON string
    let request = DefineWorkflowRequest {
        name: workflow_name,
        definition: content,
    };

    let response = client
        .define_workflow(Request::new(request))
        .await
        .map_err(|e| format!("gRPC error: {}", e))?
        .into_inner();

    println!("✓ Workflow defined successfully!");
    println!("  Workflow ID: {}", response.workflow_id);
    println!("  Name: {}", response.name);
    println!("  Status: {}", response.status);

    Ok(())
}

/// Handles workflow trigger command
///
/// Triggers a workflow execution with optional inputs via gRPC.
pub async fn handle_workflow_trigger(
    workflow_id: String,
    input: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use flow_raft_proto::proto::flow_raft_service_client::FlowRaftServiceClient;
    use flow_raft_proto::proto::*;
    use std::fs;
    use tonic::Request;

    // Parse inputs
    let inputs_json = match input {
        Some(input_str) => {
            // Check if it's a file path or JSON string
            if input_str.starts_with('{') || input_str.starts_with('[') {
                // It's a JSON string
                input_str
            } else {
                // Assume it's a file path
                fs::read_to_string(&input_str)
                    .map_err(|e| format!("Failed to read input file {:?}: {}", input_str, e))?
            }
        }
        None => "{}".to_string(),
    };

    // Validate JSON
    serde_json::from_str::<serde_json::Value>(&inputs_json)
        .map_err(|e| format!("Invalid JSON in inputs: {}", e))?;

    // Connect to gRPC server (default to localhost:50051)
    let endpoint =
        std::env::var("FLOWRAFT_ENDPOINT").unwrap_or_else(|_| "http://localhost:50051".to_string());

    let mut client = FlowRaftServiceClient::connect(endpoint.clone())
        .await
        .map_err(|e| format!("Failed to connect to server at {}: {}", endpoint, e))?;

    // Call trigger_workflow RPC
    let request = TriggerWorkflowRequest {
        workflow_id: workflow_id.clone(),
        inputs: Some(inputs_json),
    };

    let response = client
        .trigger_workflow(Request::new(request))
        .await
        .map_err(|e| format!("gRPC error: {}", e))?
        .into_inner();

    println!("✓ Workflow triggered successfully!");
    println!("  Workflow ID: {}", response.workflow_id);
    println!("  Execution ID: {}", response.execution_id);
    println!("  Status: {}", response.status);
    if let Some(error) = response.error {
        println!("  Error: {}", error);
    }

    Ok(())
}

/// Runs the CLI
#[cfg(feature = "cli")]
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Node { command } => match command {
            NodeCommands::Start {
                node_id,
                mode,
                storage_path,
            } => handle_node_start(node_id, mode, storage_path).await,
            NodeCommands::Join { node_id, cluster } => handle_node_join(node_id, cluster).await,
        },
        Commands::Workflow { command } => match command {
            WorkflowCommands::Define { file } => handle_workflow_define(file).await,
            WorkflowCommands::Trigger { workflow_id, input } => {
                handle_workflow_trigger(workflow_id, input).await
            }
        },
    }
}
