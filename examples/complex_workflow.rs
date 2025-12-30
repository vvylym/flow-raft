//! Complex workflow example
//!
//! Demonstrates a complex e-commerce order processing pipeline with:
//! - Conditional branching
//! - Parallel execution (split/merge)
//! - Error handling and retries
//! - Multiple workflow dependencies

use flow_raft::api::graph::{GraphBuilder, NodeName};
use flow_raft::api::graph::builder::{ConditionObject, MergeObject, SplitObject};
use flow_raft::api::handlers::HandlerRegistry;
use flow_raft::core::{RetryConfig, TaskId, WorkflowId};
use flow_raft::raft::app::FlowRaftApp;
use flow_raft::raft::config::default_config;
use flow_raft::raft::executor::{TaskHandler, WorkflowExecutor};
use flow_raft::raft::network::MemoryNetworkFactory;
use flow_raft::raft::storage::{LogStore, StateMachineStore};
use flow_raft::raft::types::Request;
use flow_raft::api::graph::converter::graph_to_workflow;
use std::sync::Arc;
use futures::future::BoxFuture;

// Order validation condition
#[derive(Debug)]
struct OrderValidationCondition;

impl ConditionObject for OrderValidationCondition {
    fn evaluate(&self, input: serde_json::Value) -> BoxFuture<'static, Result<NodeName, String>> {
        Box::pin(async move {
            let valid = input
                .get("valid")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            
            if valid {
                Ok(NodeName::new("process_payment"))
            } else {
                Ok(NodeName::new("reject_order"))
            }
        })
    }

    fn input_typeid(&self) -> std::any::TypeId {
        std::any::TypeId::of::<serde_json::Value>()
    }
}

// Inventory split - check multiple items in parallel
#[derive(Debug)]
struct InventorySplit;

impl SplitObject for InventorySplit {
    fn split(&self, input: serde_json::Value) -> BoxFuture<'static, Result<Vec<NodeName>, String>> {
        Box::pin(async move {
            let items = input
                .get("items")
                .and_then(|i| i.as_array())
                .ok_or_else(|| "Missing items array".to_string())?;
            
            let mut targets = Vec::new();
            for (idx, _) in items.iter().enumerate() {
                targets.push(NodeName::new(&format!("check_inventory_{}", idx)));
            }
            Ok(targets)
        })
    }

    fn input_typeid(&self) -> std::any::TypeId {
        std::any::TypeId::of::<serde_json::Value>()
    }
}

// Merge inventory results
#[derive(Debug)]
struct InventoryMerge;

impl MergeObject for InventoryMerge {
    fn merge(&self, inputs: Vec<serde_json::Value>) -> BoxFuture<'static, Result<serde_json::Value, String>> {
        Box::pin(async move {
            let mut all_available = true;
            let mut results = serde_json::json!([]);
            
            for input in inputs {
                let available = input
                    .get("available")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                
                if !available {
                    all_available = false;
                }
                
                if let serde_json::Value::Array(ref mut arr) = results {
                    arr.push(input);
                }
            }
            
            Ok(serde_json::json!({
                "all_available": all_available,
                "results": results
            }))
        })
    }
}

// Task handlers
struct OrderHandler;

impl TaskHandler for OrderHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("Processing order: {:?}", inputs);
        Ok(serde_json::json!({
            "order_id": inputs.get("order_id"),
            "status": "validated"
        }))
    }
}

struct PaymentHandler;

impl TaskHandler for PaymentHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("Processing payment: {:?}", inputs);
        Ok(serde_json::json!({
            "payment_id": "pay_123",
            "status": "completed"
        }))
    }
}

struct InventoryHandler {
    item_index: usize,
}

impl TaskHandler for InventoryHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("Checking inventory for item {}: {:?}", self.item_index, inputs);
        Ok(serde_json::json!({
            "item_index": self.item_index,
            "available": true
        }))
    }
}

struct ShippingHandler;

impl TaskHandler for ShippingHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("Shipping order: {:?}", inputs);
        Ok(serde_json::json!({
            "tracking_number": "TRACK123",
            "status": "shipped"
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Complex E-commerce Order Processing Workflow");

    // Build complex workflow graph
    let mut builder = GraphBuilder::new("order_processing")
        .with_default_retry_config(RetryConfig::new(3));

    // Add nodes
    builder
        .add_node("validate_order", "order_handler", vec![], vec![], None)
        .add_node("process_payment", "payment_handler", vec![], vec![], None)
        .add_node("reject_order", "reject_handler", vec![], vec![], None)
        .add_node("check_inventory_0", "inventory_handler_0", vec![], vec![], None)
        .add_node("check_inventory_1", "inventory_handler_1", vec![], vec![], None)
        .add_node("ship_order", "shipping_handler", vec![], vec![], None);

    // Add edges
    builder
        .add_simple_edge("validate_order", "process_payment")
        .add_conditional_edge(
            "validate_order",
            Arc::new(OrderValidationCondition) as Arc<dyn ConditionObject>,
            "process_payment",
            "reject_order",
        )
        .add_split_edge(
            "process_payment",
            Arc::new(InventorySplit) as Arc<dyn SplitObject>,
            vec!["check_inventory_0", "check_inventory_1"],
        )
        .add_merge_edge(
            vec!["check_inventory_0", "check_inventory_1"],
            Arc::new(InventoryMerge) as Arc<dyn MergeObject>,
            "ship_order",
        )
        .set_root("validate_order");

    let graph = builder.build()?;
    println!("Built complex graph with {} nodes", graph.nodes.len());

    // Convert to workflow
    let workflow_id = WorkflowId::default();
    let retry_config = RetryConfig::new(3);
    let workflow = graph_to_workflow(
        graph,
        workflow_id,
        retry_config.clone(),
        serde_json::json!({
            "order_id": "ORD123",
            "items": [{"id": "item1"}, {"id": "item2"}],
            "valid": true
        }),
    )?;

    // Schedule and start
    let scheduled = workflow.schedule()?;
    let running = scheduled.start()?;

    println!("Workflow scheduled and started");

    // Setup Raft infrastructure
    let node_id = 1;
    let config = Arc::new(default_config().validate().unwrap());
    let network = MemoryNetworkFactory::new();
    let log_store = LogStore::default();
    let state_machine = StateMachineStore::default();

    let raft = openraft::Raft::new(node_id, config, network, log_store, state_machine.clone())
        .await?;
    let raft = Arc::new(raft);

    raft.initialize([1u64].into_iter().collect::<std::collections::BTreeSet<_>>())
        .await?;

    let app = Arc::new(FlowRaftApp::new(raft.clone(), state_machine.clone()));
    let executor = Arc::new(WorkflowExecutor::new(raft, state_machine.clone(), node_id));
    let registry = Arc::new(HandlerRegistry::new());

    // Register handlers
    registry
        .register_handler(
            workflow_id,
            "order_handler".to_string(),
            Arc::new(OrderHandler) as Arc<dyn TaskHandler>,
        )
        .await;
    registry
        .register_handler(
            workflow_id,
            "payment_handler".to_string(),
            Arc::new(PaymentHandler) as Arc<dyn TaskHandler>,
        )
        .await;
    registry
        .register_handler(
            workflow_id,
            "inventory_handler_0".to_string(),
            Arc::new(InventoryHandler { item_index: 0 }) as Arc<dyn TaskHandler>,
        )
        .await;
    registry
        .register_handler(
            workflow_id,
            "inventory_handler_1".to_string(),
            Arc::new(InventoryHandler { item_index: 1 }) as Arc<dyn TaskHandler>,
        )
        .await;
    registry
        .register_handler(
            workflow_id,
            "shipping_handler".to_string(),
            Arc::new(ShippingHandler) as Arc<dyn TaskHandler>,
        )
        .await;

    // Create workflow in Raft
    let snapshot = flow_raft::core::WorkflowSnapshot::from_workflow(&running);
    let request = Request::CreateWorkflow {
        workflow: snapshot.clone(),
    };
    app.create_workflow(request).await?;

    println!("Complex workflow created in Raft cluster");
    
    // Wait for workflow to be stored
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Execute the workflow
    println!("\nExecuting workflow...");
    let handler_executor = flow_raft::api::handlers::executor::HandlerExecutor::new(
        executor.clone(),
        registry.clone(),
    );
    
    match handler_executor.execute_workflow(workflow_id, 200).await {
        Ok(()) => {
            println!("\n✓ Workflow execution completed successfully!");
            
            // Show final workflow state
            if let Some(final_workflow) = app.get_workflow(&workflow_id).await {
                println!("\nFinal workflow state:");
                println!("  State: {:?}", final_workflow.state);
                println!("  Tasks completed: {}/{}", 
                    final_workflow.executions.len(),
                    final_workflow.task_definitions.len());
                
                if let Some(outputs) = &final_workflow.outputs {
                    println!("  Final outputs: {}", serde_json::to_string_pretty(outputs).unwrap_or_default());
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Workflow execution failed: {:?}", e);
        }
    }

    println!("\nExample completed!");
    Ok(())
}
