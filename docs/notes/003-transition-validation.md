# Phase 2 - Transition Validation

## Alternatives Considered

1. **Runtime-only vs Compile-time + Runtime**
   - Runtime-only: Simple, but errors only caught at runtime
   - Compile-time + Runtime: Catches errors early, prevents impossible states
   - **Choice**: Compile-time (phantom types) + runtime (business rules)
   - **Reasoning**: Best of both worlds - impossible transitions prevented at compile time, business rules validated at runtime

2. **Centralized vs Distributed Validation**
   - Centralized validator: Single place for all validation logic
   - Distributed (in transitions): Validation at transition point, clear ownership
   - **Choice**: Distributed validation in transition methods
   - **Reasoning**: 
     - Each transition method validates its own preconditions
     - Clear ownership of validation logic
     - Easier to understand and maintain
     - Type-driven design makes centralized validator unnecessary

3. **Validation Strategy**
   - Fail-fast: Return errors immediately
   - Best-effort: Try to fix issues automatically
   - **Choice**: Fail-fast with descriptive errors
   - **Reasoning**: Deterministic behavior, explicit error handling, no silent failures

4. **Error Message Handling**
   - Overwrite on state change: Simple, but loses context
   - Preserve on cancellation: Maintains context, more informative
   - **Choice**: Preserve existing error messages during cancellation, set new ones on failure
   - **Reasoning**: Cancellation is a terminal state that should preserve debugging context, while failures set new error information

5. **Error Return Types**
   - Panic on invalid: Simple, but crashes program
   - Result<T, E>: Explicit error handling, recoverable
   - **Choice**: `Result<T, E>` for recoverable errors (dependencies, retries)
   - **Reasoning**: Allows callers to handle errors gracefully, supports retry logic

## Choice Made

**Compile-time Type Safety + Runtime Business Rule Validation + Error Preservation**

- Phantom types prevent invalid state transitions at compile time
- Transition methods validate business rules (dependencies, retries, etc.) at runtime
- Return `Result` for recoverable errors, panic for impossible states (should never happen)
- Preserve existing error messages during cancellation operations
- Set new error messages on failure transitions

## Purpose

Ensure deterministic, correct state transitions while providing clear error messages for invalid operations. Maintain error context throughout the state machine lifecycle for better debugging and observability.

## Pros

- **Two-layer Safety**: Compile-time prevents impossible transitions, runtime validates business rules
- **Clear Errors**: Specific error types for each failure mode (`TaskError`, `WorkflowError`)
- **Deterministic**: Same input always produces same result
- **Maintainable**: Validation logic co-located with transitions
- **Type-safe**: Invalid operations are impossible at compile time
- **Error Context**: Preserved error messages provide better debugging information
- **Explicit Handling**: `Result` types force explicit error handling

## Cons

- **More Code**: Each transition method needs validation
- **Error Handling**: Must handle `Result` types (but this is good practice)
- **Complexity**: Two validation layers can be confusing initially
- **Error Message Logic**: Need to carefully decide when to preserve vs set error messages

## Implementation Details

### Compile-time Safety Example
```rust
// This is impossible - Task<TaskRunning> doesn't have a schedule() method
let task: Task<TaskRunning> = ...;
task.schedule(...); // Compile error!
```

### Runtime Validation Example
```rust
impl Task<TaskPending> {
    pub fn schedule(self, completed: &HashSet<TaskId>) -> Result<Task<TaskScheduled>, TaskError> {
        // Validate all prerequisites are completed
        if !self.dependencies.has_all_prerequisites_completed(completed) {
            return Err(TaskError::DependencyNotSatisfied { ... });
        }
        Ok(Task { ... })
    }
}
```

### Error Preservation Example
```rust
impl Task<TaskFailed> {
    pub fn cancel(self) -> Task<TaskCancelled> {
        Task {
            // ...
            // Preserve existing error message if present
            last_error: self.last_error.or_else(|| Some("Cancelled after failure".to_string())),
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

1. **Error Message Preservation**: Initially, cancellation would always set a new error message. Preserving existing errors provides better debugging context, especially when cancelling a failed task. The pattern `self.error.or_else(|| Some("default".to_string()))` works well.

2. **Distributed Validation**: Having validation in each transition method makes the code more maintainable. Each method is responsible for its own preconditions, making it easier to understand and modify.

3. **Error Type Design**: Having specific error types (`TaskError`, `WorkflowError`) with descriptive variants makes error handling clearer. The `thiserror` crate helps with this.

4. **Parallel Validation**: Some validation operations (like checking all prerequisites) benefit from parallel processing. Using rayon's `par_iter()` improves performance for large dependency sets.

5. **Immutable Validation**: Since transitions return new instances, validation can be done without side effects. This makes validation logic easier to test and reason about.

6. **Error Message Naming**: Standardizing on `error_message` (instead of `error`) avoids conflicts with error types and makes the intent clearer.

7. **Validation vs Business Logic**: The line between validation and business logic can be blurry. For example, checking if retries are exhausted is both validation and business logic. Keeping them together in transition methods works well.

## What to Do Better Next

1. **Validation Metrics**: Add metrics/logging for validation failures to understand common failure patterns in production.

2. **Validation Caching**: For frequently accessed workflows, consider caching validation results (e.g., DAG validity) until the workflow is modified.

3. **Structured Errors**: Instead of simple error messages, consider structured error types that include error codes, context, and suggested remediation.

4. **Validation Testing**: More comprehensive test coverage for edge cases in validation logic (e.g., very large dependency graphs, complex cycles).

5. **Error Recovery**: Consider adding error recovery strategies (e.g., automatic retry for transient dependency failures).

6. **Validation Performance**: Profile validation operations to identify bottlenecks, especially for large workflows with many dependencies.

7. **Error Message Formatting**: Consider a more sophisticated error message format that includes structured data (task IDs, dependency chains, etc.) for better debugging.

8. **Validation Documentation**: Better document which validations are performed at compile-time vs runtime, and what errors can occur at each transition.

9. **Error Context Chains**: Consider error context chains that preserve the full history of errors through state transitions, not just the most recent.

10. **Validation Macros**: Consider macros or derive attributes to automatically generate validation logic for common patterns (e.g., dependency checking).
