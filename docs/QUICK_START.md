# FlowRaft Quick Start Guide

## Installation

```bash
git clone <repository>
cd flow-raft
cargo build --release
```

## Running Examples

### Simple Sequential Workflow

```bash
cargo run --example simple_sequential
```

This demonstrates:
- Creating a 3-task sequential workflow
- Registering task handlers
- Executing the workflow
- Viewing results

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
cargo run --example distributed
```

This demonstrates:
- 4-node Raft cluster (1 leader + 3 followers)
- Workflow replication
- Distributed task execution

### Graph Builder Examples

```bash
cargo run --example graph_builder_examples
```

This demonstrates:
- Type-safe graph builder
- Dynamic graph builder
- Conditional edges
- Split/merge edges

## Basic Usage

### 1. Define a Workflow

```rust
use flow_raft::api::graph::GraphBuilder;
use flow_raft::api::graph::converter::graph_to_workflow;

let mut builder = GraphBuilder::new("my_workflow");
builder
    .add_node("task1", "handler1", vec![], vec![], None)
    .add_node("task2", "handler2", vec![], vec![], None)
    .add_simple_edge("task1", "task2")
    .set_root("task1");

let graph = builder.build()?;
let workflow = graph_to_workflow(
    graph,
    WorkflowId::default(),
    RetryConfig::default(),
    serde_json::json!({}),
)?;
```

### 2. Set Up Raft Infrastructure

```rust
use flow_raft::raft::config::default_config;
use flow_raft::raft::network::MemoryNetworkFactory;
use flow_raft::raft::storage::{LogStore, StateMachineStore};
use flow_raft::raft::app::FlowRaftApp;

let node_id = 1;
let config = Arc::new(default_config().validate().unwrap());
let network = MemoryNetworkFactory::new();
let log_store = LogStore::default();
let state_machine = StateMachineStore::default();

let raft = openraft::Raft::new(
    node_id,
    config,
    network,
    log_store,
    state_machine.clone(),
).await?;

let raft = Arc::new(raft);
raft.initialize([1u64].into_iter().collect()).await?;

let app = Arc::new(FlowRaftApp::new(raft.clone(), state_machine.clone()));
```

### 3. Register Task Handlers

```rust
use flow_raft::api::handlers::HandlerRegistry;
use flow_raft::raft::executor::TaskHandler;

struct MyHandler;

impl TaskHandler for MyHandler {
    fn execute(&self, _task_id: TaskId, inputs: serde_json::Value) 
        -> Result<serde_json::Value, String> 
    {
        Ok(serde_json::json!({"result": "success"}))
    }
}

let registry = Arc::new(HandlerRegistry::new());
registry.register_handler(
    workflow_id,
    "handler1".to_string(),
    Arc::new(MyHandler) as Arc<dyn TaskHandler>,
).await;
```

### 4. Create and Execute Workflow

```rust
use flow_raft::api::handlers::executor::HandlerExecutor;

// Create workflow
let scheduled = workflow.schedule()?;
let running = scheduled.start()?;
let snapshot = WorkflowSnapshot::from_workflow(&running);
let request = Request::CreateWorkflow { workflow: snapshot };
app.create_workflow(request).await?;

// Execute workflow
let executor = Arc::new(WorkflowExecutor::new(raft, state_machine, node_id));
let handler_executor = HandlerExecutor::new(executor, registry);
handler_executor.execute_workflow(workflow_id, 100).await?;
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
cargo bench --bench workflow_execution
cargo bench --bench temporal_comparison
```

## Next Steps

- Read [ARCHITECTURE.md](ARCHITECTURE.md) for system design
- Read [API_GUIDE.md](API_GUIDE.md) for detailed API documentation
- Read [SCOPE.md](SCOPE.md) for system guarantees
- Read [DESIGN.md](DESIGN.md) for design rationale
- Check [ROADMAP.md](../../ROADMAP.md) for implementation status
