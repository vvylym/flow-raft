//! Type conversions for gRPC service
//!
//! Converts between protocol buffer types and internal FlowRaft types.

// DateTime and Utc not currently used in this module

use flow_raft_core::{
    TaskExecution, TaskId, TaskState, WorkflowId, WorkflowSnapshot, WorkflowState,
};

// Use proto types from shared proto crate
use flow_raft_proto::proto::*;

/// Converts WorkflowSnapshot to WorkflowStatus
pub fn workflow_snapshot_to_status(snapshot: &WorkflowSnapshot) -> WorkflowStatus {
    let tasks: Vec<TaskStatus> = snapshot
        .executions
        .iter()
        .map(|(task_id, execution)| task_execution_to_status(*task_id, execution))
        .collect();

    WorkflowStatus {
        workflow_id: snapshot.workflow_id.as_ref().to_string(),
        state: workflow_state_to_string(&snapshot.state),
        tasks,
        error_message: snapshot.error_message.clone(),
        outputs: snapshot
            .outputs
            .as_ref()
            .map(|o| serde_json::to_string(o).unwrap_or_default()),
        created_at: snapshot.created_at.timestamp(),
        started_at: snapshot.started_at.map(|dt| dt.timestamp()),
        completed_at: snapshot.completed_at.map(|dt| dt.timestamp()),
    }
}

/// Converts TaskExecution to TaskStatus
pub fn task_execution_to_status(task_id: TaskId, execution: &TaskExecution) -> TaskStatus {
    TaskStatus {
        task_id: task_id.to_string(),
        state: task_state_to_string(&execution.state),
        attempts: execution.attempts as i32,
        error: execution.last_error.clone(),
        outputs: execution
            .outputs
            .as_ref()
            .map(|o| serde_json::to_string(o).unwrap_or_default()),
        started_at: execution.started_at.map(|dt| dt.timestamp()),
        completed_at: execution.completed_at.map(|dt| dt.timestamp()),
    }
}

/// Converts WorkflowState to string
fn workflow_state_to_string(state: &WorkflowState) -> String {
    match state {
        WorkflowState::Draft => "draft".to_string(),
        WorkflowState::Scheduled => "scheduled".to_string(),
        WorkflowState::Running => "running".to_string(),
        WorkflowState::Paused => "paused".to_string(),
        WorkflowState::Completed => "completed".to_string(),
        WorkflowState::Failed { .. } => "failed".to_string(),
        WorkflowState::Cancelled => "cancelled".to_string(),
    }
}

/// Converts TaskState to string
fn task_state_to_string(state: &TaskState) -> String {
    match state {
        TaskState::Pending => "pending".to_string(),
        TaskState::Scheduled => "scheduled".to_string(),
        TaskState::Running => "running".to_string(),
        TaskState::Completed => "completed".to_string(),
        TaskState::Failed { .. } => "failed".to_string(),
        TaskState::PermanentlyFailed { .. } => "permanently_failed".to_string(),
        TaskState::Cancelled => "cancelled".to_string(),
    }
}

/// Parses WorkflowId from string
/// Handles both "workflow:uuid" format and plain UUID
pub fn parse_workflow_id(id: &str) -> Result<WorkflowId, String> {
    // Strip "workflow:" prefix if present
    let uuid_str = id.strip_prefix("workflow:").unwrap_or(id);
    WorkflowId::parse(uuid_str).map_err(|e| format!("Invalid workflow ID: {}", e))
}

/// Parses TaskId from string
/// Handles both "task:uuid" format and plain UUID
pub fn parse_task_id(id: &str) -> Result<TaskId, String> {
    // Strip "task:" prefix if present
    let uuid_str = id.strip_prefix("task:").unwrap_or(id);
    TaskId::parse(uuid_str).map_err(|e| format!("Invalid task ID: {}", e))
}

/// Parses JSON inputs
pub fn parse_inputs(inputs: Option<String>) -> Result<serde_json::Value, String> {
    match inputs {
        Some(s) => serde_json::from_str(&s).map_err(|e| format!("Invalid JSON: {}", e)),
        None => Ok(serde_json::json!({})),
    }
}
