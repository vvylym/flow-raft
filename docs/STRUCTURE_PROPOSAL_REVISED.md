# Revised Core Module Structure Proposal

## Design Principles

1. **State-Based Transition Organization**: One file per source state (e.g., `from_pending.rs`) containing all transitions FROM that state
2. **Isolated Transitions**: Each transition method is clearly separated and independently testable
3. **Clear Module Names**: Self-documenting, easy-to-understand module structure
4. **Single Responsibility**: Each module/file has one clear purpose
5. **Testability**: Each transition can be tested in isolation with comprehensive coverage

## Proposed Structure

```
src/core/
├── mod.rs                    # Root module, re-exports
├── macros/                   # Macro definitions (unchanged)
│   ├── mod.rs
│   ├── id_types.rs
│   └── state_types.rs
├── retry/                    # Retry policy (unchanged)
│   ├── mod.rs
│   ├── config.rs
│   └── error.rs
├── dag/                      # NEW: Shared DAG operations
│   ├── mod.rs
│   ├── dependencies.rs       # TaskDependencies (moved from task/)
│   └── utils.rs             # DAG validation, cycle detection, topological sort
├── task/                     # Task type-driven state machine
│   ├── mod.rs               # Task struct, TaskDefinition, TaskExecution
│   ├── id.rs
│   ├── state.rs
│   ├── definition.rs        # TaskDefinition (split from mod.rs)
│   ├── execution.rs         # TaskExecution (split from mod.rs)
│   ├── error.rs
│   └── transitions/         # NEW: State-based transition modules
│       ├── mod.rs           # Re-exports all transition impls
│       ├── from_pending.rs   # schedule(), cancel() from Pending
│       ├── from_scheduled.rs # start(), cancel() from Scheduled
│       ├── from_running.rs   # complete(), fail(), cancel() from Running
│       └── from_failed.rs    # retry(), permanent_fail(), cancel() from Failed
└── workflow/                 # Workflow type-driven state machine
    ├── mod.rs               # Workflow struct
    ├── id.rs
    ├── state.rs
    ├── snapshot.rs
    ├── error.rs
    └── transitions/         # NEW: State-based transition modules
        ├── mod.rs           # Re-exports all transition impls
        ├── from_draft.rs    # add_task(), schedule() from Draft
        ├── from_scheduled.rs # start(), cancel() from Scheduled
        ├── from_running.rs   # complete(), fail(), pause(), cancel() from Running
        └── from_paused.rs    # resume(), cancel() from Paused
```

## Detailed Module Breakdown

### `dag/` Module (NEW)

**Purpose**: Shared DAG operations used by both task and workflow modules

**Files**:
- `dependencies.rs`: `TaskDependencies` struct with prerequisite/dependent tracking
- `utils.rs`: DAG validation (cycle detection), topological sort, ready task computation

**Benefits**:
- Reusable DAG operations
- Clear separation of concerns
- No circular dependencies

### `task/` Module

**Purpose**: Task type-driven state machine with isolated transitions

**Core Files**:
- `mod.rs`: `Task<State>` struct definition, re-exports
- `definition.rs`: `TaskDefinition` (immutable task metadata)
- `execution.rs`: `TaskExecution` (runtime execution state)
- `id.rs`, `state.rs`, `error.rs`: Supporting types

**Transition Files** (in `task/transitions/`):
- `from_pending.rs`: 
  - `Task<TaskPending>::new()` - Constructor
  - `Task<TaskPending>::schedule()` - Transition to Scheduled (validates dependencies)
  - `Task<TaskPending>::cancel()` - Transition to Cancelled
- `from_scheduled.rs`:
  - `Task<TaskScheduled>::start()` - Transition to Running
  - `Task<TaskScheduled>::cancel()` - Transition to Cancelled
- `from_running.rs`:
  - `Task<TaskRunning>::complete()` - Transition to Completed
  - `Task<TaskRunning>::fail()` - Transition to Failed
  - `Task<TaskRunning>::cancel()` - Transition to Cancelled
- `from_failed.rs`:
  - `Task<TaskFailed>::retry()` - Transition to Scheduled (validates retry config)
  - `Task<TaskFailed>::permanent_fail()` - Transition to PermanentlyFailed
  - `Task<TaskFailed>::cancel()` - Transition to Cancelled

**Benefits**:
- Each transition file is focused and testable
- Easy to find transitions from a specific state
- Clear test boundaries (one test file per transition file)

### `workflow/` Module

**Purpose**: Workflow type-driven state machine with isolated transitions

**Core Files**:
- `mod.rs`: `Workflow<State>` struct definition, re-exports
- `id.rs`, `state.rs`, `error.rs`, `snapshot.rs`: Supporting types

**Transition Files** (in `workflow/transitions/`):
- `from_draft.rs`:
  - `Workflow<WorkflowDraft>::new()` - Constructor
  - `Workflow<WorkflowDraft>::add_task()` - Add task to workflow (validates dependencies)
  - `Workflow<WorkflowDraft>::schedule()` - Transition to Scheduled (validates DAG)
- `from_scheduled.rs`:
  - `Workflow<WorkflowScheduled>::start()` - Transition to Running
  - `Workflow<WorkflowScheduled>::cancel()` - Transition to Cancelled
- `from_running.rs`:
  - `Workflow<WorkflowRunning>::get_ready_tasks()` - Query ready tasks
  - `Workflow<WorkflowRunning>::complete()` - Transition to Completed
  - `Workflow<WorkflowRunning>::fail()` - Transition to Failed
  - `Workflow<WorkflowRunning>::pause()` - Transition to Paused
  - `Workflow<WorkflowRunning>::cancel()` - Transition to Cancelled (cancels all tasks)
- `from_paused.rs`:
  - `Workflow<WorkflowPaused>::resume()` - Transition to Running
  - `Workflow<WorkflowPaused>::cancel()` - Transition to Cancelled

**Benefits**:
- Each transition file is focused and testable
- Easy to understand workflow lifecycle
- Clear test boundaries

## Transition File Structure Example

Each transition file follows this pattern:

```rust
//! Transitions from TaskPending state
//!
//! This module contains all state transitions that originate from the
//! TaskPending state. Each transition is clearly documented and independently
//! testable.

use crate::core::{Task, TaskPending, TaskScheduled, TaskCancelled, ...};

impl Task<TaskPending> {
    /// Transitions from Pending to Scheduled
    ///
    /// Validates that all prerequisites are completed before allowing the transition.
    ///
    /// # Arguments
    /// * `completed` - Set of completed task IDs
    ///
    /// # Errors
    /// Returns `TaskError::DependencyNotSatisfied` if prerequisites are not met.
    pub fn schedule(self, completed: &HashSet<TaskId>) -> Result<Task<TaskScheduled>, TaskError> {
        // Implementation with clear validation logic
    }

    /// Transitions from Pending to Cancelled
    ///
    /// Preserves existing error messages if present, otherwise sets a cancellation message.
    pub fn cancel(self) -> Task<TaskCancelled> {
        // Implementation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    // Comprehensive tests for schedule()
    #[rstest]
    // ... test cases
    fn test_schedule_success() { }

    #[rstest]
    // ... test cases
    fn test_schedule_dependency_not_satisfied() { }

    // Comprehensive tests for cancel()
    #[rstest]
    // ... test cases
    fn test_cancel_from_pending() { }
}
```

## Test Organization

### Test Structure
- Each transition file contains its own test module
- Tests are co-located with implementation for easy discovery
- Use `rstest` for parameterized testing
- Each transition method has comprehensive test coverage

### Test Coverage Goals
- **100% line coverage** for all transition methods
- **All error paths** tested
- **Edge cases** covered (empty sets, boundary conditions, etc.)
- **State preservation** verified (fields maintained through transitions)

### Example Test Organization
```
task/transitions/from_pending.rs
  ├── test_schedule_success()
  ├── test_schedule_no_dependencies()
  ├── test_schedule_dependencies_met()
  ├── test_schedule_dependency_not_satisfied()
  ├── test_schedule_multiple_prerequisites()
  ├── test_cancel_from_pending()
  └── test_cancel_preserves_error()
```

## Migration Plan

### Phase 1: Create DAG Module
1. Create `dag/` directory
2. Move `task/dependencies.rs` → `dag/dependencies.rs`
3. Move `workflow/utils.rs` → `dag/utils.rs`
4. Update imports in `task/` and `workflow/`

### Phase 2: Split Task Module
1. Create `task/definition.rs` - move `TaskDefinition` from `task/mod.rs`
2. Create `task/execution.rs` - move `TaskExecution` from `task/mod.rs`
3. Update imports

### Phase 3: Reorganize Task Transitions
1. Create `task/transitions/` directory
2. Split `task/transitions.rs` into:
   - `from_pending.rs` - `impl Task<TaskPending>`
   - `from_scheduled.rs` - `impl Task<TaskScheduled>`
   - `from_running.rs` - `impl Task<TaskRunning>`
   - `from_failed.rs` - `impl Task<TaskFailed>`
3. Create `task/transitions/mod.rs` with re-exports
4. Move tests to respective transition files
5. Update `task/mod.rs` to use `transitions::*`

### Phase 4: Reorganize Workflow Transitions
1. Create `workflow/transitions/` directory
2. Split `workflow/transitions.rs` into:
   - `from_draft.rs` - `impl Workflow<WorkflowDraft>`
   - `from_scheduled.rs` - `impl Workflow<WorkflowScheduled>`
   - `from_running.rs` - `impl Workflow<WorkflowRunning>`
   - `from_paused.rs` - `impl Workflow<WorkflowPaused>`
3. Create `workflow/transitions/mod.rs` with re-exports
4. Move tests to respective transition files
5. Update `workflow/mod.rs` to use `transitions::*`

### Phase 5: Test Coverage Enhancement
1. Review each transition file for test coverage
2. Add missing test cases
3. Ensure 100% line coverage
4. Document test strategy in each transition file

## Benefits Summary

### Maintainability
- ✅ Clear module boundaries
- ✅ Easy to locate specific transitions
- ✅ Single responsibility per file
- ✅ Self-documenting structure

### Testability
- ✅ Each transition is isolated and testable
- ✅ Tests co-located with implementation
- ✅ Clear test boundaries
- ✅ Easy to achieve 100% coverage

### Discoverability
- ✅ Intuitive file names (`from_pending.rs` clearly indicates source state)
- ✅ Easy to find all transitions from a state
- ✅ Clear module hierarchy

### Scalability
- ✅ Easy to add new transitions (add to appropriate file)
- ✅ Easy to add new states (create new `from_*.rs` file)
- ✅ No file size bloat (each file focused on one state)

## Comparison with Current Structure

| Aspect | Current | Proposed |
|--------|---------|----------|
| Transition Files | 2 large files (1165, 1146 lines) | 8 focused files (~100-300 lines each) |
| Test Organization | Inline in large files | Co-located with each transition |
| Test Discoverability | Hard to find specific tests | Easy - tests next to implementation |
| Module Clarity | Mixed concerns | Clear single responsibility |
| DAG Operations | Workflow-specific | Shared in `dag/` module |
| Dependencies | Workflow depends on task internals | Clear ownership boundaries |

## File Size Estimates

### Current
- `task/transitions.rs`: ~1165 lines
- `workflow/transitions.rs`: ~1146 lines

### Proposed
- `task/transitions/from_pending.rs`: ~200 lines (2 transitions + tests)
- `task/transitions/from_scheduled.rs`: ~150 lines (2 transitions + tests)
- `task/transitions/from_running.rs`: ~250 lines (3 transitions + tests)
- `task/transitions/from_failed.rs`: ~300 lines (3 transitions + tests)
- `workflow/transitions/from_draft.rs`: ~250 lines (3 transitions + tests)
- `workflow/transitions/from_scheduled.rs`: ~150 lines (2 transitions + tests)
- `workflow/transitions/from_running.rs`: ~400 lines (5 transitions + tests)
- `workflow/transitions/from_paused.rs`: ~150 lines (2 transitions + tests)

**Total**: ~1850 lines (similar to current, but better organized)

## Next Steps

1. Review and approve this structure
2. Execute migration plan phase by phase
3. Ensure tests pass after each phase
4. Verify 100% test coverage
5. Update documentation

