//! Order pipeline: Order -> process -> charge -> Receipt.
//!
//! Linear two-step workflow. Test cases assert different inputs produce expected outputs.

#![allow(missing_docs)]

use flow_raft::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payment {
    pub order_id: String,
    pub amount: f64,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub order_id: String,
    pub payment_id: String,
    pub total: f64,
}

fn process_order(order: Order) -> Result<Payment, String> {
    Ok(Payment {
        order_id: order.id.clone(),
        amount: order.amount,
        status: "processed".to_string(),
    })
}

fn charge_payment(payment: Payment) -> Result<Receipt, String> {
    Ok(Receipt {
        order_id: payment.order_id.clone(),
        payment_id: format!("pay_{}", payment.order_id),
        total: payment.amount,
    })
}

/// Builds the order pipeline graph: process -> charge.
pub fn order_pipeline_graph() -> TypedGraph {
    let mut b = TypedGraphBuilder::new("order_pipeline").with_retry_config(RetryConfig::new(3));
    b.add_node("process", node(process_order), None)
        .add_node("charge", node(charge_payment), None)
        .add_simple_edge("process", "charge")
        .set_root("process");
    b.build().expect("order_pipeline graph")
}

/// Extracts the final Receipt from a completed workflow by reading the "charge" task output.
pub fn receipt_from_snapshot(s: &flow_raft_core::WorkflowSnapshot) -> Option<Receipt> {
    let charge_id = s
        .task_definitions
        .iter()
        .find(|(_, d)| d.name == "charge")?
        .0;
    let out = s.executions.get(charge_id)?.outputs.as_ref()?;
    serde_json::from_value(out.clone()).ok()
}

/// (input, expected_output) cases for order_pipeline.
pub fn order_pipeline_cases() -> Vec<(Order, Receipt)> {
    vec![
        (
            Order {
                id: "o1".into(),
                amount: 10.0,
            },
            Receipt {
                order_id: "o1".into(),
                payment_id: "pay_o1".into(),
                total: 10.0,
            },
        ),
        (
            Order {
                id: "order_123".into(),
                amount: 99.99,
            },
            Receipt {
                order_id: "order_123".into(),
                payment_id: "pay_order_123".into(),
                total: 99.99,
            },
        ),
        (
            Order {
                id: "x".into(),
                amount: 0.01,
            },
            Receipt {
                order_id: "x".into(),
                payment_id: "pay_x".into(),
                total: 0.01,
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow_raft_raft::command::WorkflowCommandBuilder;
    use std::sync::Arc;

    async fn run_order_pipeline(
        input: Order,
        max_iter: usize,
    ) -> Result<flow_raft_core::WorkflowSnapshot, Box<dyn std::error::Error>> {
        let g = order_pipeline_graph();
        let def = g.workflow_def("order_pipeline")?;
        let app = FlowRaftAppBuilder::new()
            .with_node_id(1)
            .with_workflows(vec![def.clone()])
            .enable_metrics(false)
            .build_single_node()
            .await?;
        let registry = Arc::new(HandlerRegistry::new());
        register_typed_graph_handlers(registry.as_ref(), def.workflow_id, &g).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        if let Some(mut w) = app.get_workflow(&def.workflow_id).await {
            w.inputs = serde_json::to_value(&input)?;
            app.create_workflow(WorkflowCommandBuilder::transition_workflow(
                def.workflow_id,
                w,
            ))
            .await
            .map_err(|e| format!("transition: {:?}", e))?;
        }

        let exec = Arc::new(WorkflowExecutor::new(
            app.raft().clone(),
            app.state_machine().clone(),
            1,
        ));
        let handler = HandlerExecutor::new(exec, registry);
        handler.execute_workflow(def.workflow_id, max_iter).await?;
        app.get_workflow(&def.workflow_id)
            .await
            .ok_or_else(|| "workflow not found".into())
    }

    #[tokio::test]
    async fn order_pipeline_different_inputs_produce_expected_outputs() {
        for (input, expected) in order_pipeline_cases() {
            let snap = run_order_pipeline(input.clone(), 50).await.expect("run");
            assert!(
                matches!(snap.state, flow_raft_core::WorkflowState::Completed),
                "state {:?}",
                snap.state
            );
            let got = receipt_from_snapshot(&snap).expect("receipt");
            assert_eq!(got, expected, "input: {:?}", input);
        }
    }
}
