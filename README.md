# FlowRaft

**A distributed, stateful workflow engine written in Rust**, designed for correctness, fault tolerance, and predictable execution under failure.

---

## Why FlowRaft Exists

Most workflow engines are optimized for *feature velocity* or *developer ergonomics*. FlowRaft is intentionally optimized for a different set of constraints:

* **Deterministic execution** in the presence of failures
* **Explicit state ownership** and recovery semantics
* **Clear consistency guarantees** across nodes
* **Predictable behavior under concurrency**

FlowRaft is an exploration of how far these guarantees can be pushed in a modern Rust-based distributed system without relying on external orchestration platforms. It is a deliberate systems design exercise intended to mirror real-world tradeoffs encountered in production distributed backends.

> “The system guarantees and non-goals are explicitly defined in `docs/SCOPE.md`.”

---

## Problem Statement

In distributed environments, executing multi-step workflows reliably is hard:

* Nodes fail mid-execution
* Network partitions introduce ambiguity
* Retrying tasks can cause duplication or corruption
* Parallel execution complicates state tracking

Most systems address this by either:

* pushing complexity into operators, or
* accepting weak consistency guarantees

FlowRaft instead treats workflows as **first-class, replicated state machines**.

---

## Core Design Principles

### 1. Workflows as Stateful DAGs

Workflows are modeled as **directed acyclic graphs (DAGs)** where each node represents a task and edges encode execution dependencies.

* Supports **sequential**, **parallel**, and **conditional** execution paths
* Execution state is explicit and persisted
* Task transitions are deterministic given the same inputs

This model enables recovery and replay without relying on best-effort heuristics.

---

### 2. Replicated State via Raft

FlowRaft uses the **Raft consensus algorithm** to replicate workflow state across nodes.

Why Raft:

* Simpler mental model than Paxos
* Strong leader-based consistency
* Well-understood failure modes

All workflow state transitions are:

* proposed to the leader
* replicated via the Raft log
* applied deterministically on all followers

This ensures:

* a single source of truth
* no split-brain execution
* consistent recovery after node failures

---

### 3. Separation of Coordination and Execution

FlowRaft explicitly separates:

* **coordination** (consensus, state replication)
* **execution** (running user-defined tasks)

Only *state transitions* go through consensus. Task execution itself may happen locally and in parallel, but its **effects are only committed once agreed upon**.

This avoids:

* unnecessary consensus overhead
* tightly coupling compute with coordination

---

### 4. Controlled Concurrency in Rust

Concurrency is handled deliberately, not opportunistically:

* **Tokio** for async I/O and task scheduling
* **Rayon** for CPU-bound parallelism
* Clear boundaries between async and parallel sections

Rust’s ownership model is used to:

* enforce correct state access
* avoid shared mutable state across tasks
* surface race conditions at compile time

The goal is predictable throughput, not maximal parallelism.

---

## Failure Handling & Recovery

Failure is treated as a *normal operating condition*.

FlowRaft is designed to handle:

* **Leader failure:** new leader elected via Raft, execution resumes from replicated state
* **Worker failure:** incomplete tasks are detected and rescheduled
* **Process restarts:** workflow state is reconstructed from the Raft log

There is no reliance on in-memory-only progress tracking. If state is not replicated, it is assumed lost.

---

## Execution Model

1. Workflow is submitted to the cluster
2. Leader validates and persists initial state
3. Eligible tasks are scheduled for execution
4. Task completion proposes a state transition
5. Transition is replicated and committed
6. Downstream tasks become eligible

At any point, cluster membership or leadership may change without corrupting workflow state.

---

## What FlowRaft Is *Not*

* Not a general-purpose orchestration replacement
* Not optimized for arbitrary long-running jobs
* Not focused on UI or DSL expressiveness

The focus is correctness, observability, and systems clarity.

---

## Current Status

FlowRaft is under active development.

Implemented:

* Workflow DAG model
* Raft-based state replication
* Basic execution scheduling
* Deterministic state transitions

In progress:

* Persistence backends
* Backpressure and rate limiting
* Execution retries and idempotency strategies
* Observability (metrics, tracing)

Unimplemented aspects are explicit by design — this project favors correctness-first evolution over feature completeness.

---

## Why This Project Matters

FlowRaft exists to demonstrate:

* end-to-end distributed systems design
* disciplined use of Rust for concurrency and safety
* clear reasoning about failure modes and tradeoffs

It reflects how I approach production systems: define guarantees first, then build mechanisms that uphold them.

---

## License

MIT
