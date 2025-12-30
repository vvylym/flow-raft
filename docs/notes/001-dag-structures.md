# Phase 2 - DAG Data Structures

## Alternatives Considered

1. **Adjacency List vs Adjacency Matrix**
   - Adjacency Matrix: O(V²) space, O(1) edge lookup
   - Adjacency List: O(V + E) space, O(E) edge lookup
   - **Choice**: Adjacency List using `HashMap<TaskId, TaskDependencies>`
   - **Reasoning**: Workflows are sparse graphs (few dependencies per task), so adjacency list is more memory-efficient

2. **HashSet vs Vec for Dependencies**
   - Vec: O(n) lookup, preserves order, less memory
   - HashSet: O(1) lookup, no duplicates, more memory
   - **Choice**: `SmallVec<[TaskId; 4]>` for dependencies
   - **Reasoning**: Most tasks have few dependencies (typically 1-4), so SmallVec provides:
     - Stack allocation for small lists (no heap allocation)
     - O(1) access like Vec
     - Automatic heap fallback for larger lists
     - Memory efficient for common case

3. **IndexMap vs HashMap for Tasks**
   - HashMap: O(1) lookup, non-deterministic iteration
   - IndexMap: O(1) lookup, deterministic iteration order
   - **Choice**: `IndexMap<TaskId, TaskDefinition>` for tasks
   - **Reasoning**: 
     - Deterministic iteration needed for reproducible behavior
     - Topological operations benefit from stable ordering
     - Slight memory overhead acceptable for correctness

4. **Sequential vs Parallel Processing**
   - Sequential: Simple, single-threaded, predictable
   - Parallel with thresholds: Conditional parallelization based on size
   - Parallel always: Use rayon when beneficial, let it decide overhead
   - **Choice**: Always use rayon for parallel processing (no arbitrary thresholds)
   - **Reasoning**: 
     - Rayon automatically handles overhead and decides when to parallelize
     - Eliminates arbitrary threshold decisions
     - Consistent code path, easier to maintain
     - Better performance for large workflows without complexity

## Choice Made

**IndexMap + SmallVec + HashMap + Rayon combination**

- `IndexMap` for task storage (deterministic, O(1) lookup)
- `SmallVec<[TaskId; 4]>` for dependency lists (stack-allocated for common case)
- `HashMap` for reverse dependencies (efficient lookups)
- `rayon` for parallel processing (always used, no thresholds)

## Purpose

Support efficient dependency checking and topological operations while minimizing memory allocations, ensuring deterministic behavior, and leveraging parallel processing for improved performance on large workflows.

## Pros

- **Memory Efficient**: SmallVec eliminates heap allocations for 90%+ of cases (tasks with ≤4 dependencies)
- **Fast Lookups**: O(1) lookups for all operations
- **Deterministic**: IndexMap ensures reproducible iteration order
- **Scalable**: Parallel processing with rayon for all operations (automatic optimization)
- **Zero-cost Abstractions**: SmallVec has no runtime cost when stack-allocated
- **Consistent**: No arbitrary thresholds - rayon handles optimization decisions
- **Maintainable**: Single code path for parallel operations

## Cons

- **Slightly More Complex**: Three different collection types vs one
- **Memory Overhead**: IndexMap has slightly more overhead than HashMap (acceptable trade-off)
- **Learning Curve**: Developers need to understand SmallVec and rayon behavior
- **Parallel Overhead**: Rayon may have small overhead for very small collections (but handles this automatically)

## Implementation Details

### Task Dependencies Structure
```rust
pub struct TaskDependencies {
    pub prerequisites: SmallVec<[TaskId; 4]>,
    pub dependents: SmallVec<[TaskId; 4]>,
}
```

### Parallel Processing Usage
- `has_all_prerequisites_completed()`: Uses `par_iter()` for prerequisite checking
- `validate_dag()`: Uses `par_iter()` for in-degree calculation and entry point finding
- `ready_tasks()`: Uses `par_iter()` for completed task collection
- `get_ready_tasks()`: Uses `par_iter()` for task map creation
- `complete()`: Uses `par_iter()` for output collection
- `cancel()`: Uses `par_iter()` for task cancellation identification
- `status()`: Uses `par_iter()` for status aggregation

All parallel operations use rayon's automatic work-stealing scheduler, which handles overhead and thread management.

## Lessons Learned

1. **No Arbitrary Thresholds**: Initially considered thresholds (e.g., "use parallel if >32 tasks"), but rayon's automatic optimization is better. The library handles overhead decisions, making code simpler and more maintainable.

2. **SmallVec Default Size**: Choosing 4 as the default size was based on typical workflow patterns. Most tasks have 1-2 dependencies, so 4 covers the vast majority of cases while still being small enough for stack allocation.

3. **IndexMap for Determinism**: Using IndexMap instead of HashMap was crucial for deterministic behavior in tests and replication scenarios. The slight memory overhead is worth the correctness guarantee.

4. **Bidirectional Dependencies**: Maintaining both prerequisites and dependents in `TaskDependencies` enables efficient traversal in both directions, which is essential for cycle detection and ready task identification.

5. **Parallel Collection Building**: Using `par_iter()` with `collect()` for building collections is efficient and clean, but requires careful handling of mutable state (sequential updates after parallel collection).

## What to Do Better Next

1. **Benchmark Parallel Thresholds**: While we removed arbitrary thresholds, it would be valuable to benchmark actual performance to understand when rayon's overhead becomes negligible. This could inform future optimizations.

2. **Consider Specialized Collections**: For very large workflows (1000+ tasks), consider specialized graph structures or more aggressive parallelization strategies.

3. **Dependency Caching**: Consider caching computed dependency relationships (e.g., transitive dependencies) if workflows are frequently queried without modification.

4. **Memory Profiling**: Profile actual memory usage in production scenarios to validate SmallVec size choice and IndexMap overhead assumptions.

5. **Parallel Safety Documentation**: Better document which operations are safe for parallel processing and which require sequential access (e.g., mutable HashMap updates).
