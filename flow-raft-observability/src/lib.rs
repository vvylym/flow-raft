//! FlowRaft Observability
//!
//! Observability (metrics and tracing) for FlowRaft.
//! This crate provides metrics collection, Prometheus export, and tracing setup.

pub mod history;
pub mod metrics;
pub mod prometheus;
pub mod tracing;
pub mod watcher;

pub use history::{ExecutionEvent, ExecutionEventType, ExecutionHistory, HistoryStore};
pub use metrics::{MetricsCollector, TaskMetrics, WorkflowMetrics};
pub use prometheus::PrometheusExporter;
pub use tracing::{TracingExporter, init_tracing, shutdown_tracing};
pub use watcher::{WorkflowUpdate, WorkflowWatcher};
