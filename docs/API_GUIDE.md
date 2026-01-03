# FlowRaft API Guide

## Core APIs

### Graph Builder

Type-safe workflow definition:

```rust
let graph = GraphBuilder::new("workflow")
    .add_node("task1", "handler1", vec![], vec![], None)
    .add_node("task2", "handler2", vec![], vec![], None)
    .add_simple_edge("task1", "task2")
    .set_root("task1")
    .build()?;
```

**Key Methods**:
- `add_node(name, handler, inputs, outputs, timeout)`: Add task node
- `add_simple_edge(from, to)`: Sequential dependency
- `add_conditional_edge(from, condition, then, else)`: Conditional branching
- `add_split_edge(from, split, branches)`: Parallel split
- `add_merge_edge(branches, merge, to)`: Parallel merge
- `set_root(name)`: Set entry point

### FlowRaftApp

Application layer for workflow management:

```rust
let app = FlowRaftApp::builder()
    .with_node_id(1)
    .enable_metrics(true)
    .build_single_node()
    .await?;

// Register workflow
let workflow_id = app.register_workflow(workflow_def).await?;

// Get workflow state
let workflow = app.get_workflow(&workflow_id).await;

// Get all workflows
let workflows = app.get_all_workflows().await;
```

### WorkflowExecutor

Execute workflows with task handlers:

```rust
let executor = WorkflowExecutor::new(raft, state_machine, node_id);

// Execute single task
executor.execute_task(workflow_id, task_id, &handler, inputs).await?;

// Get ready tasks (dependencies satisfied)
let ready = executor.get_ready_tasks(&workflow_id).await;
```

### HandlerRegistry

Register and manage task handlers:

```rust
let registry = HandlerRegistry::new();
registry.register_handler(
    workflow_id,
    "handler_name".to_string(),
    Arc::new(MyHandler) as Arc<dyn TaskHandler>,
).await;
```

### gRPC Client

Remote workflow management:

```rust
let client = FlowRaftClient::builder()
    .with_endpoint("http://localhost:50051")
    .with_timeout(Duration::from_secs(30))
    .build()?;

// Submit workflow
let exec_id = client.submit_workflow("workflow_name", json!({})).await?;

// Get status
let status = client.get_workflow_status(exec_id).await?;

// Watch execution
let mut stream = client.watch_workflow(exec_id).await?;
while let Some(update) = stream.next().await {
    // Handle update
}

// Control operations
client.pause_workflow(exec_id).await?;
client.resume_workflow(exec_id).await?;
client.cancel_workflow(exec_id).await?;
```

## Error Handling

All APIs return `Result<T, E>` where errors are:

- **Raft errors**: Consensus failures, network issues
- **State machine errors**: Invalid transitions, missing workflows
- **Validation errors**: Invalid graph structure, missing dependencies
- **Execution errors**: Handler failures, timeouts

## Type System

### WorkflowId / TaskId

UUID-based identifiers with type safety:

```rust
let workflow_id = WorkflowId::default(); // or parse from string
let task_id = TaskId::default();
```

### WorkflowState / TaskState

Type-safe state machines:

```rust
match workflow.state {
    WorkflowState::Draft => { /* ... */ }
    WorkflowState::Running => { /* ... */ }
    WorkflowState::Completed => { /* ... */ }
    WorkflowState::Failed { error_message } => { /* ... */ }
    // ...
}
```

### RetryConfig

Configurable retry behavior:

```rust
let retry = RetryConfig::new(3); // max 3 attempts
let retry = RetryConfig::with_backoff(3, 2.0, 1000); // with exponential backoff
```

## Advanced Patterns

### Conditional Execution

```rust
builder.add_conditional_edge(
    "input",
    Arc::new(MyCondition) as Arc<dyn ConditionObject>,
    "then_task",
    "else_task",
);
```

### Parallel Execution

```rust
builder.add_split_edge(
    "start",
    Arc::new(MySplit) as Arc<dyn SplitObject>,
    vec!["branch1", "branch2", "branch3"],
);

builder.add_merge_edge(
    vec!["branch1", "branch2", "branch3"],
    Arc::new(MyMerge) as Arc<dyn MergeObject>,
    "merge_task",
);
```

### Dynamic Workflows

```rust
use flow_raft_api::graph::DynamicGraphBuilder;

let mut builder = DynamicGraphBuilder::new("dynamic");
// Build graph dynamically...
let graph = builder.build()?;
let workflow = dynamic_graph_to_workflow(graph, workflow_id, retry_config, inputs)?;
```

## Observability

### Metrics

Prometheus metrics exposed at `/metrics`:
- `flowraft_workflows_registered_total`
- `flowraft_tasks_executed_total`
- `flowraft_raft_replication_duration_seconds`
- `flowraft_task_execution_duration_seconds`

### History

Execution history via `HistoryStore`:

```rust
let history = history_store.get_history(&workflow_id, None).await;
```

### Event Streaming

Watch workflow updates:

```rust
let mut stream = watcher.watch_workflow(workflow_id).await?;
while let Some(update) = stream.next().await {
    // Handle update
}
```
