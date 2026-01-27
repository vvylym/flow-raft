//! Node launcher for FlowRaft
//!
//! Provides init_tracing, start_metrics_server, and NodeLaunchError.
//! Use flowraft-node serve for production node processes.

use std::sync::Arc;

use flow_raft_observability::MetricsCollector;
use flow_raft_observability::PrometheusExporter;
use tracing_subscriber::EnvFilter;

/// Error type for node launching
#[derive(Debug, thiserror::Error)]
pub enum NodeLaunchError {
    /// Raft error
    #[error("Raft error: {0}")]
    RaftError(
        openraft::error::RaftError<flow_raft_raft::types::NodeId, openraft::error::Infallible>,
    ),
    /// Initialization error
    #[error("Initialization error: {0}")]
    InitializeError(
        openraft::error::RaftError<
            flow_raft_raft::types::NodeId,
            openraft::error::InitializeError<flow_raft_raft::types::NodeId, openraft::BasicNode>,
        >,
    ),
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),
}

/// Initialize tracing with environment-based filtering
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}

/// Start metrics server
///
/// # Arguments
/// * `port` - Port for metrics endpoint (default: 9090)
/// * `metrics_collector` - Metrics collector instance
///
/// # Returns
/// Join handle for the metrics server task
pub async fn start_metrics_server(
    port: u16,
    metrics_collector: Arc<MetricsCollector>,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error + Send + Sync>> {
    let exporter = PrometheusExporter::new(port, metrics_collector)?;
    exporter.start_server().await
}
