//! Conditional workflow: validate -> [process | reject] based on Order.valid.
//!
//! Two test branches: valid=true -> ProcessedOrder, valid=false -> RejectedOrder.

#![allow(missing_docs)]

use flow_raft::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderValid {
    pub id: String,
    pub amount: f64,
    pub valid: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationResult {
    pub order_id: String,
    pub valid: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessedOrder {
    pub order_id: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RejectedOrder {
    pub order_id: String,
    pub reason: String,
}

fn validate(o: OrderValid) -> Result<ValidationResult, String> {
    Ok(ValidationResult {
        order_id: o.id,
        valid: o.valid,
    })
}
fn process(r: ValidationResult) -> Result<ProcessedOrder, String> {
    Ok(ProcessedOrder {
        order_id: r.order_id,
        status: "processed".into(),
    })
}
fn reject(r: ValidationResult) -> Result<RejectedOrder, String> {
    Ok(RejectedOrder {
        order_id: r.order_id,
        reason: "validation failed".into(),
    })
}

pub fn order_conditional_graph() -> TypedGraph {
    let mut b = TypedGraphBuilder::new("order_conditional");
    b.add_node("validate", node(validate), None)
        .add_node("process", node(process), None)
        .add_node("reject", node(reject), None)
        .add_conditional_edge(
            "validate",
            condition(|r: ValidationResult| r.valid),
            "process",
            "reject",
        )
        .set_root("validate");
    b.build().expect("order_conditional")
}

pub fn processed_from_snapshot(s: &flow_raft_core::WorkflowSnapshot) -> Option<ProcessedOrder> {
    let tid = s
        .task_definitions
        .iter()
        .find(|(_, d)| d.name == "process")?
        .0;
    let out = s.executions.get(tid)?.outputs.as_ref()?;
    serde_json::from_value(out.clone()).ok()
}

pub fn rejected_from_snapshot(s: &flow_raft_core::WorkflowSnapshot) -> Option<RejectedOrder> {
    let tid = s
        .task_definitions
        .iter()
        .find(|(_, d)| d.name == "reject")?
        .0;
    let out = s.executions.get(tid)?.outputs.as_ref()?;
    serde_json::from_value(out.clone()).ok()
}

pub fn order_conditional_cases() -> Vec<(OrderValid, Option<ProcessedOrder>, Option<RejectedOrder>)>
{
    vec![
        (
            OrderValid {
                id: "a".into(),
                amount: 1.0,
                valid: true,
            },
            Some(ProcessedOrder {
                order_id: "a".into(),
                status: "processed".into(),
            }),
            None,
        ),
        (
            OrderValid {
                id: "b".into(),
                amount: 2.0,
                valid: false,
            },
            None,
            Some(RejectedOrder {
                order_id: "b".into(),
                reason: "validation failed".into(),
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
        inp: OrderValid,
        max: usize,
    ) -> Result<flow_raft_core::WorkflowSnapshot, Box<dyn std::error::Error>> {
        let g = order_conditional_graph();
        let def = g.workflow_def("order_conditional")?;
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

    #[tokio::test]
    async fn conditional_different_inputs_produce_expected_branches() {
        for (inp, want_ok, want_rej) in order_conditional_cases() {
            let s = run(inp, 50).await.expect("run");
            assert!(matches!(s.state, flow_raft_core::WorkflowState::Completed));
            if let Some(e) = want_ok {
                assert_eq!(processed_from_snapshot(&s), Some(e));
            }
            if let Some(e) = want_rej {
                assert_eq!(rejected_from_snapshot(&s), Some(e));
            }
        }
    }
}
