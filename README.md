# FlowRaft

A distributed, stateful workflow engine in Rust with Raft-based consensus for deterministic execution and fault tolerance.

## Features

- **Type-safe workflows** – DAGs built from plain Rust functions with compile-time edge checks
- **Raft-based replication** – Strong consistency and fault tolerance
- **gRPC API** – Remote workflow management and execution
- **Observability** – Metrics, history, and event streaming

## Installation

### Prerequisites

- **Rust toolchain:** minimum supported version (MSRV) is **1.85** (see workspace `rust-version` in the root `Cargo.toml`).
- `protoc` (for `flow-raft-proto`)

```bash
git clone https://github.com/vvylym/flow-raft
cd flow-raft
cargo build --release
```

## Usage

Define workflows with typed functions and connect them via edges; output/input types are checked at `build()`:

```rust
use flow_raft::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Order { id: String, amount: f64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Payment { order_id: String, amount: f64, status: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Receipt { order_id: String, total: f64 }

fn process_order(order: Order) -> Result<Payment, String> {
    Ok(Payment { order_id: order.id, amount: order.amount, status: "processed".into() })
}

fn charge_payment(payment: Payment) -> Result<Receipt, String> {
    Ok(Receipt { order_id: payment.order_id, total: payment.amount })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = TypedGraphBuilder::new("example");
    builder
        .add_node("process", node(process_order), None)
        .add_node("charge", node(charge_payment), None)
        .add_simple_edge("process", "charge")
        .set_root("process");
    let typed_graph = builder.build()?;
    let workflow_def = typed_graph.workflow_def("example")?;

    let app = FlowRaftAppBuilder::new()
        .with_node_id(1)
        .with_workflows(vec![workflow_def.clone()])
        .build_single_node()
        .await?;

    let workflow_id = workflow_def.workflow_id;
    let registry = HandlerRegistry::new();
    register_typed_graph_handlers(&registry, workflow_id, &typed_graph).await;
    // Wire HandlerExecutor and run; see docs/QUICK_START.md
    Ok(())
}
```

**Run a node:** `flowraft-node serve --id 1 --raft 127.0.0.1:5010 --grpc 127.0.0.1:50051 --http 127.0.0.1:9090 --bootstrap`  
**CLI:** `flowraft workflow define|trigger|get|list|cancel`, `flowraft cluster status`. See [Quick Start](docs/QUICK_START.md).

Run examples (`flow-raft-testing`):

```bash
cargo run -p flow-raft-testing --example simple_single_node
cargo run -p flow-raft-testing --example conditional_workflow
cargo run -p flow-raft-testing --example parallel_workflow
```

## Testing

```bash
cargo test
```

Coverage (requires `cargo-llvm-cov`):

```bash
./scripts/coverage.sh
```

**Coverage gate:** the script enforces at least **85% line coverage** over the workspace for the paths it measures. The following are **not** counted toward that percentage: generated or integration-heavy client and network glue (`flow-raft-api` client surfaces, TCP network helpers), Prometheus and tracing setup shims, the large graph `builder` module, and **thin `flow-raft` / `flow-raft-server` CLI entrypoints** (`cli_handlers.rs`, `launcher.rs`) that mostly issue gRPC calls and print results—those paths are better covered by manual or integration tests against a live server. CI runs the same scope as `./scripts/coverage.sh`.

## Documentation

| Document | Description |
|----------|-------------|
| [Quick Start](docs/QUICK_START.md) | Installation and first workflow |
| [API Guide](docs/API_GUIDE.md) | GraphBuilder, App, gRPC client |
| [Architecture](docs/ARCHITECTURE.md) | Design and components |
| [Cluster Operations](docs/CLUSTER_OPERATIONS.md) | Multi-node deployment |
| [Performance](docs/PERFORMANCE.md) | Benchmarks and notes |
| [Contributing](docs/CONTRIBUTING.md) | How to contribute |

## Crate Structure

| Crate | Role |
|-------|------|
| **flow-raft** | Main facade, re-exports, examples |
| **flow-raft-core** | Workflow/task state machines, DAG utilities |
| **flow-raft-raft** | Raft consensus and replication |
| **flow-raft-api** | Graph builder, gRPC client, workflow definitions |
| **flow-raft-server** | gRPC service, cluster, task handlers |
| **flow-raft-observability** | Metrics, history, watcher |
| **flow-raft-proto** | Protobuf and gRPC definitions |

## License

MIT
