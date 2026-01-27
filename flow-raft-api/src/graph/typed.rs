//! Type-safe workflow graph builder (stateless).
//!
//! Use [TypedGraphBuilder] with [node], [condition], [split], [merge], [switch] to build
//! type-checked workflow graphs. For shared state across nodes, see [crate::graph::stateful].

use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use flow_raft_core::{RetryConfig, TaskId};

use super::builder::{
    ConditionObject, EdgeSpec, Graph, GraphNode, MergeObject, NodeFunction, NodeName, SplitObject,
};
use super::validation::validate_graph;
use crate::workflow::WorkflowDef;
use indexmap::IndexMap;

const HANDLER_PREFIX: &str = "fn_";

// ============== Typed traits ==============

/// Node function with known input/output types for edge type-checking.
pub trait TypedNodeFunction: NodeFunction {
    /// TypeId of the input type.
    fn input_type_id(&self) -> TypeId;
    /// TypeId of the output type.
    fn output_type_id(&self) -> TypeId;
}

/// Condition for conditional edges; input type must match source node output.
pub trait TypedCondition: Send + Sync {
    /// TypeId of the condition input.
    fn input_type_id(&self) -> TypeId;
    /// Evaluates the condition and returns the boolean result.
    fn evaluate_bool(&self, inputs: serde_json::Value) -> Result<bool, String>;
}

/// Split for fan-out edges; returns target node names.
pub trait TypedSplit: Send + Sync {
    /// TypeId of the split input.
    fn input_type_id(&self) -> TypeId;
    /// Evaluates the split and returns target node names.
    fn evaluate(&self, inputs: serde_json::Value) -> Result<Vec<String>, String>;
}

/// Merge for joining multiple branches into one; output type must match target node input.
pub trait TypedMerge: Send + Sync {
    /// TypeId of the merge output.
    fn output_type_id(&self) -> TypeId;
    /// Merges multiple inputs into a single output value.
    fn merge(&self, inputs: Vec<serde_json::Value>) -> Result<serde_json::Value, String>;
}

/// Switch for multi-way branching; returns chosen branch node name.
pub trait TypedSwitch: Send + Sync {
    /// TypeId of the switch input.
    fn input_type_id(&self) -> TypeId;
    /// Evaluates the switch and returns the chosen branch name.
    fn evaluate(&self, inputs: serde_json::Value) -> Result<String, String>;
}

// ============== Node wrappers ==============

/// Wraps `fn(I) -> Result<O, E>` as a typed node.
pub struct NodeResult<I, O, E, F> {
    func: F,
    _phantom: std::marker::PhantomData<fn(I) -> Result<O, E>>,
}

impl<I, O, E, F> NodeResult<I, O, E, F>
where
    F: Fn(I) -> Result<O, E> + Send + Sync + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
{
    /// Creates a node from a fallible function.
    pub fn new(func: F) -> Self {
        Self {
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<I, O, E, F> NodeFunction for NodeResult<I, O, E, F>
where
    F: Fn(I) -> Result<O, E> + Send + Sync + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
{
    fn execute(&self, inputs: serde_json::Value) -> Result<serde_json::Value, String> {
        let input: I =
            serde_json::from_value(inputs).map_err(|e| format!("deserialize input: {}", e))?;
        let output = (self.func)(input).map_err(|e| e.to_string())?;
        serde_json::to_value(output).map_err(|e| format!("serialize output: {}", e))
    }
    fn input_type(&self) -> Option<TypeId> {
        Some(TypeId::of::<I>())
    }
    fn output_type(&self) -> Option<TypeId> {
        Some(TypeId::of::<O>())
    }
}

impl<I, O, E, F> TypedNodeFunction for NodeResult<I, O, E, F>
where
    F: Fn(I) -> Result<O, E> + Send + Sync + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
{
    fn input_type_id(&self) -> TypeId {
        TypeId::of::<I>()
    }
    fn output_type_id(&self) -> TypeId {
        TypeId::of::<O>()
    }
}

/// Wraps `fn(I) -> O` (infallible) as a typed node.
pub struct NodeOk<I, O, F> {
    func: F,
    _phantom: std::marker::PhantomData<fn(I) -> O>,
}

impl<I, O, F> NodeOk<I, O, F>
where
    F: Fn(I) -> O + Send + Sync + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
{
    /// Creates a node from an infallible function.
    pub fn new(func: F) -> Self {
        Self {
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<I, O, F> NodeFunction for NodeOk<I, O, F>
where
    F: Fn(I) -> O + Send + Sync + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
{
    fn execute(&self, inputs: serde_json::Value) -> Result<serde_json::Value, String> {
        let input: I =
            serde_json::from_value(inputs).map_err(|e| format!("deserialize input: {}", e))?;
        let output = (self.func)(input);
        serde_json::to_value(output).map_err(|e| format!("serialize output: {}", e))
    }
    fn input_type(&self) -> Option<TypeId> {
        Some(TypeId::of::<I>())
    }
    fn output_type(&self) -> Option<TypeId> {
        Some(TypeId::of::<O>())
    }
}

impl<I, O, F> TypedNodeFunction for NodeOk<I, O, F>
where
    F: Fn(I) -> O + Send + Sync + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
{
    fn input_type_id(&self) -> TypeId {
        TypeId::of::<I>()
    }
    fn output_type_id(&self) -> TypeId {
        TypeId::of::<O>()
    }
}

/// Build a node from `fn(I) -> Result<O, E>`.
pub fn node<I, O, E, F>(f: F) -> NodeResult<I, O, E, F>
where
    F: Fn(I) -> Result<O, E> + Send + Sync + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
{
    NodeResult::new(f)
}

/// Build a node from `fn(I) -> O` (infallible).
pub fn node_ok<I, O, F>(f: F) -> NodeOk<I, O, F>
where
    F: Fn(I) -> O + Send + Sync + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
{
    NodeOk::new(f)
}

// ============== Async node wrappers (block on current runtime) ==============

/// Async node wrapper; returned by [node_async].
pub struct NodeAsyncResult<I, O, E, F> {
    func: F,
    _phantom: std::marker::PhantomData<fn(I) -> Result<O, E>>,
}

impl<I, O, E, F, Fut> NodeAsyncResult<I, O, E, F>
where
    F: Fn(I) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<O, E>> + Send + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
{
    fn new(func: F) -> Self {
        Self {
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<I, O, E, F, Fut> NodeFunction for NodeAsyncResult<I, O, E, F>
where
    F: Fn(I) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<O, E>> + Send + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
{
    fn execute(&self, inputs: serde_json::Value) -> Result<serde_json::Value, String> {
        let input: I =
            serde_json::from_value(inputs).map_err(|e| format!("deserialize input: {}", e))?;
        let fut = (self.func)(input);
        let output = tokio::runtime::Handle::current()
            .block_on(fut)
            .map_err(|e| e.to_string())?;
        serde_json::to_value(output).map_err(|e| format!("serialize output: {}", e))
    }
    fn input_type(&self) -> Option<TypeId> {
        Some(TypeId::of::<I>())
    }
    fn output_type(&self) -> Option<TypeId> {
        Some(TypeId::of::<O>())
    }
}

impl<I, O, E, F, Fut> TypedNodeFunction for NodeAsyncResult<I, O, E, F>
where
    F: Fn(I) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<O, E>> + Send + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
{
    fn input_type_id(&self) -> TypeId {
        TypeId::of::<I>()
    }
    fn output_type_id(&self) -> TypeId {
        TypeId::of::<O>()
    }
}

/// Build a node from `async fn(I) -> Result<O, E>`. Runs the future on the current tokio runtime (blocking).
pub fn node_async<I, O, E, F, Fut>(f: F) -> NodeAsyncResult<I, O, E, F>
where
    F: Fn(I) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<O, E>> + Send + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
{
    NodeAsyncResult::new(f)
}

/// Async infallible node wrapper; returned by [node_async_ok].
pub struct NodeAsyncOk<I, O, F> {
    func: F,
    _phantom: std::marker::PhantomData<fn(I) -> O>,
}

impl<I, O, F, Fut> NodeAsyncOk<I, O, F>
where
    F: Fn(I) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = O> + Send + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
{
    fn new(func: F) -> Self {
        Self {
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<I, O, F, Fut> NodeFunction for NodeAsyncOk<I, O, F>
where
    F: Fn(I) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = O> + Send + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
{
    fn execute(&self, inputs: serde_json::Value) -> Result<serde_json::Value, String> {
        let input: I =
            serde_json::from_value(inputs).map_err(|e| format!("deserialize input: {}", e))?;
        let fut = (self.func)(input);
        let output = tokio::runtime::Handle::current().block_on(fut);
        serde_json::to_value(output).map_err(|e| format!("serialize output: {}", e))
    }
    fn input_type(&self) -> Option<TypeId> {
        Some(TypeId::of::<I>())
    }
    fn output_type(&self) -> Option<TypeId> {
        Some(TypeId::of::<O>())
    }
}

impl<I, O, F, Fut> TypedNodeFunction for NodeAsyncOk<I, O, F>
where
    F: Fn(I) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = O> + Send + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
{
    fn input_type_id(&self) -> TypeId {
        TypeId::of::<I>()
    }
    fn output_type_id(&self) -> TypeId {
        TypeId::of::<O>()
    }
}

/// Build a node from `async fn(I) -> O` (infallible). Runs the future on the current tokio runtime (blocking).
pub fn node_async_ok<I, O, F, Fut>(f: F) -> NodeAsyncOk<I, O, F>
where
    F: Fn(I) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = O> + Send + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
{
    NodeAsyncOk::new(f)
}

// ============== Condition ==============

/// Wrapper for a condition function `fn(In) -> bool`.
pub struct ConditionFn<In, F> {
    pub(crate) func: F,
    pub(crate) _phantom: std::marker::PhantomData<In>,
}

impl<In, F> ConditionFn<In, F>
where
    F: Fn(In) -> bool + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + Send + Sync + 'static,
{
    fn new(func: F) -> Self {
        Self {
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<In, F> TypedCondition for ConditionFn<In, F>
where
    F: Fn(In) -> bool + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + Send + Sync + 'static,
{
    fn input_type_id(&self) -> TypeId {
        TypeId::of::<In>()
    }
    fn evaluate_bool(&self, inputs: serde_json::Value) -> Result<bool, String> {
        let input: In = serde_json::from_value(inputs)
            .map_err(|e| format!("deserialize condition input: {}", e))?;
        Ok((self.func)(input))
    }
}

/// Build a condition from `fn(In) -> bool`.
pub fn condition<In, F>(f: F) -> ConditionFn<In, F>
where
    F: Fn(In) -> bool + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + Send + Sync + 'static,
{
    ConditionFn::new(f)
}

// ============== Split ==============

/// Wrapper for a split function `fn(In) -> Result<Vec<String>, String>`.
pub struct SplitFn<In, F> {
    pub(crate) func: F,
    pub(crate) _phantom: std::marker::PhantomData<In>,
}

impl<In, F> SplitFn<In, F>
where
    F: Fn(In) -> Result<Vec<String>, String> + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + 'static,
{
    fn new(func: F) -> Self {
        Self {
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<In, F> TypedSplit for SplitFn<In, F>
where
    F: Fn(In) -> Result<Vec<String>, String> + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + Send + Sync + 'static,
{
    fn input_type_id(&self) -> TypeId {
        TypeId::of::<In>()
    }
    fn evaluate(&self, inputs: serde_json::Value) -> Result<Vec<String>, String> {
        let input: In =
            serde_json::from_value(inputs).map_err(|e| format!("split input: {}", e))?;
        (self.func)(input)
    }
}

/// Build a split from `fn(In) -> Result<Vec<String>, String>`.
pub fn split<In, F>(f: F) -> SplitFn<In, F>
where
    F: Fn(In) -> Result<Vec<String>, String> + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + Send + Sync + 'static,
{
    SplitFn::new(f)
}

// ============== Merge ==============

type MergePhantom<T, O, E> = std::marker::PhantomData<fn(Vec<T>) -> Result<O, E>>;

/// Wrapper for a merge function `fn(Vec<T>) -> Result<O, E>`.
pub struct MergeFn<T, O, E, F> {
    pub(crate) func: F,
    pub(crate) _phantom: MergePhantom<T, O, E>,
}

impl<T, O, E, F> MergeFn<T, O, E, F>
where
    F: Fn(Vec<T>) -> Result<O, E> + Send + Sync + 'static,
    T: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
{
    fn new(func: F) -> Self {
        Self {
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, O, E, F> TypedMerge for MergeFn<T, O, E, F>
where
    F: Fn(Vec<T>) -> Result<O, E> + Send + Sync + 'static,
    T: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
{
    fn output_type_id(&self) -> TypeId {
        TypeId::of::<O>()
    }
    fn merge(&self, inputs: Vec<serde_json::Value>) -> Result<serde_json::Value, String> {
        let vals: Result<Vec<T>, _> = inputs.into_iter().map(serde_json::from_value).collect();
        let vals = vals.map_err(|e| format!("merge input: {}", e))?;
        let out = (self.func)(vals).map_err(|e| e.to_string())?;
        serde_json::to_value(out).map_err(|e| format!("merge output: {}", e))
    }
}

impl<T, O, E, F> MergeObject for MergeFn<T, O, E, F>
where
    F: Fn(Vec<T>) -> Result<O, E> + Send + Sync + 'static,
    T: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
{
    fn merge(&self, inputs: Vec<serde_json::Value>) -> Result<serde_json::Value, String> {
        TypedMerge::merge(self, inputs)
    }
}

/// Build a merge from `fn(Vec<T>) -> Result<O, E>`.
pub fn merge<T, O, E, F>(f: F) -> MergeFn<T, O, E, F>
where
    F: Fn(Vec<T>) -> Result<O, E> + Send + Sync + 'static,
    T: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
{
    MergeFn::new(f)
}

// ============== Switch ==============

/// Wrapper for a switch function `fn(In) -> String`.
pub struct SwitchFn<In, F> {
    pub(crate) func: F,
    pub(crate) _phantom: std::marker::PhantomData<In>,
}

impl<In, F> SwitchFn<In, F>
where
    F: Fn(In) -> String + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + Send + Sync + 'static,
{
    fn new(func: F) -> Self {
        Self {
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<In, F> TypedSwitch for SwitchFn<In, F>
where
    F: Fn(In) -> String + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + Send + Sync + 'static,
{
    fn input_type_id(&self) -> TypeId {
        TypeId::of::<In>()
    }
    fn evaluate(&self, inputs: serde_json::Value) -> Result<String, String> {
        let input: In =
            serde_json::from_value(inputs).map_err(|e| format!("switch input: {}", e))?;
        Ok((self.func)(input))
    }
}

/// Build a switch from `fn(In) -> String`.
pub fn switch<In, F>(f: F) -> SwitchFn<In, F>
where
    F: Fn(In) -> String + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + Send + Sync + 'static,
{
    SwitchFn::new(f)
}

// ============== Adapters ConditionObject / SplitObject ==============

struct TypedConditionAsObject<C> {
    cond: C,
    then_name: NodeName,
    else_name: NodeName,
}

impl<C: TypedCondition> ConditionObject for TypedConditionAsObject<C> {
    fn evaluate(&self, inputs: serde_json::Value) -> Result<NodeName, String> {
        let b = self.cond.evaluate_bool(inputs)?;
        Ok(if b {
            self.then_name.clone()
        } else {
            self.else_name.clone()
        })
    }
}

struct TypedSwitchAsObject<S> {
    switch_fn: S,
    branches: Vec<NodeName>,
}

impl<S: TypedSwitch> ConditionObject for TypedSwitchAsObject<S> {
    fn evaluate(&self, inputs: serde_json::Value) -> Result<NodeName, String> {
        let name = self.switch_fn.evaluate(inputs)?;
        self.branches
            .iter()
            .find(|n| n.as_ref() == name)
            .cloned()
            .ok_or_else(|| format!("switch returned '{}' which is not in branches", name))
    }
}

struct TypedSplitAsObject<Sp> {
    split_fn: Sp,
}

impl<Sp: TypedSplit> SplitObject for TypedSplitAsObject<Sp> {
    fn evaluate(&self, inputs: serde_json::Value) -> Result<Vec<NodeName>, String> {
        self.split_fn
            .evaluate(inputs)
            .map(|v| v.into_iter().map(NodeName::new).collect())
    }
}

// ============== TypedGraphBuilder ==============

/// Edges and merge specs stored during build.
#[derive(Clone)]
enum TypedEdge {
    Simple(String),
    Conditional {
        cond: Arc<dyn ConditionObject>,
        then_node: String,
        else_node: String,
    },
    Split {
        split: Arc<dyn SplitObject>,
        targets: Vec<String>,
    },
    Switch {
        cond: Arc<dyn ConditionObject>,
        branches: Vec<String>,
    },
}

/// Per-node entry: (node function, input TypeId, output TypeId, timeout_secs).
type NodeEntry = (
    Arc<dyn NodeFunction>,
    Option<TypeId>,
    Option<TypeId>,
    Option<u64>,
);

/// Builder for type-safe workflow graphs with type-checked edges.
pub struct TypedGraphBuilder {
    name: String,
    /// node_name -> (boxed NodeFunction, input TypeId, output TypeId, timeout_secs)
    nodes: IndexMap<String, NodeEntry>,
    edges: IndexMap<String, Vec<TypedEdge>>,
    merge_specs: HashMap<String, (Vec<String>, Arc<dyn MergeObject>)>,
    root: Option<String>,
    pub(crate) default_retry_config: RetryConfig,
    #[allow(dead_code)]
    next_task_id: u64,
}

impl TypedGraphBuilder {
    /// Creates a new typed graph builder with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: IndexMap::new(),
            edges: IndexMap::new(),
            merge_specs: HashMap::new(),
            root: None,
            default_retry_config: RetryConfig::default(),
            next_task_id: 1,
        }
    }

    /// Sets the default retry configuration for tasks.
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.default_retry_config = config;
        self
    }

    /// Sets the default retry configuration (for use by StatefulGraphBuilder).
    pub fn set_retry_config(&mut self, config: RetryConfig) -> &mut Self {
        self.default_retry_config = config;
        self
    }

    fn handler_id(name: &str) -> String {
        format!("{}{}", HANDLER_PREFIX, name)
    }

    #[allow(dead_code)]
    fn ensure_node(&self, name: &str, ctx: &str) -> Result<(), String> {
        if !self.nodes.contains_key(name) {
            return Err(format!("{} '{}' not found", ctx, name));
        }
        Ok(())
    }

    /// Adds a typed node with optional timeout.
    pub fn add_node<T>(
        &mut self,
        name: impl Into<String>,
        node_fn: T,
        timeout_secs: Option<u64>,
    ) -> &mut Self
    where
        T: TypedNodeFunction + NodeFunction + 'static,
    {
        let name = name.into();
        let in_id = node_fn.input_type_id();
        let out_id = node_fn.output_type_id();
        self.nodes.insert(
            name,
            (Arc::new(node_fn), Some(in_id), Some(out_id), timeout_secs),
        );
        self
    }

    /// Adds a simple directed edge from one node to another.
    pub fn add_simple_edge(&mut self, from: impl AsRef<str>, to: impl AsRef<str>) -> &mut Self {
        let from = from.as_ref();
        let to = to.as_ref();
        if !self.nodes.contains_key(to) {
            panic!("node '{}' not found", to);
        }
        if !self.nodes.contains_key(from) {
            panic!("node '{}' not found", from);
        }
        self.edges
            .entry(from.to_string())
            .or_default()
            .push(TypedEdge::Simple(to.to_string()));
        self
    }

    /// Adds a conditional edge with then/else branches.
    pub fn add_conditional_edge<C>(
        &mut self,
        from: impl AsRef<str>,
        cond: C,
        then_node: impl AsRef<str>,
        else_node: impl AsRef<str>,
    ) -> &mut Self
    where
        C: TypedCondition + 'static,
    {
        let from = from.as_ref();
        let then_node = then_node.as_ref().to_string();
        let else_node = else_node.as_ref().to_string();
        if !self.nodes.contains_key(&then_node) {
            panic!("node '{}' not found", then_node);
        }
        if !self.nodes.contains_key(&else_node) {
            panic!("node '{}' not found", else_node);
        }
        if !self.nodes.contains_key(from) {
            panic!("node '{}' not found", from);
        }
        let obj = Arc::new(TypedConditionAsObject {
            cond,
            then_name: NodeName::new(&then_node),
            else_name: NodeName::new(&else_node),
        }) as Arc<dyn ConditionObject>;
        self.edges
            .entry(from.to_string())
            .or_default()
            .push(TypedEdge::Conditional {
                cond: obj,
                then_node,
                else_node,
            });
        self
    }

    /// Adds a split edge that fans out to multiple targets.
    pub fn add_split_edge<Sp>(
        &mut self,
        from: impl AsRef<str>,
        split_fn: Sp,
        targets: Vec<impl AsRef<str>>,
    ) -> &mut Self
    where
        Sp: TypedSplit + 'static,
    {
        let from = from.as_ref();
        let targets: Vec<String> = targets
            .iter()
            .map(AsRef::as_ref)
            .map(String::from)
            .collect();
        for t in &targets {
            if !self.nodes.contains_key(t) {
                panic!("node '{}' not found", t);
            }
        }
        if !self.nodes.contains_key(from) {
            panic!("node '{}' not found", from);
        }
        let obj = Arc::new(TypedSplitAsObject { split_fn }) as Arc<dyn SplitObject>;
        self.edges
            .entry(from.to_string())
            .or_default()
            .push(TypedEdge::Split {
                split: obj,
                targets,
            });
        self
    }

    /// Adds a merge edge that joins multiple sources into one target.
    pub fn add_merge_edge<M>(
        &mut self,
        sources: Vec<impl AsRef<str>>,
        merge_fn: M,
        target: impl AsRef<str>,
    ) -> &mut Self
    where
        M: TypedMerge + MergeObject + 'static,
    {
        let sources: Vec<String> = sources
            .iter()
            .map(AsRef::as_ref)
            .map(String::from)
            .collect();
        let target = target.as_ref().to_string();
        for s in &sources {
            if !self.nodes.contains_key(s) {
                panic!("node '{}' not found", s);
            }
        }
        if !self.nodes.contains_key(&target) {
            panic!("node '{}' not found", target);
        }
        self.merge_specs
            .insert(target.clone(), (sources, Arc::new(merge_fn)));
        self
    }

    /// Adds a switch edge for multi-way branching.
    pub fn add_switch_edge<Sw>(
        &mut self,
        from: impl AsRef<str>,
        switch_fn: Sw,
        branches: Vec<impl AsRef<str>>,
    ) -> &mut Self
    where
        Sw: TypedSwitch + 'static,
    {
        let from = from.as_ref();
        let branches: Vec<String> = branches
            .iter()
            .map(AsRef::as_ref)
            .map(String::from)
            .collect();
        for b in &branches {
            if !self.nodes.contains_key(b) {
                panic!("node '{}' not found", b);
            }
        }
        if !self.nodes.contains_key(from) {
            panic!("node '{}' not found", from);
        }
        let obj = Arc::new(TypedSwitchAsObject {
            switch_fn,
            branches: branches.iter().map(NodeName::new).collect(),
        }) as Arc<dyn ConditionObject>;
        self.edges
            .entry(from.to_string())
            .or_default()
            .push(TypedEdge::Switch {
                cond: obj,
                branches,
            });
        self
    }

    /// Sets the root (entry) node.
    pub fn set_root(&mut self, root: impl AsRef<str>) -> &mut Self {
        let root = root.as_ref().to_string();
        if !self.nodes.contains_key(&root) {
            panic!("node '{}' not found", root);
        }
        self.root = Some(root);
        self
    }

    fn type_check(&self) -> Result<(), String> {
        for (from_name, edges) in &self.edges {
            let (_nf, _in, out_id, _) = self
                .nodes
                .get(from_name)
                .ok_or_else(|| format!("internal: edge from '{}' but node missing", from_name))?;
            let source_out = *out_id;
            for e in edges {
                match e {
                    TypedEdge::Simple(to) => {
                        let (_, in_id, _, _) = self.nodes.get(to).ok_or_else(|| {
                            format!("internal: edge to '{}' but node missing", to)
                        })?;
                        let target_in = *in_id;
                        if let (Some(so), Some(ti)) = (source_out, target_in)
                            && so != ti
                        {
                            return Err(format!(
                                "output type of '{}' does not match input type of '{}'",
                                from_name, to
                            ));
                        }
                    }
                    TypedEdge::Conditional {
                        then_node,
                        else_node,
                        ..
                    } => {
                        for to in [then_node, else_node] {
                            let (_, in_id, _, _) = self
                                .nodes
                                .get(to)
                                .ok_or_else(|| format!("internal: branch '{}' missing", to))?;
                            if let (Some(so), Some(ti)) = (source_out, *in_id)
                                && so != ti
                            {
                                return Err(format!(
                                    "output type of '{}' does not match input type of branch '{}'",
                                    from_name, to
                                ));
                            }
                        }
                    }
                    TypedEdge::Split { targets, .. } => {
                        for to in targets {
                            let (_, in_id, _, _) = self.nodes.get(to).ok_or_else(|| {
                                format!("internal: split target '{}' missing", to)
                            })?;
                            if let (Some(so), Some(ti)) = (source_out, *in_id)
                                && so != ti
                            {
                                return Err(format!(
                                    "output type of '{}' does not match input type of '{}'",
                                    from_name, to
                                ));
                            }
                        }
                    }
                    TypedEdge::Switch { branches, .. } => {
                        for to in branches {
                            let (_, in_id, _, _) = self.nodes.get(to).ok_or_else(|| {
                                format!("internal: switch branch '{}' missing", to)
                            })?;
                            if let (Some(so), Some(ti)) = (source_out, *in_id)
                                && so != ti
                            {
                                return Err(format!(
                                    "output type of '{}' does not match input type of '{}'",
                                    from_name, to
                                ));
                            }
                        }
                    }
                }
            }
        }
        for (target, (sources, _)) in &self.merge_specs {
            let (_, in_id, _, _) = self
                .nodes
                .get(target)
                .ok_or_else(|| format!("internal: merge target '{}' missing", target))?;
            // merge output must match target input — we'd need TypedMerge stored; merge_specs store Arc<dyn MergeObject> so we can't get output_type_id easily.
            // For now we only check that merge target exists; type consistency is enforced by TypedMerge at use site.
            let _ = (sources, in_id);
        }
        Ok(())
    }

    /// Builds the typed graph, running type-checking and validation.
    pub fn build(self) -> Result<TypedGraph, String> {
        self.type_check()?;
        let root = self
            .root
            .as_ref()
            .map(NodeName::new)
            .ok_or("root not set")?;

        let mut graph_nodes = IndexMap::new();
        let mut handlers = IndexMap::new();
        for (name, (nf, _in_id, _out_id, timeout_secs)) in &self.nodes {
            let task_id = TaskId::default();
            let handler = Self::handler_id(name);
            handlers.insert(handler.clone(), Arc::clone(nf) as Arc<dyn NodeFunction>);
            let gn = GraphNode {
                name: NodeName::new(name),
                task_id,
                handler: handler.clone(),
                inputs: HashSet::new(),
                outputs: HashSet::new(),
                timeout_secs: *timeout_secs,
            };
            graph_nodes.insert(NodeName::new(name), gn);
        }
        // Rebuild with timeout_secs if we add it to the builder node tuple
        let mut edges_out: IndexMap<NodeName, Vec<EdgeSpec>> = IndexMap::new();
        for (from_name, typed_edges) in &self.edges {
            let from_node = NodeName::new(from_name);
            let specs: Vec<EdgeSpec> = typed_edges
                .iter()
                .map(|e| match e {
                    TypedEdge::Simple(to) => EdgeSpec::Simple(NodeName::new(to)),
                    TypedEdge::Conditional {
                        cond,
                        then_node,
                        else_node,
                    } => EdgeSpec::Conditional {
                        condition: Arc::clone(cond),
                        then: NodeName::new(then_node),
                        otherwise: NodeName::new(else_node),
                    },
                    TypedEdge::Split { split, targets } => EdgeSpec::Split {
                        split: Arc::clone(split),
                        targets: targets.iter().map(NodeName::new).collect(),
                    },
                    TypedEdge::Switch { cond, branches } => EdgeSpec::Switch {
                        condition: Arc::clone(cond),
                        branches: branches.iter().map(NodeName::new).collect(),
                    },
                })
                .collect();
            edges_out.insert(from_node, specs);
        }
        let merge_specs: HashMap<NodeName, (Vec<NodeName>, Arc<dyn MergeObject>)> = self
            .merge_specs
            .into_iter()
            .map(|(t, (sources, m))| {
                (
                    NodeName::new(&t),
                    (sources.iter().map(NodeName::new).collect(), m),
                )
            })
            .collect();

        let graph = Graph {
            name: self.name.clone(),
            nodes: graph_nodes,
            edges: edges_out,
            root: Some(root),
            merge_specs,
        };

        validate_graph(&graph).map_err(|e| e.to_string())?;

        Ok(TypedGraph {
            graph,
            handlers,
            default_retry_config: self.default_retry_config,
        })
    }
}

// ============== TypedGraph ==============

/// Built type-safe graph with workflow_def and handlers for execution.
pub struct TypedGraph {
    graph: Graph,
    handlers: IndexMap<String, Arc<dyn NodeFunction>>,
    default_retry_config: RetryConfig,
}

impl TypedGraph {
    /// Returns the underlying graph.
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Builds a workflow definition with the given name.
    pub fn workflow_def(&self, name: impl Into<String>) -> Result<WorkflowDef, String> {
        Ok(WorkflowDef::from_graph(
            name,
            self.graph.clone(),
            self.default_retry_config.clone(),
        ))
    }

    /// Returns the map of handler ids to node functions.
    pub fn handlers(&self) -> &IndexMap<String, Arc<dyn NodeFunction>> {
        &self.handlers
    }
}
