//! Workflow definition API for FlowRaft
//!
//! Define workflows via [TypedGraphBuilder](crate::graph::TypedGraphBuilder), then use
//! [TypedGraph::workflow_def](crate::graph::TypedGraph::workflow_def) or [WorkflowDef::from_graph].

pub mod define_parse;

use flow_raft_core::{RetryConfig, WorkflowId};

use crate::graph::Graph;

pub use define_parse::{ParsedWorkflow, parse_workflow_from_json};

/// Workflow definition
#[derive(Debug, Clone)]
pub struct WorkflowDef {
    /// Workflow name
    pub name: String,
    /// Workflow ID
    pub workflow_id: WorkflowId,
    /// Graph representation
    pub graph: Graph,
    /// Default retry configuration
    pub default_retry_config: RetryConfig,
}

impl WorkflowDef {
    /// Create a workflow definition from a graph
    pub fn from_graph(name: impl Into<String>, graph: Graph, retry_config: RetryConfig) -> Self {
        Self {
            name: name.into(),
            workflow_id: WorkflowId::default(),
            graph,
            default_retry_config: retry_config,
        }
    }

    /// Get the workflow name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the workflow ID
    pub fn workflow_id(&self) -> &WorkflowId {
        &self.workflow_id
    }

    /// Get the graph
    pub fn graph(&self) -> &Graph {
        &self.graph
    }
}
