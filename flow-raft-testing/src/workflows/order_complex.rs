//! Complex workflow: validate -> [process|reject]; process -> split -> [c0,c1] -> merge -> finalize.
//!
//! Input OrderInput { order_id, items, valid }. valid=false -> RejectResult; valid=true -> MergeResult.

#![allow(missing_docs)]

use flow_raft::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderInput {
    pub order_id: String,
    pub items: Vec<String>,
    pub valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub order_id: String,
    pub valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResult {
    pub order_id: String,
    pub payment_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RejectResult {
    pub order_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCheckResult {
    pub order_id: String,
    pub item_id: String,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MergeResult {
    pub order_id: String,
    pub all_available: bool,
}

fn validate_order(inp: OrderInput) -> Result<ValidationResult, String> {
    Ok(ValidationResult {
        order_id: inp.order_id,
        valid: inp.valid,
    })
}
fn process_payment(v: ValidationResult) -> Result<PaymentResult, String> {
    Ok(PaymentResult {
        order_id: v.order_id.clone(),
        payment_id: format!("pay_{}", v.order_id),
    })
}
fn reject_order(v: ValidationResult) -> Result<RejectResult, String> {
    Ok(RejectResult {
        order_id: v.order_id,
        reason: "validation_failed".into(),
    })
}
fn check_inv_0(p: PaymentResult) -> Result<ItemCheckResult, String> {
    Ok(ItemCheckResult {
        order_id: p.order_id,
        item_id: "item_0".into(),
        available: true,
    })
}
fn check_inv_1(p: PaymentResult) -> Result<ItemCheckResult, String> {
    Ok(ItemCheckResult {
        order_id: p.order_id,
        item_id: "item_1".into(),
        available: true,
    })
}
fn finalize(m: MergeResult) -> Result<MergeResult, String> {
    Ok(m)
}

pub fn order_complex_graph() -> TypedGraph {
    let mut b = TypedGraphBuilder::new("order_complex").with_retry_config(RetryConfig::new(3));
    b.add_node("validate_order", node(validate_order), None)
        .add_node("process_payment", node(process_payment), None)
        .add_node("reject_order", node(reject_order), None)
        .add_node("check_inventory_0", node(check_inv_0), None)
        .add_node("check_inventory_1", node(check_inv_1), None)
        .add_node("finalize", node(finalize), None)
        .add_conditional_edge(
            "validate_order",
            condition(|r: ValidationResult| r.valid),
            "process_payment",
            "reject_order",
        )
        .add_split_edge(
            "process_payment",
            split(|_p: PaymentResult| {
                Ok(vec!["check_inventory_0".into(), "check_inventory_1".into()])
            }),
            vec!["check_inventory_0", "check_inventory_1"],
        )
        .add_merge_edge(
            vec!["check_inventory_0", "check_inventory_1"],
            merge(|inputs: Vec<ItemCheckResult>| {
                let order_id = inputs
                    .first()
                    .map(|r| r.order_id.clone())
                    .unwrap_or_else(|| "?".into());
                let all_available = inputs.iter().all(|r| r.available);
                Ok::<MergeResult, String>(MergeResult {
                    order_id,
                    all_available,
                })
            }),
            "finalize",
        )
        .set_root("validate_order");
    b.build().expect("order_complex")
}

pub fn merge_result_from_snapshot(s: &flow_raft_core::WorkflowSnapshot) -> Option<MergeResult> {
    for (_, exec) in &s.executions {
        if let Some(ref out) = exec.outputs
            && let Ok(m) = serde_json::from_value::<MergeResult>(out.clone())
        {
            return Some(m);
        }
    }
    None
}

pub fn reject_result_from_snapshot(s: &flow_raft_core::WorkflowSnapshot) -> Option<RejectResult> {
    for (_, exec) in &s.executions {
        if let Some(ref out) = exec.outputs
            && let Ok(r) = serde_json::from_value::<RejectResult>(out.clone())
        {
            return Some(r);
        }
    }
    None
}

pub fn order_complex_cases() -> Vec<(OrderInput, Option<MergeResult>, Option<RejectResult>)> {
    vec![
        (
            OrderInput {
                order_id: "ORD1".into(),
                items: vec!["a".into(), "b".into()],
                valid: true,
            },
            Some(MergeResult {
                order_id: "ORD1".into(),
                all_available: true,
            }),
            None,
        ),
        (
            OrderInput {
                order_id: "ORD2".into(),
                items: vec![] as Vec<String>,
                valid: false,
            },
            None,
            Some(RejectResult {
                order_id: "ORD2".into(),
                reason: "validation_failed".into(),
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow_raft_raft::command::WorkflowCommandBuilder;
    use std::sync::Arc;

    async fn run(
        inp: OrderInput,
        max: usize,
    ) -> Result<flow_raft_core::WorkflowSnapshot, Box<dyn std::error::Error>> {
        let g = order_complex_graph();
        let def = g.workflow_def("order_complex")?;
        let app = FlowRaftAppBuilder::new()
            .with_node_id(1)
            .with_workflows(vec![def.clone()])
            .enable_metrics(false)
            .build_single_node()
            .await?;
        let reg = Arc::new(HandlerRegistry::new());
        register_typed_graph_handlers(reg.as_ref(), def.workflow_id, &g).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        if let Some(mut w) = app.get_workflow(&def.workflow_id).await {
            w.inputs = serde_json::to_value(&inp)?;
            app.create_workflow(WorkflowCommandBuilder::transition_workflow(
                def.workflow_id,
                w,
            ))
            .await
            .map_err(|e| format!("{:?}", e))?;
        }
        let exec = Arc::new(WorkflowExecutor::new(
            app.raft().clone(),
            app.state_machine().clone(),
            1,
        ));
        HandlerExecutor::new(exec, reg)
            .execute_workflow(def.workflow_id, max)
            .await?;
        app.get_workflow(&def.workflow_id)
            .await
            .ok_or_else(|| "no workflow".into())
    }

    /// Merge/split wiring may differ; disabled until engine behavior is confirmed.
    #[tokio::test]
    async fn complex_different_inputs_produce_expected_branches() {
        for (inp, want_merge, want_rej) in order_complex_cases() {
            let s = run(inp, 200).await.expect("run");
            assert!(matches!(s.state, flow_raft_core::WorkflowState::Completed));
            if let Some(e) = want_merge {
                assert_eq!(merge_result_from_snapshot(&s), Some(e));
            }
            if let Some(e) = want_rej {
                assert_eq!(reject_result_from_snapshot(&s), Some(e));
            }
        }
    }
}
