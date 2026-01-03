# FlowRaft

**A distributed, stateful workflow engine in Rust** with Raft-based consensus for deterministic execution and fault tolerance.

## Overview

FlowRaft provides a production-ready workflow engine that treats workflows as replicated state machines. All state transitions are coordinated through Raft consensus, ensuring strong consistency guarantees even under node failures.

### Key Features

- **Type-safe workflow definition** via compile-time validated DAGs
- **Raft-based state replication** for strong consistency
- **Deterministic state transitions** with explicit recovery semantics
- **gRPC API** for distributed workflow management
- **Comprehensive observability** (metrics, history, event streaming)
- **Production-ready** with 192+ passing tests and extensive benchmarks

## Architecture

```
┌─────────────────────────────────────────┐
│         API Layer (gRPC/Graph)          │
├─────────────────────────────────────────┤
│    Raft Layer (Consensus & Replication) │
├─────────────────────────────────────────┤
│  Core Layer (Workflow/Task State Machines)│
└─────────────────────────────────────────┘
```

**Core Principle**: Only state transitions go through consensus. Task execution happens locally, with effects committed via Raft.

## Quick Start

```rust
use flow_raft::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build workflow
    let graph = GraphBuilder::new("example")
        .add_node("task1", "handler1", vec![], vec![], None)
        .add_node("task2", "handler2", vec![], vec![], None)
        .add_simple_edge("task1", "task2")
        .set_root("task1")
        .build()?;

    let workflow_def = WorkflowDef::from_graph("example", graph, RetryConfig::default());

    // Create single-node app
    let app = FlowRaftApp::builder()
        .with_node_id(1)
        .build_single_node()
        .await?;

    // Register and execute
    let workflow_id = app.register_workflow(workflow_def).await?;
    // ... execute workflow
    Ok(())
}
```

## Performance

**Current MVP Performance** (measured on modern hardware):
- **Workflow registration**: ~500-1000 workflows/sec
- **Task execution**: ~37µs per task (27K tasks/sec)
- **Large workflows** (100 tasks): ~3.3ms registration
- **Parallel workflows**: ~736µs registration

See [PERFORMANCE.md](docs/PERFORMANCE.md) for detailed benchmarks and optimization roadmap.

## Documentation

- **[Quick Start](docs/QUICK_START.md)**: Get running in minutes
- **[Architecture](docs/ARCHITECTURE.md)**: System design and components
- **[API Guide](docs/API_GUIDE.md)**: Complete API reference
- **[Cluster Operations](docs/CLUSTER_OPERATIONS.md)**: Multi-node deployment
- **[Performance](docs/PERFORMANCE.md)**: Benchmarks and optimization

## Crate Structure

- `flow-raft-core`: Core workflow/task state machines and DAG utilities
- `flow-raft-raft`: Raft consensus layer and state replication
- `flow-raft-api`: Graph builder, gRPC client, and workflow definitions
- `flow-raft-server`: gRPC service implementation and cluster management
- `flow-raft-observability`: Metrics, history, and event streaming

## Design Principles

1. **Workflows as State Machines**: Explicit state with deterministic transitions
2. **Raft for Consistency**: All state changes replicated via consensus
3. **Separation of Concerns**: Coordination (Raft) vs execution (handlers)
4. **Type Safety**: Compile-time validation of workflow structure
5. **Observability First**: Built-in metrics, history, and event streaming

## Status

**MVP Complete** ✅
- [x] Core workflow engine with state machines
- [x] Raft-based state replication
- [x] Graph builder API (type-safe & dynamic)
- [x] gRPC service and client
- [x] Observability (metrics, history, watcher)
- [x] Comprehensive test suite (192+ tests)
- [x] Production examples and benchmarks

## License

MIT
