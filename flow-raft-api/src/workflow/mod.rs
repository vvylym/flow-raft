//! Workflow definition API for FlowRaft
//!
//! Provides a simple, programmatic API for defining workflows.

use flow_raft_core::{RetryConfig, WorkflowId};

use crate::graph::{Graph, GraphBuilder};

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

/// Workflow builder for programmatic workflow definition
pub struct WorkflowBuilder {
    name: String,
    builder: GraphBuilder,
    default_retry_config: RetryConfig,
}

impl WorkflowBuilder {
    /// Create a new workflow builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            builder: GraphBuilder::new(""),
            default_retry_config: RetryConfig::default(),
        }
    }

    /// Set the default retry configuration
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        let config_clone = config.clone();
        self.default_retry_config = config_clone.clone();
        self.builder = self.builder.with_default_retry_config(config_clone);
        self
    }

    /// Add a task to the workflow
    pub fn add_task(
        &mut self,
        name: impl Into<String>,
        handler: impl Into<String>,
        inputs: Vec<String>,
        outputs: Vec<String>,
        timeout_secs: Option<u64>,
    ) -> &mut Self {
        self.builder
            .add_node(name, handler, inputs, outputs, timeout_secs);
        self
    }

    /// Add a simple edge from one task to another
    pub fn add_edge(&mut self, from: &str, to: &str) -> &mut Self {
        self.builder.add_simple_edge(from, to);
        self
    }

    /// Add a conditional edge
    pub fn add_conditional_edge<C>(
        &mut self,
        from: &str,
        condition: C,
        then: &str,
        otherwise: &str,
    ) -> &mut Self
    where
        C: crate::graph::builder::ConditionObject + 'static,
    {
        use std::sync::Arc;
        self.builder.add_conditional_edge(
            from,
            Arc::new(condition) as Arc<dyn crate::graph::builder::ConditionObject>,
            then,
            otherwise,
        );
        self
    }

    /// Add a split edge
    pub fn add_split_edge<S>(&mut self, from: &str, split: S, targets: Vec<&str>) -> &mut Self
    where
        S: crate::graph::builder::SplitObject + 'static,
    {
        use std::sync::Arc;
        self.builder.add_split_edge(
            from,
            Arc::new(split) as Arc<dyn crate::graph::builder::SplitObject>,
            targets,
        );
        self
    }

    /// Add a merge edge
    pub fn add_merge_edge<M>(&mut self, sources: Vec<&str>, merge: M, to: &str) -> &mut Self
    where
        M: crate::graph::builder::MergeObject + 'static,
    {
        use std::sync::Arc;
        self.builder.add_merge_edge(
            sources,
            Arc::new(merge) as Arc<dyn crate::graph::builder::MergeObject>,
            to,
        );
        self
    }

    /// Set the root node
    pub fn set_root(&mut self, root: &str) -> &mut Self {
        self.builder.set_root(root);
        self
    }

    /// Build the workflow definition
    pub fn build(self) -> Result<WorkflowDef, String> {
        let graph = self.builder.build()?;
        Ok(WorkflowDef {
            name: self.name,
            workflow_id: WorkflowId::default(),
            graph,
            default_retry_config: self.default_retry_config,
        })
    }
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
