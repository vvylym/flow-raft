//! Task dependencies representation
//!
//! Defines the DAG structure for task dependencies using efficient collections.

use rayon::prelude::*;
use smallvec::SmallVec;
use std::collections::HashSet;

use crate::TaskId;

/// Default size of the task dependencies
const TASK_DEPENDENCIES_DEFAULT_SIZE: usize = 4;

/// Task dependencies in a DAG structure
///
/// Uses `SmallVec` for small dependency lists to reduce allocations.
/// Most tasks have few dependencies, so this optimizes for the common case.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskDependencies {
    /// Tasks that must complete before this task can start (prerequisites)
    ///
    /// Uses SmallVec with inline storage for up to 4 dependencies (common case).
    /// Falls back to heap allocation for larger dependency lists.
    pub prerequisites: SmallVec<[TaskId; TASK_DEPENDENCIES_DEFAULT_SIZE]>,
    /// Tasks that depend on this task's completion (dependents)
    ///
    /// Uses SmallVec with inline storage for up to 4 dependents (common case).
    pub dependents: SmallVec<[TaskId; TASK_DEPENDENCIES_DEFAULT_SIZE]>,
}

impl TaskDependencies {
    /// Creates a new empty TaskDependencies
    #[inline]
    pub fn new(
        prerequisites: impl IntoIterator<Item = TaskId>,
        dependents: impl IntoIterator<Item = TaskId>,
    ) -> Self {
        Self {
            prerequisites: prerequisites.into_iter().collect(),
            dependents: dependents.into_iter().collect(),
        }
    }

    /// Create dependencies with only prerequisites (no dependents)
    pub fn with_prerequisites(prerequisites: impl IntoIterator<Item = TaskId>) -> Self {
        Self {
            prerequisites: prerequisites.into_iter().collect(),
            dependents: SmallVec::new(),
        }
    }

    /// Adds a prerequisite task
    #[inline]
    pub fn add_prerequisite(&mut self, task_id: TaskId) {
        self.prerequisites.push(task_id);
    }

    /// Adds a dependent task
    #[inline]
    pub fn add_dependent(&mut self, task_id: TaskId) {
        self.dependents.push(task_id);
    }

    /// Returns true if there are no prerequisites
    #[inline]
    pub fn has_no_prerequisites(&self) -> bool {
        self.prerequisites.is_empty()
    }

    /// Returns true if there are no dependents
    #[inline]
    pub fn has_no_dependents(&self) -> bool {
        self.dependents.is_empty()
    }

    /// Checks if all prerequisites are in the completed set
    ///
    /// Uses parallel iteration with rayon for improved performance.
    ///
    /// # Arguments
    /// * `completed` - Set of completed task IDs
    #[inline]
    pub fn has_all_prerequisites_completed(&self, completed: &HashSet<TaskId>) -> bool {
        if self.prerequisites.is_empty() {
            return true;
        }

        self.prerequisites
            .par_iter()
            .all(|&prereq| completed.contains(&prereq))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_dependencies_new() {
        let deps = TaskDependencies::default();
        assert!(deps.has_no_prerequisites());
        assert!(deps.has_no_dependents());
    }

    #[test]
    fn test_task_dependencies_with_prerequisites() {
        let id1 = TaskId::default();
        let id2 = TaskId::default();
        let deps = TaskDependencies::with_prerequisites([id1, id2]);
        assert_eq!(deps.prerequisites.len(), 2);
        assert!(deps.has_no_dependents());
    }

    #[test]
    fn test_all_prerequisites_completed() {
        let id1 = TaskId::default();
        let id2 = TaskId::default();
        let deps = TaskDependencies::with_prerequisites([id1, id2]);

        let mut completed = HashSet::new();
        assert!(!deps.has_all_prerequisites_completed(&completed));

        completed.insert(id1);
        assert!(!deps.has_all_prerequisites_completed(&completed));

        completed.insert(id2);
        assert!(deps.has_all_prerequisites_completed(&completed));
    }

    #[test]
    fn test_task_dependencies_new_with_iterators() {
        let id1 = TaskId::default();
        let id2 = TaskId::default();
        let id3 = TaskId::default();
        let id4 = TaskId::default();

        let deps = TaskDependencies::new([id1, id2], [id3, id4]);
        assert_eq!(deps.prerequisites.len(), 2);
        assert_eq!(deps.dependents.len(), 2);
    }

    #[test]
    fn test_task_dependencies_add_prerequisite() {
        let mut deps = TaskDependencies::default();
        let id = TaskId::default();
        deps.add_prerequisite(id);
        assert_eq!(deps.prerequisites.len(), 1);
        assert!(deps.prerequisites.contains(&id));
    }

    #[test]
    fn test_task_dependencies_add_dependent() {
        let mut deps = TaskDependencies::default();
        let id = TaskId::default();
        deps.add_dependent(id);
        assert_eq!(deps.dependents.len(), 1);
        assert!(deps.dependents.contains(&id));
    }

    #[test]
    fn test_all_prerequisites_completed_empty() {
        let deps = TaskDependencies::default();
        let completed = HashSet::new();
        assert!(deps.has_all_prerequisites_completed(&completed));
    }

    #[test]
    fn test_all_prerequisites_completed_large_list() {
        // Test parallel processing path (more than 8 prerequisites)
        let ids: Vec<TaskId> = (0..10).map(|_| TaskId::default()).collect();
        let deps = TaskDependencies::with_prerequisites(ids.clone());

        let mut completed = HashSet::new();
        for id in &ids {
            completed.insert(*id);
        }

        assert!(deps.has_all_prerequisites_completed(&completed));
    }

    #[test]
    fn test_all_prerequisites_completed_partial() {
        let ids: Vec<TaskId> = (0..5).map(|_| TaskId::default()).collect();
        let deps = TaskDependencies::with_prerequisites(ids.clone());

        let mut completed = HashSet::new();
        completed.insert(ids[0]);
        completed.insert(ids[1]);

        assert!(!deps.has_all_prerequisites_completed(&completed));
    }
}
