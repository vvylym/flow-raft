//! Complex workflow example (typed API)
//!
//! Uses the shared flow_raft_testing::workflows::order_complex: validate → [process | reject];
//! process → split → [check_inv_0, check_inv_1] → merge → finalize. Runs with one of the
//! canonical test inputs.
//!
//! **Note:** The merge→finalize step may fail with "missing field `all_available`" until
//! the engine's merge-to-target input wiring is adjusted. The workflow and tests are
//! retained for when that is fixed.

use flow_raft::prelude::*;
use flow_raft_testing::workflows::{
    merge_result_from_snapshot, order_complex_cases, order_complex_graph,
    reject_result_from_snapshot,
};

#[tokio::main]
async fn main() -> Result<(), String> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("Complex workflow (typed API): conditionals + split + merge");
    println!("==========================================================");

    let typed_graph = order_complex_graph();
    let workflow_def = typed_graph
        .workflow_def("order_complex")
        .map_err(|e| e.to_string())?;

    let app = FlowRaftAppBuilder::new()
        .with_node_id(1)
        .with_workflows(vec![workflow_def.clone()])
        .enable_metrics(true)
        .build_single_node()
        .await
        .map_err(|e| e.to_string())?;

    let workflow_id = workflow_def.workflow_id;
    let executor = Arc::new(WorkflowExecutor::new(
        app.raft().clone(),
        app.state_machine().clone(),
        1,
    ));
    let registry = Arc::new(HandlerRegistry::new());
    register_typed_graph_handlers(registry.as_ref(), workflow_id, &typed_graph).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let (input, want_merge, want_rej) = order_complex_cases().into_iter().next().unwrap();
    if let Some(mut w) = app.get_workflow(&workflow_id).await {
        w.inputs = serde_json::to_value(&input).unwrap();
        let req =
            flow_raft_raft::command::WorkflowCommandBuilder::transition_workflow(workflow_id, w);
        app.create_workflow(req)
            .await
            .map_err(|e| format!("{:?}", e))?;
    }

    let handler_executor = HandlerExecutor::new(executor, registry);
    println!("\nExecuting workflow...");
    match handler_executor.execute_workflow(workflow_id, 200).await {
        Ok(()) => {
            println!("\n✓ Workflow completed successfully!");
            if let Some(s) = app.get_workflow(&workflow_id).await {
                println!("  State: {:?}", s.state);
                if let Some(m) = merge_result_from_snapshot(&s) {
                    println!(
                        "  MergeResult: order_id={} all_available={}",
                        m.order_id, m.all_available
                    );
                    if let Some(ref exp) = want_merge {
                        assert_eq!(&m, exp, "MergeResult must match expected");
                    }
                }
                if let Some(r) = reject_result_from_snapshot(&s) {
                    println!(
                        "  RejectResult: order_id={} reason={}",
                        r.order_id, r.reason
                    );
                    if let Some(ref exp) = want_rej {
                        assert_eq!(&r, exp, "RejectResult must match expected");
                    }
                }
            }
        }
        Err(e) => eprintln!("\n✗ Workflow failed: {:?}", e),
    }

    println!("\nExample completed!");
    Ok(())
}
