# flow-raft

FlowRaft’s **single entry point**: all binaries and the library facade live in this crate. Use `flow_raft::*` for the product API; use the `flowraft` and `flowraft-node` binaries for CLI and node serving.

## Binaries

- **flowraft** — Workflow and cluster CLI: `workflow define|trigger|get|list|cancel`, `cluster status`. Connects to a FlowRaft gRPC server (e.g. one started by `flowraft-node`).
- **flowraft-node** — Run a node: `serve` with `--id`, `--raft`, `--grpc`, `--http`, `--data`, `--peers`, `--bootstrap`. Exposes gRPC, Raft RPC, and HTTP `/health`, `/metrics`.

Build and run:

```bash
cargo build -p flow-raft
cargo run -p flow-raft --bin flowraft -- --help
cargo run -p flow-raft --bin flowraft-node -- serve --help
```

## Library (facade)

Prefer the prelude for common types:

```rust
use flow_raft::prelude::*;
```

Main pieces:

- **Graph building**: `TypedGraphBuilder`, `node()`, `condition()`, `merge()`, `split()`, `switch()`
- **App**: `FlowRaftApp`, `FlowRaftAppBuilder::new()`
- **Execution**: `HandlerRegistry`, `HandlerExecutor`, `register_typed_graph_handlers`
- **Workflows**: `flow_raft_testing::workflows` (e.g. `order_pipeline_graph`, `order_conditional_graph`) in the `flow-raft-testing` crate

## Examples

```bash
cargo run -p flow-raft --example simple_single_node
cargo run -p flow-raft --example conditional_workflow
cargo run -p flow-raft --example parallel_workflow
cargo run -p flow-raft --example complex_workflow
```

See the [workspace README](../README.md) and [Quick Start](../docs/QUICK_START.md) for full usage.

## Testing

```bash
cargo test -p flow-raft
```

## License

MIT
