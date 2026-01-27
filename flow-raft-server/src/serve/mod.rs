//! Serve path for flowraft-node: config, build, and run.
//!
//! Use [`ServeConfigBuilder::new()`] to build a [`ServeConfig`]; then pass it to
//! [`NodeServer::run`].

mod bootstrap;
mod build;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use flow_raft_observability::{MetricsCollector, PrometheusExporter};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

use crate::grpc::FlowRaftServiceServer;

pub use bootstrap::{BootstrapError, run_bootstrap};
pub use build::{BuildError, build_components};

/// Error building [`ServeConfig`].
#[derive(Debug, thiserror::Error)]
pub enum ServeConfigError {
    /// A required field was not set.
    #[error("Missing required field: {0}")]
    MissingField(String),
}

/// Resolved configuration for running a flowraft-node. Built via [`ServeConfigBuilder`].
#[derive(Debug, Clone)]
pub struct ServeConfig {
    /// This node's Raft node ID.
    pub node_id: u64,
    /// Socket address for gRPC API.
    pub grpc_bind: SocketAddr,
    /// Socket address for HTTP /health and /metrics.
    pub http_bind: SocketAddr,
    /// Socket address for Raft RPC.
    pub raft_bind: SocketAddr,
    /// Optional path for persistent storage (ignored in 0.2.0).
    pub data_path: Option<PathBuf>,
    /// Comma-separated Raft peer addresses (e.g. `host:port`).
    pub peers: Vec<String>,
    /// If true, bootstrap a new cluster (single node or first of many).
    pub bootstrap: bool,
}

/// Builder for [`ServeConfig`]. Use [`ServeConfigBuilder::new()`].
#[derive(Debug, Default)]
pub struct ServeConfigBuilder {
    node_id: Option<u64>,
    grpc_bind: Option<SocketAddr>,
    http_bind: Option<SocketAddr>,
    raft_bind: Option<SocketAddr>,
    data_path: Option<PathBuf>,
    peers: Vec<String>,
    bootstrap: bool,
}

impl ServeConfigBuilder {
    /// Create a new builder. Call `with_*` to set fields, then `build()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set node ID.
    pub fn with_node_id(mut self, id: u64) -> Self {
        self.node_id = Some(id);
        self
    }

    /// Set gRPC bind address.
    pub fn with_grpc_bind(mut self, a: SocketAddr) -> Self {
        self.grpc_bind = Some(a);
        self
    }

    /// Set HTTP bind address for /health and /metrics.
    pub fn with_http_bind(mut self, a: SocketAddr) -> Self {
        self.http_bind = Some(a);
        self
    }

    /// Set Raft RPC bind address.
    pub fn with_raft_bind(mut self, a: SocketAddr) -> Self {
        self.raft_bind = Some(a);
        self
    }

    /// Set optional data path for persistence.
    pub fn with_data_path(mut self, p: Option<PathBuf>) -> Self {
        self.data_path = p;
        self
    }

    /// Set Raft peer addresses.
    pub fn with_peers(mut self, p: Vec<String>) -> Self {
        self.peers = p;
        self
    }

    /// Set bootstrap flag for new cluster.
    pub fn with_bootstrap(mut self, b: bool) -> Self {
        self.bootstrap = b;
        self
    }

    /// Build `ServeConfig`. Fails if any required field is missing.
    pub fn build(self) -> Result<ServeConfig, ServeConfigError> {
        let node_id = self
            .node_id
            .ok_or_else(|| ServeConfigError::MissingField("node_id".to_string()))?;
        let grpc_bind = self
            .grpc_bind
            .ok_or_else(|| ServeConfigError::MissingField("grpc_bind".to_string()))?;
        let http_bind = self
            .http_bind
            .ok_or_else(|| ServeConfigError::MissingField("http_bind".to_string()))?;
        let raft_bind = self
            .raft_bind
            .ok_or_else(|| ServeConfigError::MissingField("raft_bind".to_string()))?;
        Ok(ServeConfig {
            node_id,
            grpc_bind,
            http_bind,
            raft_bind,
            data_path: self.data_path,
            peers: self.peers,
            bootstrap: self.bootstrap,
        })
    }
}

/// Error running [`NodeServer::run`].
#[derive(Debug, thiserror::Error)]
pub enum NodeServerError {
    /// Build failed.
    #[error("build: {0}")]
    Build(#[from] BuildError),
    /// Bootstrap failed.
    #[error("bootstrap: {0}")]
    Bootstrap(#[from] BootstrapError),
    /// Bind (gRPC or HTTP) failed.
    #[error("bind: {0}")]
    Bind(#[from] std::io::Error),
    /// Prometheus exporter creation or start failed.
    #[error("prometheus: {0}")]
    Prometheus(String),
}

/// Entrypoint for running a flowraft-node: Raft, gRPC, and HTTP /health, /metrics.
pub struct NodeServer;

impl NodeServer {
    /// Build components, bootstrap Raft, start gRPC and HTTP, then block until Ctrl+C.
    /// On shutdown, Raft RPC and gRPC tasks are aborted.
    pub async fn run(config: ServeConfig) -> Result<(), NodeServerError> {
        let components = build::build_components(&config).await?;
        bootstrap::run_bootstrap(&config, &components.node).await?;

        let listener = TcpListener::bind(components.grpc_bind).await?;
        let incoming = TcpListenerStream::new(listener);
        let svc = components.service;
        let grpc = Server::builder()
            .add_service(FlowRaftServiceServer::new(svc))
            .serve_with_incoming(incoming);
        let grpc_handle = tokio::spawn(grpc);

        let exporter =
            PrometheusExporter::new(components.http_port, Arc::new(MetricsCollector::new()))
                .map_err(|e| NodeServerError::Prometheus(e.to_string()))?;
        let http_handle = exporter
            .start_server()
            .await
            .map_err(|e| NodeServerError::Prometheus(e.to_string()))?;

        tokio::signal::ctrl_c().await?;
        components.raft_rpc_handle.abort();
        let _ = components.raft_rpc_handle.await;
        grpc_handle.abort();
        let _ = grpc_handle.await;
        http_handle.abort();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::*;

    #[test]
    fn serve_config_builder_builds_with_all_required() {
        let grpc: SocketAddr = "127.0.0.1:50051".parse().unwrap();
        let http: SocketAddr = "127.0.0.1:9090".parse().unwrap();
        let raft: SocketAddr = "127.0.0.1:5010".parse().unwrap();
        let cfg = ServeConfigBuilder::new()
            .with_node_id(1)
            .with_grpc_bind(grpc)
            .with_http_bind(http)
            .with_raft_bind(raft)
            .with_peers(vec!["127.0.0.1:5011".into()])
            .with_bootstrap(false)
            .build()
            .unwrap();
        assert_eq!(cfg.node_id, 1);
        assert_eq!(cfg.grpc_bind, grpc);
        assert_eq!(cfg.http_bind, http);
        assert_eq!(cfg.raft_bind, raft);
        assert_eq!(cfg.peers, vec!["127.0.0.1:5011"]);
        assert!(!cfg.bootstrap);
    }

    #[test]
    fn serve_config_builder_fails_when_node_id_missing() {
        let grpc: SocketAddr = "127.0.0.1:50051".parse().unwrap();
        let http: SocketAddr = "127.0.0.1:9090".parse().unwrap();
        let raft: SocketAddr = "127.0.0.1:5010".parse().unwrap();
        let err = ServeConfigBuilder::new()
            .with_grpc_bind(grpc)
            .with_http_bind(http)
            .with_raft_bind(raft)
            .build()
            .unwrap_err();
        assert!(matches!(err, ServeConfigError::MissingField(s) if s == "node_id"));
    }

    #[test]
    fn serve_config_builder_fails_when_grpc_bind_missing() {
        let http: SocketAddr = "127.0.0.1:9090".parse().unwrap();
        let raft: SocketAddr = "127.0.0.1:5010".parse().unwrap();
        let err = ServeConfigBuilder::new()
            .with_node_id(1)
            .with_http_bind(http)
            .with_raft_bind(raft)
            .build()
            .unwrap_err();
        assert!(matches!(err, ServeConfigError::MissingField(s) if s == "grpc_bind"));
    }
}
