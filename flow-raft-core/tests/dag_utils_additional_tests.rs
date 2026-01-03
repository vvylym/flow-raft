//! Additional tests for DAG utils to increase coverage

use flow_raft_core::{TaskDependencies, TaskId, ready_tasks};
use indexmap::IndexMap;
use std::collections::HashSet;

#[test]
fn test_ready_tasks_with_all_completed() {
    let task1 = TaskId::default();
    let task2 = TaskId::default();

    let mut tasks = IndexMap::new();
    tasks.insert(task1, ());
    tasks.insert(task2, ());

    let mut dependencies = IndexMap::new();
    dependencies.insert(task2, TaskDependencies::with_prerequisites(vec![task1]));

    let mut completed = HashSet::new();
    completed.insert(task1);
    completed.insert(task2);

    let ready = ready_tasks(&tasks, &dependencies, &completed);
    // All tasks completed, so none should be ready
    assert!(ready.is_empty());
}

#[test]
fn test_ready_tasks_partial_completion() {
    let task1 = TaskId::default();
    let task2 = TaskId::default();
    let task3 = TaskId::default();

    let mut tasks = IndexMap::new();
    tasks.insert(task1, ());
    tasks.insert(task2, ());
    tasks.insert(task3, ());

    let mut dependencies = IndexMap::new();
    dependencies.insert(task2, TaskDependencies::with_prerequisites(vec![task1]));
    dependencies.insert(task3, TaskDependencies::with_prerequisites(vec![task2]));

    let mut completed = HashSet::new();
    completed.insert(task1);
    // task1 completed, so task2 should be ready
    // task3 should not be ready (task2 not completed)

    let ready = ready_tasks(&tasks, &dependencies, &completed);
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&task2));
}
