# FlowRaft — Definition of Done

The purpose of this document is to lock the scope, make guarantees explicit, and remove ambiguity before any further implementation.

Below is the **contract** the system must uphold in accordance to the initial design documentation in `docs/DESIGN.md`.”

---

## 1. Explicit System Guarantees

FlowRaft provides the following guarantees **by design**:

### G1 — Deterministic Workflow State

Given the same workflow definition and inputs, FlowRaft will:

* produce the same sequence of state transitions
* reach the same terminal state
* regardless of node failures or restarts

Determinism applies to **state transitions**, not necessarily to task execution timing.

### G2 — Single Source of Truth

At any point in time:

* there is exactly one authoritative workflow state
* all state transitions are committed via Raft
* no local-only state is considered authoritative

If a state transition is not replicated, it is assumed lost.

### G3 — No Split-Brain Execution

FlowRaft guarantees that:

* only the Raft leader may propose workflow state transitions
* followers never independently advance workflow state

This prevents divergent workflow histories.

### G4 — Crash Recovery

FlowRaft guarantees that:

* node crashes do not corrupt workflow state
* workflows can resume after leader or worker failures
* recovery uses replicated state, not in-memory heuristics

Progress may pause during failure, but correctness is preserved.

### G5 — Explicit Retry Semantics

FlowRaft guarantees that:

* retries are observable and state-driven
* duplicate execution attempts are detectable
* side effects are not committed twice *within defined constraints*

FlowRaft does **not** claim exactly-once execution. It aims for **effectively-once behavior**.

---

## 2. Non-Goals

These are deliberate exclusions.

### NG1 — No Exactly-Once Illusion

FlowRaft does not promise:

* exactly-once execution across arbitrary side effects
* transparent retries without idempotency support

External systems must cooperate or provide compensating actions.

### NG2 — No Arbitrary Long-Running Jobs

FlowRaft does not attempt to:

* manage unbounded, multi-hour tasks
* checkpoint arbitrary user code

Tasks are expected to be bounded and restartable.

### NG3 — No Workflow DSL or UI

FlowRaft does not include:

* a high-level workflow DSL
* a UI for authoring or visualizing workflows

The focus is execution correctness, not developer ergonomics.

### NG4 — No Throughput Optimization

FlowRaft does not prioritize:

* peak throughput
* micro-optimizations
* benchmark-driven tuning

Performance work begins only after correctness is proven.

---

## 3. MVP Scope Definition

The MVP is considered complete when **all** of the following are true:

### S1 — Workflow Model

* DAG-based workflow definition
* Explicit task states (Pending, Ready, Running, Completed, Failed)
* Deterministic state transitions

### S2 — Replicated State Machine

* Workflow state stored in Raft
* All transitions applied via log entries
* Leader-only writes enforced

### S3 — Execution Separation

* Coordination logic separated from execution logic
* Task execution does not mutate replicated state directly

### S4 — Failure Recovery

* Leader crash during workflow execution does not corrupt state
* Worker crash mid-task triggers recovery or retry

### S5 — Observability (Minimal)

* Workflow and task metrics emitted
* State transitions logged
* Failures visible without debugging tools

## 4. Invariants (Must Never Be Violated)

These are runtime truths that must always hold:

* A workflow is in exactly one state at a time
* A task cannot be *Completed* without being *Started*
* A workflow cannot regress to an earlier state
* Followers never advance workflow state

Violating an invariant is a **bug**, not an edge case.

---

## 5. Guiding Principle Going Forward

> "Every new feature must either preserve or strengthen an existing guarantee. If it weakens a guarantee, it does not belong in the MVP."

This principle should guide all future implementation decisions.
