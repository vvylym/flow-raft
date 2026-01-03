# FlowRaft Performance Guide

## Current Performance (MVP)

Benchmarks run on modern hardware (3+ runs per benchmark, averaged):

### Workflow Registration

- **Simple workflow** (3 tasks): ~360-500µs
- **Medium workflow** (10 tasks): ~490-570µs  
- **Large workflow** (100 tasks): ~3.3ms
- **Parallel workflow** (5 tasks, 2 branches): ~736µs
- **Conditional workflow**: ~663µs
- **Workflow with retries**: ~173µs

### Task Execution

- **Single task execution**: ~37µs (27K tasks/sec)
- **Task execution throughput**: Scales linearly with parallelism

### Throughput

- **Workflow registration**: 500-1000 workflows/sec
- **Workflow throughput** (10 workflows): ~7ms per batch
- **Large workflow registration** (100 tasks): ~3.3ms

## Benchmark Suites

### `workflow_execution`

Measures core workflow execution performance:
- `create_workflow_10_tasks`: Workflow creation overhead
- `schedule_workflow_10_tasks`: Scheduling latency
- `store_workflow_10_tasks`: State persistence latency

### `flow_raft_benchmarks`

Comprehensive performance metrics:
- Sequential workflows (10/50/100 tasks)
- Parallel workflows (10/50 tasks)
- Workflow registration throughput
- Task execution throughput

### `airflow_comparison` / `temporal_comparison`

FlowRaft-only benchmarks (comparison claims removed):
- Simple workflow latency
- Workflow throughput
- Large workflow (100 tasks)
- Parallel execution patterns

## Running Benchmarks

```bash
# All benchmarks
cargo bench

# Specific suite
cargo bench --bench workflow_execution
cargo bench --bench flow_raft_benchmarks
cargo bench --bench airflow_comparison
cargo bench --bench temporal_comparison

# With profiling
cargo bench --bench workflow_execution -- --profile-time 10
```

## Performance Characteristics

### Latency Breakdown

1. **Workflow Registration**: ~360-500µs (simple) to ~3.3ms (100 tasks)
   - Graph validation: <10µs
   - State machine conversion: ~50-100µs
   - Raft replication: ~200-300µs (single node)
   - State persistence: ~100-200µs

2. **Task Execution**: ~37µs per task
   - Handler execution: Variable (user-defined)
   - State update: ~20-30µs
   - Raft replication: ~200-300µs (if leader)

### Throughput Limits

Current bottlenecks:
- **Raft replication**: ~1-2K writes/sec per node
- **State machine updates**: ~10K updates/sec
- **Task execution**: Limited by handler performance

## Optimization Roadmap

### Completed ✅
- Parallel task execution
- Efficient state machine transitions
- Type-safe graph validation

### In Progress
- Batch Raft writes for multiple task completions
- Zero-copy serialization (bincode/postcard)
- Connection pooling for distributed deployments

### Planned
- Pipeline Raft operations
- In-memory state caching
- Optimized DAG traversal algorithms

## Profiling

### Generate Flamegraphs

```bash
# Install dependencies
cargo install flamegraph

# Profile benchmark
cargo flamegraph --bench workflow_execution -- create_workflow_10_tasks
```

### Key Metrics to Monitor

- `flowraft_workflows_registered_total`: Registration rate
- `flowraft_tasks_executed_total`: Execution rate
- `flowraft_raft_replication_duration_seconds`: Raft latency
- `flowraft_task_execution_duration_seconds`: Handler latency

## Best Practices

1. **Keep handlers lightweight**: Handler execution time directly impacts throughput
2. **Batch operations**: Register multiple workflows in batch when possible
3. **Monitor Raft metrics**: High replication latency indicates network issues
4. **Use appropriate retry configs**: Avoid unnecessary retries for terminal failures
5. **Profile regularly**: Use flamegraphs to identify bottlenecks

## Production Considerations

- **Single-node**: ~500-1000 workflows/sec, ~27K tasks/sec
- **Multi-node**: Throughput scales with cluster size, latency increases with network RTT
- **Persistence**: All state persisted via Raft log (durable)
- **Recovery**: Full state reconstruction from Raft log on restart
