//! flowraft: CLI for FlowRaft (workflow and cluster operations).

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use flow_raft_server::cli_handlers;

#[derive(Parser)]
#[command(name = "flowraft", about = "FlowRaft workflow and cluster CLI")]
struct Cli {
    /// gRPC server endpoint (e.g. http://localhost:50051).
    #[arg(
        long,
        global = true,
        default_value = "http://localhost:50051",
        env = "FLOWRAFT_ENDPOINT"
    )]
    server: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Workflow operations.
    Workflow {
        #[command(subcommand)]
        sub: WorkflowSub,
    },
    /// Cluster operations.
    Cluster {
        #[command(subcommand)]
        sub: ClusterSub,
    },
}

#[derive(Subcommand)]
enum WorkflowSub {
    /// Define a workflow from a JSON file.
    Define {
        /// Path to workflow definition JSON.
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
    /// Trigger a workflow execution.
    Trigger {
        /// Workflow ID.
        #[arg(long)]
        workflow_id: String,
        /// Input JSON string or path to JSON file.
        #[arg(long)]
        input: Option<String>,
    },
    /// Get workflow status.
    Get {
        /// Workflow ID.
        #[arg(long)]
        workflow_id: String,
    },
    /// List workflows.
    List {
        /// Max items to return.
        #[arg(long, default_value = "100")]
        limit: i32,
        /// Offset for pagination.
        #[arg(long, default_value = "0")]
        offset: i32,
    },
    /// Cancel a workflow.
    Cancel {
        /// Workflow ID.
        #[arg(long)]
        workflow_id: String,
    },
}

#[derive(Subcommand)]
enum ClusterSub {
    /// Show cluster status.
    Status {
        /// Node ID to query (default 1).
        #[arg(long, default_value = "1")]
        node_id: u64,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();
    let server = &cli.server;

    match cli.command {
        Commands::Workflow { sub } => match sub {
            WorkflowSub::Define { file } => {
                cli_handlers::handle_workflow_define(server, file).await?
            }
            WorkflowSub::Trigger { workflow_id, input } => {
                cli_handlers::handle_workflow_trigger(server, workflow_id, input).await?
            }
            WorkflowSub::Get { workflow_id } => {
                cli_handlers::handle_workflow_get(server, workflow_id).await?
            }
            WorkflowSub::List { limit, offset } => {
                cli_handlers::handle_workflow_list(server, limit, offset).await?
            }
            WorkflowSub::Cancel { workflow_id } => {
                cli_handlers::handle_workflow_cancel(server, workflow_id).await?
            }
        },
        Commands::Cluster { sub } => match sub {
            ClusterSub::Status { node_id } => {
                cli_handlers::handle_cluster_status(server, node_id).await?
            }
        },
    }

    Ok(())
}
