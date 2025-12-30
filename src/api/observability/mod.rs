//! Observability service for FlowRaft
//!
//! Provides real-time workflow watching, metrics collection, and execution history.

pub mod history;
pub mod metrics;
pub mod watcher;

pub use history::{ExecutionEvent, ExecutionEventType, ExecutionHistory, HistoryStore};
pub use metrics::{MetricsCollector, TaskMetrics, WorkflowMetrics};
pub use watcher::{WorkflowUpdate, WorkflowWatcher};
