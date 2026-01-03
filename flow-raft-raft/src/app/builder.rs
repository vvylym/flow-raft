//! Builder pattern for FlowRaft application creation

use std::sync::Arc;

use crate::FlowRaftApp;
use crate::config::RaftConfig;
use crate::network::MemoryNetworkFactory;
use crate::node::FlowRaftNode;
use crate::storage::{LogStore, StateMachineStore};
use crate::types::NodeId;
use flow_raft_api::WorkflowDef;
use flow_raft_observability::MetricsCollector;

/// Error type for app builder
#[derive(Debug, thiserror::Error)]
pub enum AppBuilderError {
    /// Missing required field
    #[error("Missing required field: {0}")]
    MissingField(String),
    /// Raft error
    #[error("Raft error: {0}")]
    Raft(String),
    /// Cluster error
    #[error("Cluster error: {0}")]
    Cluster(String),
}

/// Builder for FlowRaft application
///
/// Provides a fluent API for creating FlowRaft applications
/// for both single-node and cluster deployments.
pub struct FlowRaftAppBuilder {
    node_id: Option<NodeId>,
    config: Option<RaftConfig>,
    workflows: Vec<WorkflowDef>,
    enable_metrics: bool,
    metrics_collector: Option<Arc<MetricsCollector>>,
    storage_path: Option<String>,
    metrics_port: Option<u16>,
    tracing_exporter: Option<flow_raft_observability::TracingExporter>,
    tracing_endpoint: Option<String>,
}

impl FlowRaftAppBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            node_id: None,
            config: None,
            workflows: Vec::new(),
            enable_metrics: false,
            metrics_collector: None,
            storage_path: None,
            metrics_port: None,
            tracing_exporter: None,
            tracing_endpoint: None,
        }
    }

    /// Set the node ID
    pub fn with_node_id(mut self, node_id: NodeId) -> Self {
        self.node_id = Some(node_id);
        self
    }

    /// Set the Raft configuration
    pub fn with_config(mut self, config: RaftConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set the workflows to register
    pub fn with_workflows(mut self, workflows: Vec<WorkflowDef>) -> Self {
        self.workflows = workflows;
        self
    }

    /// Set the metrics collector
    pub fn with_metrics(mut self, collector: Arc<MetricsCollector>) -> Self {
        self.metrics_collector = Some(collector);
        self.enable_metrics = true;
        self
    }

    /// Enable or disable metrics
    pub fn enable_metrics(mut self, enable: bool) -> Self {
        self.enable_metrics = enable;
        if enable && self.metrics_collector.is_none() {
            self.metrics_collector = Some(Arc::new(MetricsCollector::new()));
        }
        self
    }

    /// Set the storage path
    ///
    /// # Note
    /// This method is reserved for future persistent storage implementation.
    /// Currently, FlowRaft uses in-memory storage. When persistent storage is
    /// implemented, this path will be used to store Raft logs and state machine snapshots.
    pub fn with_storage(mut self, path: String) -> Self {
        self.storage_path = Some(path);
        self
    }

    /// Set the metrics port for Prometheus exporter
    ///
    /// # Arguments
    /// * `port` - Port number for the metrics HTTP server (default: 9090)
    pub fn with_metrics_port(mut self, port: u16) -> Self {
        self.metrics_port = Some(port);
        self
    }

    /// Enable distributed tracing
    ///
    /// # Arguments
    /// * `exporter` - Type of tracing exporter to use
    /// * `endpoint` - Optional endpoint for the exporter
    pub fn with_tracing(
        mut self,
        exporter: flow_raft_observability::TracingExporter,
        endpoint: Option<String>,
    ) -> Self {
        self.tracing_exporter = Some(exporter);
        self.tracing_endpoint = endpoint;
        self
    }

    /// Build a single-node FlowRaft application
    pub async fn build_single_node(self) -> Result<FlowRaftApp, AppBuilderError> {
        let node_id = self
            .node_id
            .ok_or_else(|| AppBuilderError::MissingField("node_id".to_string()))?;
        let config = self.config.unwrap_or_else(crate::config::default_config);
        let network = MemoryNetworkFactory::new();
        let log_store = LogStore::default();
        let state_machine = StateMachineStore::default();

        let node = FlowRaftNode::new(node_id, config, network, log_store, state_machine.clone())
            .await
            .map_err(|e| AppBuilderError::Raft(format!("Failed to create node: {:?}", e)))?;

        node.initialize_single_node()
            .await
            .map_err(|e| AppBuilderError::Raft(format!("Failed to initialize: {:?}", e)))?;

        let raft = node.raft.clone();
        let app = FlowRaftApp::new(raft, state_machine);

        // Register workflows if provided
        if !self.workflows.is_empty() {
            for workflow_def in self.workflows {
                app.register_workflow(workflow_def)
                    .await
                    .map_err(AppBuilderError::Raft)?;
            }
        }

        // Initialize distributed tracing if configured
        if let Some(exporter) = self.tracing_exporter {
            let service_name = format!("flow-raft-node-{}", node_id);
            flow_raft_observability::init_tracing(service_name, exporter, self.tracing_endpoint)
                .map_err(|e| {
                    AppBuilderError::Raft(format!("Failed to initialize tracing: {}", e))
                })?;
        }

        // Start Prometheus metrics server if metrics are enabled
        if self.enable_metrics {
            let metrics_collector = self
                .metrics_collector
                .unwrap_or_else(|| Arc::new(MetricsCollector::new()));
            let metrics_port = self.metrics_port.unwrap_or(9090);
            let metrics_collector_clone = metrics_collector.clone();

            use flow_raft_observability::PrometheusExporter;
            let exporter = PrometheusExporter::new(metrics_port, metrics_collector.clone())
                .map_err(|e| {
                    AppBuilderError::Raft(format!("Failed to create metrics exporter: {}", e))
                })?;

            // Start the server in the background
            // If port is already in use, try alternative ports or log warning
            match exporter.start_server().await {
                Ok(_) => {}
                Err(e) => {
                    let error_str = e.to_string();
                    // If port conflict, try to find an available port
                    if error_str.contains("Address already in use")
                        || error_str.contains("address already in use")
                    {
                        // Try ports 9091-9100
                        let mut found_port = None;
                        for port in 9091..=9100 {
                            match PrometheusExporter::new(port, metrics_collector_clone.clone()) {
                                Ok(exporter) => {
                                    if exporter.start_server().await.is_ok() {
                                        found_port = Some(port);
                                        tracing::info!(
                                            "Metrics server started on alternative port {}",
                                            port
                                        );
                                        break;
                                    }
                                }
                                Err(_) => continue,
                            }
                        }
                        if found_port.is_none() {
                            // If no port found, just log and continue without metrics server
                            tracing::warn!(
                                "Could not start metrics server on port {} or alternatives (9091-9100). Continuing without metrics server.",
                                metrics_port
                            );
                        }
                    } else {
                        // For other errors, log but don't fail the app startup
                        tracing::warn!(
                            "Failed to start metrics server on port {}: {}. Continuing without metrics server.",
                            metrics_port,
                            e
                        );
                    }
                }
            }
        }

        Ok(app)
    }

    /// Add a single workflow to the builder
    ///
    /// This is a convenience method for adding workflows one at a time.
    pub fn add_workflow(mut self, workflow: WorkflowDef) -> Self {
        self.workflows.push(workflow);
        self
    }

    /// Add multiple workflows to the builder
    ///
    /// This is a convenience method for adding multiple workflows at once.
    pub fn add_workflows(mut self, workflows: Vec<WorkflowDef>) -> Self {
        self.workflows.extend(workflows);
        self
    }
}

impl Default for FlowRaftAppBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl FlowRaftApp {
    /// Create a new builder for FlowRaft application
    pub fn builder() -> FlowRaftAppBuilder {
        FlowRaftAppBuilder::new()
    }
}
