//! Parallel workflow example
//!
//! Uses the shared flow_raft_testing::workflows::order_parallel: split → [process_item_0, 1, 2] → merge
//! → finalize. Runs with one of the canonical test inputs.
//!
//! **Note:** The merge→finalize step may fail with "missing field `order_id`" until the
//! engine's merge-to-target input wiring is adjusted. The workflow and tests are
//! retained for when that is fixed.

use flow_raft::prelude::*;
use flow_raft_testing::workflows::{
    order_parallel_cases, order_parallel_graph, order_result_from_snapshot,
};

#[tokio::main]
async fn main() -> Result<(), String> {
    tracing_subscriber::fmt::init();

    let typed_graph = order_parallel_graph();
    let workflow_def = typed_graph
        .workflow_def("order_parallel")
        .map_err(|e| e.to_string())?;

    let app = FlowRaftAppBuilder::new()
        .with_node_id(1)
        .with_workflows(vec![workflow_def.clone()])
        .enable_metrics(true)
        .build_single_node()
        .await
        .map_err(|e| e.to_string())?;

    println!("✓ FlowRaft app created successfully!");

    let workflow_id = workflow_def.workflow_id;
    let executor = Arc::new(WorkflowExecutor::new(
        app.raft().clone(),
        app.state_machine().clone(),
        1,
    ));
    let registry = Arc::new(HandlerRegistry::new());
    register_typed_graph_handlers(registry.as_ref(), workflow_id, &typed_graph).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let (input, expected) = order_parallel_cases().into_iter().next().unwrap();
    if let Some(mut w) = app.get_workflow(&workflow_id).await {
        w.inputs = serde_json::to_value(&input).unwrap();
        let req =
            flow_raft_raft::command::WorkflowCommandBuilder::transition_workflow(workflow_id, w);
        app.create_workflow(req)
            .await
            .map_err(|e| format!("{:?}", e))?;
    }

    let handler_executor = HandlerExecutor::new(executor, registry);
    println!("\nExecuting workflow {}...", workflow_id);
    match handler_executor.execute_workflow(workflow_id, 100).await {
        Ok(()) => {
            println!("\n✓ Workflow execution completed successfully!");
            if let Some(s) = app.get_workflow(&workflow_id).await {
                println!("  State: {:?}", s.state);
                if let Some(o) = order_result_from_snapshot(&s) {
                    println!(
                        "  OrderResult: order_id={} items_processed={}",
                        o.order_id, o.items_processed
                    );
                    assert_eq!(
                        o, expected,
                        "OrderResult must match expected from test case"
                    );
                }
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("✗ Workflow execution failed: {:?}", e);
            Err(format!("{:?}", e))
        }
    }
}
