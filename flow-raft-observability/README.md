# flow-raft-observability

Observability for FlowRaft: metrics, execution history, Prometheus export, and workflow watcher.

## Contents

- **Metrics**: counters/gauges for workflows and tasks
- **History**: `HistoryStore`, `ExecutionEvent`, `ExecutionEventType`
- **Prometheus**: `start_metrics_server`, `/metrics` HTTP
- **Tracing**: OpenTelemetry wiring
- **Watcher**: `WorkflowWatcher` for broadcasting workflow updates

## Usage

```rust
use flow_raft_observability::{HistoryStore, WorkflowWatcher, start_metrics_server};
```

Used by `flow-raft`, `flow-raft-raft`, and `flow-raft-server` when metrics or history are enabled.

## Testing

```bash
cargo test -p flow-raft-observability
```

## License

MIT
