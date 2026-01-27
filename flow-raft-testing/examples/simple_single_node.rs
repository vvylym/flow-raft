//! Simple single-node workflow example
//!
//! Uses the shared flow_raft_testing::workflows::order_pipeline (Order → process → charge → Receipt)
//! and runs it with one of the canonical test inputs. The same workflow is used in tests and
//! benchmarks.

use flow_raft::prelude::*;
use flow_raft_testing::workflows::{
    order_pipeline_cases, order_pipeline_graph, receipt_from_snapshot, Receipt,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let typed_graph = order_pipeline_graph();
    let workflow_def = typed_graph.workflow_def("order_pipeline")?;

    let app = FlowRaftAppBuilder::new()
        .with_node_id(1)
        .with_workflows(vec![workflow_def.clone()])
        .enable_metrics(true)
        .build_single_node()
        .await?;

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

    let (input, expected) = order_pipeline_cases().into_iter().next().unwrap();
    if let Some(mut workflow) = app.get_workflow(&workflow_id).await {
        workflow.inputs = serde_json::to_value(&input).unwrap();
        let request = flow_raft_raft::command::WorkflowCommandBuilder::transition_workflow(
            workflow_id,
            workflow,
        );
        app.create_workflow(request)
            .await
            .map_err(|e| format!("Failed to update workflow with inputs: {:?}", e))?;
    }

    let handler_executor = HandlerExecutor::new(executor, registry);
    execute_and_display(&handler_executor, &app, workflow_id, 100, Some(&expected)).await?;

    println!("✓ Example completed successfully!");
    Ok(())
}

async fn execute_and_display(
    handler_executor: &HandlerExecutor,
    app: &FlowRaftApp,
    workflow_id: WorkflowId,
    max_iterations: usize,
    expected_receipt: Option<&Receipt>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\nExecuting workflow {}...", workflow_id);

    match handler_executor
        .execute_workflow(workflow_id, max_iterations)
        .await
    {
        Ok(()) => {
            println!("\n✓ Workflow execution completed successfully!");
            if let Some(s) = app.get_workflow(&workflow_id).await {
                println!("\nFinal workflow state: {:?}", s.state);
                println!(
                    "  Tasks completed: {}/{}",
                    s.executions.len(),
                    s.task_definitions.len()
                );
                if let Some(r) = receipt_from_snapshot(&s) {
                    println!(
                        "  Receipt: order_id={} payment_id={} total={}",
                        r.order_id, r.payment_id, r.total
                    );
                    if let Some(exp) = expected_receipt {
                        assert_eq!(&r, exp, "receipt must match expected from test case");
                    }
                }
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("✗ Workflow execution failed: {:?}", e);
            Err(Box::new(std::io::Error::other(format!(
                "Execution failed: {:?}",
                e
            ))))
        }
    }
}
