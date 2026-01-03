# Phase 3 - State Structure

> **Status**: ✅ **Implemented** (Phase 3 - State Machine)  
> **Implementation**: Immutable state structures in `src/core/task/` and `src/core/workflow/`  
> **See**: [ROADMAP.md](../ROADMAP.md) for implementation status

## Alternatives Considered

1. **Mutable In-place vs Immutable Transitions**
   - Mutable in-place: Simple, direct state updates, requires careful synchronization
   - Immutable transitions: Thread-safe, easier to reason about, supports replication
   - **Choice**: Immutable transitions (return new state instances)
   - **Reasoning**: 
     - Thread-safe by default (no shared mutable state)
     - Easier to reason about (functional style)
     - Supports deterministic replication (can replay transitions)
     - No locks needed for concurrent access

2. **Arc vs Owned Data**
   - Arc for shared data: Reduces allocations, allows sharing
   - Owned data: Simple, no reference counting overhead
   - **Choice**: Owned data for state structures
   - **Reasoning**: 
     - State transitions are infrequent enough that allocation cost is acceptable
     - Simpler ownership model (no reference counting)
     - Immutable transitions mean we can clone when needed
     - IndexMap and SmallVec already optimize allocations

3. **Separate Task Definition vs Inline Task Data**
   - Inline task data: Simple, all data in one place
   - Separate TaskDefinition: Separates definition from execution state
   - **Choice**: Separate `TaskDefinition` and `TaskExecution`
   - **Reasoning**: 
     - Task definition is immutable (name, handler, inputs, outputs)
     - Task execution is mutable (state, attempts, timestamps, outputs)
     - Clear separation of concerns
     - Enables efficient storage (definition stored once, execution updated)

4. **Snapshot Format**
   - Minimal snapshot: Only essential state
   - Full snapshot: Complete state including all metadata
   - **Choice**: Full snapshot with all metadata
   - **Reasoning**: 
     - Enables complete recovery from snapshot
     - Supports debugging and inspection
     - Serialization overhead is acceptable for persistence
     - Can derive minimal views from full snapshot

5. **Timestamp Tracking Strategy**
   - Single timestamp: Simple, but less information
   - Multiple timestamps: More context, better observability
   - **Choice**: Multiple timestamps (`created_at`, `started_at`, `completed_at`)
   - **Reasoning**: 
     - Provides full lifecycle visibility
     - Enables duration calculations
     - Supports debugging and monitoring
     - Minimal overhead (DateTime is small)

6. **State Enum vs Phantom Types for Snapshots**
   - Phantom types: Type-safe, but can't serialize generic types
   - State enum: Serializable, runtime representation
   - **Choice**: State enum (`WorkflowState`, `TaskState`) for snapshots
   - **Reasoning**: 
     - Serialization requires concrete types
     - Enum provides runtime state representation
     - Phantom types used for compile-time safety in transitions
     - `From` trait bridges phantom types to enum

## Choice Made

**Immutable State Transitions + Separate Definition/Execution + Full Snapshots**

- All state transitions return new instances (immutable)
- `TaskDefinition` separates immutable task metadata from mutable execution state
- `TaskExecution` tracks runtime state (state, attempts, timestamps, outputs)
- `WorkflowSnapshot` provides complete serializable state representation
- Multiple timestamps for full lifecycle tracking
- State enums for serialization, phantom types for compile-time safety

## Purpose

Support deterministic replication and recovery by providing immutable state structures that can be serialized, replayed, and inspected. Enable thread-safe concurrent access without locks through immutability.

## Pros

- **Thread-safe**: Immutable structures are safe for concurrent access
- **Deterministic**: Same transitions produce same results (supports replication)
- **Recoverable**: Full snapshots enable complete state recovery
- **Observable**: Multiple timestamps provide full lifecycle visibility
- **Clear Separation**: Task definition vs execution state separation
- **Serializable**: State enums enable persistence and network transmission
- **No Locks**: Immutability eliminates need for synchronization primitives

## Cons

- **More Allocations**: Each transition creates new instance (mitigated by efficient collections)
- **Memory Usage**: Full snapshots include all metadata (acceptable for persistence)
- **Clone Overhead**: Need to clone data for transitions (but transitions are infrequent)
- **Type Conversion**: Need to convert between phantom types and enums (minimal overhead)

## Implementation Details

### Workflow State Structure
```rust
pub struct Workflow<State = WorkflowDraft> {
    pub id: WorkflowId,
    pub task_definitions: IndexMap<TaskId, TaskDefinition>,
    pub executions: IndexMap<TaskId, TaskExecution>,
    pub dependencies: IndexMap<TaskId, TaskDependencies>,
    pub retry_configs: IndexMap<TaskId, RetryConfig>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub inputs: serde_json::Value,
    pub outputs: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub state: State,  // Phantom type parameter
}
```

### Task Definition (Immutable)
```rust
pub struct TaskDefinition {
    pub id: TaskId,
    pub name: String,
    pub handler: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub timeout_secs: Option<u64>,
}
```

### Task Execution (Mutable Runtime State)
```rust
pub struct TaskExecution {
    pub task_id: TaskId,
    pub state: TaskState,  // Enum, not phantom type
    pub attempts: u32,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub outputs: Option<serde_json::Value>,
}
```

### Workflow Snapshot (Serializable)
```rust
pub struct WorkflowSnapshot {
    pub workflow_id: WorkflowId,
    pub state: WorkflowState,  // Enum for serialization
    pub task_definitions: IndexMap<TaskId, TaskDefinition>,
    pub executions: IndexMap<TaskId, TaskExecution>,
    pub dependencies: IndexMap<TaskId, TaskDependencies>,
    pub retry_configs: IndexMap<TaskId, RetryConfig>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub inputs: serde_json::Value,
    pub outputs: Option<serde_json::Value>,
    pub error_message: Option<String>,
}
```

### Immutable Transition Pattern
```rust
impl Workflow<WorkflowDraft> {
    pub fn schedule(self) -> Result<Workflow<WorkflowScheduled>, WorkflowError> {
        // Validation...
        Ok(Workflow {
            // ... copy all fields
            state: WorkflowScheduled,  // New state
            // ...
        })
    }
}
```

## Lessons Learned

1. **Immutable Transitions**: Returning new instances instead of mutating in-place makes the code thread-safe and easier to reason about. The allocation cost is acceptable given that transitions are infrequent compared to reads.

2. **Definition vs Execution Separation**: Separating `TaskDefinition` (immutable) from `TaskExecution` (mutable) provides clear ownership and enables efficient storage patterns. The definition can be stored once while execution state is updated.

3. **Full Snapshots**: Storing complete state in snapshots enables full recovery and debugging. The serialization overhead is acceptable for persistence operations, which are infrequent.

4. **Timestamp Tracking**: Multiple timestamps (`created_at`, `started_at`, `completed_at`) provide valuable observability. The overhead is minimal and the benefits for debugging and monitoring are significant.

5. **State Enum for Serialization**: Using enums (`WorkflowState`, `TaskState`) for serialization while using phantom types for compile-time safety provides the best of both worlds. The `From` trait elegantly bridges these representations.

6. **IndexMap for Determinism**: Using `IndexMap` for task storage ensures deterministic iteration order, which is important for reproducible behavior in replication scenarios.

7. **Parallel Status Calculation**: Using rayon for status aggregation in snapshots improves performance for large workflows without adding complexity.

## What to Do Better Next

1. **Version Tracking**: Consider adding explicit version numbers to state structures for optimistic concurrency control and conflict resolution in distributed scenarios.

2. **Incremental Snapshots**: For very large workflows, consider incremental snapshots that only store changes since last snapshot, reducing serialization size.

3. **Snapshot Compression**: Consider compressing snapshots before storage to reduce I/O and storage costs.

4. **State Diffing**: Implement state diffing to enable efficient state synchronization and reduce network traffic in replication scenarios.

5. **Memory Pooling**: For high-throughput scenarios, consider memory pooling for state structures to reduce allocation pressure.

6. **Snapshot Validation**: Add validation to ensure snapshots are consistent (e.g., all referenced tasks exist, dependencies are valid).

7. **State Size Limits**: Consider adding size limits to prevent unbounded state growth (e.g., maximum number of tasks, maximum output size).

8. **Lazy Loading**: For very large workflows, consider lazy loading of task definitions or execution states that aren't immediately needed.

9. **State Archival**: Implement archival strategies for completed workflows to reduce memory usage while preserving history.

10. **Snapshot Versioning**: Version snapshot format to enable migration and backward compatibility as the state structure evolves.

---

## Implementation Status

✅ **Fully Implemented** - All design decisions were implemented as described:

- **Immutable Transitions**: All state transitions return new instances
- **TaskDefinition/TaskExecution**: Separated immutable definition from mutable execution state
- **WorkflowSnapshot**: Full serializable state representation implemented
- **Multiple Timestamps**: `created_at`, `started_at`, `completed_at` tracked
- **State Enums**: `WorkflowState` and `TaskState` enums for serialization
- **IndexMap**: Used for deterministic iteration order

The state structure supports deterministic replication via Raft (Phase 4) and has been validated through comprehensive testing.
