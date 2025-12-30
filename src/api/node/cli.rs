//! CLI interface for FlowRaft
//!
//! Provides command-line interface for launching nodes and managing workflows.

// Note: clap is in dev-dependencies, so this module is only available when building binaries
// For library usage, use the launcher functions directly
#[cfg(feature = "cli")]
use clap::{Parser, Subcommand};
use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::api::node::config::{NodeConfig, NodeMode};
use crate::api::node::launcher::{join_cluster, launch_follower, launch_leader};
use crate::raft::network::MemoryNetworkFactory;
use crate::raft::types::NodeId;

/// FlowRaft CLI
#[cfg(feature = "cli")]
#[derive(Parser, Debug)]
#[command(name = "flowraft")]
#[command(about = "FlowRaft - A distributed workflow engine", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// CLI commands
#[cfg(feature = "cli")]
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Node management commands
    Node {
        #[command(subcommand)]
        command: NodeCommands,
    },
    /// Workflow management commands
    Workflow {
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
        _ => Err(format!("Invalid node mode: {}. Must be 'leader', 'follower', or 'auto'", mode)),
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
    let node = launch_leader(config, network).await?;

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
    let node = launch_follower(config, network, cluster_nodes.clone()).await?;

    println!("Node {} joined cluster with nodes: {:?}", node_id, cluster_nodes);
    println!("Press Ctrl+C to stop");

    // Keep the node running
    tokio::signal::ctrl_c().await?;
    println!("\nShutting down node...");

    Ok(())
}

/// Handles workflow define command
pub async fn handle_workflow_define(
    file: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement workflow definition from file
    println!("Defining workflow from file: {:?}", file);
    println!("This feature will be implemented in the gRPC service phase");
    Ok(())
}

/// Handles workflow trigger command
pub async fn handle_workflow_trigger(
    workflow_id: String,
    input: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement workflow triggering
    println!("Triggering workflow: {}", workflow_id);
    if let Some(input_str) = input {
        println!("With input: {}", input_str);
    }
    println!("This feature will be implemented in the gRPC service phase");
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
            WorkflowCommands::Trigger {
                workflow_id,
                input,
            } => handle_workflow_trigger(workflow_id, input).await,
        },
    }
}
