//! Stateful workflow graphs
//!
//! When you need shared state across nodes, conditions, splits, and merges (e.g. counters,
//! caches, or request-scoped data), use [StatefulGraphBuilder] and the stateful wrappers.
//! The state type `S` must implement [Default]. Each node/condition/split/merge can take
//! an additional `&S` parameter; the same state instance is shared for the whole workflow run.

use std::any::TypeId;
use std::sync::{Arc, RwLock};

use flow_raft_core::RetryConfig;

use super::builder::NodeFunction;
use super::typed::{
    TypedCondition, TypedGraph, TypedGraphBuilder, TypedMerge, TypedNodeFunction, TypedSplit,
    TypedSwitch,
};

// ============== Stateful node (I, &S) -> Result<O, E> ==============

type StatefulNodePhantom<I, O, S, E> = std::marker::PhantomData<fn(I, &S) -> Result<O, E>>;

/// Node function that receives shared state: `fn(I, &S) -> Result<O, E>`.
pub struct StatefulNodeResult<I, O, E, S, F> {
    state: Arc<RwLock<S>>,
    func: F,
    _phantom: StatefulNodePhantom<I, O, S, E>,
}

impl<I, O, E, S, F> StatefulNodeResult<I, O, E, S, F>
where
    F: Fn(I, &S) -> Result<O, E> + Send + Sync + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
    S: Send + Sync + 'static,
{
    /// Creates a stateful node from `fn(I, &S) -> Result<O, E>`.
    pub fn new(state: Arc<RwLock<S>>, func: F) -> Self {
        Self {
            state,
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<I, O, E, S, F> NodeFunction for StatefulNodeResult<I, O, E, S, F>
where
    F: Fn(I, &S) -> Result<O, E> + Send + Sync + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
    S: Send + Sync + 'static,
{
    fn execute(&self, inputs: serde_json::Value) -> Result<serde_json::Value, String> {
        let input: I =
            serde_json::from_value(inputs).map_err(|e| format!("deserialize input: {}", e))?;
        let guard = self
            .state
            .read()
            .map_err(|e| format!("state lock poisoned: {}", e))?;
        let output = (self.func)(input, &*guard).map_err(|e| e.to_string())?;
        drop(guard);
        serde_json::to_value(output).map_err(|e| format!("serialize output: {}", e))
    }

    fn input_type(&self) -> Option<TypeId> {
        Some(TypeId::of::<I>())
    }

    fn output_type(&self) -> Option<TypeId> {
        Some(TypeId::of::<O>())
    }
}

impl<I, O, E, S, F> TypedNodeFunction for StatefulNodeResult<I, O, E, S, F>
where
    F: Fn(I, &S) -> Result<O, E> + Send + Sync + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
    S: Send + Sync + 'static,
{
    fn input_type_id(&self) -> TypeId {
        TypeId::of::<I>()
    }
    fn output_type_id(&self) -> TypeId {
        TypeId::of::<O>()
    }
}

/// Wrap `fn(I, &S) -> Result<O, E>` for use with [StatefulGraphBuilder].
pub fn stateful_node<I, O, E, S, F>(
    state: Arc<RwLock<S>>,
    f: F,
) -> StatefulNodeResult<I, O, E, S, F>
where
    F: Fn(I, &S) -> Result<O, E> + Send + Sync + 'static,
    I: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
    S: Send + Sync + 'static,
{
    StatefulNodeResult::new(state, f)
}

// ============== Stateful condition (In, &S) -> bool) ==============

/// Condition that receives state: `fn(In, &S) -> bool`.
pub struct StatefulConditionFn<In, S, F> {
    state: Arc<RwLock<S>>,
    func: F,
    _phantom: std::marker::PhantomData<fn(In, &S) -> bool>,
}

impl<In, S, F> StatefulConditionFn<In, S, F>
where
    F: Fn(In, &S) -> bool + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + 'static,
    S: Send + Sync + 'static,
{
    /// Creates a stateful condition from `fn(In, &S) -> bool`.
    pub fn new(state: Arc<RwLock<S>>, func: F) -> Self {
        Self {
            state,
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<In, S, F> TypedCondition for StatefulConditionFn<In, S, F>
where
    F: Fn(In, &S) -> bool + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + 'static,
    S: Send + Sync + 'static,
{
    fn input_type_id(&self) -> TypeId {
        TypeId::of::<In>()
    }
    fn evaluate_bool(&self, inputs: serde_json::Value) -> Result<bool, String> {
        let input: In = serde_json::from_value(inputs)
            .map_err(|e| format!("deserialize condition input: {}", e))?;
        let guard = self
            .state
            .read()
            .map_err(|e| format!("state lock poisoned: {}", e))?;
        let out = (self.func)(input, &*guard);
        drop(guard);
        Ok(out)
    }
}

/// Wrap `fn(In, &S) -> bool` for stateful conditional edges.
pub fn stateful_condition<In, S, F>(state: Arc<RwLock<S>>, f: F) -> StatefulConditionFn<In, S, F>
where
    F: Fn(In, &S) -> bool + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + 'static,
    S: Send + Sync + 'static,
{
    StatefulConditionFn::new(state, f)
}

// ============== Stateful split (In, &S) -> Result<Vec<String>, String> ==============

/// Split that receives state: `fn(In, &S) -> Result<Vec<String>, String>`.
pub struct StatefulSplitFn<In, S, F> {
    state: Arc<RwLock<S>>,
    func: F,
    _phantom: std::marker::PhantomData<In>,
}

impl<In, S, F> StatefulSplitFn<In, S, F>
where
    F: Fn(In, &S) -> Result<Vec<String>, String> + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    /// Creates a stateful split from `fn(In, &S) -> Result<Vec<String>, String>`.
    pub fn new(state: Arc<RwLock<S>>, func: F) -> Self {
        Self {
            state,
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<In, S, F> TypedSplit for StatefulSplitFn<In, S, F>
where
    F: Fn(In, &S) -> Result<Vec<String>, String> + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    fn input_type_id(&self) -> TypeId {
        TypeId::of::<In>()
    }
    fn evaluate(&self, inputs: serde_json::Value) -> Result<Vec<String>, String> {
        let input: In =
            serde_json::from_value(inputs).map_err(|e| format!("split input: {}", e))?;
        let guard = self
            .state
            .read()
            .map_err(|e| format!("state lock poisoned: {}", e))?;
        let out = (self.func)(input, &*guard);
        drop(guard);
        out
    }
}

/// Wrap `fn(In, &S) -> Result<Vec<String>, String>` for stateful split edges.
pub fn stateful_split<In, S, F>(state: Arc<RwLock<S>>, f: F) -> StatefulSplitFn<In, S, F>
where
    F: Fn(In, &S) -> Result<Vec<String>, String> + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    StatefulSplitFn::new(state, f)
}

// ============== Stateful merge (Vec<T>, &S) -> Result<O, E> ==============

type StatefulMergePhantom<T, O, S, E> = std::marker::PhantomData<fn(Vec<T>, &S) -> Result<O, E>>;

/// Merge that receives state: `fn(Vec<T>, &S) -> Result<O, E>`.
pub struct StatefulMergeFn<T, O, E, S, F> {
    state: Arc<RwLock<S>>,
    func: F,
    _phantom: StatefulMergePhantom<T, O, S, E>,
}

impl<T, O, E, S, F> StatefulMergeFn<T, O, E, S, F>
where
    F: Fn(Vec<T>, &S) -> Result<O, E> + Send + Sync + 'static,
    T: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
    S: Send + Sync + 'static,
{
    /// Creates a stateful merge from `fn(Vec<T>, &S) -> Result<O, E>`.
    pub fn new(state: Arc<RwLock<S>>, func: F) -> Self {
        Self {
            state,
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, O, E, S, F> TypedMerge for StatefulMergeFn<T, O, E, S, F>
where
    F: Fn(Vec<T>, &S) -> Result<O, E> + Send + Sync + 'static,
    T: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
    S: Send + Sync + 'static,
{
    fn output_type_id(&self) -> TypeId {
        TypeId::of::<O>()
    }
    fn merge(&self, inputs: Vec<serde_json::Value>) -> Result<serde_json::Value, String> {
        let vals: Result<Vec<T>, _> = inputs.into_iter().map(serde_json::from_value).collect();
        let vals = vals.map_err(|e| format!("merge input: {}", e))?;
        let guard = self
            .state
            .read()
            .map_err(|e| format!("state lock poisoned: {}", e))?;
        let out = (self.func)(vals, &*guard).map_err(|e| e.to_string())?;
        drop(guard);
        serde_json::to_value(out).map_err(|e| format!("merge output: {}", e))
    }
}

impl<T, O, E, S, F> super::builder::MergeObject for StatefulMergeFn<T, O, E, S, F>
where
    F: Fn(Vec<T>, &S) -> Result<O, E> + Send + Sync + 'static,
    T: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
    S: Send + Sync + 'static,
{
    fn merge(&self, inputs: Vec<serde_json::Value>) -> Result<serde_json::Value, String> {
        TypedMerge::merge(self, inputs)
    }
}

/// Wrap `fn(Vec<T>, &S) -> Result<O, E>` for stateful merge edges.
pub fn stateful_merge<T, O, E, S, F>(state: Arc<RwLock<S>>, f: F) -> StatefulMergeFn<T, O, E, S, F>
where
    F: Fn(Vec<T>, &S) -> Result<O, E> + Send + Sync + 'static,
    T: serde::de::DeserializeOwned + 'static,
    O: serde::Serialize + 'static,
    E: std::fmt::Display + 'static,
    S: Send + Sync + 'static,
{
    StatefulMergeFn::new(state, f)
}

// ============== Stateful switch (In, &S) -> String ==============

/// Switch that receives state: `fn(In, &S) -> String`.
pub struct StatefulSwitchFn<In, S, F> {
    state: Arc<RwLock<S>>,
    func: F,
    _phantom: std::marker::PhantomData<In>,
}

impl<In, S, F> StatefulSwitchFn<In, S, F>
where
    F: Fn(In, &S) -> String + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    /// Creates a stateful switch from `fn(In, &S) -> String`.
    pub fn new(state: Arc<RwLock<S>>, func: F) -> Self {
        Self {
            state,
            func,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<In, S, F> TypedSwitch for StatefulSwitchFn<In, S, F>
where
    F: Fn(In, &S) -> String + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    fn input_type_id(&self) -> TypeId {
        TypeId::of::<In>()
    }
    fn evaluate(&self, inputs: serde_json::Value) -> Result<String, String> {
        let input: In =
            serde_json::from_value(inputs).map_err(|e| format!("switch input: {}", e))?;
        let guard = self
            .state
            .read()
            .map_err(|e| format!("state lock poisoned: {}", e))?;
        let out = (self.func)(input, &*guard);
        drop(guard);
        Ok(out)
    }
}

/// Wrap `fn(In, &S) -> String` for stateful switch edges.
pub fn stateful_switch<In, S, F>(state: Arc<RwLock<S>>, f: F) -> StatefulSwitchFn<In, S, F>
where
    F: Fn(In, &S) -> String + Send + Sync + 'static,
    In: serde::de::DeserializeOwned + Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    StatefulSwitchFn::new(state, f)
}

// ============== StatefulGraphBuilder ==============

/// Graph builder for workflows with shared state.
///
/// `S` must implement [Default]. The same state instance is shared across all nodes,
/// conditions, splits, and merges during a run. Use [stateful_node], [stateful_condition],
/// [stateful_split], [stateful_merge], [stateful_switch] with a shared `Arc<RwLock<S>>`
/// from [StatefulGraphBuilder::state], or create with `Arc::new(RwLock::new(S::default()))`
/// and pass via [StatefulGraphBuilder::with_state].
pub struct StatefulGraphBuilder<S> {
    state: Arc<RwLock<S>>,
    inner: TypedGraphBuilder,
}

impl<S> StatefulGraphBuilder<S>
where
    S: Default + Send + Sync + 'static,
{
    /// Creates a new stateful graph builder. State is initialized to `S::default()`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            state: Arc::new(RwLock::new(S::default())),
            inner: TypedGraphBuilder::new(name),
        }
    }

    /// Uses the given state instead of `S::default()`.
    pub fn with_state(state: Arc<RwLock<S>>) -> Self
    where
        S: Default,
    {
        Self {
            state: state.clone(),
            inner: TypedGraphBuilder::new("stateful_workflow"),
        }
    }

    /// Builder from name and pre-initialized state.
    pub fn with_name_and_state(name: impl Into<String>, state: Arc<RwLock<S>>) -> Self {
        Self {
            state,
            inner: TypedGraphBuilder::new(name),
        }
    }

    /// Returns the shared state handle for use in stateful_* wrappers.
    pub fn state(&self) -> Arc<RwLock<S>> {
        self.state.clone()
    }

    /// Sets the default retry configuration.
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.inner.set_retry_config(config);
        self
    }

    /// Add a stateful node: `f: fn(I, &S) -> Result<O, E>`.
    pub fn add_node<I, O, E, F>(
        &mut self,
        name: impl Into<String>,
        f: F,
        timeout_secs: Option<u64>,
    ) -> &mut Self
    where
        F: Fn(I, &S) -> Result<O, E> + Send + Sync + 'static,
        I: serde::de::DeserializeOwned + 'static,
        O: serde::Serialize + 'static,
        E: std::fmt::Display + 'static,
    {
        let wrapped = stateful_node(self.state(), f);
        self.inner.add_node(name, wrapped, timeout_secs);
        self
    }

    /// Add a simple edge.
    pub fn add_simple_edge(&mut self, from: impl AsRef<str>, to: impl AsRef<str>) -> &mut Self {
        self.inner.add_simple_edge(from, to);
        self
    }

    /// Add a stateful conditional edge: `f: fn(In, &S) -> bool`.
    pub fn add_conditional_edge<In, F>(
        &mut self,
        from: impl AsRef<str>,
        f: F,
        then_node: impl AsRef<str>,
        else_node: impl AsRef<str>,
    ) -> &mut Self
    where
        F: Fn(In, &S) -> bool + Send + Sync + 'static,
        In: serde::de::DeserializeOwned + 'static,
    {
        let cond = stateful_condition(self.state(), f);
        self.inner
            .add_conditional_edge(from, cond, then_node, else_node);
        self
    }

    /// Add a stateful split edge: `f: fn(In, &S) -> Result<Vec<String>, String>`.
    pub fn add_split_edge<In, F>(
        &mut self,
        from: impl AsRef<str>,
        f: F,
        targets: Vec<impl AsRef<str>>,
    ) -> &mut Self
    where
        F: Fn(In, &S) -> Result<Vec<String>, String> + Send + Sync + 'static,
        In: serde::de::DeserializeOwned + Send + Sync + 'static,
    {
        let split_fn = stateful_split(self.state(), f);
        self.inner.add_split_edge(from, split_fn, targets);
        self
    }

    /// Add a stateful merge edge: `f: fn(Vec<T>, &S) -> Result<O, E>`.
    pub fn add_merge_edge<T, O, E, F>(
        &mut self,
        sources: Vec<impl AsRef<str>>,
        f: F,
        target: impl AsRef<str>,
    ) -> &mut Self
    where
        F: Fn(Vec<T>, &S) -> Result<O, E> + Send + Sync + 'static,
        T: serde::de::DeserializeOwned + 'static,
        O: serde::Serialize + 'static,
        E: std::fmt::Display + 'static,
    {
        let merge_fn = stateful_merge(self.state(), f);
        self.inner.add_merge_edge(sources, merge_fn, target);
        self
    }

    /// Add a stateful switch edge: `f: fn(In, &S) -> String`.
    pub fn add_switch_edge<In, F>(
        &mut self,
        from: impl AsRef<str>,
        f: F,
        branches: Vec<impl AsRef<str>>,
    ) -> &mut Self
    where
        F: Fn(In, &S) -> String + Send + Sync + 'static,
        In: serde::de::DeserializeOwned + Send + Sync + 'static,
    {
        let switch_fn = stateful_switch(self.state(), f);
        self.inner.add_switch_edge(from, switch_fn, branches);
        self
    }

    /// Set the root node.
    pub fn set_root(&mut self, root: impl AsRef<str>) -> &mut Self {
        self.inner.set_root(root);
        self
    }

    /// Build the stateful graph. Returns a [StatefulGraph] that exposes the same
    /// [workflow_def](StatefulGraph::workflow_def) and [handlers](StatefulGraph::handlers)
    /// as [TypedGraph], so it works with the same executor and registration.
    pub fn build(self) -> Result<StatefulGraph<S>, String> {
        let typed = self.inner.build()?;
        Ok(StatefulGraph {
            inner: typed,
            state: self.state,
        })
    }
}

// ============== StatefulGraph ==============

/// Built stateful graph; exposes the same API as [TypedGraph] for workflow_def and handlers.
pub struct StatefulGraph<S> {
    inner: TypedGraph,
    state: Arc<RwLock<S>>,
}

impl<S> StatefulGraph<S> {
    /// Returns the underlying graph.
    pub fn graph(&self) -> &super::builder::Graph {
        self.inner.graph()
    }

    /// Builds a workflow definition with the given name.
    pub fn workflow_def(
        &self,
        name: impl Into<String>,
    ) -> Result<crate::workflow::WorkflowDef, String> {
        self.inner.workflow_def(name)
    }

    /// Returns the map of handler ids to node functions.
    pub fn handlers(&self) -> &indexmap::IndexMap<String, Arc<dyn NodeFunction>> {
        self.inner.handlers()
    }

    /// Shared state handle for this run.
    pub fn state(&self) -> Arc<RwLock<S>> {
        self.state.clone()
    }
}
