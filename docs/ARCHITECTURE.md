# FlowRaft Architecture

## Overview

FlowRaft is a distributed workflow engine built on Raft consensus, providing deterministic workflow execution with fault tolerance. The architecture is organized into three main layers:

1. **Core Layer**: Type-driven workflow engine with compile-time state enforcement
2. **Raft Layer**: Distributed consensus and state replication
3. **API Layer**: User-facing interfaces for workflow definition and execution

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                      API Layer                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │ Graph Builder│  │   gRPC API   │  │  Observability│     │
│  │ (Type-safe & │  │              │  │  (Metrics,    │     │
│  │  Dynamic)    │  │              │  │   History,    │     │
│  └──────────────┘  └──────────────┘  │   Watcher)   │     │
│                                       └──────────────┘     │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                      Raft Layer                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   App Layer  │  │   Executor   │  │   Storage    │     │
│  │ (FlowRaftApp)│  │(WorkflowExec)│  │(Log & State) │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
│  ┌──────────────┐  ┌──────────────┐                        │
│  │   Network    │  │     Node     │                        │
│  │  (Memory/    │  │  (Leader/    │                        │
│  │   Real RPC)  │  │  Follower)   │                        │
│  └──────────────┘  └──────────────┘                        │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                      Core Layer                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   Workflow   │  │     Task     │  │     DAG      │     │
│  │  (State      │  │  (State      │  │  (Deps,      │     │
│  │   Machine)   │  │   Machine)   │  │   Utils)     │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
│  ┌──────────────┐                                          │
│  │     Retry    │                                          │
│  │   (Config)   │                                          │
│  └──────────────┘                                          │
└─────────────────────────────────────────────────────────────┘
```

## Core Layer

### Workflow State Machine

Workflows are type-driven state machines with compile-time state enforcement:

- **States**: `Draft` → `Scheduled` → `Running` → `Completed`/`Failed`/`Cancelled`
- **Type Safety**: Each state is a distinct type parameter (`Workflow<State>`)
- **Transitions**: Methods only available on appropriate states
- **Validation**: DAG validation, dependency checking built into transitions

### Task State Machine

Tasks follow a similar pattern:

- **States**: `Pending` → `Scheduled` → `Running` → `Completed`/`Failed`/`PermanentlyFailed`/`Cancelled`
- **Dependencies**: Tasks can depend on other tasks completing
- **Retry**: Configurable retry policies with exponential backoff

### DAG Utilities

- **Validation**: Cycle detection, dependency validation
- **Ready Tasks**: Identify tasks ready for execution
- **Topological Sort**: Order tasks by dependencies

## Raft Layer

### State Replication

All workflow state is replicated via Raft consensus:

- **Log Store**: Persists Raft log entries (in-memory for MVP)
- **State Machine**: Applies log entries to workflow state
- **Consensus**: Leader-only writes, followers replicate

### Execution Model

- **Any Node Execution**: Any node can execute tasks (not just leader)
- **State Updates**: Task execution results written via Raft
- **Coordination**: Leader coordinates workflow state, workers execute tasks

### Network

- **Memory Network**: In-memory network for testing/single-node
- **Real Network**: Ready for real RPC implementation

## API Layer

### Graph Builder

Two approaches for workflow definition:

1. **Type-Safe Builder**: Compile-time workflow definition
2. **Dynamic Builder**: Runtime workflow definition

Both convert to the same internal `Workflow` structure.

### gRPC Service

- **Workflow Management**: Create, get, list workflows
- **Execution Control**: Start, pause, cancel workflows
- **Status Queries**: Get workflow status, task results

### Observability

- **Metrics**: Task execution metrics, workflow statistics
- **History**: Execution history with filtering
- **Watcher**: Real-time workflow updates via channels

## Data Flow

### Workflow Creation

1. User defines workflow via Graph Builder
2. Graph converted to `Workflow<Draft>`
3. Workflow scheduled: `Workflow<Scheduled>`
4. Workflow started: `Workflow<Running>`
5. Snapshot created: `WorkflowSnapshot`
6. Snapshot written to Raft: `Request::CreateWorkflow`
7. State machine applies: Workflow stored in replicated state

### Task Execution

1. Executor queries ready tasks: `get_ready_tasks()`
2. Handler executes task: `TaskHandler::execute()`
3. Result written to Raft: `Request::UpdateTaskExecution`
4. State machine applies: Task state updated
5. Workflow checks completion: All tasks done?
6. Workflow transitions: `Running` → `Completed`

## Key Design Decisions

### Type-Driven State Machines

- **Why**: Compile-time safety prevents invalid state transitions
- **Tradeoff**: More complex type signatures, but catches bugs at compile time

### Raft for State Replication

- **Why**: Strong consistency guarantees, well-understood algorithm
- **Tradeoff**: Higher latency than eventual consistency, but correctness is priority

### Any-Node Execution

- **Why**: Better resource utilization, horizontal scaling
- **Tradeoff**: More complex coordination, but enables distributed execution

### In-Memory Storage (MVP)

- **Why**: Simplicity, focus on correctness first
- **Future**: Persistent storage (RocksDB, etc.) for production

## Performance Characteristics

### Current (Baseline)

- **Latency**: ~240µs per workflow operation
- **Throughput**: ~1,000-2,000 workflows/second
- **Scalability**: Linear up to tested limits

### Target (After Optimization)

- **Latency**: <1ms (p99)
- **Throughput**: 100,000+ workflows/second
- **Scalability**: Linear up to 1M+ workflows

See `OPTIMIZATION_PLAN.md` for detailed optimization strategy.

## Testing Strategy

- **Unit Tests**: 192 tests covering core functionality
- **Integration Tests**: Single-node and multi-node cluster tests
- **Examples**: Real workflow execution demonstrations
- **Benchmarks**: Performance measurement and comparison

## Future Enhancements

- Persistent storage (RocksDB, etc.)
- Real network RPC (gRPC, HTTP)
- Failure injection testing
- Advanced retry strategies
- Backpressure and rate limiting
- Full observability integration
