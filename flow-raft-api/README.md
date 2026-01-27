# flow-raft-api

Public API for FlowRaft: graph builder, workflow definitions, and gRPC client.

## Contents

- **Graph** (`graph`): `TypedGraphBuilder`, `GraphBuilder`, `graph_to_workflow`, `node`, `condition`, `merge`, `split`, `switch`
- **Workflow** (`workflow`): `WorkflowDef`, `define_parse`
- **Client** (`client`): `FlowRaftClient`, `FlowRaftClientBuilder`, gRPC-based workflow submit/watch/control

## Usage

```rust
use flow_raft_api::graph::{TypedGraphBuilder, node, graph_to_workflow};
use flow_raft_api::workflow::WorkflowDef;
use flow_raft_api::client::FlowRaftClient;
```

## Testing

```bash
cargo test -p flow-raft-api
```

## License

MIT
