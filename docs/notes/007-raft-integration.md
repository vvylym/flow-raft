# Phase 4 - Raft Integration

> **Status**: ✅ **Fully Implemented** (Phase 4 - Raft Integration)  
> **Implementation**: `src/raft/` module with OpenRaft integration  
> **See**: [ROADMAP.md](../ROADMAP.md) for implementation status

## Alternatives Considered

1. **Raft Library Choice**
   - Custom Raft implementation: Full control, but complex and error-prone
   - OpenRaft: Mature, well-tested, production-ready
   - **Choice**: OpenRaft (version 0.9.21 from crates.io)
   - **Reasoning**: 
     - Battle-tested implementation
     - Active maintenance and community
     - Comprehensive test coverage
     - Reduces implementation risk significantly

2. **Storage Strategy**
   - Persistent storage: Disk-based, survives restarts
   - In-memory storage: Fast, simple, good for testing
   - **Choice**: In-memory storage for MVP (`LogStore`, `StateMachineStore`)
   - **Reasoning**: 
     - Simpler implementation for MVP
     - Sufficient for testing and development
     - Can be extended to persistent storage later
     - Clear separation of concerns

3. **Network Implementation**
   - Real network (gRPC/TCP): Production-ready, complex
   - Memory network: Simple, fast, good for testing
   - **Choice**: Memory network (`MemoryNetwork`) for MVP
   - **Reasoning**: 
     - Enables fast testing and development
     - Simplifies initial implementation
     - Can be extended to real network later
     - Clear interface allows swapping implementations

4. **Request/Response Design**
   - Single request type: Simple, but less type-safe
   - Enum-based requests: Type-safe, extensible
   - **Choice**: Enum-based `Request` and `Response` types
   - **Reasoning**: 
     - Type-safe operation dispatch
     - Easy to extend with new operations
     - Clear operation semantics
     - Pattern matching for operation handling

5. **State Machine Design**
   - Mutable state machine: Simple, but requires careful synchronization
   - Immutable state machine: Thread-safe, easier to reason about
   - **Choice**: Immutable state machine with async mutex
   - **Reasoning**: 
     - Thread-safe by default
     - Supports concurrent reads
     - Clear ownership model
     - Async-friendly design

6. **Command Builder Pattern**
   - Direct request construction: Simple, but verbose
   - Builder pattern: Cleaner API, less error-prone
   - **Choice**: `WorkflowCommandBuilder` for request construction
   - **Reasoning**: 
     - Cleaner API surface
     - Reduces construction errors
     - Easier to extend
     - Better documentation through method names

## Choice Made

**OpenRaft + In-Memory Storage + Memory Network + Enum-Based Requests + Builder Pattern**

- OpenRaft 0.9.21 for Raft consensus
- In-memory `LogStore` and `StateMachineStore` for MVP
- `MemoryNetwork` for testing and development
- Enum-based `Request`/`Response` types for type-safe operations
- `WorkflowCommandBuilder` for clean request construction
- `FlowRaftApp` as high-level API over Raft

## Purpose

Provide distributed consensus for workflow state replication across nodes, ensuring consistency and fault tolerance. Enable leader-based coordination for single-writer semantics.

## Pros

- **Production-Ready Raft**: OpenRaft is battle-tested and well-maintained
- **Type Safety**: Enum-based requests prevent invalid operations
- **Simple MVP**: In-memory storage simplifies initial implementation
- **Testable**: Memory network enables fast, deterministic tests
- **Extensible**: Clear interfaces allow swapping implementations
- **Clean API**: Builder pattern provides ergonomic request construction
- **Leader-Based**: Natural single-writer semantics via Raft leader

## Cons

- **In-Memory Only**: State lost on restart (acceptable for MVP)
- **Memory Network**: Not suitable for real distributed deployment
- **No Persistence**: Cannot recover from crashes (to be addressed)
- **Limited Scalability**: In-memory storage limits workflow count
- **No Snapshot Compression**: Full state snapshots (can be optimized)

## Implementation Details

### Raft Type Configuration
```rust
pub struct TypeConfig;
impl RaftTypeConfig for TypeConfig {
    type D = Request;
    type R = Response;
    type NodeId = NodeId;
    type Node = ();
    type Entry = openraft::Entry<TypeConfig>;
    type SnapshotData = Cursor<Vec<u8>>;
    type AsyncRuntime = openraft::TokioRuntime;
}
```

### Request Types
```rust
pub enum Request {
    CreateWorkflow { workflow: WorkflowSnapshot },
    TransitionWorkflow { workflow_id: WorkflowId, workflow: WorkflowSnapshot },
    UpdateTaskExecution { workflow_id: WorkflowId, task_id: TaskId, execution: TaskExecution },
    CancelWorkflow { workflow_id: WorkflowId, workflow: WorkflowSnapshot },
}
```

### State Machine Store
```rust
pub struct StateMachineStore<C: RaftTypeConfig> {
    inner: Arc<Mutex<StateMachineStoreInner<C>>>,
}

struct StateMachineStoreInner<C: RaftTypeConfig> {
    last_applied_log: Option<LogId<C::NodeId>>,
    last_membership: StoredMembership<C::NodeId, C::Node>,
    state_machine: StateMachineData,
    snapshot_idx: AtomicU64,
    current_snapshot: Option<StoredSnapshot<C>>,
}
```

### FlowRaft Application Layer
```rust
pub struct FlowRaftApp {
    raft: Arc<Raft<TypeConfig>>,
    state_machine: StateMachineStore<TypeConfig>,
}

impl FlowRaftApp {
    pub async fn create_workflow(&self, request: Request) -> Result<Response, RaftError> {
        let result = self.raft.client_write(request).await?;
        Ok(result.data)
    }
}
```

### Memory Network
```rust
pub struct MemoryNetwork {
    nodes: Arc<RwLock<HashMap<NodeId, RaftNetwork>>>,
}

impl RaftNetwork for MemoryNetwork {
    async fn send_append_entries(&self, rpc: AppendEntriesRequest) -> Result<AppendEntriesResponse, RaftError> {
        // Route to target node via in-memory channel
    }
}
```

## Lessons Learned

1. **OpenRaft Integration**: OpenRaft's trait-based design made integration straightforward. The `RaftTypeConfig` trait provides clear extension points.

2. **In-Memory Storage**: Starting with in-memory storage was the right choice. It allowed rapid iteration and testing without I/O complexity.

3. **Request Enum Design**: Using an enum for requests made operation dispatch type-safe and extensible. Pattern matching in the state machine is clean and clear.

4. **Builder Pattern**: The `WorkflowCommandBuilder` significantly improved API ergonomics. It's easier to construct requests and reduces errors.

5. **State Machine Mutex**: Using `Arc<Mutex<>>` for state machine storage provides thread-safety while maintaining simple ownership. Async mutex prevents blocking.

6. **Memory Network**: The memory network implementation was simpler than expected. It provides deterministic testing and fast iteration.

7. **Snapshot Serialization**: Using `Cursor<Vec<u8>>` for snapshot data allows efficient serialization. Bincode provides fast binary serialization.

8. **Leader-Only Writes**: Raft's leader-based model naturally provides single-writer semantics. No additional coordination needed.

## What to Do Better Next

1. **Persistent Storage**: Implement disk-based `LogStore` and `StateMachineStore` for production use.

2. **Real Network**: Replace memory network with gRPC-based network implementation for distributed deployment.

3. **Snapshot Compression**: Add compression to snapshots to reduce memory usage and network traffic.

4. **Snapshot Frequency**: Implement configurable snapshot frequency to balance memory usage and recovery time.

5. **Network Timeouts**: Add configurable timeouts for network operations to handle network partitions gracefully.

6. **Connection Pooling**: Implement connection pooling for real network to improve performance.

7. **Metrics Integration**: Add Raft-specific metrics (e.g., log size, replication lag, election frequency).

8. **Leader Election Monitoring**: Add monitoring for leader elections to detect instability.

9. **State Machine Validation**: Add validation in state machine apply logic to catch invalid transitions early.

10. **Batch Operations**: Support batching multiple operations in a single Raft entry for improved throughput.

---

## Implementation Status

✅ **Fully Implemented** - All design decisions were implemented as described:

- **OpenRaft Integration**: OpenRaft 0.9.21 integrated via `TypeConfig` trait
- **Request/Response Types**: Enum-based `Request` and `Response` types defined
- **State Machine Store**: `StateMachineStore` implements `RaftStateMachine` trait
- **Log Store**: `LogStore` implements `RaftLogStorage` trait (in-memory)
- **Memory Network**: `MemoryNetwork` implements `RaftNetwork` trait
- **FlowRaftApp**: High-level API for workflow operations via Raft
- **Command Builder**: `WorkflowCommandBuilder` for clean request construction
- **Node Setup**: `FlowRaftNode` for node initialization and cluster setup

The Raft integration provides distributed consensus for workflow state replication. All state transitions go through Raft consensus, ensuring consistency across the cluster. The implementation supports single-node and multi-node clusters, with leader-based coordination enforcing single-writer semantics.
