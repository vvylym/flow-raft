# flow-raft-core

Core domain types for the FlowRaft workflow engine: workflows, tasks, DAG utilities, and retry configuration.

## Contents

- **Workflow**: state machine, snapshot, transitions
- **Task**: definition, execution, state, transitions
- **DAG**: `TaskDependencies`, `ready_tasks`, `topological_order`, `validate_dag`
- **Retry**: `RetryConfig`, `RetryError`

## Usage

This crate is used by `flow-raft-raft`, `flow-raft-api`, and `flow-raft-server`. Most users should go through the `flow-raft` facade.

```rust
use flow_raft_core::{WorkflowId, TaskId, WorkflowSnapshot, WorkflowState, RetryConfig};
```

## Testing

```bash
cargo test -p flow-raft-core
```

## License

MIT
