# Design Documentation for FlowRaft

## Problem

In a distributed workflow engine, failures are inevitable:

* workers crash mid-task
* network calls time out
* leaders change during execution

Naively retrying tasks risks:

* duplicate side effects
* corrupted external state
* inconsistent workflow outcomes

---

## Design Goals

* Preserve deterministic workflow state
* Avoid duplicate *effects*, not just duplicate execution
* Keep retry logic explicit and observable

---

## Chosen Approach

1. **State-Driven Retries**

   Retries are initiated based on replicated workflow state, not local execution outcomes.

2. **Idempotency Keys**

   Each task execution is associated with a deterministic idempotency key derived from:

   * workflow ID
   * task ID
   * execution attempt

3. **Effect Acknowledgment**

   A task is only marked complete once its effects are acknowledged and committed via consensus.

---

## What This Solves

* Duplicate execution does not imply duplicate effects
* Leader changes do not corrupt workflow progress
* State recovery remains deterministic

---

## Known Limitations

* External systems must support idempotency or compensating actions
* Some side effects cannot be safely retried
* Increased coordination overhead for strong guarantees

---

## Why This Tradeoff Is Acceptable

FlowRaft prioritizes correctness and recoverability over raw throughput.

In systems where side effects matter, *exactly-once semantics are an illusion*. FlowRaft instead aims for **effectively-once behavior** under clearly defined constraints.

---

## Implementation Status

FlowRaft has been implemented with:

* ✅ State-driven retries via `RetryConfig`
* ✅ Idempotency key structure (workflow ID + task ID + attempt)
* ✅ Effect acknowledgment via Raft consensus
* ✅ Distributed execution with Raft replication
* ✅ Type-driven state machines for compile-time safety

## Future Improvements

* Pluggable retry strategies
* Compensation workflows
* Stronger execution fencing
* Persistent storage (currently in-memory)
* Real network RPC (currently memory-based for testing)

---

## Closing Note

This design favors explicit guarantees and visible failure modes over optimistic execution. The complexity is intentional and localized.

For current implementation details, see [ARCHITECTURE.md](ARCHITECTURE.md) and [API_GUIDE.md](API_GUIDE.md).