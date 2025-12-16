# Roadmap

## Phase 1 — Scope & Guarantees

* [ ] Write explicit system guarantees (README & DESIGN.md)
* [ ] Define what "done" means for MVP
* [ ] Identify non-goals

---

## Phase 2 — Workflow Model

* [ ] Define DAG data structures
* [ ] Encode workflow and task states
* [ ] Validate deterministic transitions

---

## Phase 3 — State Machine

* [ ] Define replicated state structure
* [ ] Implement state transition logic
* [ ] Enforce single-writer semantics

---

## Phase 4 — Raft Integration

* [ ] Define Raft log entries
* [ ] Implement apply logic
* [ ] Handle leader-only writes

---

## Phase 5 — Execution Layer

* [ ] Separate coordination from execution
* [ ] Implement worker execution interface
* [ ] Track in-flight tasks

---

## Phase 6 — Failure Injection

* [ ] Simulate leader crashes
* [ ] Simulate worker crashes
* [ ] Verify recovery behavior

---

## Phase 7 — Retries & Idempotency

* [ ] Define retry policy
* [ ] Implement idempotency keys
* [ ] Handle duplicate execution

---

## Phase 8 — Backpressure & Limits

* [ ] Concurrency limits
* [ ] Rate limiting
* [ ] Resource exhaustion handling

---

## Phase 9 — Observability

* [ ] Structured logging
* [ ] Minimal metrics (task states)
* [ ] Trace workflow execution

---

## Phase 10 — Hardening & Narrative

* [ ] Clean README and DESIGN.md
* [ ] Add failure scenarios documentation
* [ ] Prepare interview walkthrough