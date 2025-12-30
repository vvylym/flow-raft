# Phase 5 - Execution Layer

> **Status**: ✅ **Fully Implemented** (Phase 5 - Execution Layer)  
> **Implementation**: `src/raft/executor.rs` and `src/api/handlers/` modules  
> **See**: [ROADMAP.md](../ROADMAP.md) for implementation status

## Alternatives Considered

1. **Coordination vs Execution Separation**
   - Tight coupling: Simple, but mixes concerns
   - Clear separation: Complex, but maintainable
   - **Choice**: Clear separation (`WorkflowExecutor` for coordination, `TaskHandler` for execution)
   - **Reasoning**: 
     - Separation of concerns improves maintainability
     - Allows independent scaling of coordination and execution
     - Easier to test and reason about
     - Supports different execution models (local, remote, async)

2. **Task Execution Location**
   - Leader-only execution: Simple, but limits scalability
   - Any-node execution: Complex, but scalable
   - **Choice**: Any-node execution
   - **Reasoning**: 
     - Better resource utilization
     - Improved scalability
     - Fault tolerance (work continues if leader fails)
     - Matches distributed workflow execution model

3. **Handler Registration Strategy**
   - Global handlers: Simple, but less flexible
   - Per-workflow handlers: Complex, but flexible
   - **Choice**: Per-workflow handler registration (`HandlerRegistry`)
   - **Reasoning**: 
     - Supports different handlers for same task in different workflows
     - Enables workflow-specific customization
     - Better isolation between workflows
     - More flexible execution model

4. **Execution Loop Design**
   - Polling-based: Simple, but inefficient
   - Event-driven: Complex, but efficient
   - **Choice**: Polling-based with configurable interval
   - **Reasoning**: 
     - Simpler implementation for MVP
     - Sufficient for initial use cases
     - Can be extended to event-driven later
     - Clear control flow

5. **Task Input Resolution**
   - Static inputs: Simple, but limited
   - Dynamic inputs from dependencies: Complex, but powerful
   - **Choice**: Dynamic inputs from workflow inputs and prerequisite outputs
   - **Reasoning**: 
     - Enables data flow between tasks
     - Supports complex workflow patterns
     - More flexible than static inputs
     - Matches typical workflow execution model

6. **State Update Strategy**
   - Optimistic updates: Fast, but may conflict
   - Pessimistic updates via Raft: Slower, but consistent
   - **Choice**: Pessimistic updates via Raft
   - **Reasoning**: 
     - Ensures consistency across cluster
     - Prevents race conditions
     - Matches Raft-based coordination model
     - Acceptable latency for workflow execution

## Choice Made

**Separated Coordination and Execution + Any-Node Execution + Per-Workflow Handlers + Polling Loop + Raft Updates**

- `WorkflowExecutor` bridges Raft state machine with task execution
- `TaskHandler` trait for user-defined task execution
- `HandlerRegistry` for per-workflow handler registration
- `HandlerExecutor` for workflow execution loop
- Task execution on any node, state updates via Raft
- Dynamic input resolution from workflow inputs and prerequisite outputs

## Purpose

Enable task execution while maintaining consistency through Raft coordination. Support flexible handler registration and any-node execution for scalability.

## Pros

- **Separation of Concerns**: Clear boundary between coordination and execution
- **Scalability**: Any-node execution improves resource utilization
- **Flexibility**: Per-workflow handlers support different execution models
- **Consistency**: Raft ensures state updates are consistent across cluster
- **Fault Tolerance**: Execution continues even if leader fails
- **Data Flow**: Dynamic input resolution enables complex workflows
- **Testability**: Clear interfaces make testing straightforward

## Cons

- **Polling Overhead**: Polling-based loop has some overhead
- **Raft Latency**: State updates require Raft consensus (acceptable trade-off)
- **Handler Management**: Per-workflow handlers require careful lifecycle management
- **Input Resolution**: Dynamic input resolution adds complexity
- **No Parallel Execution**: Tasks executed sequentially (can be optimized)

## Implementation Details

### Task Handler Trait
```rust
pub trait TaskHandler: Send + Sync {
    fn execute(
        &self,
        task_id: TaskId,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
}
```

### Workflow Executor
```rust
pub struct WorkflowExecutor {
    raft: Arc<Raft<TypeConfig>>,
    state_machine: StateMachineStore<TypeConfig>,
    node_id: u64,
}

impl WorkflowExecutor {
    pub async fn get_ready_tasks(&self, workflow_id: &WorkflowId) -> Vec<TaskId> {
        // Get workflow from state machine
        // Filter for ready tasks (dependencies satisfied, not in progress)
    }

    pub async fn execute_task(
        &self,
        workflow_id: WorkflowId,
        task_id: TaskId,
        handler: &dyn TaskHandler,
        inputs: serde_json::Value,
    ) -> Result<(), RaftError> {
        // Execute task locally
        // Update state via Raft
    }
}
```

### Handler Registry
```rust
pub struct HandlerRegistry {
    handlers: Arc<RwLock<HashMap<(WorkflowId, String), Arc<dyn TaskHandler>>>>,
}

impl HandlerRegistry {
    pub async fn register_handler(
        &self,
        workflow_id: WorkflowId,
        handler_name: String,
        handler: Arc<dyn TaskHandler>,
    ) {
        // Store handler per workflow
    }
}
```

### Handler Executor
```rust
pub struct HandlerExecutor {
    executor: Arc<WorkflowExecutor>,
    registry: Arc<HandlerRegistry>,
}

impl HandlerExecutor {
    pub async fn execute_workflow(
        &self,
        workflow_id: WorkflowId,
        max_iterations: usize,
    ) -> Result<(), HandlerExecutionError> {
        // Loop until workflow complete:
        // 1. Get ready tasks
        // 2. Execute each task using registered handler
        // 3. Wait for state updates
        // 4. Check if workflow complete
    }
}
```

### Input Resolution
```rust
// Merge workflow inputs with prerequisite task outputs
let mut task_inputs = workflow.inputs.clone();
if let Some(deps) = workflow.dependencies.get(&task_id) {
    for prereq_id in &deps.prerequisites {
        if let Some(prereq_exec) = workflow.executions.get(prereq_id) {
            if let Some(outputs) = &prereq_exec.outputs {
                // Merge outputs into task inputs
            }
        }
    }
}
```

## Lessons Learned

1. **Separation of Concerns**: Separating coordination (`WorkflowExecutor`) from execution (`TaskHandler`) made the code much cleaner and easier to test.

2. **Any-Node Execution**: Allowing any node to execute tasks significantly improved scalability. The Raft state machine ensures consistency regardless of execution location.

3. **Per-Workflow Handlers**: Per-workflow handler registration provides flexibility but requires careful lifecycle management. The `HandlerRegistry` handles this well.

4. **Input Resolution**: Dynamic input resolution from prerequisite outputs enables powerful workflow patterns. The merge logic is straightforward but important.

5. **Raft Updates**: Updating state via Raft after task execution ensures consistency but adds latency. This is an acceptable trade-off for correctness.

6. **Execution Loop**: The polling-based execution loop is simple but effective. It can be optimized later with event-driven updates.

7. **Error Handling**: Task execution errors are captured and stored in task state. This enables retry logic and error reporting.

8. **State Consistency**: Reading workflow state from the Raft state machine ensures all nodes see consistent state, even during execution.

## What to Do Better Next

1. **Parallel Execution**: Execute independent tasks in parallel to improve throughput.

2. **Event-Driven Loop**: Replace polling with event-driven updates for better efficiency.

3. **Handler Lifecycle**: Add explicit handler lifecycle management (registration, cleanup, versioning).

4. **Input Validation**: Add validation for task inputs before execution to catch errors early.

5. **Execution Timeouts**: Add configurable timeouts for task execution to prevent hanging workflows.

6. **Retry Integration**: Integrate retry logic into execution loop for automatic retry of failed tasks.

7. **Execution Metrics**: Add metrics for task execution (duration, success rate, retry count).

8. **Remote Execution**: Support remote task execution via gRPC or HTTP for distributed execution.

9. **Execution Queues**: Add execution queues to manage task execution order and priority.

10. **Resource Limits**: Add resource limits (CPU, memory) for task execution to prevent resource exhaustion.

---

## Implementation Status

✅ **Fully Implemented** - All design decisions were implemented as described:

- **WorkflowExecutor**: Bridges Raft state machine with task execution
- **TaskHandler Trait**: User-defined task execution interface
- **HandlerRegistry**: Per-workflow handler registration and lookup
- **HandlerExecutor**: Workflow execution loop that drives task execution
- **Any-Node Execution**: Tasks can be executed on any node
- **Raft State Updates**: All task execution updates go through Raft consensus
- **Dynamic Input Resolution**: Task inputs resolved from workflow inputs and prerequisite outputs
- **Ready Task Detection**: Efficient detection of tasks ready for execution

The execution layer cleanly separates coordination (Raft) from execution (handlers), enabling scalable distributed workflow execution. Tasks can be executed on any node while maintaining consistency through Raft state updates. The per-workflow handler registration provides flexibility for different execution models.
