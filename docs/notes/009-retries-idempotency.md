# Phase 7 - Retries & Idempotency

> **Status**: ⚠️ **Partially Implemented** (Phase 7 - Retries & Idempotency)  
> **Implementation**: `src/core/retry/config.rs` - RetryConfig implemented, idempotency pending  
> **See**: [ROADMAP.md](../ROADMAP.md) for implementation status

## Alternatives Considered

1. **Retry Strategy**
   - No retries: Simple, but fragile
   - Fixed retries: Simple, but inefficient
   - Exponential backoff: Complex, but efficient
   - **Choice**: Exponential backoff with configurable parameters
   - **Reasoning**: 
     - Balances retry frequency with resource usage
     - Reduces load on failing systems
     - Configurable for different use cases
     - Industry-standard approach

2. **Failure Classification**
   - Binary (retryable/not): Simple, but limited
   - Categorized failures: Complex, but flexible
   - **Choice**: Categorized failures (`FailureKind`: Retryable vs Terminal)
   - **Reasoning**: 
     - Terminal failures should not be retried
     - Retryable failures can be retried with backoff
     - Clear semantics for different failure types
     - Enables smart retry decisions

3. **Retry Configuration Location**
   - Global config: Simple, but inflexible
   - Per-task config: Complex, but flexible
   - **Choice**: Per-task retry configuration
   - **Reasoning**: 
     - Different tasks may need different retry strategies
     - Enables fine-grained control
     - Supports workflow-specific retry policies
     - More flexible than global config

4. **Idempotency Strategy**
   - No idempotency: Simple, but risky
   - Idempotency keys: Complex, but safe
   - **Choice**: Idempotency keys (structure exists, not fully integrated)
   - **Reasoning**: 
     - Prevents duplicate execution
     - Essential for reliable workflows
     - Structure defined, integration pending
     - Can be added incrementally

5. **Duplicate Detection**
   - No detection: Simple, but unsafe
   - Detection only: Partial solution
   - Detection + prevention: Complete solution
   - **Choice**: Detection exists, prevention pending
   - **Reasoning**: 
     - Detection is first step
     - Prevention requires idempotency integration
     - Incremental approach reduces risk
     - Can be completed in future phase

6. **Retry State Tracking**
   - No tracking: Simple, but no visibility
   - Basic tracking: Moderate complexity
   - Detailed tracking: Complex, but informative
   - **Choice**: Basic tracking (attempts, last failure kind)
   - **Reasoning**: 
     - Sufficient for retry decisions
     - Enables observability
     - Not too complex for MVP
     - Can be extended later

## Choice Made

**Exponential Backoff + Categorized Failures + Per-Task Config + Basic Tracking + Idempotency Structure**

- `RetryConfig` with exponential backoff support
- `FailureKind` enum (Retryable vs Terminal)
- Per-task retry configuration in workflow
- Attempt tracking and failure kind tracking
- Idempotency key structure defined (not fully integrated)
- Duplicate detection exists (prevention pending)

## Purpose

Enable reliable task execution through retries while preventing duplicate execution via idempotency. Support different retry strategies per task.

## Pros

- **Flexible Retry**: Configurable exponential backoff per task
- **Smart Decisions**: Failure kind classification prevents unnecessary retries
- **Per-Task Control**: Different retry strategies for different tasks
- **Observability**: Retry state tracking enables monitoring
- **Extensible**: Structure supports future idempotency integration
- **Safe Defaults**: Sensible defaults (3 attempts, 2x backoff, 1s initial delay)

## Cons

- **Incomplete Idempotency**: Idempotency keys not fully integrated
- **No Duplicate Prevention**: Detection exists but prevention pending
- **No Automatic Retry**: Retry logic not integrated into execution loop
- **Limited Tracking**: Basic tracking may not be sufficient for complex scenarios
- **No Retry Metrics**: No metrics for retry frequency or success rate

## Implementation Details

### Retry Configuration
```rust
pub struct RetryConfig {
    pub max_attempts: u8,
    pub current_attempt: u8,
    pub last_failure_kind: Option<FailureKind>,
    pub backoff_factor: f64,
    pub initial_delay_ms: u64,
}

impl RetryConfig {
    pub fn can_retry(&self) -> bool {
        if let Some(FailureKind::Terminal) = self.last_failure_kind {
            return false;
        }
        self.current_attempt < self.max_attempts
    }

    pub fn calculate_delay(&self) -> u64 {
        if self.current_attempt == 0 {
            return self.initial_delay_ms;
        }
        (self.initial_delay_ms as f64 * self.backoff_factor.powi(self.current_attempt as i32 - 1)) as u64
    }
}
```

### Failure Kind
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FailureKind {
    Retryable,
    Terminal,
}
```

### Retry State in Task Execution
```rust
pub struct TaskExecution {
    pub task_id: TaskId,
    pub state: TaskState,
    pub attempts: u32,  // Tracks retry attempts
    // ...
}
```

### Idempotency Key Structure (Defined, Not Integrated)
```rust
// Structure exists in workflow/task definitions
// Integration with execution pending
pub struct TaskDefinition {
    // ...
    // idempotency_key: Option<String>,  // Pending
}
```

## Lessons Learned

1. **Exponential Backoff**: Exponential backoff is essential for retry strategies. The configurable parameters allow fine-tuning for different scenarios.

2. **Failure Classification**: Categorizing failures as retryable vs terminal prevents wasted retries on permanent failures. This is crucial for efficiency.

3. **Per-Task Config**: Per-task retry configuration provides flexibility but requires careful management. The structure supports this well.

4. **Incremental Implementation**: Starting with retry config and adding idempotency later was the right approach. It reduces risk and allows validation.

5. **State Tracking**: Basic attempt and failure kind tracking is sufficient for MVP. More detailed tracking can be added as needed.

6. **Integration Pending**: Retry logic is defined but not fully integrated into execution loop. This is a known gap to be addressed.

7. **Idempotency Complexity**: Idempotency is more complex than initially anticipated. The structure is defined, but integration requires careful design.

8. **Default Values**: Sensible defaults (3 attempts, 2x backoff, 1s delay) work well for most cases. Users can customize as needed.

## What to Do Better Next

1. **Integrate Retry Logic**: Integrate retry logic into `HandlerExecutor::execute_workflow` for automatic retries.

2. **Complete Idempotency**: Fully integrate idempotency keys into task execution to prevent duplicates.

3. **Duplicate Prevention**: Implement duplicate prevention using idempotency keys in execution loop.

4. **Retry Metrics**: Add metrics for retry attempts, success rate, and backoff delays.

5. **Retry Policies**: Support more sophisticated retry policies (e.g., jitter, max delay, retry on specific errors).

6. **Idempotency Storage**: Implement idempotency key storage and lookup for duplicate detection.

7. **Retry Observability**: Add logging and metrics for retry attempts to improve observability.

8. **Retry Testing**: Add comprehensive tests for retry scenarios (exhaustion, terminal failures, backoff).

9. **Idempotency Testing**: Add tests for idempotency key handling and duplicate prevention.

10. **Retry Documentation**: Document retry behavior and best practices for users.

---

## Implementation Status

⚠️ **Partially Implemented** - Retry configuration is complete, but integration is pending:

- ✅ **RetryConfig**: Fully implemented with exponential backoff
- ✅ **FailureKind**: Categorized failures (Retryable vs Terminal)
- ✅ **Per-Task Config**: Retry configuration stored per task in workflow
- ✅ **State Tracking**: Attempt count and failure kind tracked in `TaskExecution`
- ✅ **Idempotency Structure**: Structure defined (not fully integrated)
- ⚠️ **Retry Integration**: Retry logic not integrated into execution loop
- ⚠️ **Idempotency Integration**: Idempotency keys not integrated into execution
- ⚠️ **Duplicate Prevention**: Detection exists, prevention pending

The retry configuration provides a solid foundation for reliable task execution. The exponential backoff and failure classification enable smart retry decisions. However, the retry logic needs to be integrated into the execution loop, and idempotency needs to be fully implemented to prevent duplicate execution. These are planned for future phases.
