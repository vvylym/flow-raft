# Roadmap

## Phase 1 — Scope & Guarantees

* [x] Write explicit system guarantees (README & DESIGN.md)
* [x] Define what "done" means for MVP
* [x] Identify non-goals

---

## Phase 2 — Workflow Model

* [x] Define DAG data structures
* [x] Encode workflow and task states
* [x] Validate deterministic transitions

---

## Phase 3 — State Machine

* [x] Define replicated state structure (`WorkflowSnapshot`)
* [x] Implement state transition logic (type-driven transitions)
* [x] Enforce single-writer semantics (via Raft leader)

---

## Phase 4 — Raft Integration

* [x] Define Raft log entries (`Request`/`Response` types)
* [x] Implement apply logic (`StateMachineStore`)
* [x] Handle leader-only writes (Raft consensus)
* [x] In-memory log storage (`LogStore`)
* [x] In-memory state machine (`StateMachineStore`)
* [x] Memory network for testing (`MemoryNetwork`)

---

## Phase 5 — Execution Layer

* [x] Separate coordination from execution (`WorkflowExecutor`)
* [x] Implement worker execution interface (`TaskHandler` trait)
* [x] Track in-flight tasks (via state machine)
* [x] Handler registry (`HandlerRegistry`)
* [x] Workflow execution loop (`HandlerExecutor::execute_workflow`)

---

## Phase 6 — Failure Injection

* [ ] Simulate leader crashes
* [ ] Simulate worker crashes
* [ ] Verify recovery behavior

---

## Phase 7 — Retries & Idempotency

* [x] Define retry policy (`RetryConfig`)
* [ ] Implement idempotency keys (structure exists, not fully integrated)
* [ ] Handle duplicate execution (detection exists, prevention pending)

---

## Phase 8 — Backpressure & Limits

* [ ] Concurrency limits
* [ ] Rate limiting
* [ ] Resource exhaustion handling

---

## Phase 9 — Observability

* [x] Structured logging (via `tracing`)
* [x] Minimal metrics (`MetricsCollector`)
* [x] Execution history (`ExecutionHistory`)
* [x] Real-time watcher (`WorkflowWatcher`)
* [ ] Trace workflow execution (partial)

---

## Phase 10 — Hardening & Narrative

* [x] Clean README and DESIGN.md
* [ ] Add failure scenarios documentation
* [ ] Prepare interview walkthrough

---

## Additional Implementations

### API Layer
* [x] Graph builder API (type-safe and dynamic)
* [x] gRPC service definition
* [x] Node launcher (leader/follower)
* [x] CLI interface

### Testing & Examples
* [x] Comprehensive unit tests (192 tests passing)
* [x] Integration tests (single-node and multi-node clusters)
* [x] Example workflows (simple, complex, distributed)
* [x] Benchmarks (workflow execution, temporal comparison)

### Performance
* [x] Initial benchmarks (~240µs latency, ~1-2K workflows/sec)
* [x] Optimization plan (target: 100K workflows/sec, <1ms latency)