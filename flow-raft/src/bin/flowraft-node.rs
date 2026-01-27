//! flowraft-node: run a FlowRaft node (Raft, gRPC, HTTP /health and /metrics).

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use flow_raft_server::serve::{NodeServer, ServeConfigBuilder};

#[derive(Parser)]
#[command(name = "flowraft-node", about = "Run a FlowRaft node")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run this node: Raft, gRPC API, and HTTP /health, /metrics.
    Serve(ServeArgs),
}

#[derive(Args)]
struct ServeArgs {
    /// This node's Raft node ID.
    #[arg(long, env = "NODE_ID")]
    id: u64,

    /// gRPC bind address (e.g. 0.0.0.0:50051).
    #[arg(long, env = "GRPC_BIND", value_parser = parse_socket_addr)]
    grpc: SocketAddr,

    /// HTTP bind address for /health and /metrics (e.g. 0.0.0.0:9090).
    #[arg(long, env = "HTTP_BIND", value_parser = parse_socket_addr)]
    http: SocketAddr,

    /// Raft RPC bind address (e.g. 0.0.0.0:5010).
    #[arg(long, env = "RAFT_BIND", value_parser = parse_socket_addr)]
    raft: SocketAddr,

    /// Optional data directory for persistence (ignored in 0.2.0).
    #[arg(long, env = "DATA_PATH")]
    data: Option<PathBuf>,

    /// Comma-separated peers as id=addr (e.g. 2=127.0.0.1:5011,3=127.0.0.1:5012).
    /// Omit or leave empty for single-node. When non-empty and not --bootstrap,
    /// this node joins an existing cluster (must not be used if cluster already initialized).
    #[arg(long, env = "PEERS", value_delimiter = ',')]
    peers: Vec<String>,

    /// Bootstrap a new cluster (single node or first of a multi-node).
    #[arg(long, env = "BOOTSTRAP")]
    bootstrap: bool,
}

fn parse_socket_addr(s: &str) -> Result<SocketAddr, String> {
    s.parse()
        .map_err(|e| format!("invalid socket address: {}", e))
}

fn to_box_err(
    e: impl std::error::Error + Send + Sync + 'static,
) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(e)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Serve(s) => {
            let config = ServeConfigBuilder::new()
                .with_node_id(s.id)
                .with_grpc_bind(s.grpc)
                .with_http_bind(s.http)
                .with_raft_bind(s.raft)
                .with_data_path(s.data)
                .with_peers(s.peers)
                .with_bootstrap(s.bootstrap)
                .build()
                .map_err(to_box_err)?;
            NodeServer::run(config).await.map_err(to_box_err)
        }
    }
}
