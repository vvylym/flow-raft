# FlowRaft Cluster Operations Guide

## Overview

FlowRaft supports both single-node and multi-node cluster deployments. This guide covers cluster setup, operations, and best practices.

## Cluster Architecture

### Node Roles

- **Leader**: Coordinates the cluster, handles writes, manages state replication
- **Follower**: Receives replicated state, can execute tasks, participates in leader election

### Cluster Requirements

- **Minimum Nodes**: 1 (single-node mode)
- **Recommended**: 3 or 5 nodes for production (quorum-based consensus)
- **Maximum**: No hard limit (tested up to 100+ nodes)

## Setup

### Single-Node Deployment

```rust
use flow_raft::prelude::*;

let app = FlowRaftAppBuilder::new()
    .with_node_id(1)
    .with_workflows(vec![workflow_def])
    .enable_metrics(true)
    .build_single_node()
    .await?;
```

### Multi-Node Cluster

Run one `flowraft-node serve` process per node. The bootstrap node uses `--bootstrap` and `--peers "2=host2:port,3=host3:port"`; other nodes use empty `--peers` to join. Use `flowraft cluster status` to inspect the cluster. See `scripts/serve.sh` for a local 3-node setup.

### TCP transport (production)

For real multi-node deployments across machines, use the TCP Raft transport instead of the in-memory one:

- **[TcpNetworkFactory]**: Raft network that sends AppendEntries, Vote, and InstallSnapshot over TCP (bincode-framed).
- **[TcpRaftRpcServer]**: On each node, run an RPC server bound to that node’s Raft address; it accepts RPCs and dispatches them to the local `Raft`.
- **[tcp_nodes]**: Builds the `(NodeId -> BasicNode)` map so each node’s [BasicNode::addr] is the `"host:port"` where [TcpRaftRpcServer] listens.
- **FlowRaftNode::initialize_cluster_with_nodes**: Like `initialize_cluster`, but takes that map so Raft can reach peers over TCP.

Each node must:

1. Create `FlowRaftNode::new(..., TcpNetworkFactory::new(), log_store, state_machine)` (separate `LogStore`/`StateMachineStore` per node).
2. Start `TcpRaftRpcServer::new(node.raft.clone(), bind_addr).spawn()` (or `.run()`) before initializing the cluster.
3. On the bootstrap node, call `initialize_cluster_with_nodes(tcp_nodes([(1, "host:port"), ...]))`.

See `examples/tcp_multi_node_cluster.rs` for a 3-node TCP example.

## Builder Pattern (single-node)

For in-process single-node usage:

```rust
// Single node
let app = FlowRaftAppBuilder::new()
    .with_node_id(1)
    .with_workflows(vec![workflow_def])
    .enable_metrics(true)
    .build_single_node()
    .await?;
```

For multi-node, run `flowraft-node serve` on each machine and use the `flowraft` CLI or gRPC client.

## Leader Election

FlowRaft uses Raft consensus for leader election:

- **Automatic**: Leader election happens automatically when the current leader fails
- **Quorum Required**: Majority of nodes must be available for leader election
- **Split-Brain Prevention**: Raft ensures only one leader exists at a time

### Monitoring Leader Election

Use the CLI: `flowraft cluster status [--node-id 1]` to see the current leader and node role. The gRPC `GetNodeStatus` can be used from a client.

## State Replication

All workflow state is replicated across all nodes via Raft:

- **Consistency**: Strong consistency guaranteed by Raft
- **Durability**: State is persisted (when persistent storage is enabled)
- **Performance**: Optimized for low-latency replication

### Metrics

Monitor replication via Prometheus metrics:

- `flowraft_raft_replication_total`: Total replication operations
- `flowraft_raft_replication_success_total`: Successful replications
- `flowraft_raft_replication_duration_seconds`: Replication latency
- `flowraft_raft_replication_lag_entries`: Replication lag per follower

## Failure Scenarios

### Follower Failure

When a follower fails:
- Cluster continues operating (as long as quorum is maintained)
- Failed node can rejoin when restarted
- State is automatically synchronized

### Leader Failure

When the leader fails:
- Automatic leader election occurs
- New leader is elected from remaining followers
- Workflow execution continues after election completes
- Typical election time: <1 second

### Network Partition

During a network partition:
- Only the partition with majority (quorum) can elect a leader
- Minority partition cannot process writes
- When partition heals, state is automatically synchronized

## Node Management

### Adding a Node

Start a new `flowraft-node serve` with the appropriate `--id` and `--peers` (or join config). The new node will join the existing cluster.

### Removing a Node

Stop the `flowraft-node` process. As long as a quorum remains, the cluster continues. The node can rejoin later by restarting `flowraft-node serve` with the same `--peers` or join configuration.

### Restarting a Node

Restart the `flowraft-node serve` process. It will rejoin and catch up from the Raft log.

## Workflow Distribution

### Registering Workflows

Use `flowraft workflow define --file /path/to/workflow.json` (or the gRPC `DefineWorkflow`) to register workflows. The defining request is replicated via Raft.

### Workflow Execution

Use `flowraft workflow trigger` or the gRPC `TriggerWorkflow`. Tasks can run on any node; state is replicated to all nodes.

## Metrics and Observability

### Cluster Metrics

```rust
let metrics = MetricsCollector::new();

// Record state replication
metrics.record_state_replication(bytes, duration, success);

// Record leader election
metrics.record_leader_election(old_leader, new_leader, duration);

// Get cluster metrics summary
let summary = metrics.get_cluster_metrics().await;
```

### Prometheus Integration

`flowraft-node serve --http 127.0.0.1:9090` exposes `/health` and `/metrics` on the HTTP port. Point Prometheus at `http://<node>:9090/metrics`.

## Production Best Practices

### 1. Use Odd Number of Nodes

For quorum-based consensus, use 3, 5, or 7 nodes:
- 3 nodes: Can tolerate 1 failure
- 5 nodes: Can tolerate 2 failures
- 7 nodes: Can tolerate 3 failures

### 2. Monitor Leader Health

Set up alerts for:
- Leader election events
- High replication lag
- Node failures

### 3. Graceful Shutdown

Stop `flowraft-node` cleanly (e.g. SIGTERM) so it can flush and close. Avoid SIGKILL when possible.

### 4. Regular Health Checks

Monitor cluster health with `flowraft cluster status` and HTTP `GET /health` on each node’s `--http` address.

### 5. Backup and Recovery

- Regular snapshots (when persistent storage is enabled)
- Backup Raft logs
- Test recovery procedures

## Example: TCP multi-node cluster

See `examples/tcp_multi_node_cluster.rs` for a 3-node TCP example. For production, run `flowraft-node serve` on each host and use `scripts/serve.sh` or your orchestrator.

## Troubleshooting

### Node Cannot Join Cluster

- Verify leader address is correct
- Check network connectivity
- Ensure leader is running and accessible

### High Replication Lag

- Check network latency between nodes
- Monitor node CPU/memory usage
- Consider increasing Raft batch size

### Leader Election Issues

- Ensure quorum is maintained (majority of nodes available)
- Check network connectivity
- Review Raft logs for errors
