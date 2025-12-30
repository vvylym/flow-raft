//! Dynamic graph builder for FlowRaft
//!
//! Provides runtime-defined workflows with type erasure using serde_json::Value.

use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;

use crate::api::graph::builder::{ConditionObject, MergeObject, NodeName, SplitObject};

/// Dynamic edge specification
#[derive(Debug, Clone)]
pub enum DynamicEdgeSpec {
    /// Simple directed edge
    Simple(NodeName),
    /// Conditional edge
    Conditional {
        /// Condition object
        condition: Arc<dyn ConditionObject>,
        /// Then branch
        then: NodeName,
        /// Otherwise branch
        otherwise: NodeName,
    },
    /// Split edge
    Split {
        /// Split object
        split: Arc<dyn SplitObject>,
        /// Target nodes
        targets: Vec<NodeName>,
    },
}

/// Dynamic graph node
#[derive(Debug, Clone)]
pub struct DynamicNode {
    /// Node name
    pub name: NodeName,
    /// Task ID
    pub task_id: crate::core::TaskId,
    /// Handler identifier
    pub handler: String,
    /// Input parameter names
    pub inputs: std::collections::HashSet<String>,
    /// Output parameter names
    pub outputs: std::collections::HashSet<String>,
    /// Optional timeout
    pub timeout_secs: Option<u64>,
}

/// Dynamic graph structure
#[derive(Debug, Clone)]
pub struct DynamicGraph {
    /// Graph name
    pub name: String,
    /// Nodes in the graph
    pub nodes: IndexMap<NodeName, DynamicNode>,
    /// Edges from each node
    pub edges: IndexMap<NodeName, Vec<DynamicEdgeSpec>>,
    /// Root node
    pub root: Option<NodeName>,
    /// Merge specifications
    pub merge_specs: HashMap<NodeName, (Vec<NodeName>, Arc<dyn MergeObject>)>,
}

/// Dynamic graph builder
pub struct DynamicGraphBuilder {
    /// Graph name
    name: String,
    /// Next task ID counter
    next_task_id: u64,
    /// Name to task ID mapping
    name_map: HashMap<String, crate::core::TaskId>,
    /// Nodes in the graph
    nodes: HashMap<NodeName, DynamicNode>,
    /// Edges from each node
    edges: HashMap<NodeName, Vec<DynamicEdgeSpec>>,
    /// Merge specifications
    merge_specs: HashMap<NodeName, (Vec<NodeName>, Arc<dyn MergeObject>)>,
}

impl DynamicGraphBuilder {
    /// Creates a new dynamic graph builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            next_task_id: 0,
            name_map: HashMap::new(),
            nodes: HashMap::new(),
            edges: HashMap::new(),
            merge_specs: HashMap::new(),
        }
    }

    /// Allocates a new task ID
    fn allocate_task_id(&mut self) -> crate::core::TaskId {
        self.next_task_id += 1;
        crate::core::TaskId::default()
    }

    /// Adds a node to the graph
    ///
    /// # Arguments
    /// * `name` - Node name
    /// * `handler` - Handler identifier
    /// * `inputs` - Input parameter names
    /// * `outputs` - Output parameter names
    /// * `timeout_secs` - Optional timeout
    pub fn add_node(
        &mut self,
        name: impl Into<String>,
        handler: impl Into<String>,
        inputs: impl IntoIterator<Item = String>,
        outputs: impl IntoIterator<Item = String>,
        timeout_secs: Option<u64>,
    ) -> &mut Self {
        let name = NodeName::new(name);
        let task_id = self.allocate_task_id();
        let name_str = name.as_ref().to_string();
        self.name_map.insert(name_str.clone(), task_id);

        let node = DynamicNode {
            name: name.clone(),
            task_id,
            handler: handler.into(),
            inputs: inputs.into_iter().collect(),
            outputs: outputs.into_iter().collect(),
            timeout_secs,
        };

        self.nodes.insert(name, node);
        self
    }

    /// Adds a simple directed edge
    pub fn add_simple_edge(&mut self, from: impl AsRef<str>, to: impl AsRef<str>) -> &mut Self {
        let from_name = NodeName::new(from.as_ref());
        let to_name = NodeName::new(to.as_ref());

        if !self.nodes.contains_key(&from_name) {
            panic!("from node '{}' not found", from.as_ref());
        }
        if !self.nodes.contains_key(&to_name) {
            panic!("to node '{}' not found", to.as_ref());
        }

        self.edges
            .entry(from_name)
            .or_default()
            .push(DynamicEdgeSpec::Simple(to_name));
        self
    }

    /// Adds a conditional edge
    pub fn add_conditional_edge(
        &mut self,
        from: impl AsRef<str>,
        condition: Arc<dyn ConditionObject>,
        then: impl AsRef<str>,
        otherwise: impl AsRef<str>,
    ) -> &mut Self {
        let from_name = NodeName::new(from.as_ref());
        let then_name = NodeName::new(then.as_ref());
        let else_name = NodeName::new(otherwise.as_ref());

        if !self.nodes.contains_key(&from_name) {
            panic!("from node '{}' not found", from.as_ref());
        }
        if !self.nodes.contains_key(&then_name) {
            panic!("then node '{}' not found", then.as_ref());
        }
        if !self.nodes.contains_key(&else_name) {
            panic!("otherwise node '{}' not found", otherwise.as_ref());
        }

        self.edges
            .entry(from_name)
            .or_default()
            .push(DynamicEdgeSpec::Conditional {
                condition,
                then: then_name,
                otherwise: else_name,
            });
        self
    }

    /// Adds a split edge
    pub fn add_split_edge(
        &mut self,
        from: impl AsRef<str>,
        split: Arc<dyn SplitObject>,
        targets: Vec<impl AsRef<str>>,
    ) -> &mut Self {
        let from_name = NodeName::new(from.as_ref());
        let target_names: Vec<NodeName> = targets.iter().map(|t| NodeName::new(t.as_ref())).collect();

        if !self.nodes.contains_key(&from_name) {
            panic!("from node '{}' not found", from.as_ref());
        }
        for target in &target_names {
            if !self.nodes.contains_key(target) {
                panic!("split target node '{}' not found", target.as_ref());
            }
        }

        self.edges
            .entry(from_name)
            .or_default()
            .push(DynamicEdgeSpec::Split {
                split,
                targets: target_names,
            });
        self
    }

    /// Adds a merge edge
    pub fn add_merge_edge(
        &mut self,
        sources: Vec<impl AsRef<str>>,
        merge: Arc<dyn MergeObject>,
        target: impl AsRef<str>,
    ) -> &mut Self {
        let source_names: Vec<NodeName> = sources.iter().map(|s| NodeName::new(s.as_ref())).collect();
        let target_name = NodeName::new(target.as_ref());

        for source in &source_names {
            if !self.nodes.contains_key(source) {
                panic!("merge source node '{}' not found", source.as_ref());
            }
        }
        if !self.nodes.contains_key(&target_name) {
            panic!("merge target node '{}' not found", target.as_ref());
        }

        self.merge_specs.insert(target_name, (source_names, merge));
        self
    }

    /// Sets the root node
    pub fn set_root(&mut self, root: impl AsRef<str>) -> &mut Self {
        let root_name = NodeName::new(root.as_ref());
        if !self.nodes.contains_key(&root_name) {
            panic!("root node '{}' not found", root.as_ref());
        }
        self
    }

    /// Builds the dynamic graph
    pub fn build(&self) -> Result<DynamicGraph, String> {
        if self.nodes.is_empty() {
            return Err("graph has no nodes".to_string());
        }

        // Determine root node
        let root = self.nodes.keys().next().cloned();

        // Convert to IndexMap
        let mut nodes_map = IndexMap::new();
        for (name, node) in &self.nodes {
            nodes_map.insert(name.clone(), node.clone());
        }

        let mut edges_map = IndexMap::new();
        for (name, edges) in &self.edges {
            edges_map.insert(name.clone(), edges.clone());
        }

        Ok(DynamicGraph {
            name: self.name.clone(),
            nodes: nodes_map,
            edges: edges_map,
            root,
            merge_specs: self.merge_specs.clone(),
        })
    }
}
