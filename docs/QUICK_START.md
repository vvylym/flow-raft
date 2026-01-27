# FlowRaft Quick Start

## Installation

```bash
git clone <repository>
cd flow-raft
cargo build --release
```

## Running a Node

**Single node** (gRPC, HTTP /health and /metrics, Raft):

```bash
flowraft-node serve --id 1 --raft 127.0.0.1:5010 --grpc 127.0.0.1:50051 --http 127.0.0.1:9090 --bootstrap
```

**Three-node cluster** (local): use `scripts/serve.sh` or Docker (`docker compose up -d`). The first node uses `--peers "2=127.0.0.1:5011,3=127.0.0.1:5012"` to initialize the cluster; nodes 2 and 3 have empty `--peers` and join.

## CLI (flowraft)

Workflow and cluster operations via gRPC (`--server` defaults to `http://localhost:50051`):

```bash
flowraft workflow define /path/to/workflow.json
flowraft workflow trigger --workflow-id <id> [--input '{}'|/path/to.json]
flowraft workflow get --workflow-id <id>
flowraft workflow list [--limit 100] [--offset 0]
flowraft workflow cancel --workflow-id <id>
flowraft cluster status [--node-id 1]
```

## Basic Usage

### 1. Define a Workflow (TypedGraphBuilder)

Use the type-safe graph builder: define nodes as plain Rust functions and connect them with edges. Output types of upstream nodes are checked against input types of downstream nodes at `build()`.

```rust
use flow_raft::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct In { x: i64 }
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Out { y: i64 }

fn task1(inp: In) -> Result<Out, String> { Ok(Out { y: inp.x + 1 }) }
fn task2(inp: Out) -> Result<Out, String> { Ok(Out { y: inp.y * 2 }) }

let mut builder = TypedGraphBuilder::new("my_workflow");
builder
    .add_node("task1", node(task1), None)
    .add_node("task2", node(task2), None)
    .add_simple_edge("task1", "task2")
    .set_root("task1");
let typed_graph = builder.build()?;
let workflow_def = typed_graph.workflow_def("my_workflow")?;
```

### 2. Create a Single-Node App

```rust
let app = FlowRaftAppBuilder::new()
    .with_node_id(1)
    .with_workflows(vec![workflow_def.clone()])
    .build_single_node()
    .await?;

let workflow_id = workflow_def.workflow_id;
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
register_typed_graph_handlers(registry.as_ref(), workflow_id, &typed_graph).await;
let handler_executor = HandlerExecutor::new(executor, registry);
handler_executor.execute_workflow(workflow_id, 100).await?;
```

## Multi-Node Cluster

Run multiple `flowraft-node serve` processes and point the `flowraft` CLI at the gRPC server. For a 3-node cluster, use `scripts/serve.sh` or start each node with `--peers` on the bootstrap node. Use `flowraft cluster status` to inspect the cluster.

## gRPC Client

```rust
use flow_raft_api::client::FlowRaftClient;

let client = FlowRaftClient::new("http://localhost:50051");
let exec_id = client.trigger_workflow_by_id(workflow_id, serde_json::json!({})).await?;
let status = client.get_workflow_status(exec_id).await?;
```

## Examples

Examples live in the `flow-raft-testing` crate:

```bash
# Single node
cargo run -p flow-raft-testing --example simple_single_node

# Conditional workflow
cargo run -p flow-raft-testing --example conditional_workflow

# TCP multi-node (production-style)
cargo run -p flow-raft-testing --example tcp_multi_node_cluster
```

## Next Steps

- **[API Guide](API_GUIDE.md)**: Complete API reference
- **[Architecture](ARCHITECTURE.md)**: System design details
- **[Cluster Operations](CLUSTER_OPERATIONS.md)**: Multi-node deployment
- **[Performance](PERFORMANCE.md)**: Benchmarks and optimization
