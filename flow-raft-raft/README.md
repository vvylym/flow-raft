# flow-raft-raft

Raft consensus and replication layer for FlowRaft. Wraps OpenRaft and wires it to the workflow state machine.

## Contents

- **App**: `FlowRaftApp`, `FlowRaftAppBuilder::new()`, `FlowRaftNode`
- **Executor**: `WorkflowExecutor`, `TaskHandler`
- **Storage**: `LogStore`, `StateMachineStore`
- **Network**: `MemoryNetworkFactory`, `TcpNetworkFactory` (in-memory and TCP Raft RPC)
- **Command / types**: `Request`, `WorkflowCommandBuilder`

## Usage

Typically used via `flow-raft` or `flow-raft-server`. For custom setups:

```rust
use flow_raft_raft::{FlowRaftApp, WorkflowExecutor};
use flow_raft_raft::storage::{LogStore, StateMachineStore};
```

## Testing

```bash
cargo test -p flow-raft-raft
```

## License

MIT
