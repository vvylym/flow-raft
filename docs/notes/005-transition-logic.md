# Phase 3 - Transition Logic

## Alternatives Considered

1. **State Pattern vs Type-Driven Design**
   - State pattern: Runtime state machine, flexible but less type-safe
   - Type-driven: Compile-time state machine, type-safe but more complex types
   - **Choice**: Type-driven design with phantom types
   - **Reasoning**: 
     - Compile-time safety prevents entire classes of bugs
     - Invalid transitions are impossible at compile time
     - Zero runtime cost for state checking
     - Clear API (methods only available on appropriate states)

2. **Centralized vs Distributed Validation**
   - Centralized validator: Single place for all validation logic
   - Distributed (in transitions): Validation at transition point, clear ownership
   - **Choice**: Distributed validation in transition methods
   - **Reasoning**: 
     - Each transition method validates its own preconditions
     - Clear ownership of validation logic
     - Easier to understand and maintain
     - Type-driven design makes centralized validator unnecessary

3. **Transition Return Types**
   - Void transitions: Mutate in-place, simple but not thread-safe
   - Return new state: Immutable, thread-safe, supports replication
   - **Choice**: Return new state instance
   - **Reasoning**: 
     - Thread-safe by default
     - Supports deterministic replication
     - Easier to reason about (functional style)
     - No locks needed

4. **Error Handling Strategy**
   - Panic on invalid: Simple, but crashes program
   - Result<T, E>: Explicit error handling, recoverable
   - **Choice**: `Result<T, E>` for recoverable errors
   - **Reasoning**: 
     - Allows callers to handle errors gracefully
     - Supports retry logic
     - Explicit error handling is good practice
     - Type system enforces error handling

5. **Validation Timing**
   - Pre-validation: Validate before transition
   - Post-validation: Validate after transition
   - **Choice**: Pre-validation in transition methods
   - **Reasoning**: 
     - Fail fast (don't create invalid state)
     - Clear error messages (know what failed)
     - Atomic transitions (all-or-nothing)

6. **Transition Method Organization**
   - Single trait: All transitions in one place
   - Per-state impl blocks: Organized by state type
   - **Choice**: Per-state `impl` blocks
   - **Reasoning**: 
     - Clear organization (all transitions for a state together)
     - Type system enforces correct state
     - Easier to find relevant code
     - Matches type-driven design

## Choice Made

**Type-Driven Transitions + Distributed Validation + Immutable Returns**

- Phantom types enforce valid transitions at compile time
- Each state type has its own `impl` block with state-specific transitions
- Transition methods validate business rules (dependencies, retries, etc.)
- Return `Result<T, E>` for recoverable errors
- All transitions return new state instances (immutable)
- Validation happens before state creation (fail-fast)

## Purpose

Ensure all state transitions are valid and deterministic while providing clear error messages for invalid operations. Maintain type safety throughout the state machine lifecycle.

## Pros

- **Type Safety**: Invalid transitions are impossible at compile time
- **Clear API**: Methods only available on appropriate states
- **Deterministic**: Same input always produces same result
- **Maintainable**: Validation logic co-located with transitions
- **Thread-safe**: Immutable transitions enable concurrent access
- **Explicit Errors**: `Result` types force explicit error handling
- **Organized**: Per-state impl blocks make code easy to navigate

## Cons

- **More Methods**: Each state has its own transition methods
- **Complex Types**: Generic state types (`Task<TaskPending>`) are more complex
- **Error Handling**: Must handle `Result` types (but this is good practice)
- **Code Duplication**: Some field copying logic is repeated (but macros help)

## Implementation Details

### Task State Transitions

#### Pending → Scheduled
```rust
impl Task<TaskPending> {
    pub fn schedule(
        self,
        completed: &HashSet<TaskId>,
    ) -> Result<Task<TaskScheduled>, TaskError> {
        // Validate all prerequisites are completed
        if !self.dependencies.has_all_prerequisites_completed(completed) {
            return Err(TaskError::DependencyNotSatisfied { ... });
        }
        Ok(Task {
            // ... copy all fields
            state: TaskScheduled,
            // ...
        })
    }
}
```

#### Scheduled → Running
```rust
impl Task<TaskScheduled> {
    pub fn start(self) -> Task<TaskRunning> {
        Task {
            // ... copy all fields
            state: TaskRunning,
            started_at: Some(Utc::now()),
            // ...
        }
    }
}
```

#### Running → Completed/Failed
```rust
impl Task<TaskRunning> {
    pub fn complete(self, outputs: Option<serde_json::Value>) -> Task<TaskCompleted> {
        Task {
            // ... copy all fields
            state: TaskCompleted,
            completed_at: Some(Utc::now()),
            outputs_data: outputs,
            // ...
        }
    }

    pub fn fail(
        self,
        error_message: Option<String>,
        failure_kind: FailureKind,
    ) -> Task<TaskFailed> {
        Task {
            // ... copy all fields
            state: TaskFailed {
                error_message,
                failure_kind,
            },
            completed_at: Some(Utc::now()),
            last_error: error_message,
            retry_config: {
                let mut config = self.retry_config;
                config.last_failure_kind = Some(failure_kind);
                config
            },
            // ...
        }
    }
}
```

### Workflow State Transitions

#### Draft → Scheduled
```rust
impl Workflow<WorkflowDraft> {
    pub fn schedule(self) -> Result<Workflow<WorkflowScheduled>, WorkflowError> {
        // Validate DAG has no cycles
        validate_dag(...)?;
        // Validate workflow has tasks
        if self.task_definitions.is_empty() {
            return Err(WorkflowError::EmptyWorkflow);
        }
        Ok(Workflow {
            // ... copy all fields
            state: WorkflowScheduled,
            // ...
        })
    }
}
```

#### Running → Completed/Failed/Paused
```rust
impl Workflow<WorkflowRunning> {
    pub fn complete(self) -> Workflow<WorkflowCompleted> {
        // Collect outputs from completed tasks
        let outputs = self.collect_outputs();
        Workflow {
            // ... copy all fields
            state: WorkflowCompleted,
            completed_at: Some(Utc::now()),
            outputs: Some(outputs),
            // ...
        }
    }

    pub fn fail(self, error_message: String) -> Workflow<WorkflowFailed> {
        Workflow {
            // ... copy all fields
            state: WorkflowFailed {
                error_message: Some(error_message.clone()),
            },
            completed_at: Some(Utc::now()),
            error_message: Some(error_message),
            // ...
        }
    }

    pub fn pause(self) -> Workflow<WorkflowPaused> {
        Workflow {
            // ... copy all fields
            state: WorkflowPaused,
            // ...
        }
    }
}
```

### Validation Points

- **Task Scheduling**: Validates all prerequisites are completed
- **Task Retry**: Validates retry is possible (attempts < max, not terminal failure)
- **Workflow Scheduling**: Validates DAG has no cycles, has tasks
- **Workflow Start**: Validates workflow has tasks
- **Dependency Addition**: Validates all prerequisite tasks exist in workflow

## Lessons Learned

1. **Type-Driven Organization**: Organizing transitions by state type (`impl Task<TaskPending>`, `impl Task<TaskScheduled>`) makes the code much easier to navigate and understand. The type system enforces that transitions are only available on appropriate states.

2. **Immutable Transitions**: Returning new instances instead of mutating in-place makes the code thread-safe and easier to reason about. The allocation cost is acceptable given that transitions are infrequent.

3. **Distributed Validation**: Having validation in each transition method makes the code more maintainable. Each method is responsible for its own preconditions, making it easier to understand and modify.

4. **Error Type Design**: Having specific error types (`TaskError`, `WorkflowError`) with descriptive variants makes error handling clearer. The `thiserror` crate helps with this.

5. **Field Preservation**: When creating new state instances, it's important to carefully copy all fields to preserve state. Missing a field can lead to subtle bugs.

6. **Validation Order**: Validating before creating new state (fail-fast) is better than validating after. This prevents creating invalid state that needs to be rolled back.

7. **Parallel Validation**: Some validation operations (like checking all prerequisites) benefit from parallel processing. Using rayon's `par_iter()` improves performance for large dependency sets.

8. **State-Specific Logic**: Some transitions have state-specific logic (e.g., collecting outputs on completion). Keeping this logic in the transition method keeps it close to where it's used.

## What to Do Better Next

1. **Transition Macros**: Consider macros to reduce boilerplate in transition methods (field copying, timestamp setting, etc.).

2. **Transition Tracing**: Add optional transition tracing/logging to help debug state machine issues in production.

3. **Transition Metrics**: Add metrics for transition frequencies, validation failures, and transition durations.

4. **Transition Testing**: More comprehensive test coverage for edge cases in transition logic (e.g., concurrent transitions, invalid states).

5. **Transition Documentation**: Better document which transitions are available on which states, and what validations are performed.

6. **Transition Batching**: Consider batching multiple transitions together for efficiency (e.g., complete multiple tasks at once).

7. **Transition Rollback**: Consider adding rollback capability for failed transitions (though immutability makes this less necessary).

8. **Transition Hooks**: Consider adding hooks/callbacks for transitions to enable observability and side effects.

9. **Transition Validation Caching**: For frequently accessed workflows, consider caching validation results until the workflow is modified.

10. **Transition Performance**: Profile transition operations to identify bottlenecks, especially for large workflows with many tasks.

