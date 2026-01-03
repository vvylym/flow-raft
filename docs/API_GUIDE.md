# FlowRaft API Guide

## Overview

FlowRaft provides multiple APIs for defining and executing workflows:

1. **Graph Builder API**: Type-safe and dynamic workflow definition
2. **gRPC API**: Remote workflow management
3. **Direct API**: Programmatic workflow creation and execution

## Graph Builder API

### Simplified Function-Based API

The recommended approach is to use the simplified function-based API:

```rust
use flow_raft::prelude::*;

// Define your functions
fn task1(input: MyInput) -> Result<MyOutput, String> { ... }
fn task2(input: MyOutput) -> Result<FinalResult, String> { ... }

// Build workflow using functions
let workflow_graph = GraphBuilder::new("my_workflow")
    .add_node_fn("task1", wrap_function(task1), None)
    .add_node_fn("task2", wrap_function(task2), None)
    .add_edge("task1", "task2")
    .set_root("task1")
    .build()?;

// Convert to workflow definition
let workflow_def = WorkflowDef::from_graph("my_workflow", workflow_graph, RetryConfig::default());
```

### Type-Safe Builder (Alternative)

You can also use the traditional handler-based approach:

```rust
use flow_raft::prelude::*;

let workflow_graph = GraphBuilder::new("my_workflow")
    .add_node("task1", "handler1", vec![], vec![], None)
    .add_node("task2", "handler2", vec![], vec![], None)
    .add_simple_edge("task1", "task2")
    .set_root("task1")
    .build()?;

let workflow_def = WorkflowDef::from_graph("my_workflow", workflow_graph, RetryConfig::default());
```

### Dynamic Builder

```rust
use flow_raft::api::graph::DynamicGraphBuilder;

let mut builder = DynamicGraphBuilder::new("my_workflow");
builder
    .add_node("task1", "handler1", vec![], vec![], None)
    .add_node("task2", "handler2", vec![], vec![], None)
    .add_simple_edge("task1", "task2");

let graph = builder.build()?;
let workflow = dynamic_graph_to_workflow(graph, workflow_id, retry_config, inputs)?;
```

### Conditional Edges

```rust
builder.add_conditional_edge(
    "input",
    Arc::new(MyCondition) as Arc<dyn ConditionObject>,
    "then_task",
    "else_task",
);
```

### Split/Merge Edges

```rust
builder.add_split_edge(
    "start",
    Arc::new(MySplit) as Arc<dyn SplitObject>,
    vec!["branch1", "branch2"],
);

builder.add_merge_edge(
    vec!["branch1", "branch2"],
    Arc::new(MyMerge) as Arc<dyn MergeObject>,
    "merge_task",
);
```

## Builder Pattern API

### Creating a Single-Node App

```rust
use flow_raft::prelude::*;

let app = FlowRaftApp::builder()
    .with_node_id(1)
    .with_workflows(vec![workflow_def])
    .enable_metrics(true)
    .build_single_node()
    .await?;
```

### Creating a Cluster

```rust
use flow_raft::prelude::*;

let nodes = launch_cluster(vec![
    (1, NodeMode::Leader, vec![workflow1_def]),
    (2, NodeMode::Follower, vec![workflow2_def]),
    (3, NodeMode::Follower, vec![]),
])
.await?;
```

## Direct API (Advanced)

### Creating a Workflow Manually

```rust
use flow_raft::prelude::*;

let app = FlowRaftApp::new(raft, state_machine);
let snapshot = WorkflowSnapshot::from_workflow(&running_workflow);
let request = Request::CreateWorkflow { workflow: snapshot };
let response = app.create_workflow(request).await?;
```

### Registering Workflows

```rust
// Using the builder pattern (recommended)
let app = FlowRaftApp::builder()
    .with_node_id(1)
    .with_workflows(vec![workflow_def])
    .build_single_node()
    .await?;

// Or register after creation
app.register_workflow(workflow_def).await?;
```

### Executing a Workflow

```rust
use flow_raft::api::handlers::executor::HandlerExecutor;

let handler_executor = HandlerExecutor::new(executor, registry);

// Register handlers
registry.register_handler(workflow_id, "handler1", handler1).await;
registry.register_handler(workflow_id, "handler2", handler2).await;

// Execute workflow
handler_executor.execute_workflow(workflow_id, max_iterations).await?;
```

### Task Handler Implementation

```rust
use flow_raft::raft::executor::TaskHandler;

struct MyTaskHandler;

impl TaskHandler for MyTaskHandler {
    fn execute(
        &self,
        task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        // Execute task logic
        Ok(serde_json::json!({"result": "success"}))
    }
}
```

## gRPC API

### Service Definition

See `src/api/grpc/proto/flowraft.proto` for full service definition.

### Key Methods

- `CreateWorkflow`: Create a new workflow
- `GetWorkflow`: Get workflow status
- `ListWorkflows`: List all workflows
- `StartWorkflow`: Start workflow execution
- `CancelWorkflow`: Cancel a running workflow

### Example Usage

```rust
// gRPC client usage (pseudo-code)
let mut client = FlowRaftClient::connect("http://localhost:50051").await?;

let request = CreateWorkflowRequest {
    definition: workflow_json,
    inputs: inputs_json,
};

let response = client.create_workflow(request).await?;
```

## Observability API

### Metrics

```rust
use flow_raft::api::observability::metrics::MetricsCollector;

let metrics = MetricsCollector::new();
metrics.record_workflow_start(workflow_id).await;
metrics.record_task_execution(task_id, duration).await;
```

### History

```rust
use flow_raft::api::observability::history::ExecutionHistory;

let history = ExecutionHistory::new();
history.record_event(workflow_id, event).await;
let events = history.get_history(workflow_id, limit).await;
```

### Watcher

```rust
use flow_raft::api::observability::watcher::WorkflowWatcher;

let watcher = WorkflowWatcher::new();
let mut receiver = watcher.watch_workflow(workflow_id).await;

while let Ok(update) = receiver.recv().await {
    println!("Workflow update: {:?}", update);
}
```

## Node Management

### Launching a Leader Node

```rust
use flow_raft::api::node::launcher::launch_leader;

let config = NodeConfig {
    node_id: 1,
    raft_config: default_config(),
    // ...
};

let node = launch_leader(config, network).await?;
```

### Launching a Follower Node

```rust
use flow_raft::api::node::launcher::launch_follower;

let cluster_nodes = vec![1, 2, 3];
let node = launch_follower(config, network, cluster_nodes).await?;
```

## Error Handling

All APIs return `Result` types with specific error variants:

- `HandlerExecutionError`: Handler-related errors
- `RaftError`: Raft consensus errors
- `WorkflowError`: Workflow state errors
- `TaskError`: Task execution errors

## Best Practices

1. **Always register handlers before executing workflows**
2. **Use type-safe builder for compile-time workflows**
3. **Use dynamic builder for runtime-defined workflows**
4. **Handle errors appropriately (retry, log, etc.)**
5. **Monitor workflows using observability APIs**
6. **Use watchers for real-time updates**
