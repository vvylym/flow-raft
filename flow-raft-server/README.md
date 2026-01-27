# flow-raft-server

Server implementation for FlowRaft: gRPC service, cluster management, and task handlers.

## Contents

- **gRPC** (`grpc`): `FlowRaftService`, `run_grpc_on_cluster`
- **Handlers** (`handlers`): `HandlerRegistry`, `HandlerExecutor`, `TaskRouter`
- **Raft cluster** (`raft_cluster`): `launch_raft_cluster`, `RaftClusterHandle`
- **Node** (`node`): `NodeConfig`, `NodeMode`, `init_tracing`, `start_metrics_server`, `NodeLaunchError`

## Usage

Used by the `flow-raft` binaries (`flowraft`, `flowraft-node`). For cluster setup, run `flowraft-node serve` on each node and use the `flowraft` CLI or gRPC. See [Cluster Operations](../docs/CLUSTER_OPERATIONS.md).

## Testing

```bash
cargo test -p flow-raft-server
```

## License

MIT
