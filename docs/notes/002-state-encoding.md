# Phase 2 - State Encoding

> **Status**: ✅ **Implemented** (Phase 2 - Workflow Model)  
> **Implementation**: `src/core/macros/state_types.rs` with `define_state_types!` macro  
> **See**: [ROADMAP.md](../ROADMAP.md) for implementation status

## Alternatives Considered

1. **Enum-only vs Phantom Types**
   - Enum-only: Simple, runtime checks, no compile-time safety
   - Phantom types: Compile-time safety, zero runtime cost, more complex types
   - **Choice**: Phantom types (marker types) + enum for serialization
   - **Reasoning**: Compile-time safety prevents entire classes of bugs (invalid transitions)

2. **Separate Marker Types vs Trait Objects**
   - Trait objects: Dynamic dispatch, runtime overhead
   - Marker types: Zero-cost, compile-time only
   - **Choice**: Marker types (unit structs or structs with fields) as phantom type parameters
   - **Reasoning**: Zero runtime cost, maximum type safety, supports states with associated data

3. **State Storage Strategy**
   - Mutable in-place: Simple, but requires careful synchronization
   - Immutable transitions: Thread-safe, easier to reason about, supports replication
   - **Choice**: Immutable transitions (return new state)
   - **Reasoning**: Thread-safe by default, supports deterministic replication

4. **Manual Code vs Macro Generation**
   - Manual: Full control, explicit, verbose
   - Macro: DRY principle, less boilerplate, more complex
   - **Choice**: Macro `define_state_types!` for code generation
   - **Reasoning**: Reduces boilerplate significantly, ensures consistency, easier to maintain

5. **Error Message Field Naming**
   - `error`: Short, but conflicts with potential error module
   - `error_message`: Explicit, clear, no conflicts
   - **Choice**: `error_message` consistently across all modules
   - **Reasoning**: Avoids naming conflicts, makes intent clear, consistent with state enum variants

6. **Error Message Preservation**
   - Overwrite on cancellation: Simple, loses context
   - Preserve on cancellation: Maintains context, more informative
   - **Choice**: Preserve existing error messages during cancellation
   - **Reasoning**: Cancellation doesn't erase previous error context, which is valuable for debugging

## Choice Made

**Phantom Types (Marker Types) + Enum for Serialization + Macro Generation**

- Marker types (`TaskPending`, `TaskScheduled`, `TaskFailed { error_message, failure_kind }`, etc.) for compile-time enforcement
- Enum (`TaskState`, `WorkflowState`) for serialization and runtime representation
- `From<&MarkerType>` implementations to convert from marker types to enum
- Macro `define_state_types!` generates marker types and `From` implementations
- Consistent `error_message` field naming across all state types
- Error message preservation during cancellation operations

## Purpose

Prevent invalid state transitions at compile time while maintaining serialization capability for persistence and replication. Ensure error context is preserved throughout the state machine lifecycle.

## Pros

- **Compile-time Safety**: Invalid transitions are impossible (e.g., can't call `start()` on `Task<TaskRunning>`)
- **Zero Runtime Cost**: Phantom types compile away completely
- **Type-safe API**: Method availability depends on state type
- **Serialization Support**: Enum variant for persistence
- **Clear Intent**: Type signature shows current state
- **DRY Principle**: Macro reduces boilerplate significantly
- **Consistent Naming**: `error_message` avoids conflicts and is explicit
- **Error Context Preservation**: Cancellation maintains previous error messages for debugging

## Cons

- **More Complex Types**: `Task<TaskPending>` vs `Task`
- **Requires Generics**: All state-specific code uses generics
- **Macro Complexity**: Macro implementation is non-trivial (handles variants with and without fields)
- **Learning Curve**: Developers need to understand phantom types and macro usage
- **Error Message Handling**: Need to be careful about when to preserve vs set error messages

## Implementation Details

### State Enum Definition
```rust
pub enum TaskState {
    Pending,
    Scheduled,
    Running,
    Completed,
    Failed {
        error_message: Option<String>,
        failure_kind: FailureKind,
    },
    PermanentlyFailed {
        error_message: Option<String>,
    },
    Cancelled,
}
```

### Macro Usage
```rust
crate::define_state_types! {
    TaskState {
        Pending => TaskPending,
        Scheduled => TaskScheduled,
        Failed { error_message: Option<String>, failure_kind: FailureKind } => TaskFailed,
        // ...
    }
}
```

### Error Message Preservation
```rust
// Cancellation preserves existing error messages
pub fn cancel(self) -> Task<TaskCancelled> {
    Task {
        // ...
        last_error: self.last_error.or_else(|| Some("Cancelled".to_string())),
        // ...
    }
}
```

### Marker Types with Fields
States with associated data (like `Failed`) have marker types that are structs with fields, not unit structs. The macro handles both cases automatically.

## Lessons Learned

1. **Macro Complexity**: The `define_state_types!` macro needed to handle both unit structs and structs with fields. This required a recursive macro with multiple match arms, which was more complex than initially anticipated.

2. **Error Message Consistency**: Initially used both `error` and `error_message` inconsistently. Standardizing on `error_message` throughout the codebase required careful refactoring but improved clarity.

3. **Error Preservation**: Initially, cancellation would overwrite error messages. Preserving existing errors provides better debugging context, but requires careful handling in all cancellation paths.

4. **State Enum vs Marker Types**: The enum is necessary for serialization, but the marker types provide compile-time safety. The `From` trait bridges these two representations elegantly.

5. **Field Naming in Macros**: When generating marker types with fields, the macro must handle field types correctly (e.g., `Option<String>`, `FailureKind`). This required careful macro design.

6. **Immutable Transitions**: Returning new instances instead of mutating in-place makes the code thread-safe and easier to reason about, but requires careful field copying to preserve all state.

## What to Do Better Next

1. **Macro Documentation**: The macro is powerful but complex. Better inline documentation and examples would help developers understand how to use it for new state types.

2. **Error Message Strategy**: Consider a more sophisticated error message strategy (e.g., error chains, structured errors) rather than simple strings. This could improve debugging capabilities.

3. **State Transition Tracing**: Consider adding optional transition tracing/logging to help debug state machine issues in production.

4. **Validation at Macro Expansion**: Could the macro validate state definitions at compile time? For example, ensure all states are reachable or validate state transition graphs.

5. **Derive Macros**: Consider using derive macros instead of declarative macros for better IDE support and error messages.

6. **State Machine Visualization**: Generate state transition diagrams from the type definitions to help with documentation and understanding.

7. **Error Message Types**: Instead of `Option<String>`, consider a more structured error type that can carry additional context (error codes, timestamps, etc.).

---

## Implementation Status

✅ **Fully Implemented** - All design decisions were implemented as described:

- **Macro System**: `define_state_types!` macro implemented in `src/core/macros/state_types.rs`
- **Marker Types**: All state marker types generated (TaskPending, TaskScheduled, etc.)
- **State Enums**: `TaskState` and `WorkflowState` enums for serialization
- **From Implementations**: Automatic conversion from marker types to enums
- **Error Messages**: Consistent `error_message` field naming throughout
- **Type Safety**: Phantom types enforce compile-time state transitions

The implementation provides full compile-time safety while maintaining serialization capability. All 192 tests pass, validating the design.
