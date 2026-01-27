//! Dynamic graph builder for FlowRaft
//!
//! Provides runtime-defined workflows with type erasure using serde_json::Value.
//! Kept crate-internal for dynamic_graph_to_workflow; not part of public API.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;

use crate::graph::builder::{ConditionObject, MergeObject, NodeName, SplitObject};

/// Dynamic edge specification
#[derive(Clone)]
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

impl std::fmt::Debug for DynamicEdgeSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DynamicEdgeSpec::Simple(name) => {
                write!(f, "DynamicEdgeSpec::Simple({})", name.as_ref())
            }
            DynamicEdgeSpec::Conditional {
                then, otherwise, ..
            } => {
                write!(
                    f,
                    "DynamicEdgeSpec::Conditional(then: {}, otherwise: {})",
                    then.as_ref(),
                    otherwise.as_ref()
                )
            }
            DynamicEdgeSpec::Split { targets, .. } => {
                write!(
                    f,
                    "DynamicEdgeSpec::Split(targets: {:?})",
                    targets.iter().map(|n| n.as_ref()).collect::<Vec<_>>()
                )
            }
        }
    }
}

/// Dynamic graph node
#[derive(Debug, Clone)]
pub struct DynamicNode {
    /// Node name
    pub name: NodeName,
    /// Task ID
    pub task_id: flow_raft_core::TaskId,
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
#[derive(Clone)]
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

impl std::fmt::Debug for DynamicGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicGraph")
            .field("name", &self.name)
            .field("nodes", &self.nodes)
            .field("edges", &self.edges)
            .field("root", &self.root)
            .field("merge_specs", &"<merge_specs>")
            .finish()
    }
}

/// Dynamic graph builder
pub struct DynamicGraphBuilder {
    /// Graph name
    name: String,
    /// Next task ID counter
    next_task_id: u64,
    /// Name to task ID mapping
    name_map: HashMap<String, flow_raft_core::TaskId>,
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
    fn allocate_task_id(&mut self) -> flow_raft_core::TaskId {
        self.next_task_id += 1;
        flow_raft_core::TaskId::default()
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
        let source_names: Vec<NodeName> =
            sources.iter().map(|s| NodeName::new(s.as_ref())).collect();
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

#[cfg(test)]
mod tests {
    use crate::graph::builder::{ConditionObject, MergeObject, SplitObject};

    use super::*;

    #[test]
    fn dynamic_builder_new_and_add_node() {
        let mut b = DynamicGraphBuilder::new("t");
        b.add_node("n1", "h1", vec![], vec![], None);
        let g = b.build().unwrap();
        assert_eq!(g.name, "t");
        assert_eq!(g.nodes.len(), 1);
    }

    #[test]
    fn dynamic_builder_set_root_and_build() {
        let mut b = DynamicGraphBuilder::new("t");
        b.add_node("n1", "h1", vec![], vec![], None).set_root("n1");
        let g = b.build().unwrap();
        assert_eq!(g.root.as_ref().map(|n| n.as_ref()), Some("n1"));
    }

    #[test]
    fn dynamic_builder_add_simple_edge() {
        let mut b = DynamicGraphBuilder::new("t");
        b.add_node("n1", "h1", vec![], vec![], None)
            .add_node("n2", "h2", vec![], vec![], None)
            .add_simple_edge("n1", "n2")
            .set_root("n1");
        let g = b.build().unwrap();
        assert_eq!(g.nodes.len(), 2);
        assert!(g.edges.get(&NodeName::new("n1")).is_some());
    }

    #[test]
    fn dynamic_builder_add_conditional_edge() {
        let mut b = DynamicGraphBuilder::new("t");
        b.add_node("n1", "h1", vec![], vec![], None)
            .add_node("then", "h2", vec![], vec![], None)
            .add_node("else", "h3", vec![], vec![], None);
        let cond = Arc::new(AlwaysThen);
        b.add_conditional_edge("n1", cond, "then", "else")
            .set_root("n1");
        let g = b.build().unwrap();
        assert_eq!(g.nodes.len(), 3);
    }

    #[test]
    fn dynamic_builder_add_split_edge() {
        let mut b = DynamicGraphBuilder::new("t");
        b.add_node("n1", "h1", vec![], vec![], None)
            .add_node("a", "ha", vec![], vec![], None)
            .add_node("b", "hb", vec![], vec![], None);
        let split = Arc::new(AllTargets);
        b.add_split_edge("n1", split, vec!["a", "b"]).set_root("n1");
        let g = b.build().unwrap();
        assert_eq!(g.nodes.len(), 3);
        let edges = g.edges.get(&NodeName::new("n1")).unwrap();
        assert_eq!(edges.len(), 1);
        assert!(matches!(edges[0], DynamicEdgeSpec::Split { .. }));
    }

    #[test]
    fn dynamic_builder_add_merge_edge() {
        let mut b = DynamicGraphBuilder::new("t");
        b.add_node("a", "ha", vec![], vec![], None)
            .add_node("b", "hb", vec![], vec![], None)
            .add_node("out", "hout", vec![], vec![], None);
        let merge = Arc::new(ConcatMerge);
        b.add_merge_edge(vec!["a", "b"], merge, "out").set_root("a");
        let g = b.build().unwrap();
        assert_eq!(g.merge_specs.len(), 1);
        let (sources, _) = g.merge_specs.get(&NodeName::new("out")).unwrap();
        assert_eq!(sources.len(), 2);
    }

    #[test]
    fn dynamic_builder_build_empty_err() {
        let b = DynamicGraphBuilder::new("empty");
        let res = b.build();
        assert!(matches!(res, Err(e) if e == "graph has no nodes"));
    }

    #[test]
    fn dynamic_edge_spec_debug_split() {
        let spec = DynamicEdgeSpec::Split {
            split: Arc::new(AllTargets),
            targets: vec![NodeName::new("a"), NodeName::new("b")],
        };
        let s = format!("{:?}", spec);
        assert!(s.contains("Split"));
        assert!(s.contains("a"));
        assert!(s.contains("b"));
    }

    #[test]
    fn dynamic_graph_debug() {
        let mut b = DynamicGraphBuilder::new("dbg");
        b.add_node("n1", "h1", vec![], vec![], None);
        let g = b.build().unwrap();
        let s = format!("{:?}", g);
        assert!(s.contains("DynamicGraph"));
        assert!(s.contains("dbg"));
    }

    struct AlwaysThen;
    impl ConditionObject for AlwaysThen {
        fn evaluate(&self, _: serde_json::Value) -> Result<NodeName, String> {
            Ok(NodeName::new("then"))
        }
    }

    struct AllTargets;
    impl SplitObject for AllTargets {
        fn evaluate(&self, _: serde_json::Value) -> Result<Vec<NodeName>, String> {
            Ok(vec![NodeName::new("a"), NodeName::new("b")])
        }
    }

    struct ConcatMerge;
    impl MergeObject for ConcatMerge {
        fn merge(&self, inputs: Vec<serde_json::Value>) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!(inputs))
        }
    }
}
