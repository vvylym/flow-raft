//! Structural workflows for benchmarks: nop handlers, no I/O.
//!
//! Used to measure registration, scheduling, and execution overhead.

#![allow(missing_docs)]

use flow_raft::prelude::*;

fn nop(_: ()) -> Result<(), String> {
    Ok(())
}

/// Linear chain of n tasks: task_0 -> task_1 -> ... -> task_{n-1}.
pub fn linear_nop_graph(n: usize, name: &str) -> TypedGraph {
    let mut b = TypedGraphBuilder::new(name);
    for i in 0..n {
        b.add_node(format!("task_{}", i), node(nop), None);
        if i > 0 {
            b.add_simple_edge(format!("task_{}", i - 1), format!("task_{}", i));
        }
    }
    b.set_root("task_0");
    b.build().expect("linear_nop")
}

/// Fan-out from "start" to n branches, each feeding "merge".
pub fn parallel_nop_graph(n: usize, name: &str) -> TypedGraph {
    let mut b = TypedGraphBuilder::new(name);
    b.add_node("start", node(nop), None);
    b.add_node("merge", node(nop), None);
    for i in 0..n {
        let t = format!("branch_{}", i);
        b.add_node(&t, node(nop), None);
        b.add_simple_edge("start", &t);
        b.add_simple_edge(&t, "merge");
    }
    b.set_root("start");
    b.build().expect("parallel_nop")
}

/// Conditional: n1 -> condition(true=>n2, false=>n3).
pub fn conditional_nop_graph(name: &str) -> TypedGraph {
    let mut b = TypedGraphBuilder::new(name);
    b.add_node("n1", node(nop), None)
        .add_node("n2", node(nop), None)
        .add_node("n3", node(nop), None)
        .add_conditional_edge("n1", condition(|_: ()| true), "n2", "n3")
        .set_root("n1");
    b.build().expect("conditional_nop")
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow_raft_raft::command::WorkflowCommandBuilder;
    use std::sync::Arc;

    #[tokio::test]
    async fn linear_nop_runs_to_completion() {
        let g = linear_nop_graph(5, "linear_smoke");
        let def = g.workflow_def("linear_smoke").unwrap();
        let app = FlowRaftAppBuilder::new()
            .with_node_id(1)
            .with_workflows(vec![def.clone()])
            .enable_metrics(false)
            .build_single_node()
            .await
            .unwrap();
        let reg = Arc::new(HandlerRegistry::new());
        register_typed_graph_handlers(reg.as_ref(), def.workflow_id, &g).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        if let Some(mut w) = app.get_workflow(&def.workflow_id).await {
            w.inputs = serde_json::Value::Null;
            app.create_workflow(WorkflowCommandBuilder::transition_workflow(
                def.workflow_id,
                w,
            ))
            .await
            .unwrap();
        }
        let exec = Arc::new(WorkflowExecutor::new(
            app.raft().clone(),
            app.state_machine().clone(),
            1,
        ));
        let handler = HandlerExecutor::new(exec, reg);
        handler.execute_workflow(def.workflow_id, 50).await.unwrap();
        let s = app.get_workflow(&def.workflow_id).await.unwrap();
        assert!(matches!(s.state, flow_raft_core::WorkflowState::Completed));
    }
}
