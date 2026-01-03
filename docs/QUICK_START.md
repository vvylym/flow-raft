# FlowRaft Quick Start

## Installation

```bash
git clone <repository>
cd flow-raft
cargo build --release
```

## Basic Usage

### 1. Define a Workflow

```rust
use flow_raft::prelude::*;

let graph = GraphBuilder::new("my_workflow")
    .add_node("task1", "handler1", vec![], vec![], None)
    .add_node("task2", "handler2", vec![], vec![], None)
    .add_simple_edge("task1", "task2")
    .set_root("task1")
    .build()?;

let workflow_def = WorkflowDef::from_graph("my_workflow", graph, RetryConfig::default());
```

### 2. Create a Single-Node App

```rust
let app = FlowRaftApp::builder()
    .with_node_id(1)
    .build_single_node()
    .await?;

let workflow_id = app.register_workflow(workflow_def).await?;
```

### 3. Execute Workflow

```rust
use flow_raft_raft::executor::WorkflowExecutor;
use flow_raft_server::handlers::{HandlerRegistry, HandlerExecutor};

let executor = Arc::new(WorkflowExecutor::new(
    app.raft().clone(),
    app.state_machine().clone(),
    1,
));

let registry = Arc::new(HandlerRegistry::new());
// Register handlers...
let handler_executor = HandlerExecutor::new(executor, registry);
handler_executor.execute_workflow(workflow_id, timeout_ms).await?;
```

## Multi-Node Cluster

```rust
use flow_raft_server::node::launcher::launch_cluster;

let nodes = launch_cluster(vec![
    (1, NodeMode::Leader, vec![workflow_def.clone()]),
    (2, NodeMode::Follower, vec![]),
    (3, NodeMode::Follower, vec![]),
]).await?;
```

## gRPC Client

```rust
use flow_raft_api::client::FlowRaftClient;

let client = FlowRaftClient::new("http://localhost:50051");
let exec_id = client.submit_workflow("workflow_name", json!({})).await?;
let status = client.get_workflow_status(exec_id).await?;
```

## Examples

```bash
# Single node
cargo run --example simple_single_node

# Multi-node cluster
cargo run --example distributed_cluster

# Production scenarios
cargo run --example production_cluster
```

## Next Steps

- **[API Guide](API_GUIDE.md)**: Complete API reference
- **[Architecture](ARCHITECTURE.md)**: System design details
- **[Cluster Operations](CLUSTER_OPERATIONS.md)**: Multi-node deployment
- **[Performance](PERFORMANCE.md)**: Benchmarks and optimization
