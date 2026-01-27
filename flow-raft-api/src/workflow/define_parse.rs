//! Parse define_workflow JSON payload into Graph + RetryConfig.
//!
//! Matches the JSON shape produced by [FlowRaftClient::submit_workflow](crate::client::FlowRaftClient::submit_workflow).
//! Conditional/split/switch edges use placeholder condition/split objects so the graph structure
//! is valid; execution behavior depends on handlers and runtime.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use flow_raft_core::{RetryConfig, TaskId};
use indexmap::IndexMap;
use serde::Deserialize;

use crate::graph::builder::{ConditionObject, EdgeSpec, Graph, GraphNode, NodeName, SplitObject};

/// Result of parsing a define_workflow JSON payload.
pub struct ParsedWorkflow {
    /// Workflow name
    pub name: String,
    /// Graph built from the payload
    pub graph: Graph,
    /// Default retry config
    pub default_retry_config: RetryConfig,
}

/// Placeholder condition: always returns the "then" branch (used when deserializing conditional edges).
struct StubConditionThen {
    then: NodeName,
}
impl ConditionObject for StubConditionThen {
    fn evaluate(&self, _: serde_json::Value) -> Result<NodeName, String> {
        Ok(self.then.clone())
    }
}

/// Placeholder condition: returns the first branch (used for switch edges).
struct StubConditionFirst {
    first: NodeName,
}
impl ConditionObject for StubConditionFirst {
    fn evaluate(&self, _: serde_json::Value) -> Result<NodeName, String> {
        Ok(self.first.clone())
    }
}

/// Placeholder split: returns all targets (used when deserializing split edges).
struct StubSplitTargets {
    targets: Vec<NodeName>,
}
impl SplitObject for StubSplitTargets {
    fn evaluate(&self, _: serde_json::Value) -> Result<Vec<NodeName>, String> {
        Ok(self.targets.clone())
    }
}

#[derive(Deserialize)]
struct WireNode {
    name: String,
    task_id: String,
    handler: String,
    #[serde(default)]
    inputs: Vec<String>,
    #[serde(default)]
    outputs: Vec<String>,
    timeout_secs: Option<u64>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum WireEdge {
    #[serde(rename = "simple")]
    Simple { to: String },
    #[serde(rename = "conditional")]
    Conditional { then: String, otherwise: String },
    #[serde(rename = "split")]
    Split { targets: Vec<String> },
    #[serde(rename = "switch")]
    Switch { branches: Vec<String> },
}

#[derive(Deserialize)]
struct WireEdgeGroup {
    from: String,
    edges: Vec<WireEdge>,
}

#[derive(Deserialize)]
struct WireGraph {
    name: String,
    nodes: Vec<WireNode>,
    edges: Vec<WireEdgeGroup>,
    root: Option<String>,
}

#[derive(Deserialize)]
struct WireRetryConfig {
    #[serde(default = "default_max_attempts")]
    max_attempts: u8,
    #[serde(default = "default_initial_delay_ms")]
    initial_delay_ms: u64,
    #[serde(default = "default_backoff_factor")]
    backoff_factor: f64,
}

fn default_max_attempts() -> u8 {
    3
}
fn default_initial_delay_ms() -> u64 {
    1000
}
fn default_backoff_factor() -> f64 {
    2.0
}

#[derive(Deserialize)]
struct WirePayload {
    name: String,
    #[allow(dead_code)]
    workflow_id: Option<String>,
    graph: WireGraph,
    #[serde(default)]
    default_retry_config: Option<WireRetryConfig>,
}

/// Parses the define_workflow JSON string into name, graph, and retry config.
///
/// The format must match what [FlowRaftClient::submit_workflow](crate::client::FlowRaftClient::submit_workflow)
/// sends: `{ "name", "graph": { "name", "nodes", "edges", "root" }, "default_retry_config": { ... } }`.
pub fn parse_workflow_from_json(json: &str) -> Result<ParsedWorkflow, String> {
    let wire: WirePayload =
        serde_json::from_str(json).map_err(|e| format!("Invalid define_workflow JSON: {}", e))?;

    let mut nodes: IndexMap<NodeName, GraphNode> = IndexMap::new();
    for n in &wire.graph.nodes {
        let task_id = TaskId::parse(&n.task_id)
            .map_err(|e| format!("Invalid task_id for node {}: {}", n.name, e))?;
        let name = NodeName::new(&n.name);
        nodes.insert(
            name.clone(),
            GraphNode {
                name,
                task_id,
                handler: n.handler.clone(),
                inputs: n.inputs.iter().cloned().collect::<HashSet<_>>(),
                outputs: n.outputs.iter().cloned().collect::<HashSet<_>>(),
                timeout_secs: n.timeout_secs,
            },
        );
    }

    let mut edges: IndexMap<NodeName, Vec<EdgeSpec>> = IndexMap::new();
    for eg in &wire.graph.edges {
        let from_name = NodeName::new(&eg.from);
        if !nodes.contains_key(&from_name) {
            return Err(format!("Edge from unknown node '{}'", eg.from));
        }
        let mut specs = Vec::new();
        for e in &eg.edges {
            let spec = match e {
                WireEdge::Simple { to } => {
                    let to_name = NodeName::new(to);
                    if !nodes.contains_key(&to_name) {
                        return Err(format!("Edge to unknown node '{}'", to));
                    }
                    EdgeSpec::Simple(to_name)
                }
                WireEdge::Conditional { then, otherwise } => {
                    let then_name = NodeName::new(then);
                    let else_name = NodeName::new(otherwise);
                    if !nodes.contains_key(&then_name) {
                        return Err(format!("Conditional then node '{}' not found", then));
                    }
                    if !nodes.contains_key(&else_name) {
                        return Err(format!(
                            "Conditional otherwise node '{}' not found",
                            otherwise
                        ));
                    }
                    EdgeSpec::Conditional {
                        condition: Arc::new(StubConditionThen {
                            then: then_name.clone(),
                        }),
                        then: then_name,
                        otherwise: else_name,
                    }
                }
                WireEdge::Split { targets } => {
                    let target_names: Vec<NodeName> = targets.iter().map(NodeName::new).collect();
                    for t in &target_names {
                        if !nodes.contains_key(t) {
                            return Err(format!("Split target node '{}' not found", t.as_ref()));
                        }
                    }
                    EdgeSpec::Split {
                        split: Arc::new(StubSplitTargets {
                            targets: target_names.clone(),
                        }),
                        targets: target_names,
                    }
                }
                WireEdge::Switch { branches } => {
                    if branches.is_empty() {
                        return Err("Switch edge has no branches".to_string());
                    }
                    let first = NodeName::new(&branches[0]);
                    if !nodes.contains_key(&first) {
                        return Err(format!("Switch branch '{}' not found", branches[0]));
                    }
                    let branch_names: Vec<NodeName> = branches.iter().map(NodeName::new).collect();
                    for b in &branch_names {
                        if !nodes.contains_key(b) {
                            return Err(format!("Switch branch '{}' not found", b.as_ref()));
                        }
                    }
                    EdgeSpec::Switch {
                        condition: Arc::new(StubConditionFirst { first }),
                        branches: branch_names,
                    }
                }
            };
            specs.push(spec);
        }
        edges.insert(from_name, specs);
    }

    let root = wire
        .graph
        .root
        .map(|r| NodeName::new(&r))
        .filter(|r| nodes.contains_key(r));

    let default_retry_config = wire
        .default_retry_config
        .map(|c| RetryConfig::with_backoff(c.max_attempts, c.backoff_factor, c.initial_delay_ms))
        .unwrap_or_default();

    let graph = Graph {
        name: wire.graph.name,
        nodes,
        edges,
        root,
        merge_specs: HashMap::new(),
    };

    Ok(ParsedWorkflow {
        name: wire.name,
        graph,
        default_retry_config,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_workflow() {
        let json = serde_json::json!({
            "name": "minimal",
            "graph": {
                "name": "minimal",
                "nodes": [{
                    "name": "n1",
                    "task_id": "00000000-0000-0000-0000-000000000001",
                    "handler": "h1",
                    "inputs": [],
                    "outputs": []
                }],
                "edges": [],
                "root": "n1"
            }
        })
        .to_string();
        let parsed = parse_workflow_from_json(&json).unwrap();
        assert_eq!(parsed.name, "minimal");
        assert_eq!(parsed.graph.nodes.len(), 1);
        assert_eq!(parsed.graph.root.as_ref().map(|r| r.as_ref()), Some("n1"));
    }

    #[test]
    fn parse_workflow_with_simple_edge() {
        let json = serde_json::json!({
            "name": "two_node",
            "graph": {
                "name": "two_node",
                "nodes": [
                    { "name": "a", "task_id": "00000000-0000-0000-0000-000000000001", "handler": "h1", "inputs": [], "outputs": [] },
                    { "name": "b", "task_id": "00000000-0000-0000-0000-000000000002", "handler": "h2", "inputs": [], "outputs": [] }
                ],
                "edges": [{ "from": "a", "edges": [{ "type": "simple", "to": "b" }] }],
                "root": "a"
            }
        })
        .to_string();
        let parsed = parse_workflow_from_json(&json).unwrap();
        assert_eq!(parsed.graph.nodes.len(), 2);
        assert_eq!(parsed.graph.edges.len(), 1);
    }

    #[test]
    fn parse_workflow_with_conditional_edge() {
        let json = serde_json::json!({
            "name": "cond",
            "graph": {
                "name": "cond",
                "nodes": [
                    { "name": "n1", "task_id": "00000000-0000-0000-0000-000000000001", "handler": "h1", "inputs": [], "outputs": [] },
                    { "name": "then", "task_id": "00000000-0000-0000-0000-000000000002", "handler": "h2", "inputs": [], "outputs": [] },
                    { "name": "else", "task_id": "00000000-0000-0000-0000-000000000003", "handler": "h3", "inputs": [], "outputs": [] }
                ],
                "edges": [{ "from": "n1", "edges": [{ "type": "conditional", "then": "then", "otherwise": "else" }] }],
                "root": "n1"
            }
        })
        .to_string();
        let parsed = parse_workflow_from_json(&json).unwrap();
        assert_eq!(parsed.graph.nodes.len(), 3);
        assert_eq!(parsed.graph.edges.len(), 1);
    }

    #[test]
    fn parse_invalid_json_fails() {
        assert!(parse_workflow_from_json("not json").is_err());
        assert!(parse_workflow_from_json("{}").is_err());
    }
}
