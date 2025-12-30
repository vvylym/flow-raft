//! Type-safe graph builder for FlowRaft
//!
//! Provides compile-time type-safe workflow definition that converts to FlowRaft's Workflow structure.

use std::collections::HashMap;
use std::sync::Arc;

use futures::future::BoxFuture;
use indexmap::IndexMap;

use crate::core::{RetryConfig, TaskId};

/// Node name for graph nodes
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeName(String);

impl NodeName {
    /// Creates a new node name
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

impl AsRef<str> for NodeName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Edge specification for graph edges
#[derive(Clone)]
pub enum EdgeSpec {
    /// Simple directed edge from one node to another
    Simple(NodeName),
    /// Conditional edge that chooses between two paths based on condition
    Conditional {
        /// Condition function that evaluates to a node name
        condition: Arc<dyn ConditionObject>,
        /// Node to execute if condition is true
        then: NodeName,
        /// Node to execute if condition is false
        otherwise: NodeName,
    },
    /// Split edge that executes multiple nodes in parallel
    Split {
        /// Split function that returns a list of node names to execute
        split: Arc<dyn SplitObject>,
        /// Target nodes that can be executed
        targets: Vec<NodeName>,
    },
}

impl std::fmt::Debug for EdgeSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeSpec::Simple(name) => write!(f, "EdgeSpec::Simple({})", name.as_ref()),
            EdgeSpec::Conditional { then, otherwise, .. } => {
                write!(
                    f,
                    "EdgeSpec::Conditional(then: {}, otherwise: {})",
                    then.as_ref(),
                    otherwise.as_ref()
                )
            }
            EdgeSpec::Split { targets, .. } => {
                write!(
                    f,
                    "EdgeSpec::Split(targets: {:?})",
                    targets.iter().map(|n| n.as_ref()).collect::<Vec<_>>()
                )
            }
        }
    }
}

/// Condition object trait for conditional edges
pub trait ConditionObject: Send + Sync + std::fmt::Debug {
    /// Evaluates the condition and returns the chosen node name
    fn evaluate(&self, input: serde_json::Value) -> BoxFuture<'static, Result<NodeName, String>>;
    /// Returns the input type ID for type checking
    fn input_typeid(&self) -> std::any::TypeId;
}

/// Split object trait for split edges
pub trait SplitObject: Send + Sync + std::fmt::Debug {
    /// Splits the input and returns a list of node names to execute
    fn split(&self, input: serde_json::Value) -> BoxFuture<'static, Result<Vec<NodeName>, String>>;
    /// Returns the input type ID for type checking
    fn input_typeid(&self) -> std::any::TypeId;
}

/// Merge object trait for merging results from split execution
pub trait MergeObject: Send + Sync + std::fmt::Debug {
    /// Merges multiple inputs into a single output
    fn merge(&self, inputs: Vec<serde_json::Value>) -> BoxFuture<'static, Result<serde_json::Value, String>>;
}

/// Graph node representation
#[derive(Debug, Clone)]
pub struct GraphNode {
    /// Node name
    pub name: NodeName,
    /// Task ID (generated from node name)
    pub task_id: TaskId,
    /// Handler identifier
    pub handler: String,
    /// Input parameter names
    pub inputs: std::collections::HashSet<String>,
    /// Output parameter names
    pub outputs: std::collections::HashSet<String>,
    /// Optional timeout in seconds
    pub timeout_secs: Option<u64>,
}

/// Graph structure
#[derive(Debug, Clone)]
pub struct Graph {
    /// Graph name
    pub name: String,
    /// Nodes in the graph
    pub nodes: IndexMap<NodeName, GraphNode>,
    /// Edges from each node
    pub edges: IndexMap<NodeName, Vec<EdgeSpec>>,
    /// Root node (entry point)
    pub root: Option<NodeName>,
    /// Merge specifications for merging split results
    pub merge_specs: HashMap<NodeName, (Vec<NodeName>, Arc<dyn MergeObject>)>,
}

/// Type-safe graph builder
pub struct GraphBuilder {
    /// Graph name
    name: String,
    /// Next task ID counter
    next_task_id: u64,
    /// Name to task ID mapping
    name_map: HashMap<String, TaskId>,
    /// Nodes in the graph
    nodes: HashMap<NodeName, GraphNode>,
    /// Edges from each node
    edges: HashMap<NodeName, Vec<EdgeSpec>>,
    /// Merge specifications
    merge_specs: HashMap<NodeName, (Vec<NodeName>, Arc<dyn MergeObject>)>,
    /// Default retry configuration
    default_retry_config: RetryConfig,
}

impl GraphBuilder {
    /// Creates a new graph builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            next_task_id: 0,
            name_map: HashMap::new(),
            nodes: HashMap::new(),
            edges: HashMap::new(),
            merge_specs: HashMap::new(),
            default_retry_config: RetryConfig::default(),
        }
    }

    /// Sets the default retry configuration for all tasks
    pub fn with_default_retry_config(mut self, config: RetryConfig) -> Self {
        self.default_retry_config = config;
        self
    }

    /// Allocates a new task ID
    fn allocate_task_id(&mut self) -> TaskId {
        self.next_task_id += 1;
        TaskId::default()
    }

    /// Adds a node to the graph
    ///
    /// # Arguments
    /// * `name` - Node name
    /// * `handler` - Handler identifier for executing this node
    /// * `inputs` - Input parameter names
    /// * `outputs` - Output parameter names
    /// * `timeout_secs` - Optional timeout in seconds
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

        let node = GraphNode {
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

    /// Adds a simple directed edge from one node to another
    pub fn add_simple_edge(&mut self, from: impl AsRef<str>, to: impl AsRef<str>) -> &mut Self {
        let from_name = NodeName::new(from.as_ref());
        let to_name = NodeName::new(to.as_ref());

        if !self.nodes.contains_key(&from_name) {
            panic!("from node '{}' not found", from.as_ref());
        }
        if !self.nodes.contains_key(&to_name) {
            panic!("to node '{}' not found", to.as_ref());
        }

        self.edges.entry(from_name).or_default().push(EdgeSpec::Simple(to_name));
        self
    }

    /// Adds a conditional edge that chooses between two paths
    ///
    /// # Arguments
    /// * `from` - Source node name
    /// * `condition` - Condition object that evaluates to a node name
    /// * `then` - Node to execute if condition evaluates to this node
    /// * `otherwise` - Node to execute if condition evaluates to this node
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

        self.edges.entry(from_name).or_default().push(EdgeSpec::Conditional {
            condition,
            then: then_name,
            otherwise: else_name,
        });
        self
    }

    /// Adds a split edge that executes multiple nodes in parallel
    ///
    /// # Arguments
    /// * `from` - Source node name
    /// * `split` - Split object that returns a list of node names to execute
    /// * `targets` - List of possible target node names
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

        self.edges.entry(from_name).or_default().push(EdgeSpec::Split {
            split,
            targets: target_names,
        });
        self
    }

    /// Adds a merge edge that merges results from multiple sources
    ///
    /// # Arguments
    /// * `sources` - List of source node names
    /// * `merge` - Merge object that combines the results
    /// * `target` - Target node name that receives the merged result
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

    /// Sets the root node (entry point) of the graph
    pub fn set_root(&mut self, root: impl AsRef<str>) -> &mut Self {
        let root_name = NodeName::new(root.as_ref());
        if !self.nodes.contains_key(&root_name) {
            panic!("root node '{}' not found", root.as_ref());
        }
        // Root will be set during build
        self
    }

    /// Builds the graph structure
    pub fn build(&self) -> Result<Graph, String> {
        if self.nodes.is_empty() {
            return Err("graph has no nodes".to_string());
        }

        // Determine root node (first node if not explicitly set)
        let root = self.nodes.keys().next().cloned();

        // Convert to IndexMap for deterministic ordering
        let mut nodes_map = IndexMap::new();
        for (name, node) in &self.nodes {
            nodes_map.insert(name.clone(), node.clone());
        }

        let mut edges_map = IndexMap::new();
        for (name, edges) in &self.edges {
            edges_map.insert(name.clone(), edges.clone());
        }

        Ok(Graph {
            name: self.name.clone(),
            nodes: nodes_map,
            edges: edges_map,
            root,
            merge_specs: self.merge_specs.clone(),
        })
    }
}
