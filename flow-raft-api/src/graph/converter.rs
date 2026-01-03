//! Graph to Workflow converter
//!
//! Converts graph builder structures to FlowRaft's Workflow structure.

use indexmap::IndexMap;

use crate::graph::builder::Graph;
use flow_raft_core::{
    RetryConfig, Task, TaskDependencies, Workflow, WorkflowDraft, WorkflowError, WorkflowId,
};

/// Converts a Graph to a Workflow<WorkflowDraft>
///
/// This function:
/// 1. Creates tasks from graph nodes
/// 2. Builds task dependencies from graph edges
/// 3. Creates a workflow with all tasks in Draft state
///
/// # Arguments
/// * `graph` - The graph to convert
/// * `workflow_id` - The workflow ID to use
/// * `default_retry_config` - Default retry configuration for tasks
/// * `inputs` - Initial workflow inputs
///
/// # Returns
/// A Workflow in Draft state ready to be scheduled
pub fn graph_to_workflow(
    graph: Graph,
    workflow_id: WorkflowId,
    default_retry_config: RetryConfig,
    inputs: serde_json::Value,
) -> Result<Workflow<WorkflowDraft>, WorkflowError> {
    let mut workflow = Workflow::new(workflow_id, inputs);

    // First pass: Add all nodes as tasks
    for (_node_name, graph_node) in &graph.nodes {
        let task = Task::new(
            graph_node.task_id,
            graph_node.name.as_ref(),
            &graph_node.handler,
            default_retry_config.clone(),
            TaskDependencies::default(), // Will be set in second pass
        );

        workflow = workflow.add_task(task, default_retry_config.clone())?;
    }

    // Second pass: Build dependencies from edges
    // For simple edges: target depends on source
    // For conditional/split edges: all targets depend on source
    for (from_name, edges) in &graph.edges {
        let from_node = graph
            .nodes
            .get(from_name)
            .ok_or_else(|| WorkflowError::DependencyNotFound(from_name.as_ref().to_string()))?;
        let from_task_id = from_node.task_id;

        for edge in edges {
            match edge {
                crate::graph::builder::EdgeSpec::Simple(to_name) => {
                    let to_node = graph.nodes.get(to_name).ok_or_else(|| {
                        WorkflowError::DependencyNotFound(to_name.as_ref().to_string())
                    })?;
                    let to_task_id = to_node.task_id;

                    // Add dependency: to depends on from
                    if let Some(deps) = workflow.dependencies.get_mut(&to_task_id) {
                        deps.add_prerequisite(from_task_id);
                    }
                }
                crate::graph::builder::EdgeSpec::Conditional {
                    then, otherwise, ..
                } => {
                    // Both branches depend on the source
                    for branch_name in [then, otherwise] {
                        if let Some(branch_node) = graph.nodes.get(branch_name) {
                            let branch_task_id = branch_node.task_id;
                            if let Some(deps) = workflow.dependencies.get_mut(&branch_task_id) {
                                deps.add_prerequisite(from_task_id);
                            }
                        }
                    }
                }
                crate::graph::builder::EdgeSpec::Split { targets, .. } => {
                    // All split targets depend on the source
                    for target_name in targets {
                        if let Some(target_node) = graph.nodes.get(target_name) {
                            let target_task_id = target_node.task_id;
                            if let Some(deps) = workflow.dependencies.get_mut(&target_task_id) {
                                deps.add_prerequisite(from_task_id);
                            }
                        }
                    }
                }
            }
        }
    }

    // Third pass: Handle merge specs
    // For merge targets, they depend on all merge sources
    for (target_name, (sources, _merge)) in &graph.merge_specs {
        if let Some(target_node) = graph.nodes.get(target_name) {
            let target_task_id = target_node.task_id;

            for source_name in sources {
                if let Some(source_node) = graph.nodes.get(source_name) {
                    let source_task_id = source_node.task_id;
                    if let Some(deps) = workflow.dependencies.get_mut(&target_task_id) {
                        deps.add_prerequisite(source_task_id);
                    }
                }
            }
        }
    }

    Ok(workflow)
}

/// Converts a DynamicGraph to a Workflow<WorkflowDraft>
///
/// Similar to graph_to_workflow but works with dynamic graphs that use
/// serde_json::Value for inputs/outputs.
pub fn dynamic_graph_to_workflow(
    graph: crate::graph::dynamic::DynamicGraph,
    workflow_id: WorkflowId,
    default_retry_config: RetryConfig,
    inputs: serde_json::Value,
) -> Result<Workflow<WorkflowDraft>, WorkflowError> {
    // Convert dynamic graph nodes to regular graph nodes
    let mut nodes_map = IndexMap::new();
    for (name, dyn_node) in &graph.nodes {
        let graph_node = crate::graph::builder::GraphNode {
            name: name.clone(),
            task_id: dyn_node.task_id,
            handler: dyn_node.handler.clone(),
            inputs: dyn_node.inputs.clone(),
            outputs: dyn_node.outputs.clone(),
            timeout_secs: dyn_node.timeout_secs,
        };
        nodes_map.insert(name.clone(), graph_node);
    }

    // Convert dynamic edges to regular edges
    let mut edges_map = IndexMap::new();
    for (from_name, dyn_edges) in &graph.edges {
        let mut edges = Vec::new();
        for dyn_edge in dyn_edges {
            match dyn_edge {
                crate::graph::dynamic::DynamicEdgeSpec::Simple(to_name) => {
                    edges.push(crate::graph::builder::EdgeSpec::Simple(to_name.clone()));
                }
                crate::graph::dynamic::DynamicEdgeSpec::Conditional {
                    condition,
                    then,
                    otherwise,
                } => {
                    edges.push(crate::graph::builder::EdgeSpec::Conditional {
                        condition: condition.clone(),
                        then: then.clone(),
                        otherwise: otherwise.clone(),
                    });
                }
                crate::graph::dynamic::DynamicEdgeSpec::Split { split, targets } => {
                    edges.push(crate::graph::builder::EdgeSpec::Split {
                        split: split.clone(),
                        targets: targets.clone(),
                    });
                }
            }
        }
        edges_map.insert(from_name.clone(), edges);
    }

    // Convert merge specs
    let mut merge_specs = std::collections::HashMap::new();
    for (target_name, (sources, merge)) in &graph.merge_specs {
        merge_specs.insert(target_name.clone(), (sources.clone(), merge.clone()));
    }

    let regular_graph = Graph {
        name: graph.name.clone(),
        nodes: nodes_map,
        edges: edges_map,
        root: graph.root.clone(),
        merge_specs,
    };

    graph_to_workflow(regular_graph, workflow_id, default_retry_config, inputs)
}
