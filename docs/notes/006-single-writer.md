# Phase 3 - Single-Writer Semantics

> **Status**: ✅ **Implemented** (Phase 3 - State Machine, Phase 4 - Raft Integration)  
> **Implementation**: Raft consensus provides single-writer coordination via leader-only writes  
> **See**: [ROADMAP.md](../ROADMAP.md) for implementation status

## Alternatives Considered

1. **Lock-based vs Immutable State**
   - Lock-based: Mutex/RwLock for synchronization, mutable state
   - Immutable: No locks needed, return new state instances
   - **Choice**: Immutable state transitions
   - **Reasoning**: 
     - No locks needed (thread-safe by default)
     - Easier to reason about (no deadlocks, no race conditions)
     - Supports replication (deterministic transitions)
     - Better performance (no lock contention)

2. **Optimistic vs Pessimistic Concurrency**
   - Pessimistic: Lock before access, simple but can block
   - Optimistic: Check version/timestamp, retry on conflict
   - **Choice**: Immutable transitions (neither optimistic nor pessimistic needed)
   - **Reasoning**: 
     - Immutability eliminates need for concurrency control
     - No conflicts possible (each transition creates new state)
     - Simpler than optimistic locking
     - Better performance than pessimistic locking

3. **Engine Coordination vs Distributed Updates**
   - Distributed: Multiple writers, requires consensus
   - Engine coordination: Single coordinator, simpler model
   - **Choice**: Engine as single coordinator (to be implemented in Phase 4)
   - **Reasoning**: 
     - Simpler model (single source of truth)
     - Easier to reason about (linear history)
     - Supports deterministic replication
     - Clear ownership of state updates

4. **Version Tracking Strategy**
   - No version: Simple, but no conflict detection
   - Version number: Enables optimistic concurrency
   - Timestamp: Provides ordering, but clock skew issues
   - **Choice**: Timestamps for ordering (version numbers to be added in engine)
   - **Reasoning**: 
     - Timestamps provide natural ordering
     - Useful for debugging and observability
     - Version numbers can be added later for conflict resolution
     - Clock skew can be handled by engine layer

5. **State Update Model**
   - In-place mutation: Simple, but requires locks
   - Copy-on-write: Efficient for reads, expensive for writes
   - Immutable transitions: Simple, thread-safe, supports replication
   - **Choice**: Immutable transitions (return new state)
   - **Reasoning**: 
     - Thread-safe by default
     - Supports deterministic replication
     - No locks needed
     - Allocation cost acceptable for transition frequency

6. **Coordination Layer**
   - No coordination: Multiple writers, complex
   - Engine coordination: Single coordinator, simpler
   - **Choice**: Engine as single coordinator (Phase 4)
   - **Reasoning**: 
     - Single source of truth
     - Linear history (easier to reason about)
     - Supports log-based replication
     - Clear ownership of updates

## Choice Made

**Immutable State Transitions + Engine Coordination + Timestamp Tracking**

- All state transitions return new instances (immutable)
- Engine acts as single coordinator (to be implemented)
- Timestamps (`created_at`, `started_at`, `completed_at`) provide ordering
- No locks needed (immutability provides thread-safety)
- Deterministic transitions enable replication

## Purpose

Prevent race conditions and ensure consistency by enforcing single-writer semantics through immutable state transitions and engine coordination. Enable deterministic replication and recovery.

## Pros

- **No Locks**: Immutability eliminates need for synchronization primitives
- **Thread-safe**: Immutable structures are safe for concurrent reads
- **Deterministic**: Same transitions produce same results (supports replication)
- **Simple Model**: Single coordinator is easier to reason about
- **No Deadlocks**: No locks means no deadlock possibilities
- **Better Performance**: No lock contention, better cache locality
- **Replicable**: Deterministic transitions enable log-based replication

## Cons

- **Allocation Cost**: Each transition creates new instance (mitigated by efficient collections)
- **Engine Dependency**: Requires engine layer for coordination (not yet implemented)
- **No Concurrent Writes**: Single coordinator limits write throughput (acceptable trade-off)
- **Version Tracking**: Timestamps provide ordering but version numbers needed for conflict resolution (to be added)

## Implementation Details

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

### Timestamp Tracking
```rust
pub struct Workflow<State = WorkflowDraft> {
    // ...
    pub created_at: DateTime<Utc>,      // Set on creation
    pub started_at: Option<DateTime<Utc>>,  // Set on start
    pub completed_at: Option<DateTime<Utc>>, // Set on completion
    // ...
}
```

### Task Execution Timestamps
```rust
pub struct TaskExecution {
    // ...
    pub started_at: Option<DateTime<Utc>>,    // Set when task starts
    pub completed_at: Option<DateTime<Utc>>,   // Set when task completes
    // ...
}
```

### Engine Coordination (✅ Implemented via Raft)
The Raft consensus layer provides single-writer coordination:
- All state transitions go through Raft leader (`FlowRaftApp`)
- Raft validates and replicates transitions before applying
- Raft maintains linear history (log entries)
- Raft handles conflict resolution via consensus
- Raft supports replication via log replay on followers
- Implementation: `src/raft/app.rs`, `src/raft/storage/state_machine.rs`

### Concurrent Read Access
```rust
// Multiple readers can access state concurrently
let workflow1 = workflow.clone();  // Safe to clone (immutable)
let workflow2 = workflow.clone();  // Safe to clone (immutable)
// Both can be read concurrently without locks
```

### Single Writer Pattern
```rust
// Engine ensures only one writer at a time
// All transitions go through engine:
engine.apply_transition(transition)?;  // Single point of coordination
```

## Lessons Learned

1. **Immutability Eliminates Locks**: Using immutable state transitions eliminates the need for locks entirely. This makes the code simpler, safer, and more performant.

2. **Timestamp Ordering**: Timestamps provide natural ordering for state transitions, which is useful for debugging and observability. However, they don't provide conflict resolution (version numbers needed for that).

3. **Engine Coordination**: Having a single coordinator (engine) simplifies the model significantly. All state updates go through one place, making it easier to reason about and implement replication.

4. **Deterministic Transitions**: Immutable transitions are deterministic (same input produces same output), which is essential for replication. This enables log-based replication where transitions can be replayed.

5. **Allocation Trade-off**: The allocation cost of creating new state instances is acceptable given that transitions are infrequent compared to reads. The benefits (thread-safety, simplicity, replication) outweigh the cost.

6. **No Concurrent Writes**: The single-writer model limits write throughput, but this is an acceptable trade-off for correctness and simplicity. Most workflow systems have low write throughput compared to reads.

7. **Version Numbers Needed**: While timestamps provide ordering, version numbers are needed for conflict resolution in distributed scenarios. This will be added in the engine layer.

## What to Do Better Next

1. **Version Numbers**: Add explicit version numbers to state structures for conflict resolution and optimistic concurrency control.

2. **Engine Implementation**: Implement the engine layer to provide single-writer coordination and log-based replication.

3. **Conflict Resolution**: Implement conflict resolution strategies for distributed scenarios (e.g., last-write-wins, merge strategies).

4. **Write Batching**: Consider batching multiple transitions together for efficiency when the engine processes multiple updates.

5. **Read Replicas**: Implement read replicas to improve read throughput while maintaining single-writer semantics.

6. **Transition Logging**: Add transition logging to enable audit trails and debugging of state changes.

7. **Clock Synchronization**: Ensure clock synchronization for timestamp accuracy in distributed scenarios (e.g., NTP, logical clocks).

8. **State Compression**: Consider compressing state structures to reduce memory usage and improve cache locality.

9. **Lazy State Loading**: For very large workflows, consider lazy loading of state components that aren't immediately needed.

10. **State Archival**: Implement archival strategies for completed workflows to reduce memory usage while preserving history for recovery.

11. **Distributed Coordination**: Consider distributed coordination mechanisms (e.g., Raft, Paxos) for high-availability scenarios while maintaining single-writer semantics.

12. **Transition Validation**: Add validation at the engine level to ensure transitions are valid before applying (defense in depth).

