//! Complex workflow example
//!
//! Demonstrates a complex e-commerce order processing pipeline with:
//! - Simple edges
//! - Conditional branching (validation, shipping method)
//! - Parallel execution (split/merge for inventory, notifications)
//! - Multiple end nodes based on inputs
//! - Proper input/output flow for all nodes
//! - Error handling and retries

use flow_raft::prelude::*;
use std::sync::Arc;

// ============================================================================
// Conditions
// ============================================================================

/// Order validation condition
#[derive(Debug)]
struct OrderValidationCondition;

impl ConditionObject for OrderValidationCondition {
    fn evaluate(&self, input: serde_json::Value) -> Result<NodeName, String> {
        let valid = input
            .get("valid")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if valid {
            Ok(NodeName::new("process_payment"))
        } else {
            Ok(NodeName::new("reject_order"))
        }
    }
}

/// Shipping method condition
#[derive(Debug)]
struct ShippingMethodCondition;

impl ConditionObject for ShippingMethodCondition {
    fn evaluate(&self, input: serde_json::Value) -> Result<NodeName, String> {
        let method = input
            .get("shipping_method")
            .and_then(|v| v.as_str())
            .unwrap_or("standard");

        match method {
            "express" => Ok(NodeName::new("notify_express")),
            "standard" => Ok(NodeName::new("notify_standard")),
            _ => Ok(NodeName::new("notify_standard")),
        }
    }
}

// ============================================================================
// Splits
// ============================================================================

/// Inventory split - check multiple items in parallel
#[derive(Debug)]
struct InventorySplit;

impl SplitObject for InventorySplit {
    fn evaluate(&self, input: serde_json::Value) -> Result<Vec<NodeName>, String> {
        let items = input
            .get("items")
            .and_then(|i| i.as_array())
            .ok_or_else(|| "Missing items array".to_string())?;

        let mut targets = Vec::new();
        for (idx, _) in items.iter().enumerate() {
            targets.push(NodeName::new(&format!("check_inventory_{}", idx)));
        }
        Ok(targets)
    }
}

/// Notification split - send email and SMS in parallel
#[derive(Debug)]
struct NotificationSplit;

impl SplitObject for NotificationSplit {
    fn evaluate(&self, _input: serde_json::Value) -> Result<Vec<NodeName>, String> {
        Ok(vec![NodeName::new("send_email"), NodeName::new("send_sms")])
    }
}

// ============================================================================
// Merges
// ============================================================================

/// Merge inventory results
#[derive(Debug)]
struct InventoryMerge;

impl MergeObject for InventoryMerge {
    fn merge(
        &self,
        inputs: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
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
    }
}

/// Merge notification results
#[derive(Debug)]
struct NotificationMerge;

impl MergeObject for NotificationMerge {
    fn merge(
        &self,
        inputs: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let mut sent_channels = Vec::new();
        let mut all_sent = true;

        for input in inputs {
            let channel = input
                .get("channel")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let sent = input.get("sent").and_then(|v| v.as_bool()).unwrap_or(false);

            sent_channels.push(channel.to_string());
            if !sent {
                all_sent = false;
            }
        }

        Ok(serde_json::json!({
            "all_sent": all_sent,
            "channels": sent_channels
        }))
    }
}

// ============================================================================
// Task Handlers with Input/Output
// ============================================================================

/// Order validation handler
struct OrderValidationHandler;

impl TaskHandler for OrderValidationHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("[OrderValidation] Processing order: {:?}", inputs);

        let order_id = inputs
            .get("order_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let valid = inputs
            .get("valid")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(serde_json::json!({
            "order_id": order_id,
            "valid": valid,
            "status": if valid { "validated" } else { "invalid" },
            "output": {
                "order_id": order_id,
                "validated_at": chrono::Utc::now().to_rfc3339(),
            }
        }))
    }
}

/// Payment processing handler
struct PaymentHandler;

impl TaskHandler for PaymentHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("[Payment] Processing payment: {:?}", inputs);

        let order_id = inputs
            .get("order_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let amount = inputs.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);

        Ok(serde_json::json!({
            "order_id": order_id,
            "payment_id": format!("pay_{}", uuid::Uuid::new_v4()),
            "amount": amount,
            "status": "completed",
            "transaction_id": format!("txn_{}", uuid::Uuid::new_v4()),
            "output": {
                "payment_id": format!("pay_{}", uuid::Uuid::new_v4()),
                "status": "completed",
                "paid_at": chrono::Utc::now().to_rfc3339(),
            }
        }))
    }
}

/// Order rejection handler (end node)
struct RejectOrderHandler;

impl TaskHandler for RejectOrderHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("[RejectOrder] Rejecting order: {:?}", inputs);

        let order_id = inputs
            .get("order_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        Ok(serde_json::json!({
            "order_id": order_id,
            "status": "rejected",
            "reason": "validation_failed",
            "output": {
                "order_id": order_id,
                "status": "rejected",
                "rejected_at": chrono::Utc::now().to_rfc3339(),
            }
        }))
    }
}

/// Inventory check handler (for split items)
struct InventoryHandler {
    item_index: usize,
}

impl TaskHandler for InventoryHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!(
            "[Inventory {}] Checking inventory: {:?}",
            self.item_index, inputs
        );

        let items = inputs
            .get("items")
            .and_then(|i| i.as_array())
            .ok_or_else(|| "Missing items array".to_string())?;

        let item = items
            .get(self.item_index)
            .ok_or_else(|| format!("Item {} not found", self.item_index))?;

        let item_id = item.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");

        Ok(serde_json::json!({
            "item_index": self.item_index,
            "item_id": item_id,
            "available": true,
            "quantity_available": 10,
            "output": {
                "item_id": item_id,
                "available": true,
                "checked_at": chrono::Utc::now().to_rfc3339(),
            }
        }))
    }
}

/// Shipping handler
struct ShippingHandler;

impl TaskHandler for ShippingHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("[Shipping] Shipping order: {:?}", inputs);

        let order_id = inputs
            .get("order_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let shipping_method = inputs
            .get("shipping_method")
            .and_then(|v| v.as_str())
            .unwrap_or("standard");

        Ok(serde_json::json!({
            "order_id": order_id,
            "tracking_number": format!("TRACK_{}", uuid::Uuid::new_v4()),
            "carrier": "UPS",
            "shipping_method": shipping_method,
            "estimated_delivery": "2024-01-10",
            "output": {
                "tracking_number": format!("TRACK_{}", uuid::Uuid::new_v4()),
                "shipped_at": chrono::Utc::now().to_rfc3339(),
            }
        }))
    }
}

/// Express notification handler
struct ExpressNotificationHandler;

impl TaskHandler for ExpressNotificationHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!(
            "[ExpressNotification] Sending express notification: {:?}",
            inputs
        );

        let order_id = inputs
            .get("order_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        Ok(serde_json::json!({
            "order_id": order_id,
            "channel": "express",
            "sent": true,
            "output": {
                "channel": "express",
                "sent_at": chrono::Utc::now().to_rfc3339(),
            }
        }))
    }
}

/// Standard notification handler
struct StandardNotificationHandler;

impl TaskHandler for StandardNotificationHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!(
            "[StandardNotification] Sending standard notification: {:?}",
            inputs
        );

        let order_id = inputs
            .get("order_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        Ok(serde_json::json!({
            "order_id": order_id,
            "channel": "standard",
            "sent": true,
            "output": {
                "channel": "standard",
                "sent_at": chrono::Utc::now().to_rfc3339(),
            }
        }))
    }
}

/// Email notification handler
struct EmailHandler;

impl TaskHandler for EmailHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("[Email] Sending email: {:?}", inputs);

        let order_id = inputs
            .get("order_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        Ok(serde_json::json!({
            "order_id": order_id,
            "channel": "email",
            "sent": true,
            "output": {
                "channel": "email",
                "sent_at": chrono::Utc::now().to_rfc3339(),
            }
        }))
    }
}

/// SMS notification handler
struct SMSHandler;

impl TaskHandler for SMSHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("[SMS] Sending SMS: {:?}", inputs);

        let order_id = inputs
            .get("order_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        Ok(serde_json::json!({
            "order_id": order_id,
            "channel": "sms",
            "sent": true,
            "output": {
                "channel": "sms",
                "sent_at": chrono::Utc::now().to_rfc3339(),
            }
        }))
    }
}

/// Finalization handler (end node)
struct FinalizeOrderHandler;

impl TaskHandler for FinalizeOrderHandler {
    fn execute(
        &self,
        _task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        println!("[FinalizeOrder] Finalizing order: {:?}", inputs);

        let order_id = inputs
            .get("order_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        Ok(serde_json::json!({
            "order_id": order_id,
            "status": "completed",
            "output": {
                "order_id": order_id,
                "status": "completed",
                "completed_at": chrono::Utc::now().to_rfc3339(),
            }
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("Complex E-commerce Order Processing Workflow");
    println!("==============================================");

    // Build complex workflow graph with all edge types
    let mut builder =
        GraphBuilder::new("order_processing").with_default_retry_config(RetryConfig::new(3));

    // Add nodes with proper input/output definitions
    builder
        .add_node(
            "validate_order",
            "order_handler",
            vec![
                "order_id".to_string(),
                "items".to_string(),
                "customer_id".to_string(),
            ],
            vec![
                "order_id".to_string(),
                "valid".to_string(),
                "status".to_string(),
            ],
            None,
        )
        .add_node(
            "process_payment",
            "payment_handler",
            vec!["order_id".to_string(), "amount".to_string()],
            vec![
                "payment_id".to_string(),
                "status".to_string(),
                "transaction_id".to_string(),
            ],
            None,
        )
        .add_node(
            "reject_order",
            "reject_handler",
            vec!["order_id".to_string()],
            vec![
                "order_id".to_string(),
                "status".to_string(),
                "reason".to_string(),
            ],
            None,
        )
        .add_node(
            "check_inventory_0",
            "inventory_handler_0",
            vec!["items".to_string()],
            vec!["item_id".to_string(), "available".to_string()],
            None,
        )
        .add_node(
            "check_inventory_1",
            "inventory_handler_1",
            vec!["items".to_string()],
            vec!["item_id".to_string(), "available".to_string()],
            None,
        )
        .add_node(
            "aggregate_inventory",
            "inventory_aggregator",
            vec!["results".to_string()],
            vec!["all_available".to_string()],
            None,
        )
        .add_node(
            "ship_order",
            "shipping_handler",
            vec![
                "order_id".to_string(),
                "items".to_string(),
                "shipping_method".to_string(),
            ],
            vec![
                "tracking_number".to_string(),
                "carrier".to_string(),
                "estimated_delivery".to_string(),
            ],
            None,
        )
        .add_node(
            "notify_express",
            "express_notification_handler",
            vec!["order_id".to_string()],
            vec!["channel".to_string(), "sent".to_string()],
            None,
        )
        .add_node(
            "notify_standard",
            "standard_notification_handler",
            vec!["order_id".to_string()],
            vec!["channel".to_string(), "sent".to_string()],
            None,
        )
        .add_node(
            "send_email",
            "email_handler",
            vec!["order_id".to_string()],
            vec!["channel".to_string(), "sent".to_string()],
            None,
        )
        .add_node(
            "send_sms",
            "sms_handler",
            vec!["order_id".to_string()],
            vec!["channel".to_string(), "sent".to_string()],
            None,
        )
        .add_node(
            "finalize_order",
            "finalize_handler",
            vec![
                "order_id".to_string(),
                "payment_id".to_string(),
                "tracking_number".to_string(),
            ],
            vec!["order_id".to_string(), "status".to_string()],
            None,
        );

    // Add edges: Simple, Conditional, Split, Merge
    builder
        // Simple edge: validate_order -> process_payment (if valid)
        .add_simple_edge("validate_order", "process_payment")
        // Conditional edge: validate_order -> [process_payment | reject_order]
        .add_conditional_edge(
            "validate_order",
            Arc::new(OrderValidationCondition) as Arc<dyn ConditionObject>,
            "process_payment",
            "reject_order",
        )
        // Simple edge: process_payment -> check_inventory (split point)
        .add_simple_edge("process_payment", "check_inventory_0")
        // Split edge: process_payment -> [check_inventory_0, check_inventory_1]
        .add_split_edge(
            "process_payment",
            Arc::new(InventorySplit) as Arc<dyn SplitObject>,
            vec!["check_inventory_0", "check_inventory_1"],
        )
        // Merge edge: [check_inventory_0, check_inventory_1] -> aggregate_inventory
        .add_merge_edge(
            vec!["check_inventory_0", "check_inventory_1"],
            Arc::new(InventoryMerge) as Arc<dyn MergeObject>,
            "aggregate_inventory",
        )
        // Simple edge: aggregate_inventory -> ship_order
        .add_simple_edge("aggregate_inventory", "ship_order")
        // Conditional edge: ship_order -> [notify_express | notify_standard]
        .add_conditional_edge(
            "ship_order",
            Arc::new(ShippingMethodCondition) as Arc<dyn ConditionObject>,
            "notify_express",
            "notify_standard",
        )
        // Split edge: notify_express -> [send_email, send_sms]
        .add_split_edge(
            "notify_express",
            Arc::new(NotificationSplit) as Arc<dyn SplitObject>,
            vec!["send_email", "send_sms"],
        )
        // Split edge: notify_standard -> [send_email, send_sms]
        .add_split_edge(
            "notify_standard",
            Arc::new(NotificationSplit) as Arc<dyn SplitObject>,
            vec!["send_email", "send_sms"],
        )
        // Merge edge: [send_email, send_sms] -> finalize_order
        .add_merge_edge(
            vec!["send_email", "send_sms"],
            Arc::new(NotificationMerge) as Arc<dyn MergeObject>,
            "finalize_order",
        )
        .set_root("validate_order");

    let graph = builder.build()?;
    println!("✓ Built complex graph with {} nodes", graph.nodes.len());

    // Convert graph to workflow definition
    let retry_config = RetryConfig::new(3);
    let workflow_def = WorkflowDef::from_graph("order_processing", graph, retry_config.clone());

    println!("✓ Workflow definition created");

    // Create single-node app using builder pattern
    let app = FlowRaftApp::builder()
        .with_node_id(1)
        .with_workflows(vec![workflow_def.clone()])
        .enable_metrics(true)
        .build_single_node()
        .await?;

    println!("✓ FlowRaft app created using builder pattern");

    // Get workflow ID from the definition
    let workflow_id = workflow_def.workflow_id;

    // Verify workflow was registered
    if let Some(registered_workflow) = app.get_workflow(&workflow_id).await {
        println!("\n✓ Workflow verified in Raft cluster");
        println!("  Workflow ID: {:?}", workflow_id);
        println!("  State: {:?}", registered_workflow.state);
        println!("  Tasks: {}", registered_workflow.task_definitions.len());
    }

    // Note: This example demonstrates workflow definition and registration using the builder pattern.
    // For full execution with handlers, see the execution layer documentation.
    // The workflow has been registered via the builder pattern and is ready for execution.
    
    println!("\n✓ Complex workflow example completed!");
    println!("  The workflow 'order_processing' has been registered and is ready for execution.");
    println!("  To execute this workflow, you would:");
    println!("  1. Register task handlers for each node");
    println!("  2. Use WorkflowExecutor to execute the workflow");
    println!("  3. Monitor execution via metrics and state queries");
    
    Ok(())
}
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
            "reject_handler".to_string(),
            Arc::new(RejectOrderHandler) as Arc<dyn TaskHandler>,
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
            "inventory_aggregator".to_string(),
            Arc::new(InventoryHandler { item_index: 0 }) as Arc<dyn TaskHandler>, // Reuse for merge
        )
        .await;
    registry
        .register_handler(
            workflow_id,
            "shipping_handler".to_string(),
            Arc::new(ShippingHandler) as Arc<dyn TaskHandler>,
        )
        .await;
    registry
        .register_handler(
            workflow_id,
            "express_notification_handler".to_string(),
            Arc::new(ExpressNotificationHandler) as Arc<dyn TaskHandler>,
        )
        .await;
    registry
        .register_handler(
            workflow_id,
            "standard_notification_handler".to_string(),
            Arc::new(StandardNotificationHandler) as Arc<dyn TaskHandler>,
        )
        .await;
    registry
        .register_handler(
            workflow_id,
            "email_handler".to_string(),
            Arc::new(EmailHandler) as Arc<dyn TaskHandler>,
        )
        .await;
    registry
        .register_handler(
            workflow_id,
            "sms_handler".to_string(),
            Arc::new(SMSHandler) as Arc<dyn TaskHandler>,
        )
        .await;
    registry
        .register_handler(
            workflow_id,
            "finalize_handler".to_string(),
            Arc::new(FinalizeOrderHandler) as Arc<dyn TaskHandler>,
        )
        .await;

    // Create workflow in Raft
    let snapshot = flow_raft_core::WorkflowSnapshot::from_workflow(&running);
    let request = Request::CreateWorkflow {
        workflow: snapshot.clone(),
    };
    app.create_workflow(request).await?;

    println!("✓ Complex workflow created in Raft cluster");

    // Wait for workflow to be stored
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Execute the workflow
    println!("\nExecuting workflow...");
    let handler_executor =
        flow_raft::handlers::executor::HandlerExecutor::new(executor.clone(), registry.clone());

    match handler_executor.execute_workflow(workflow_id, 200).await {
        Ok(()) => {
            println!("\n✓ Workflow execution completed successfully!");

            // Show final workflow state
            if let Some(final_workflow) = app.get_workflow(&workflow_id).await {
                println!("\nFinal workflow state:");
                println!("  State: {:?}", final_workflow.state);
                println!(
                    "  Tasks completed: {}/{}",
                    final_workflow.executions.len(),
                    final_workflow.task_definitions.len()
                );

                if let Some(outputs) = &final_workflow.outputs {
                    println!(
                        "  Final outputs: {}",
                        serde_json::to_string_pretty(outputs).unwrap_or_default()
                    );
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
