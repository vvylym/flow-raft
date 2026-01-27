//! Graph validation utilities
//!
//! Provides validation for graphs including cycle detection and reachability analysis.

use indexmap::IndexMap;
use rayon::prelude::*;
use std::collections::{HashSet, VecDeque};

use crate::graph::builder::{Graph, NodeName};

// Type alias for complex return type
type DependencyGraph = (
    IndexMap<NodeName, ()>,
    IndexMap<NodeName, Vec<NodeName>>,
    IndexMap<NodeName, Vec<NodeName>>,
);

/// Graph validation error
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphValidationError {
    /// Cycle detected in the graph
    CycleDetected {
        /// Nodes involved in the cycle
        cycle: Vec<String>,
    },
    /// Node referenced in edge but not found
    NodeNotFound {
        /// Node name that was not found
        node: String,
        /// Edge that references the missing node
        edge: String,
    },
    /// Node is unreachable from root
    UnreachableNode {
        /// Unreachable node name
        node: String,
    },
    /// Multiple root nodes specified
    MultipleRoots {
        /// Root nodes
        roots: Vec<String>,
    },
    /// No root node specified and cannot infer
    NoRoot,
    /// Input/output type mismatch
    TypeMismatch {
        /// Source node
        from: String,
        /// Target node
        to: String,
        /// Expected type
        expected: String,
        /// Actual type
        actual: String,
    },
}

impl std::fmt::Display for GraphValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphValidationError::CycleDetected { cycle } => {
                write!(f, "Cycle detected: {}", cycle.join(" -> "))
            }
            GraphValidationError::NodeNotFound { node, edge } => {
                write!(
                    f,
                    "Node '{}' not found (referenced in edge from '{}')",
                    node, edge
                )
            }
            GraphValidationError::UnreachableNode { node } => {
                write!(f, "Node '{}' is unreachable from root", node)
            }
            GraphValidationError::MultipleRoots { roots } => {
                write!(f, "Multiple root nodes specified: {}", roots.join(", "))
            }
            GraphValidationError::NoRoot => {
                write!(f, "No root node specified and cannot infer")
            }
            GraphValidationError::TypeMismatch {
                from,
                to,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Type mismatch from '{}' to '{}': expected {}, got {}",
                    from, to, expected, actual
                )
            }
        }
    }
}

impl std::error::Error for GraphValidationError {}

/// Validates a graph structure
///
/// Performs comprehensive validation including:
/// - Cycle detection
/// - Node existence verification
/// - Reachability analysis
/// - Root node validation
///
/// Uses parallel algorithms for performance.
pub fn validate_graph(graph: &Graph) -> Result<(), GraphValidationError> {
    // Check all edges reference existing nodes
    for (from_name, edges) in &graph.edges {
        for edge in edges {
            match edge {
                crate::graph::builder::EdgeSpec::Simple(to) => {
                    if !graph.nodes.contains_key(to) {
                        return Err(GraphValidationError::NodeNotFound {
                            node: to.as_ref().to_string(),
                            edge: from_name.as_ref().to_string(),
                        });
                    }
                }
                crate::graph::builder::EdgeSpec::Conditional {
                    then, otherwise, ..
                } => {
                    if !graph.nodes.contains_key(then) {
                        return Err(GraphValidationError::NodeNotFound {
                            node: then.as_ref().to_string(),
                            edge: from_name.as_ref().to_string(),
                        });
                    }
                    if !graph.nodes.contains_key(otherwise) {
                        return Err(GraphValidationError::NodeNotFound {
                            node: otherwise.as_ref().to_string(),
                            edge: from_name.as_ref().to_string(),
                        });
                    }
                }
                crate::graph::builder::EdgeSpec::Split { targets, .. } => {
                    for target in targets {
                        if !graph.nodes.contains_key(target) {
                            return Err(GraphValidationError::NodeNotFound {
                                node: target.as_ref().to_string(),
                                edge: from_name.as_ref().to_string(),
                            });
                        }
                    }
                }
                crate::graph::builder::EdgeSpec::Switch { branches, .. } => {
                    for b in branches {
                        if !graph.nodes.contains_key(b) {
                            return Err(GraphValidationError::NodeNotFound {
                                node: b.as_ref().to_string(),
                                edge: from_name.as_ref().to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    // Check merge specs reference existing nodes
    for (target, (sources, _)) in &graph.merge_specs {
        if !graph.nodes.contains_key(target) {
            return Err(GraphValidationError::NodeNotFound {
                node: target.as_ref().to_string(),
                edge: "merge".to_string(),
            });
        }
        for source in sources {
            if !graph.nodes.contains_key(source) {
                return Err(GraphValidationError::NodeNotFound {
                    node: source.as_ref().to_string(),
                    edge: format!("merge to {}", target.as_ref()),
                });
            }
        }
    }

    // Validate root node
    if let Some(root) = &graph.root {
        if !graph.nodes.contains_key(root) {
            return Err(GraphValidationError::NodeNotFound {
                node: root.as_ref().to_string(),
                edge: "root".to_string(),
            });
        }
    } else if graph.nodes.is_empty() {
        return Err(GraphValidationError::NoRoot);
    }

    // Build dependency graph for cycle detection
    let (tasks, dependencies, dependents) = build_dependency_graph(graph);

    // Check for cycles using parallel algorithm
    validate_dag_parallel(&tasks, &dependencies, &dependents).map_err(|_| {
        // Find the cycle for better error message
        let cycle = find_cycle(graph);
        GraphValidationError::CycleDetected {
            cycle: cycle.unwrap_or_else(|| vec!["unknown".to_string()]),
        }
    })?;

    // Check reachability from root
    if let Some(root) = &graph.root {
        let reachable = compute_reachable_nodes(graph, root);
        let all_nodes: HashSet<&NodeName> = graph.nodes.keys().collect();
        let unreachable: Vec<String> = all_nodes
            .difference(&reachable)
            .map(|n| n.as_ref().to_string())
            .collect();
        if !unreachable.is_empty() {
            return Err(GraphValidationError::UnreachableNode {
                node: unreachable.join(", "),
            });
        }
    }

    Ok(())
}

/// Builds dependency graph from Graph structure
fn build_dependency_graph(graph: &Graph) -> DependencyGraph {
    let mut tasks = IndexMap::new();
    let mut dependencies: IndexMap<NodeName, Vec<NodeName>> = IndexMap::new();
    let mut dependents: IndexMap<NodeName, Vec<NodeName>> = IndexMap::new();

    // Initialize tasks
    for node_name in graph.nodes.keys() {
        tasks.insert(node_name.clone(), ());
        dependencies.insert(node_name.clone(), Vec::new());
        dependents.insert(node_name.clone(), Vec::new());
    }

    // Build dependencies from edges
    for (from_name, edges) in &graph.edges {
        for edge in edges {
            match edge {
                crate::graph::builder::EdgeSpec::Simple(to) => {
                    dependencies
                        .entry(to.clone())
                        .or_default()
                        .push(from_name.clone());
                    dependents
                        .entry(from_name.clone())
                        .or_default()
                        .push(to.clone());
                }
                crate::graph::builder::EdgeSpec::Conditional {
                    then, otherwise, ..
                } => {
                    dependencies
                        .entry(then.clone())
                        .or_default()
                        .push(from_name.clone());
                    dependencies
                        .entry(otherwise.clone())
                        .or_default()
                        .push(from_name.clone());
                    dependents
                        .entry(from_name.clone())
                        .or_default()
                        .push(then.clone());
                    dependents
                        .entry(from_name.clone())
                        .or_default()
                        .push(otherwise.clone());
                }
                crate::graph::builder::EdgeSpec::Split { targets, .. } => {
                    for target in targets {
                        dependencies
                            .entry(target.clone())
                            .or_default()
                            .push(from_name.clone());
                        dependents
                            .entry(from_name.clone())
                            .or_default()
                            .push(target.clone());
                    }
                }
                crate::graph::builder::EdgeSpec::Switch { branches, .. } => {
                    for b in branches {
                        dependencies
                            .entry(b.clone())
                            .or_default()
                            .push(from_name.clone());
                        dependents
                            .entry(from_name.clone())
                            .or_default()
                            .push(b.clone());
                    }
                }
            }
        }
    }

    // Add merge dependencies
    for (target, (sources, _)) in &graph.merge_specs {
        for source in sources {
            dependencies
                .entry(target.clone())
                .or_default()
                .push(source.clone());
            dependents
                .entry(source.clone())
                .or_default()
                .push(target.clone());
        }
    }

    (tasks, dependencies, dependents)
}

/// Validates DAG using parallel algorithm (adapted from flow-raft-core)
fn validate_dag_parallel(
    tasks: &IndexMap<NodeName, ()>,
    dependencies: &IndexMap<NodeName, Vec<NodeName>>,
    dependents: &IndexMap<NodeName, Vec<NodeName>>,
) -> Result<(), ()> {
    // Initialize in-degree using parallel processing
    let mut in_degree: IndexMap<NodeName, usize> = tasks
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|node_name| (node_name, 0))
        .collect();

    // Calculate in-degree using parallel processing
    let in_degree_updates: Vec<(NodeName, usize)> = dependencies
        .par_iter()
        .map(|(dependent, deps)| (dependent.clone(), deps.len()))
        .collect();

    for (dependent, degree) in in_degree_updates {
        *in_degree.entry(dependent).or_insert(0) = degree;
    }

    // Find nodes with zero in-degree using parallel processing
    let mut queue: VecDeque<NodeName> = in_degree
        .par_iter()
        .filter(|&(_, &degree)| degree == 0)
        .map(|(name, _)| name.clone())
        .collect();

    let mut visited = 0;
    while let Some(node_name) = queue.pop_front() {
        visited += 1;

        if let Some(dependent_nodes) = dependents.get(&node_name) {
            for dependent in dependent_nodes {
                let degree = in_degree.get_mut(dependent).ok_or(())?;
                *degree -= 1;
                if *degree == 0 {
                    queue.push_back(dependent.clone());
                }
            }
        }
    }

    if visited != tasks.len() {
        Err(())
    } else {
        Ok(())
    }
}

/// Finds a cycle in the graph (for error reporting)
fn find_cycle(graph: &Graph) -> Option<Vec<String>> {
    // Simple DFS to find cycle
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();
    let mut path = Vec::new();

    fn dfs(
        node: &NodeName,
        graph: &Graph,
        visited: &mut HashSet<NodeName>,
        rec_stack: &mut HashSet<NodeName>,
        path: &mut Vec<NodeName>,
    ) -> Option<Vec<NodeName>> {
        visited.insert(node.clone());
        rec_stack.insert(node.clone());
        path.push(node.clone());

        if let Some(edges) = graph.edges.get(node) {
            for edge in edges {
                let targets = match edge {
                    crate::graph::builder::EdgeSpec::Simple(to) => vec![to.clone()],
                    crate::graph::builder::EdgeSpec::Conditional {
                        then, otherwise, ..
                    } => {
                        vec![then.clone(), otherwise.clone()]
                    }
                    crate::graph::builder::EdgeSpec::Split { targets, .. } => targets.clone(),
                    crate::graph::builder::EdgeSpec::Switch { branches, .. } => branches.clone(),
                };

                for target in targets {
                    if !visited.contains(&target) {
                        if let Some(cycle) = dfs(&target, graph, visited, rec_stack, path) {
                            return Some(cycle);
                        }
                    } else if rec_stack.contains(&target) {
                        // Found cycle
                        let cycle_start = path.iter().position(|n| n == &target)?;
                        return Some(path[cycle_start..].to_vec());
                    }
                }
            }
        }

        rec_stack.remove(node);
        path.pop();
        None
    }

    for node_name in graph.nodes.keys() {
        if !visited.contains(node_name)
            && let Some(cycle) = dfs(node_name, graph, &mut visited, &mut rec_stack, &mut path)
        {
            return Some(cycle.iter().map(|n| n.as_ref().to_string()).collect());
        }
    }

    None
}

/// Computes reachable nodes from root using parallel BFS
fn compute_reachable_nodes<'a>(graph: &'a Graph, root: &'a NodeName) -> HashSet<&'a NodeName> {
    let mut reachable = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(root);
    reachable.insert(root);

    while let Some(node) = queue.pop_front() {
        if let Some(edges) = graph.edges.get(node) {
            for edge in edges {
                let targets: Vec<&NodeName> = match edge {
                    crate::graph::builder::EdgeSpec::Simple(to) => vec![to],
                    crate::graph::builder::EdgeSpec::Conditional {
                        then, otherwise, ..
                    } => {
                        vec![then, otherwise]
                    }
                    crate::graph::builder::EdgeSpec::Split { targets, .. } => {
                        targets.iter().collect()
                    }
                    crate::graph::builder::EdgeSpec::Switch { branches, .. } => {
                        branches.iter().collect()
                    }
                };

                for target in targets {
                    if reachable.insert(target) {
                        queue.push_back(target);
                    }
                }
            }
        }

        // Also check merge specs
        for (target, (sources, _)) in &graph.merge_specs {
            if sources.contains(node) && reachable.insert(target) {
                queue.push_back(target);
            }
        }
    }

    reachable
}
