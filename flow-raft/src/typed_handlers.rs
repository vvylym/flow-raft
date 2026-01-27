//! Helpers to use typed workflow graphs with the handler registry.
//!
//! [register_typed_graph_handlers] registers all node functions from a [TypedGraph][flow_raft_api::graph::TypedGraph]
//! as [TaskHandler]s so the workflow engine can execute them.

use std::collections::HashMap;
use std::sync::Arc;

use flow_raft_core::WorkflowId;
use flow_raft_raft::executor::TaskHandler;
use flow_raft_server::handlers::HandlerRegistry;

use flow_raft_api::graph::EdgeSpec;
use flow_raft_api::graph::TypedGraph;
use flow_raft_api::graph::builder::NodeFunction;

/// Wraps a [NodeFunction] so it can be used as a [TaskHandler].
struct NodeFunctionHandler(Arc<dyn NodeFunction>);

impl TaskHandler for NodeFunctionHandler {
    fn execute(
        &self,
        _task_id: flow_raft_core::TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.0.execute(inputs)
    }
}

/// Registers all handlers from a typed graph with the registry.
///
/// Call this after creating a [TypedGraph] and before running the workflow.
/// Handler names in the registry are the same as in the graph (e.g. `fn_process`, `fn_charge`).
pub async fn register_typed_graph_handlers(
    registry: &HandlerRegistry,
    workflow_id: WorkflowId,
    graph: &TypedGraph,
) {
    for (name, func) in graph.handlers() {
        registry
            .register_handler(
                workflow_id,
                name.clone(),
                Arc::new(NodeFunctionHandler(func.clone())) as Arc<dyn TaskHandler>,
            )
            .await;
    }

    let g = graph.graph();
    let merge_specs: HashMap<
        String,
        (
            Vec<String>,
            Arc<dyn flow_raft_api::graph::builder::MergeObject>,
        ),
    > = g
        .merge_specs
        .iter()
        .map(|(k, (v, m))| {
            (
                k.as_ref().to_string(),
                (
                    v.iter().map(|n| n.as_ref().to_string()).collect(),
                    Arc::clone(m),
                ),
            )
        })
        .collect();
    let mut conditional_edges = Vec::new();
    for (from, edge_list) in &g.edges {
        for e in edge_list {
            if let EdgeSpec::Conditional {
                then,
                otherwise,
                condition,
            } = e
            {
                conditional_edges.push((
                    from.as_ref().to_string(),
                    then.as_ref().to_string(),
                    otherwise.as_ref().to_string(),
                    Arc::clone(condition),
                ));
            }
        }
    }
    registry
        .register_graph_specs(workflow_id, merge_specs, conditional_edges)
        .await;
}
