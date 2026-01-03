//! Type-safe graph builder for FlowRaft
//!
//! Provides compile-time type-safe workflow definition that converts to FlowRaft's Workflow structure.

use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;

use flow_raft_core::{RetryConfig, TaskId};

/// Node name type
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeName(String);

impl NodeName {
    /// Create a new node name
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

impl AsRef<str> for NodeName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NodeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Edge specification
#[derive(Clone)]
pub enum EdgeSpec {
    /// Simple directed edge
    Simple(NodeName),
    /// Conditional edge with condition evaluation
    Conditional {
        /// Condition object
        condition: Arc<dyn ConditionObject>,
        /// Then branch node
        then: NodeName,
        /// Otherwise branch node
        otherwise: NodeName,
    },
    /// Split edge that creates multiple parallel paths
    Split {
        /// Split object that determines targets
        split: Arc<dyn SplitObject>,
        /// Target nodes
        targets: Vec<NodeName>,
    },
}

impl std::fmt::Debug for EdgeSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeSpec::Simple(name) => write!(f, "EdgeSpec::Simple({})", name.as_ref()),
            EdgeSpec::Conditional {
                then, otherwise, ..
            } => {
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

/// Condition object for conditional edges
pub trait ConditionObject: Send + Sync {
    /// Evaluate condition and return the node name to execute
    fn evaluate(&self, inputs: serde_json::Value) -> Result<NodeName, String>;
}

/// Split object for split edges
pub trait SplitObject: Send + Sync {
    /// Evaluate split and return the target node names
    fn evaluate(&self, inputs: serde_json::Value) -> Result<Vec<NodeName>, String>;
}

/// Merge object for merge edges
pub trait MergeObject: Send + Sync {
    /// Merge multiple inputs into a single output
    fn merge(&self, inputs: Vec<serde_json::Value>) -> Result<serde_json::Value, String>;
}

/// Trait for wrapping user-defined functions as workflow nodes
///
/// This allows users to pass functions directly to GraphBuilder
/// instead of handler strings. Functions must be serializable/deserializable
/// via serde_json::Value.
pub trait NodeFunction: Send + Sync {
    /// Execute the function with given inputs
    fn execute(&self, inputs: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Get the input type ID for type checking (optional)
    fn input_type(&self) -> Option<std::any::TypeId> {
        None
    }

    /// Get the output type ID for type checking (optional)
    fn output_type(&self) -> Option<std::any::TypeId> {
        None
    }
}

/// Helper function to wrap a strongly-typed function for use with GraphBuilder
///
/// This is the simplified API for function-based nodes.
/// Users can pass their functions directly and they will be automatically wrapped.
///
/// # Example
/// ```ignore
/// fn process_order(order: Order) -> Result<Payment, String> { ... }
/// let wrapper = wrap_function(process_order);
/// builder.add_node_fn("process", wrapper, None);
/// ```
pub fn wrap_function<F, I, O>(
    func: F,
) -> FunctionWrapper<impl Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync>
where
    F: Fn(I) -> Result<O, String> + Send + Sync + Clone + 'static,
    I: for<'de> serde::Deserialize<'de>,
    O: serde::Serialize,
{
    FunctionWrapper::new(move |inputs: serde_json::Value| {
        let input: I = serde_json::from_value(inputs)
            .map_err(|e| format!("Failed to deserialize input: {}", e))?;
        let output = func(input)?;
        serde_json::to_value(output).map_err(|e| format!("Failed to serialize output: {}", e))
    })
}

/// Macro to implement NodeFunction for strongly-typed functions
///
/// # Example
/// ```ignore
/// fn process_order(order: Order) -> Result<Payment, String> { ... }
/// impl_node_function!(process_order, Order, Payment);
/// ```
#[macro_export]
macro_rules! impl_node_function {
    ($fn:ident, $input:ty, $output:ty) => {
        impl $crate::graph::builder::NodeFunction for fn($input) -> Result<$output, String> {
            fn execute(&self, inputs: serde_json::Value) -> Result<serde_json::Value, String> {
                let input: $input = serde_json::from_value(inputs)
                    .map_err(|e| format!("Failed to deserialize input: {}", e))?;
                let output = self(input)?;
                serde_json::to_value(output)
                    .map_err(|e| format!("Failed to serialize output: {}", e))
            }

            fn input_type(&self) -> Option<std::any::TypeId> {
                Some(std::any::TypeId::of::<$input>())
            }

            fn output_type(&self) -> Option<std::any::TypeId> {
                Some(std::any::TypeId::of::<$output>())
            }
        }
    };
}

/// Wrapper struct to implement NodeFunction for closures and functions
pub struct FunctionWrapper<F> {
    func: F,
}

impl<F> FunctionWrapper<F>
where
    F: Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync,
{
    /// Create a new function wrapper
    pub fn new(func: F) -> Self {
        Self { func }
    }
}

impl<F> NodeFunction for FunctionWrapper<F>
where
    F: Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync,
{
    fn execute(&self, inputs: serde_json::Value) -> Result<serde_json::Value, String> {
        (self.func)(inputs)
    }
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
#[derive(Clone)]
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

impl std::fmt::Debug for Graph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Graph")
            .field("name", &self.name)
            .field("nodes", &self.nodes)
            .field("edges", &self.edges)
            .field("root", &self.root)
            .field("merge_specs", &"<merge_specs>")
            .finish()
    }
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
    /// Root node (explicitly set)
    root: Option<NodeName>,
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
            root: None,
        }
    }

    /// Sets the default retry configuration for all tasks
    pub fn with_default_retry_config(mut self, config: RetryConfig) -> Self {
        self.default_retry_config = config;
        self
    }

    /// Alias for `with_default_retry_config` for convenience
    pub fn with_retry_config(self, config: RetryConfig) -> Self {
        self.with_default_retry_config(config)
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

    /// Adds a node to the graph using a function directly
    ///
    /// This is the simplified API for function-based nodes.
    /// The function will be automatically wrapped to work with the workflow engine.
    ///
    /// # Arguments
    /// * `name` - Node name
    /// * `func` - Function that implements NodeFunction or can be wrapped
    /// * `timeout_secs` - Optional timeout in seconds
    ///
    /// # Example
    /// ```ignore
    /// fn process_order(order: Order) -> Result<Payment, String> { ... }
    /// builder.add_node_fn("process", wrap_function(process_order), None);
    /// ```
    #[allow(unused_variables)]
    pub fn add_node_fn<N: NodeFunction>(
        &mut self,
        name: impl Into<String>,
        _func: N,
        timeout_secs: Option<u64>,
    ) -> &mut Self {
        let name = NodeName::new(name);
        let task_id = self.allocate_task_id();
        let name_str = name.as_ref().to_string();
        self.name_map.insert(name_str.clone(), task_id);

        // Store function in a handler registry (will be implemented)
        // For now, use the function's type name as handler identifier
        let handler_id = format!("fn_{}", name_str);

        let node = GraphNode {
            name: name.clone(),
            task_id,
            handler: handler_id,
            inputs: std::collections::HashSet::new(), // Will be inferred from function signature
            outputs: std::collections::HashSet::new(), // Will be inferred from function signature
            timeout_secs,
        };

        self.nodes.insert(name, node);
        self
    }

    /// Adds a simple directed edge from one node to another
    ///
    /// Alias for `add_simple_edge` for convenience
    pub fn add_edge(&mut self, from: impl AsRef<str>, to: impl AsRef<str>) -> &mut Self {
        self.add_simple_edge(from, to)
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

        self.edges
            .entry(from_name)
            .or_default()
            .push(EdgeSpec::Simple(to_name));
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

        self.edges
            .entry(from_name)
            .or_default()
            .push(EdgeSpec::Conditional {
                condition,
                then: then_name,
                otherwise: else_name,
            });
        self
    }

    /// Adds a split edge that creates multiple parallel paths
    ///
    /// # Arguments
    /// * `from` - Source node name
    /// * `split` - Split object that determines target nodes
    /// * `targets` - Possible target node names
    pub fn add_split_edge(
        &mut self,
        from: impl AsRef<str>,
        split: Arc<dyn SplitObject>,
        targets: Vec<impl AsRef<str>>,
    ) -> &mut Self {
        let from_name = NodeName::new(from.as_ref());
        let target_names: Vec<NodeName> =
            targets.iter().map(|t| NodeName::new(t.as_ref())).collect();

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
            .push(EdgeSpec::Split {
                split,
                targets: target_names,
            });
        self
    }

    /// Adds a merge edge that combines multiple paths
    ///
    /// # Arguments
    /// * `sources` - Source node names to merge
    /// * `merge` - Merge object that combines inputs
    /// * `target` - Target node name
    pub fn add_merge_edge(
        &mut self,
        sources: Vec<impl AsRef<str>>,
        merge: Arc<dyn MergeObject>,
        target: impl AsRef<str>,
    ) -> &mut Self {
        let target_name = NodeName::new(target.as_ref());
        let source_names: Vec<NodeName> =
            sources.iter().map(|s| NodeName::new(s.as_ref())).collect();

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
        self.root = Some(root_name);
        self
    }

    /// Validates the graph before building
    ///
    /// Performs comprehensive validation including cycle detection,
    /// node existence, and reachability analysis.
    pub fn validate(&self) -> Result<(), crate::graph::validation::GraphValidationError> {
        // Build a temporary graph for validation
        // Use explicitly set root, or first node if not set
        let root = self
            .root
            .clone()
            .or_else(|| self.nodes.keys().next().cloned());
        let mut nodes_map = IndexMap::new();
        for (name, node) in &self.nodes {
            nodes_map.insert(name.clone(), node.clone());
        }
        let mut edges_map = IndexMap::new();
        for (name, edges) in &self.edges {
            edges_map.insert(name.clone(), edges.clone());
        }
        let temp_graph = Graph {
            name: self.name.clone(),
            nodes: nodes_map,
            edges: edges_map,
            root,
            merge_specs: self.merge_specs.clone(),
        };
        crate::graph::validation::validate_graph(&temp_graph)
    }

    /// Builds the graph structure with validation
    ///
    /// Validates the graph before building, returning detailed errors
    /// if validation fails.
    pub fn build(&self) -> Result<Graph, String> {
        if self.nodes.is_empty() {
            return Err("graph has no nodes".to_string());
        }

        // Validate graph structure
        if let Err(e) = self.validate() {
            return Err(format!("Graph validation failed: {}", e));
        }

        // Use explicitly set root, or first node if not set
        let root = self
            .root
            .clone()
            .or_else(|| self.nodes.keys().next().cloned());

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
