# Advanced Usage Guide

This guide covers advanced patterns and best practices for using FlowRaft in production environments.

## Table of Contents

1. [Error Handling and Retries](#error-handling-and-retries)
2. [Parallel Execution](#parallel-execution)
3. [Complex Conditionals](#complex-conditionals)
4. [Observability](#observability)
5. [Client API](#client-api)
6. [Cluster Operations](#cluster-operations)
7. [Performance Tuning](#performance-tuning)
8. [Troubleshooting](#troubleshooting)

## Error Handling and Retries

### Retry Strategies

FlowRaft supports configurable retry strategies with exponential backoff:

```rust
use flow_raft::prelude::*;

let workflow = GraphBuilder::new("retry_workflow")
    .with_retry_config(RetryConfig::new(3)) // 3 retries
    .add_node_fn("task", wrap_function(my_task), None)
    .build()?;
```

### Error Recovery Patterns

- **Partial Failure Handling**: Workflows can continue even if some tasks fail
- **Circuit Breaker**: Implement circuit breaker pattern in task handlers
- **Graceful Degradation**: Use conditional edges to route around failures

See `examples/advanced_error_handling.rs` for a complete example.

## Parallel Execution

### Dynamic Parallelism

FlowRaft supports parallel task execution through split/merge edges:

```rust
let workflow = GraphBuilder::new("parallel_workflow")
    .add_node("split", "split_handler", vec![], vec![], None)
    .add_node("process_1", "processor", vec![], vec![], None)
    .add_node("process_2", "processor", vec![], vec![], None)
    .add_node("merge", "merge_handler", vec![], vec![], None)
    .add_split_edge("split", split_object, vec!["process_1", "process_2"])
    .add_merge_edge(vec!["process_1", "process_2"], merge_object, "merge")
    .build()?;
```

### Concurrency Limits

Control concurrency at the handler level:

```rust
struct LimitedHandler {
    semaphore: Arc<tokio::sync::Semaphore>,
}

impl TaskHandler for LimitedHandler {
    fn execute(&self, _task_id: TaskId, inputs: serde_json::Value) -> Result<serde_json::Value, String> {
        // Semaphore limits concurrent executions
        // Implementation details...
    }
}
```

See `examples/advanced_parallelism.rs` for a complete example.

## Complex Conditionals

### Multi-Way Routing

Use conditional edges for complex routing logic:

```rust
#[derive(Debug)]
struct PriorityRouter;

impl ConditionObject for PriorityRouter {
    fn evaluate(&self, input: serde_json::Value) -> Result<NodeName, String> {
        let priority = input.get("priority").and_then(|v| v.as_str()).unwrap_or("standard");
        match priority {
            "high" => Ok(NodeName::new("high_priority_handler")),
            "medium" => Ok(NodeName::new("medium_priority_handler")),
            _ => Ok(NodeName::new("standard_handler")),
        }
    }
}
```

### State-Dependent Routing

Route based on workflow state or previous task outputs.

See `examples/advanced_conditionals.rs` for a complete example.

## Observability

### Metrics Collection

FlowRaft exposes Prometheus metrics at `/metrics`:

```rust
let app = FlowRaftApp::builder()
    .with_node_id(1)
    .enable_metrics(true)
    .with_metrics_port(9090)
    .build_single_node()
    .await?;
```

### Distributed Tracing

Enable OpenTelemetry tracing:

```rust
let app = FlowRaftApp::builder()
    .with_node_id(1)
    .with_tracing(TracingExporter::OTLP, Some("http://localhost:4317".to_string()))
    .build_single_node()
    .await?;
```

### Execution History

Query execution history for workflows:

```rust
let history = history_store.get_history(&workflow_id, 100).await;
```

See `examples/advanced_observability.rs` for a complete example.

## Client API

### gRPC Client Usage

```rust
use flow_raft::prelude::*;

let client = FlowRaftClient::builder()
    .with_endpoint("http://localhost:50051")
    .with_timeout(Duration::from_secs(300))
    .build();

// Submit workflow
let execution_id = client.run("my_workflow", inputs).await?;

// Watch execution
let mut stream = client.watch_execution(execution_id).await?;
while let Some(event) = stream.next().await {
    println!("Event: {:?}", event);
}
```

### Callback Patterns

Use callbacks for real-time updates:

```rust
client.run_with_callbacks(
    "workflow",
    inputs,
    |task_id, inputs| println!("Task {} started", task_id),
    |task_id, outputs| println!("Task {} completed", task_id),
    |task_id, error| println!("Task {} failed: {}", task_id, error),
).await?;
```

See `examples/advanced_client.rs` for a complete example.

## Cluster Operations

### Multi-Region Deployment

Deploy clusters across regions for high availability:

```rust
// Region 1 cluster
let region1_nodes = launch_cluster(vec![
    (1, NodeMode::Leader, workflows.clone()),
    (2, NodeMode::Follower, vec![]),
    (3, NodeMode::Follower, vec![]),
]).await?;

// Region 2 cluster
let region2_nodes = launch_cluster(vec![
    (4, NodeMode::Follower, workflows.clone()),
    (5, NodeMode::Follower, vec![]),
]).await?;
```

### Load Balancing

Distribute workflows across cluster nodes for load balancing.

### High Availability

- Minimum 3 nodes for quorum
- Leader election ensures continuous operation
- State replication across all nodes

See `examples/advanced_cluster.rs` for a complete example.

## Performance Tuning

### Workflow Optimization

1. **Minimize Dependencies**: Reduce task dependencies where possible
2. **Parallel Execution**: Use split/merge for independent tasks
3. **Batch Processing**: Group similar operations
4. **Resource Pooling**: Reuse expensive resources

### Raft Configuration

Tune Raft parameters for your workload:

```rust
let config = RaftConfig {
    election_timeout: 1000, // milliseconds
    heartbeat_interval: 100, // milliseconds
    // ... other settings
};
```

## Troubleshooting

### Common Issues

1. **Workflow Stuck**: Check for missing handlers or dependency cycles
2. **High Latency**: Review Raft configuration and network settings
3. **Memory Usage**: Monitor workflow count and state size
4. **Connection Errors**: Verify gRPC endpoint and network connectivity

### Debugging

Enable debug logging:

```bash
RUST_LOG=flow_raft=debug cargo run --example my_example
```

### Monitoring

- Check Prometheus metrics at `/metrics`
- Review distributed traces in OTLP-compatible backends (including Jaeger via OTLP endpoint)
- Monitor execution history for patterns

## Best Practices

1. **Idempotency**: Ensure all task handlers are idempotent
2. **Error Handling**: Implement comprehensive error handling
3. **Observability**: Enable metrics and tracing in production
4. **Testing**: Test failure scenarios and recovery
5. **Documentation**: Document workflow logic and dependencies

## Additional Resources

- [Quick Start Guide](QUICK_START.md)
- [API Reference](API_GUIDE.md)
- [Architecture Overview](ARCHITECTURE.md)
- [Performance Guide](PERFORMANCE.md)
