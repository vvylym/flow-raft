//! Type-safe graph builder for FlowRaft
//!
//! Provides compile-time type-safe workflow definition that converts to FlowRaft's Workflow structure.

use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;

use flow_raft_core::TaskId;

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
    /// Conditional edge with condition evaluation (two branches)
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
    /// Multi-way switch: condition returns one of the branch node names
    Switch {
        /// Condition that returns the chosen branch node name
        condition: Arc<dyn ConditionObject>,
        /// Possible branch nodes
        branches: Vec<NodeName>,
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
            EdgeSpec::Switch { branches, .. } => {
                write!(
                    f,
                    "EdgeSpec::Switch(branches: {:?})",
                    branches.iter().map(|n| n.as_ref()).collect::<Vec<_>>()
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
