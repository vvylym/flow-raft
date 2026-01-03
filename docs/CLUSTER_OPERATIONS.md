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

let app = FlowRaftApp::builder()
    .with_node_id(1)
    .with_workflows(vec![workflow_def])
    .enable_metrics(true)
    .build_single_node()
    .await?;
```

### Multi-Node Cluster

```rust
use flow_raft::prelude::*;

// Launch a 3-node cluster
let nodes = launch_cluster(vec![
    (1, NodeMode::Leader, vec![workflow1_def.clone()]),
    (2, NodeMode::Follower, vec![workflow2_def.clone()]),
    (3, NodeMode::Follower, vec![]),
])
.await?;
```

## Builder Pattern

The builder pattern provides a consistent API for both single-node and cluster deployments:

```rust
// Single node
let app = FlowRaftApp::builder()
    .with_node_id(1)
    .with_workflows(vec![workflow_def])
    .enable_metrics(true)
    .build_single_node()
    .await?;

// Cluster node (leader)
let leader = FlowRaftApp::builder()
    .with_node_id(1)
    .with_workflows(vec![workflow_def])
    .enable_metrics(true)
    .build_cluster_node(NodeRole::Leader, None)
    .await?;

// Cluster node (follower)
let follower = FlowRaftApp::builder()
    .with_node_id(2)
    .with_workflows(vec![workflow_def])
    .enable_metrics(true)
    .build_cluster_node(NodeRole::Follower, Some("http://leader:8080".to_string()))
    .await?;
```

## Leader Election

FlowRaft uses Raft consensus for leader election:

- **Automatic**: Leader election happens automatically when the current leader fails
- **Quorum Required**: Majority of nodes must be available for leader election
- **Split-Brain Prevention**: Raft ensures only one leader exists at a time

### Monitoring Leader Election

```rust
// Check cluster status
let status = node.cluster_status().await;
println!("Current leader: {:?}", status.leader);
println!("Node role: {:?}", status.role);
```

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

```rust
// Add a new follower node
let new_node = launch_cluster_node(
    NodeConfig::new(4, NodeMode::Follower),
    "http://leader:8080".to_string(),
)
.await?;
```

### Removing a Node

```rust
// Gracefully shutdown a node
node.shutdown().await?;
```

### Restarting a Node

```rust
// Restart a failed node
let restarted_node = launch_cluster_node(
    NodeConfig::new(node_id, NodeMode::Follower),
    leader_address,
)
.await?;
```

## Workflow Distribution

### Registering Workflows

Workflows can be registered on any node:

```rust
// Register on leader
nodes[0].register_workflow(workflow_def).await?;

// Register on follower
nodes[1].register_workflow(workflow_def).await?;
```

### Workflow Execution

Tasks can be executed on any node:
- Leader can execute tasks
- Followers can execute tasks
- State is replicated to all nodes

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

FlowRaft exposes metrics via Prometheus:

```rust
// Start metrics server
start_metrics_server(8080).await?;

// Metrics available at http://localhost:8080/metrics
```

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

Always use `shutdown()` for graceful node shutdown:
- Stops accepting new workflows
- Waits for in-flight operations
- Records shutdown metrics

### 4. Regular Health Checks

Monitor cluster health:
```rust
let status = node.cluster_status().await;
// Check: status.leader, status.nodes, status.role
```

### 5. Backup and Recovery

- Regular snapshots (when persistent storage is enabled)
- Backup Raft logs
- Test recovery procedures

## Example: Production Cluster

See `examples/production_cluster.rs` for a complete example demonstrating:
- 3-node cluster setup
- Leader/follower configuration
- Node shutdown scenarios
- Leader election
- Node restart and rejoin
- Metrics monitoring

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
