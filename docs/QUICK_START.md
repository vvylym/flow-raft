# FlowRaft Quick Start Guide

## Installation

```bash
git clone <repository>
cd flow-raft
cargo build --release
```

## Quick Start

All examples use `use flow_raft::prelude::*;` for convenient imports.

### Simple Example

```rust
use flow_raft::prelude::*;
use serde::{Deserialize, Serialize};

// Define your data types
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Order {
    id: String,
    amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Payment {
    order_id: String,
    amount: f64,
}

// Define simple Rust functions for workflow nodes
fn process_order(order: Order) -> Result<Payment, String> {
    println!("Processing order: {}", order.id);
    Ok(Payment {
        order_id: order.id.clone(),
        amount: order.amount,
    })
}

fn charge_payment(payment: Payment) -> Result<serde_json::Value, String> {
    println!("Charging payment: {}", payment.order_id);
    Ok(serde_json::json!({"status": "charged", "amount": payment.amount}))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build workflow using function-based nodes
    let workflow_graph = GraphBuilder::new("order_processing")
        .with_retry_config(RetryConfig::new(3))
        .add_node_fn("process", wrap_function(process_order), None)
        .add_node_fn("charge", wrap_function(charge_payment), None)
        .add_edge("process", "charge")
        .set_root("process")
        .build()?;

    // Convert to workflow definition
    let workflow_def = WorkflowDef::from_graph(
        "order_processing",
        workflow_graph,
        RetryConfig::new(3),
    );

    // Create single-node app using builder pattern
    let app = FlowRaftApp::builder()
        .with_node_id(1)
        .with_workflows(vec![workflow_def])
        .enable_metrics(true)
        .build_single_node()
        .await?;

    println!("✓ FlowRaft app created successfully!");
    Ok(())
}
```

## Running Examples

### Simple Single Node

```bash
cargo run --example simple_single_node
```

This demonstrates:
- Function-based node definitions
- Builder pattern for FlowRaftApp
- Simple workflow execution

### Conditional Workflow

```bash
cargo run --example conditional_workflow
```

This demonstrates:
- Conditional edges
- Branching logic
- Function-based nodes

### Parallel Workflow

```bash
cargo run --example parallel_workflow
```

This demonstrates:
- Split/merge edges
- Parallel task execution
- Function-based nodes

### Complex Workflow

```bash
cargo run --example complex_workflow
```

This demonstrates:
- E-commerce order processing workflow
- Conditional execution paths
- Split/merge operations
- Parallel task execution

### Distributed Cluster

```bash
cargo run --example distributed_cluster
```

This demonstrates:
- Multi-node cluster setup
- Workflow registration on different nodes
- Metrics collection

### Production Cluster

```bash
cargo run --example production_cluster
```

This demonstrates:
- 3-node cluster with production scenarios
- Leader/follower setup
- Node shutdown scenarios
- Leader election
- Node restart and rejoin

### Cluster Resilience

```bash
cargo run --example cluster_resilience
```

This demonstrates:
- Network partition handling
- Simultaneous node failures
- Leader failure during workflow execution
- Split-brain prevention
- State recovery after node restart

## Advanced Examples

For production-ready patterns and advanced usage, see the advanced examples:

### Advanced Error Handling

```bash
cargo run --example advanced_error_handling
```

This demonstrates:
- Retry strategies with exponential backoff
- Error recovery patterns
- Partial failure handling
- Circuit breaker pattern

### Advanced Parallelism

```bash
cargo run --example advanced_parallelism
```

This demonstrates:
- Dynamic parallelism
- Concurrency limits
- Resource pooling
- Batch processing

### Advanced Conditionals

```bash
cargo run --example advanced_conditionals
```

This demonstrates:
- Complex branching logic
- Multi-way conditionals
- State-dependent routing
- Conditional retries

### Advanced Observability

```bash
cargo run --example advanced_observability
```

This demonstrates:
- Metrics collection
- Distributed tracing
- Execution history
- Real-time monitoring

### Advanced Client

```bash
cargo run --example advanced_client
```

This demonstrates:
- gRPC client usage
- Execution tracking
- Callback patterns
- Stream processing

### Advanced Cluster

```bash
cargo run --example advanced_cluster
```

This demonstrates:
- Multi-region deployment
- Cross-cluster replication
- Load balancing
- High availability

See [Advanced Usage Guide](ADVANCED_USAGE.md) for detailed documentation on these patterns.

## Basic Usage

### 1. Define a Workflow with Function-Based Nodes

```rust
use flow_raft::prelude::*;

// Define your functions
fn task1(input: MyInput) -> Result<MyOutput, String> { ... }
fn task2(input: MyOutput) -> Result<FinalResult, String> { ... }

// Build workflow using functions
let workflow_graph = GraphBuilder::new("my_workflow")
    .with_retry_config(RetryConfig::new(3))
    .add_node_fn("task1", wrap_function(task1), None)
    .add_node_fn("task2", wrap_function(task2), None)
    .add_edge("task1", "task2")
    .set_root("task1")
    .build()?;

// Convert to workflow definition
let workflow_def = WorkflowDef::from_graph("my_workflow", workflow_graph, RetryConfig::new(3));
```

### 2. Create Single-Node App Using Builder Pattern

```rust
// Create single-node app using builder pattern
let app = FlowRaftApp::builder()
    .with_node_id(1)
    .with_workflows(vec![workflow_def])
    .enable_metrics(true)
    .build_single_node()
    .await?;
```

### 3. Create Cluster Using Builder Pattern

```rust
// Launch a 3-node cluster
let nodes = launch_cluster(vec![
    (1, NodeMode::Leader, vec![workflow1_def.clone()]),
    (2, NodeMode::Follower, vec![workflow2_def.clone()]),
    (3, NodeMode::Follower, vec![]),
])
.await?;
```

### 4. Execute Workflow (Optional)

For full execution with handlers, see the examples. The workflow is already registered via the builder pattern.

```rust
// Workflow is already registered via builder pattern
// To execute, you would:
// 1. Register task handlers
// 2. Use WorkflowExecutor to execute
// 3. Monitor via metrics and state queries

// Example execution setup:
let executor = Arc::new(WorkflowExecutor::new(
    app.raft().clone(),
    app.state_machine().clone(),
    1,
));
let registry = Arc::new(HandlerRegistry::new());

// Register handlers for each task
registry.register_handler(workflow_id, "handler1", handler1).await;

// Execute workflow
let handler_executor = HandlerExecutor::new(executor, registry);
handler_executor.execute_workflow(workflow_id, 100).await?;
```

## Conditional and Parallel Workflows

### Conditional Edges

```rust
use flow_raft::prelude::*;

struct MyCondition;

impl ConditionObject for MyCondition {
    fn evaluate(&self, input: Value) -> Result<NodeName, String> {
        let value = input.get("value").and_then(|v| v.as_bool()).unwrap_or(false);
        if value {
            Ok(NodeName::new("then_task"))
        } else {
            Ok(NodeName::new("else_task"))
        }
    }
}

let workflow = GraphBuilder::new("conditional")
    .add_node_fn("input", wrap_function(input_fn), None)
    .add_node_fn("then_task", wrap_function(then_fn), None)
    .add_node_fn("else_task", wrap_function(else_fn), None)
    .add_conditional_edge(
        "input",
        Arc::new(MyCondition),
        "then_task",
        "else_task",
    )
    .set_root("input")
    .build()?;
```

### Split/Merge Edges

```rust
use flow_raft::prelude::*;

struct MySplit;

impl SplitObject for MySplit {
    fn evaluate(&self, input: Value) -> Result<Vec<NodeName>, String> {
        let items = input.get("items").and_then(|v| v.as_array()).unwrap_or(&vec![]);
        Ok(items.iter().enumerate()
            .map(|(i, _)| NodeName::new(format!("process_item_{}", i)))
            .collect())
    }
}

struct MyMerge;

impl MergeObject for MyMerge {
    fn merge(&self, inputs: Vec<Value>) -> Result<Value, String> {
        // Merge logic
        Ok(serde_json::json!({"merged": true}))
    }
}

let workflow = GraphBuilder::new("parallel")
    .add_node_fn("split", wrap_function(split_fn), None)
    .add_node_fn("process_item_0", wrap_function(process_fn), None)
    .add_node_fn("process_item_1", wrap_function(process_fn), None)
    .add_node_fn("merge", wrap_function(merge_fn), None)
    .add_split_edge("split", Arc::new(MySplit), vec!["process_item_0", "process_item_1"])
    .add_merge_edge(vec!["process_item_0", "process_item_1"], Arc::new(MyMerge), "merge")
    .set_root("split")
    .build()?;
```

## Testing

Run all tests:

```bash
cargo test
```

Run specific test suites:

```bash
cargo test --lib                    # Library tests
cargo test --lib raft::tests       # Raft integration tests
cargo test --lib api::handlers     # Handler tests
```

## Benchmarking

Run benchmarks:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark suite
cargo bench --bench flow_raft_benchmarks
cargo bench --bench temporal_comparison
cargo bench --bench airflow_comparison

# Run comprehensive benchmark suite
./scripts/benchmark_suite.sh
```

## Next Steps

- Read [ARCHITECTURE.md](ARCHITECTURE.md) for system design
- Read [API_GUIDE.md](API_GUIDE.md) for detailed API documentation
- Read [PERFORMANCE.md](PERFORMANCE.md) for performance characteristics
- Read [CLUSTER_OPERATIONS.md](CLUSTER_OPERATIONS.md) for cluster management
- Read [SCOPE.md](SCOPE.md) for system guarantees
- Read [DESIGN.md](DESIGN.md) for design rationale
- Check [ROADMAP.md](ROADMAP.md) for implementation status
