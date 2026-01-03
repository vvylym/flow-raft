//! Transitions from WorkflowDraft state
//!
//! This module contains all state transitions that originate from the
//! WorkflowDraft state. Each transition is clearly documented and independently
//! testable.

use chrono::Utc;
use indexmap::IndexMap;
use rayon::prelude::*;
use smallvec::SmallVec;

use crate::dag::validate_dag;
use crate::{
    RetryConfig, Task, TaskDefinition, TaskExecution, TaskId, TaskPending, TaskState, Workflow,
    WorkflowDraft, WorkflowError, WorkflowId, WorkflowScheduled,
};

impl Workflow<WorkflowDraft> {
    /// Creates a new draft workflow
    ///
    /// # Arguments
    /// * `id` - Workflow identifier
    /// * `inputs` - Workflow inputs (JSON value)
    pub fn new(id: WorkflowId, inputs: serde_json::Value) -> Self {
        Self {
            id,
            task_definitions: IndexMap::new(),
            executions: IndexMap::new(),
            dependencies: IndexMap::new(),
            retry_configs: IndexMap::new(),
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            inputs,
            outputs: None,
            error_message: None,
            state: WorkflowDraft,
        }
    }

    /// Adds a task to the workflow
    ///
    /// Validates that the task's dependencies exist in the workflow.
    ///
    /// # Arguments
    /// * `task` - Task to add (must be in Pending state)
    /// * `retry_config` - Retry configuration for the task
    ///
    /// # Errors
    /// Returns `WorkflowError::DependencyNotFound` if a dependency doesn't exist.
    pub fn add_task(
        mut self,
        task: Task<TaskPending>,
        retry_config: RetryConfig,
    ) -> Result<Self, WorkflowError> {
        // Validate all prerequisites exist
        for prereq in &task.dependencies.prerequisites {
            if !self.task_definitions.contains_key(prereq) {
                return Err(WorkflowError::DependencyNotFound(prereq.to_string()));
            }
        }

        // Add task definition
        let task_def = TaskDefinition {
            id: task.id,
            name: task.name,
            handler: task.handler,
            inputs: task.inputs,
            outputs: task.outputs,
            timeout_secs: task.timeout_secs,
        };
        self.task_definitions.insert(task.id, task_def);

        // Add dependencies
        self.dependencies.insert(task.id, task.dependencies);

        // Add retry config
        self.retry_configs.insert(task.id, retry_config);

        // Initialize execution state
        self.executions.insert(
            task.id,
            TaskExecution {
                task_id: task.id,
                state: TaskState::Pending,
                attempts: 0,
                started_at: None,
                completed_at: None,
                last_error: None,
                outputs: None,
            },
        );

        Ok(self)
    }

    /// Transitions from Draft to Scheduled
    ///
    /// Validates the DAG has no cycles before allowing the transition.
    ///
    /// # Errors
    /// Returns `WorkflowError::CycleDetected` if a cycle is found.
    /// Returns `WorkflowError::NoTasksFound` if workflow has no tasks.
    pub fn schedule(self) -> Result<Workflow<WorkflowScheduled>, WorkflowError> {
        if self.task_definitions.is_empty() {
            return Err(WorkflowError::NoTasksFound);
        }

        // Build dependents map for cycle detection using parallel processing
        let pairs: Vec<(TaskId, TaskId)> = self
            .dependencies
            .par_iter()
            .flat_map(|(task_id, deps)| {
                deps.prerequisites
                    .iter()
                    .map(|&prereq| (prereq, *task_id))
                    .collect::<Vec<_>>()
            })
            .collect();

        let mut dependents: IndexMap<TaskId, SmallVec<[TaskId; 4]>> = IndexMap::new();
        for (prereq, dependent) in pairs {
            dependents.entry(prereq).or_default().push(dependent);
        }

        // Create task map for validation
        let tasks: IndexMap<TaskId, ()> = self
            .task_definitions
            .keys()
            .copied()
            .zip(std::iter::repeat(()))
            .collect();

        // Validate DAG
        validate_dag(&tasks, &self.dependencies, &dependents)?;

        Ok(Workflow {
            id: self.id,
            task_definitions: self.task_definitions,
            executions: self.executions,
            dependencies: self.dependencies,
            retry_configs: self.retry_configs,
            created_at: self.created_at,
            started_at: None,
            completed_at: None,
            inputs: self.inputs,
            outputs: None,
            error_message: None,
            state: WorkflowScheduled,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TaskDependencies;
    use rstest::*;
    use std::collections::HashSet;

    #[fixture]
    fn test_workflow() -> Workflow<WorkflowDraft> {
        Workflow::new(WorkflowId::default(), serde_json::json!({}))
    }

    #[fixture]
    fn test_task(#[default(TaskId::default())] id: TaskId) -> Task<TaskPending> {
        Task::new(
            id,
            "test_task",
            "test_handler",
            RetryConfig::new(3),
            TaskDependencies::default(),
        )
    }

    #[test]
    fn test_workflow_add_task() {
        let mut workflow = test_workflow();
        let task_id = TaskId::default();
        let task = test_task(task_id);
        let retry_config = RetryConfig::new(3);

        workflow = workflow.add_task(task, retry_config).unwrap();
        assert!(workflow.task_definitions.contains_key(&task_id));
        assert!(workflow.executions.contains_key(&task_id));
    }

    #[test]
    fn test_workflow_add_task_with_dependency() {
        let mut workflow = test_workflow();
        let task1_id = TaskId::default();
        let task2_id = TaskId::default();

        let task1 = test_task(task1_id);
        workflow = workflow.add_task(task1, RetryConfig::new(3)).unwrap();

        let mut deps = TaskDependencies::default();
        deps.add_prerequisite(task1_id);
        let task2 = Task::new(task2_id, "task2", "handler2", RetryConfig::new(3), deps);

        workflow = workflow.add_task(task2, RetryConfig::new(3)).unwrap();
        assert!(workflow.task_definitions.contains_key(&task2_id));
    }

    #[test]
    fn test_workflow_add_task_missing_dependency() {
        let workflow = test_workflow();
        let task_id = TaskId::default();
        let missing_id = TaskId::default();

        let mut deps = TaskDependencies::default();
        deps.add_prerequisite(missing_id);
        let task = Task::new(task_id, "task", "handler", RetryConfig::new(3), deps);

        let result = workflow.add_task(task, RetryConfig::new(3));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WorkflowError::DependencyNotFound(_)
        ));
    }

    #[test]
    fn test_workflow_schedule_empty() {
        let workflow = test_workflow();
        let result = workflow.schedule();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), WorkflowError::NoTasksFound));
    }

    #[test]
    fn test_workflow_schedule_success() {
        let mut workflow = test_workflow();
        let task_id = TaskId::default();
        let task = test_task(task_id);
        workflow = workflow.add_task(task, RetryConfig::new(3)).unwrap();

        let scheduled = workflow.schedule().unwrap();
        assert!(matches!(scheduled.state, WorkflowScheduled));
    }

    #[test]
    fn test_workflow_schedule_with_cycle() {
        // Test that schedule() detects cycles when validating the DAG
        let mut workflow = test_workflow();
        let task1_id = TaskId::default();
        let task2_id = TaskId::default();

        // Add both tasks first without dependencies
        let task1 = test_task(task1_id);
        workflow = workflow.add_task(task1, RetryConfig::new(3)).unwrap();
        let task2 = test_task(task2_id);
        workflow = workflow.add_task(task2, RetryConfig::new(3)).unwrap();

        // Now manually create a cycle: task1 depends on task2, task2 depends on task1
        let mut deps1 = TaskDependencies::default();
        deps1.add_prerequisite(task2_id);
        workflow.dependencies.insert(task1_id, deps1);

        let mut deps2 = TaskDependencies::default();
        deps2.add_prerequisite(task1_id);
        workflow.dependencies.insert(task2_id, deps2);

        // Create task2 that depends on task1 (cycle)
        let mut deps2 = TaskDependencies::default();
        deps2.add_prerequisite(task1_id);

        // This should fail because task2 depends on task1 which doesn't exist yet
        // But we can manually insert it to create the cycle
        let task2_def = TaskDefinition {
            id: task2_id,
            name: "task2".to_string(),
            handler: "handler2".to_string(),
            inputs: HashSet::new(),
            outputs: HashSet::new(),
            timeout_secs: None,
        };
        workflow.task_definitions.insert(task2_id, task2_def);
        workflow.dependencies.insert(task2_id, deps2);
        workflow.executions.insert(
            task2_id,
            TaskExecution {
                task_id: task2_id,
                state: TaskState::Pending,
                attempts: 0,
                started_at: None,
                completed_at: None,
                last_error: None,
                outputs: None,
            },
        );
        workflow.retry_configs.insert(task2_id, RetryConfig::new(3));

        // Now try to schedule - should detect cycle
        let result = workflow.schedule();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), WorkflowError::CycleDetected));
    }
}
