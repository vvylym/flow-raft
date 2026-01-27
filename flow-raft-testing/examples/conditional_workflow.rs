//! Conditional workflow example
//!
//! Uses the shared flow_raft_testing::workflows::order_conditional (validate → [process | reject]
//! based on `OrderValid.valid`). Runs with one of the canonical test inputs.

use flow_raft::prelude::*;
use flow_raft_testing::workflows::{
    order_conditional_cases, order_conditional_graph, processed_from_snapshot,
    rejected_from_snapshot,
};

#[tokio::main]
async fn main() -> Result<(), String> {
    tracing_subscriber::fmt::init();

    let typed_graph = order_conditional_graph();
    let workflow_def = typed_graph
        .workflow_def("order_conditional")
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

    let (input, want_ok, want_rej) = order_conditional_cases().into_iter().next().unwrap();
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
    match handler_executor.execute_workflow(workflow_id, 100).await {
        Ok(()) => {
            println!("\n✓ Workflow execution completed successfully!");
            if let Some(s) = app.get_workflow(&workflow_id).await {
                println!("  State: {:?}", s.state);
                if let Some(p) = processed_from_snapshot(&s) {
                    println!(
                        "  ProcessedOrder: order_id={} status={}",
                        p.order_id, p.status
                    );
                    if let Some(ref exp) = want_ok {
                        assert_eq!(&p, exp, "processed result must match expected");
                    }
                }
                if let Some(r) = rejected_from_snapshot(&s) {
                    println!(
                        "  RejectedOrder: order_id={} reason={}",
                        r.order_id, r.reason
                    );
                    if let Some(ref exp) = want_rej {
                        assert_eq!(&r, exp, "rejected result must match expected");
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Workflow execution failed: {:?}", e);
            return Err(format!("{:?}", e));
        }
    }

    println!("✓ Conditional workflow example completed!");
    Ok(())
}
