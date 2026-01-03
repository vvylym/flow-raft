//! Comprehensive tests for DAG utilities

use flow_raft_core::{TaskDependencies, TaskId, topological_order, validate_dag};
use indexmap::IndexMap;
use smallvec::SmallVec;

#[test]
fn test_topological_order_single_task() {
    let task1 = TaskId::default();
    let mut tasks = IndexMap::new();
    tasks.insert(task1, ());
    let dependencies = IndexMap::new();

    let order = topological_order(&tasks, &dependencies);
    assert_eq!(order.len(), 1);
    assert!(order.contains(&task1));
}

#[test]
fn test_validate_dag_chain() {
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

    let mut dependents: IndexMap<TaskId, SmallVec<[TaskId; 4]>> = IndexMap::new();
    let mut deps1 = SmallVec::new();
    deps1.push(task2);
    dependents.insert(task1, deps1);
    let mut deps2 = SmallVec::new();
    deps2.push(task3);
    dependents.insert(task2, deps2);

    let result = validate_dag(&tasks, &dependencies, &dependents);
    assert!(result.is_ok());
}

#[test]
fn test_topological_order_chain() {
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

    let order = topological_order(&tasks, &dependencies);
    // topological_order now visits all tasks, starting from entry tasks
    // For a chain task1 -> task2 -> task3, it should return all 3 tasks in order
    assert_eq!(order.len(), 3);
    let pos1 = order.iter().position(|&id| id == task1).unwrap();
    let pos2 = order.iter().position(|&id| id == task2).unwrap();
    let pos3 = order.iter().position(|&id| id == task3).unwrap();
    assert!(pos1 < pos2);
    assert!(pos2 < pos3);
}

#[test]
fn test_topological_order_parallel() {
    let task1 = TaskId::default();
    let task2 = TaskId::default();
    let task3 = TaskId::default();
    let task4 = TaskId::default();

    let mut tasks = IndexMap::new();
    tasks.insert(task1, ());
    tasks.insert(task2, ());
    tasks.insert(task3, ());
    tasks.insert(task4, ());

    let mut dependencies = IndexMap::new();
    dependencies.insert(task2, TaskDependencies::with_prerequisites(vec![task1]));
    dependencies.insert(task3, TaskDependencies::with_prerequisites(vec![task1]));
    dependencies.insert(
        task4,
        TaskDependencies::with_prerequisites(vec![task2, task3]),
    );

    let order = topological_order(&tasks, &dependencies);
    // topological_order now visits all tasks, starting from entry tasks
    // For parallel tasks task1 -> (task2, task3) -> task4, it should return all 4 tasks
    assert_eq!(order.len(), 4);
    // task1 should come first (entry task)
    assert_eq!(order[0], task1);
    // task4 should come last (depends on task2 and task3)
    assert_eq!(order[3], task4);
}

#[test]
fn test_validate_dag_empty() {
    let tasks = IndexMap::new();
    let dependencies = IndexMap::new();
    let dependents: IndexMap<TaskId, SmallVec<[TaskId; 4]>> = IndexMap::new();

    assert!(validate_dag(&tasks, &dependencies, &dependents).is_ok());
}

#[test]
fn test_validate_dag_single_task() {
    let task1 = TaskId::default();
    let mut tasks = IndexMap::new();
    tasks.insert(task1, ());
    let dependencies = IndexMap::new();
    let dependents: IndexMap<TaskId, SmallVec<[TaskId; 4]>> = IndexMap::new();

    assert!(validate_dag(&tasks, &dependencies, &dependents).is_ok());
}

#[test]
fn test_validate_dag_complex() {
    let task1 = TaskId::default();
    let task2 = TaskId::default();
    let task3 = TaskId::default();
    let task4 = TaskId::default();

    let mut tasks = IndexMap::new();
    tasks.insert(task1, ());
    tasks.insert(task2, ());
    tasks.insert(task3, ());
    tasks.insert(task4, ());

    let mut dependencies = IndexMap::new();
    dependencies.insert(task2, TaskDependencies::with_prerequisites(vec![task1]));
    dependencies.insert(task3, TaskDependencies::with_prerequisites(vec![task1]));
    dependencies.insert(
        task4,
        TaskDependencies::with_prerequisites(vec![task2, task3]),
    );

    let mut dependents: IndexMap<TaskId, SmallVec<[TaskId; 4]>> = IndexMap::new();
    let mut deps1 = SmallVec::new();
    deps1.push(task2);
    deps1.push(task3);
    dependents.insert(task1, deps1);
    let mut deps2 = SmallVec::new();
    deps2.push(task4);
    dependents.insert(task2, deps2);
    let mut deps3 = SmallVec::new();
    deps3.push(task4);
    dependents.insert(task3, deps3);

    assert!(validate_dag(&tasks, &dependencies, &dependents).is_ok());
}
