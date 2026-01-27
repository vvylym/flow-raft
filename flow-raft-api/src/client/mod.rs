//! Client API for FlowRaft
//!
//! Provides a client interface for submitting workflows and retrieving results.
//!
//! The client connects to FlowRaft servers via gRPC and provides methods for:
//! - Workflow submission and execution
//! - Workflow status and result retrieval
//! - Workflow control (pause, resume, cancel)
//! - Real-time execution event streaming
//!
//! # Example
//! ```no_run
//! use flow_raft_api::client::FlowRaftClient;
//! use flow_raft_api::graph::{node, TypedGraphBuilder};
//! use serde_json::json;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = FlowRaftClient::new("http://localhost:50051");
//! let mut b = TypedGraphBuilder::new("example");
//! b.add_node("n", node(|_: ()| Ok::<(), String>(())), None).set_root("n");
//! let workflow = b.build().unwrap().workflow_def("example").unwrap();
//! let execution_id = client.submit_workflow(workflow, json!({})).await?;
//! let status = client.get_workflow_status(execution_id).await?;
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

use flow_raft_core::{TaskId, WorkflowId};
use flow_raft_proto::proto::flow_raft_service_client::FlowRaftServiceClient;
use flow_raft_proto::proto::*;
use serde_json::Value;
use thiserror::Error;
use tokio_stream::Stream;
use tonic::Request;

use crate::workflow::WorkflowDef;

pub mod builder;
pub mod events;
pub mod grpc;

pub use builder::FlowRaftClientBuilder;
pub use events::ExecutionEvent;

/// Convert proto WorkflowStatus to client WorkflowStatus enum
fn proto_workflow_status_to_client(
    proto: &flow_raft_proto::proto::WorkflowStatus,
) -> Result<WorkflowStatus, ClientError> {
    match proto.state.as_str() {
        "pending" => Ok(WorkflowStatus::Pending),
        "running" => Ok(WorkflowStatus::Running),
        "completed" => {
            let outputs = proto
                .outputs
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok());
            Ok(WorkflowStatus::Completed { outputs })
        }
        "failed" => {
            let error = proto.error_message.clone();
            Ok(WorkflowStatus::Failed { error })
        }
        "cancelled" => Ok(WorkflowStatus::Cancelled),
        state => Err(ClientError::InvalidInput(format!(
            "Unknown workflow state: {}",
            state
        ))),
    }
}

/// Convert proto WorkflowUpdate to ExecutionEvent
fn proto_workflow_update_to_execution_event(
    update: &flow_raft_proto::proto::WorkflowUpdate,
) -> Result<ExecutionEvent, ClientError> {
    let data: Value = update
        .data
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    match update.event_type.as_str() {
        "task_started" => {
            let task_id_str = data
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ClientError::InvalidInput("Missing task_id in task_started event".to_string())
                })?;
            let task_id = TaskId::parse(task_id_str)
                .map_err(|e| ClientError::InvalidInput(format!("Invalid task_id: {}", e)))?;
            let inputs = data
                .get("inputs")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            Ok(ExecutionEvent::TaskStarted { task_id, inputs })
        }
        "task_completed" => {
            let task_id_str = data
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ClientError::InvalidInput("Missing task_id in task_completed event".to_string())
                })?;
            let task_id = TaskId::parse(task_id_str)
                .map_err(|e| ClientError::InvalidInput(format!("Invalid task_id: {}", e)))?;
            let outputs = data
                .get("outputs")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            Ok(ExecutionEvent::TaskCompleted { task_id, outputs })
        }
        "task_failed" => {
            let task_id_str = data
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ClientError::InvalidInput("Missing task_id in task_failed event".to_string())
                })?;
            let task_id = TaskId::parse(task_id_str)
                .map_err(|e| ClientError::InvalidInput(format!("Invalid task_id: {}", e)))?;
            let error = data
                .get("error")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Task failed".to_string());
            Ok(ExecutionEvent::TaskFailed { task_id, error })
        }
        "workflow_completed" => {
            let outputs = data
                .get("outputs")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            Ok(ExecutionEvent::WorkflowCompleted { outputs })
        }
        "workflow_failed" => {
            let error = data
                .get("error")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Workflow failed".to_string());
            Ok(ExecutionEvent::WorkflowFailed { error })
        }
        _ => Err(ClientError::InvalidInput(format!(
            "Unknown event type: {}",
            update.event_type
        ))),
    }
}

/// Workflow execution ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkflowExecutionId(pub WorkflowId);

impl std::fmt::Display for WorkflowExecutionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<WorkflowId> for WorkflowExecutionId {
    fn from(id: WorkflowId) -> Self {
        Self(id)
    }
}

impl From<WorkflowExecutionId> for WorkflowId {
    fn from(id: WorkflowExecutionId) -> Self {
        id.0
    }
}

/// Workflow status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowStatus {
    /// Workflow is pending
    Pending,
    /// Workflow is running
    Running,
    /// Workflow completed successfully
    Completed {
        /// Workflow outputs
        outputs: Option<Value>,
    },
    /// Workflow failed
    Failed {
        /// Error message
        error: Option<String>,
    },
    /// Workflow was cancelled
    Cancelled,
}

/// Client errors
#[derive(Debug, Error)]
pub enum ClientError {
    /// Network/connection error
    #[error("Connection error: {0}")]
    Connection(String),
    /// Server error
    #[error("Server error: {0}")]
    Server(String),
    /// Workflow not found
    #[error("Workflow not found: {0}")]
    NotFound(WorkflowExecutionId),
    /// Timeout waiting for workflow
    #[error("Timeout waiting for workflow: {0}")]
    Timeout(WorkflowExecutionId),
    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// FlowRaft client for submitting and managing workflows
pub struct FlowRaftClient {
    /// gRPC client (lazily initialized)
    #[allow(dead_code)]
    grpc_client: Option<crate::client::grpc::GrpcClient>,
    /// Server endpoint
    endpoint: String,
    /// Timeout for operations
    timeout: Duration,
}

impl FlowRaftClient {
    /// Create a new FlowRaft client
    ///
    /// # Note
    /// The client is created without an active connection. The connection is established
    /// lazily when the first gRPC call is made. Use `FlowRaftClientBuilder` for more control.
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            grpc_client: None,
            endpoint: endpoint.into(),
            timeout: Duration::from_secs(300), // 5 minutes default
        }
    }

    /// Set the timeout for operations
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Get or create the gRPC client
    #[allow(dead_code)]
    async fn get_grpc_client(
        &mut self,
    ) -> Result<&mut crate::client::grpc::GrpcClient, ClientError> {
        if self.grpc_client.is_none() {
            self.grpc_client = Some(
                crate::client::grpc::GrpcClient::new(self.endpoint.clone(), self.timeout).await?,
            );
        }
        Ok(self.grpc_client.as_mut().unwrap())
    }

    /// Get a gRPC service client (read-only access)
    async fn get_service_client(
        &self,
    ) -> Result<FlowRaftServiceClient<tonic::transport::Channel>, ClientError> {
        // Create a new channel for this call
        let endpoint = tonic::transport::Endpoint::from_shared(self.endpoint.clone())
            .map_err(|e| {
                ClientError::Connection(format!("Invalid endpoint '{}': {}", self.endpoint, e))
            })?
            .timeout(self.timeout)
            .connect_timeout(Duration::from_secs(5));

        let channel = endpoint.connect().await.map_err(|e| {
            ClientError::Connection(format!("Failed to connect to '{}': {}", self.endpoint, e))
        })?;

        Ok(FlowRaftServiceClient::new(channel))
    }

    /// Trigger an existing workflow by ID with input.
    ///
    /// Calls the TriggerWorkflow RPC directly. The workflow must already exist
    /// on the server (e.g. created via run_grpc_on_cluster or pre-registered).
    /// Use [submit_workflow](Self::submit_workflow) to define and trigger in one
    /// call when the server supports full define_workflow.
    ///
    /// # Arguments
    /// * `workflow_id` - The workflow ID
    /// * `input` - Input data for the workflow
    ///
    /// # Returns
    /// The execution ID (same as workflow_id for this RPC)
    pub async fn trigger_workflow_by_id(
        &self,
        workflow_id: WorkflowId,
        input: Value,
    ) -> Result<WorkflowExecutionId, ClientError> {
        let mut client = self.get_service_client().await?;
        let trigger_request = TriggerWorkflowRequest {
            workflow_id: workflow_id.to_string(),
            inputs: Some(serde_json::to_string(&input).map_err(|e| {
                ClientError::InvalidInput(format!("Failed to serialize inputs: {}", e))
            })?),
        };
        let trigger_response = client
            .trigger_workflow(Request::new(trigger_request))
            .await
            .map_err(|e| ClientError::Server(format!("gRPC error triggering workflow: {}", e)))?
            .into_inner();
        WorkflowId::parse(&trigger_response.execution_id)
            .map(WorkflowExecutionId)
            .map_err(|e| ClientError::InvalidInput(format!("Invalid execution_id: {}", e)))
    }

    /// Submit a workflow with input and get execution ID
    ///
    /// # Arguments
    /// * `workflow` - The workflow definition
    /// * `input` - Input data for the workflow
    ///
    /// # Returns
    /// The execution ID of the submitted workflow
    ///
    /// # Note
    /// This method defines the workflow and then triggers it. The workflow definition
    /// is converted to JSON format for the gRPC call.
    pub async fn submit_workflow(
        &self,
        workflow: WorkflowDef,
        input: Value,
    ) -> Result<WorkflowExecutionId, ClientError> {
        let mut client = self.get_service_client().await?;

        // Convert workflow definition to JSON
        // Since WorkflowDef doesn't implement Serialize and the gRPC service's define_workflow
        // is still a partial implementation, we construct a simplified JSON structure
        let workflow_json_value = serde_json::json!({
            "name": workflow.name(),
            "workflow_id": workflow.workflow_id().to_string(),
            "graph": {
                "name": workflow.graph().name,
                "nodes": workflow.graph().nodes.iter().map(|(name, node)| {
                    serde_json::json!({
                        "name": name.as_ref(),
                        "task_id": node.task_id.to_string(),
                        "handler": node.handler,
                        "inputs": node.inputs.iter().collect::<Vec<_>>(),
                        "outputs": node.outputs.iter().collect::<Vec<_>>(),
                        "timeout_secs": node.timeout_secs
                    })
                }).collect::<Vec<_>>(),
                "edges": workflow.graph().edges.iter().map(|(from, edges)| {
                    serde_json::json!({
                        "from": from.as_ref(),
                        "edges": edges.iter().map(|edge| {
                            match edge {
                                crate::graph::builder::EdgeSpec::Simple(to) => {
                                    serde_json::json!({"type": "simple", "to": to.as_ref()})
                                }
                                crate::graph::builder::EdgeSpec::Conditional { then, otherwise, .. } => {
                                    serde_json::json!({"type": "conditional", "then": then.as_ref(), "otherwise": otherwise.as_ref()})
                                }
                                crate::graph::builder::EdgeSpec::Split { targets, .. } => {
                                    let targets_vec: Vec<&str> = targets.iter().map(|t| t.as_ref()).collect();
                                    serde_json::json!({"type": "split", "targets": targets_vec})
                                }
                                crate::graph::builder::EdgeSpec::Switch { branches, .. } => {
                                    let branches_vec: Vec<&str> = branches.iter().map(|b| b.as_ref()).collect();
                                    serde_json::json!({"type": "switch", "branches": branches_vec})
                                }
                            }
                        }).collect::<Vec<_>>()
                    })
                }).collect::<Vec<_>>(),
                "root": workflow.graph().root.as_ref().map(|r| r.as_ref())
            },
            "default_retry_config": {
                "max_attempts": workflow.default_retry_config.max_attempts,
                "initial_delay_ms": workflow.default_retry_config.initial_delay_ms,
                "backoff_factor": workflow.default_retry_config.backoff_factor
            }
        });
        let workflow_json = serde_json::to_string(&workflow_json_value).map_err(|e| {
            ClientError::InvalidInput(format!("Failed to serialize workflow definition: {}", e))
        })?;

        // Define the workflow
        let define_request = DefineWorkflowRequest {
            name: workflow.name().to_string(),
            definition: workflow_json,
        };

        let define_response = client
            .define_workflow(Request::new(define_request))
            .await
            .map_err(|e| ClientError::Server(format!("gRPC error defining workflow: {}", e)))?
            .into_inner();

        // Parse workflow_id from response
        let workflow_id = WorkflowId::parse(&define_response.workflow_id)
            .map_err(|e| ClientError::InvalidInput(format!("Invalid workflow_id: {}", e)))?;

        // Trigger the workflow
        let trigger_request = TriggerWorkflowRequest {
            workflow_id: workflow_id.to_string(),
            inputs: Some(serde_json::to_string(&input).map_err(|e| {
                ClientError::InvalidInput(format!("Failed to serialize inputs: {}", e))
            })?),
        };

        let trigger_response = client
            .trigger_workflow(Request::new(trigger_request))
            .await
            .map_err(|e| ClientError::Server(format!("gRPC error triggering workflow: {}", e)))?
            .into_inner();

        // Parse execution_id from response
        WorkflowId::parse(&trigger_response.execution_id)
            .map(WorkflowExecutionId)
            .map_err(|e| ClientError::InvalidInput(format!("Invalid execution_id: {}", e)))
    }

    /// Run a single task on a node (for distributed execution).
    /// The caller is responsible for applying the result to the Raft state.
    ///
    /// # Arguments
    /// * `endpoint` - gRPC endpoint of the node (e.g. "http://127.0.0.1:50052")
    /// * `workflow_id` - Workflow ID
    /// * `task_id` - Task ID
    /// * `handler_name` - Handler name registered for that workflow on the node
    /// * `inputs` - Task inputs (JSON)
    ///
    /// # Returns
    /// Task outputs on success, or error string on failure.
    pub async fn run_task_on(
        &self,
        endpoint: &str,
        workflow_id: WorkflowId,
        task_id: TaskId,
        handler_name: &str,
        inputs: Value,
    ) -> Result<Value, ClientError> {
        let mut client = if endpoint == self.endpoint {
            self.get_service_client().await?
        } else {
            let channel = tonic::transport::Endpoint::from_shared(endpoint.to_string())
                .map_err(|e| {
                    ClientError::Connection(format!("Invalid endpoint '{}': {}", endpoint, e))
                })?
                .timeout(self.timeout)
                .connect_timeout(Duration::from_secs(5))
                .connect()
                .await
                .map_err(|e| {
                    ClientError::Connection(format!("Failed to connect to '{}': {}", endpoint, e))
                })?;
            FlowRaftServiceClient::new(channel)
        };
        let req = RunTaskRequest {
            workflow_id: workflow_id.to_string(),
            task_id: task_id.to_string(),
            handler_name: handler_name.to_string(),
            inputs: Some(serde_json::to_string(&inputs).map_err(|e| {
                ClientError::InvalidInput(format!("Failed to serialize inputs: {}", e))
            })?),
        };
        let r = client
            .run_task(Request::new(req))
            .await
            .map_err(|e| ClientError::Server(format!("RunTask gRPC error: {}", e)))?
            .into_inner();
        if let Some(e) = r.error {
            return Err(ClientError::Server(e));
        }
        let out = r.outputs.ok_or_else(|| {
            ClientError::Server("RunTask returned no outputs and no error".to_string())
        })?;
        serde_json::from_str(&out)
            .map_err(|e| ClientError::InvalidInput(format!("Invalid outputs JSON: {}", e)))
    }

    /// Get workflow status
    ///
    /// # Arguments
    /// * `execution_id` - The workflow execution ID
    ///
    /// # Returns
    /// The current status of the workflow
    pub async fn get_workflow_status(
        &self,
        execution_id: WorkflowExecutionId,
    ) -> Result<WorkflowStatus, ClientError> {
        let mut client = self.get_service_client().await?;

        let request = GetWorkflowRequest {
            workflow_id: execution_id.0.to_string(),
        };

        let response = client
            .get_workflow(Request::new(request))
            .await
            .map_err(|e| ClientError::Server(format!("gRPC error: {}", e)))?
            .into_inner();

        proto_workflow_status_to_client(&response)
    }

    /// Get workflow output (blocks until complete or timeout)
    ///
    /// # Arguments
    /// * `execution_id` - The workflow execution ID
    ///
    /// # Returns
    /// The workflow output when complete
    ///
    /// # Note
    /// This method polls `get_workflow_status` until the workflow reaches a terminal state
    /// (Completed, Failed, or Cancelled) or the timeout is reached.
    pub async fn get_workflow_output(
        &mut self,
        execution_id: WorkflowExecutionId,
    ) -> Result<Value, ClientError> {
        // In a full implementation, this would:
        // 1. Poll get_workflow_status in a loop
        // 2. Check if status is Completed, Failed, or Cancelled
        // 3. Return outputs or error accordingly
        // 4. Respect self.timeout
        let poll_interval = Duration::from_millis(100);
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > self.timeout {
                return Err(ClientError::Timeout(execution_id));
            }

            match self.get_workflow_status(execution_id).await {
                Ok(WorkflowStatus::Completed { outputs }) => {
                    return Ok(outputs.unwrap_or_else(|| serde_json::json!({})));
                }
                Ok(WorkflowStatus::Failed { error }) => {
                    return Err(ClientError::Server(
                        error.unwrap_or_else(|| "Workflow failed".to_string()),
                    ));
                }
                Ok(WorkflowStatus::Cancelled) => {
                    return Err(ClientError::Server("Workflow was cancelled".to_string()));
                }
                Ok(WorkflowStatus::Pending | WorkflowStatus::Running) => {
                    tokio::time::sleep(poll_interval).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Run a workflow by name with inputs
    ///
    /// # Arguments
    /// * `workflow_name` - Name of the workflow to run
    /// * `inputs` - Input data for the workflow
    ///
    /// # Returns
    /// The execution ID of the started workflow
    ///
    /// # Note
    /// This method looks up the workflow by name and triggers it. The workflow must
    /// already be defined. Use `submit_workflow` to define and run a new workflow.
    pub async fn run(
        &self,
        workflow_name: &str,
        inputs: Value,
    ) -> Result<WorkflowExecutionId, ClientError> {
        let mut client = self.get_service_client().await?;

        // First, list workflows to find the one with matching name
        let list_request = ListWorkflowsRequest {
            filter: None,
            limit: 1000,
            offset: 0,
        };

        let list_response = client
            .list_workflows(Request::new(list_request))
            .await
            .map_err(|e| ClientError::Server(format!("gRPC error listing workflows: {}", e)))?
            .into_inner();

        // Find workflow by name (we need to get full workflow details to check name)
        // Since we only have workflow_id in the summary, we'll need to get each workflow
        // For now, we'll try to find by workflow_id pattern or use the first one
        // In a full implementation, we'd need a GetWorkflowByName RPC or store name in summary
        // For now, we'll use the workflow_id directly if it matches the name pattern
        let workflow_id = list_response
            .workflows
            .iter()
            .find(|w| w.workflow_id.contains(workflow_name))
            .map(|w| w.workflow_id.clone())
            .ok_or_else(|| {
                ClientError::NotFound(WorkflowExecutionId(
                    WorkflowId::parse("00000000-0000-0000-0000-000000000000").unwrap(),
                ))
            })?;

        // Parse workflow_id
        let workflow_id = WorkflowId::parse(&workflow_id)
            .map_err(|e| ClientError::InvalidInput(format!("Invalid workflow_id: {}", e)))?;

        // Trigger the workflow
        let trigger_request = TriggerWorkflowRequest {
            workflow_id: workflow_id.to_string(),
            inputs: Some(serde_json::to_string(&inputs).map_err(|e| {
                ClientError::InvalidInput(format!("Failed to serialize inputs: {}", e))
            })?),
        };

        let trigger_response = client
            .trigger_workflow(Request::new(trigger_request))
            .await
            .map_err(|e| ClientError::Server(format!("gRPC error triggering workflow: {}", e)))?
            .into_inner();

        // Parse execution_id from response
        WorkflowId::parse(&trigger_response.execution_id)
            .map(WorkflowExecutionId)
            .map_err(|e| ClientError::InvalidInput(format!("Invalid execution_id: {}", e)))
    }

    /// Run a workflow with callbacks for execution tracking
    ///
    /// # Arguments
    /// * `workflow_name` - Name of the workflow to run
    /// * `inputs` - Input data for the workflow
    /// * `on_task_start` - Callback when a task starts
    /// * `on_task_complete` - Callback when a task completes
    /// * `on_task_failed` - Callback when a task fails
    ///
    /// # Returns
    /// The execution ID of the started workflow
    pub async fn run_with_callbacks<F>(
        &mut self,
        workflow_name: &str,
        inputs: Value,
        on_task_start: F,
        on_task_complete: F,
        on_task_failed: F,
    ) -> Result<WorkflowExecutionId, ClientError>
    where
        F: Fn(TaskId, Value) + Send + Sync + Clone + 'static,
    {
        let exec_id = self.run(workflow_name, inputs).await?;

        // Start watching the execution and invoke callbacks
        // Create a new client for the watch operation to avoid borrowing issues
        let endpoint = self.endpoint.clone();
        let execution_id = exec_id;

        // Spawn task to process events and invoke callbacks
        tokio::spawn(async move {
            use crate::client::ExecutionEvent;
            use tokio_stream::StreamExt;

            // Create a temporary client for watching
            let mut temp_client = FlowRaftClient::new(endpoint);
            if let Ok(mut event_stream) = temp_client.watch_execution(execution_id).await {
                while let Some(event) = event_stream.next().await {
                    match event {
                        ExecutionEvent::TaskStarted { task_id, inputs } => {
                            on_task_start(task_id, inputs);
                        }
                        ExecutionEvent::TaskCompleted { task_id, outputs } => {
                            on_task_complete(task_id, outputs);
                        }
                        ExecutionEvent::TaskFailed { task_id, error } => {
                            on_task_failed(task_id, serde_json::json!({ "error": error }));
                        }
                        _ => {
                            // Ignore workflow-level events for now
                        }
                    }
                }
            }
        });

        Ok(exec_id)
    }

    /// Watch execution events as a stream
    ///
    /// # Arguments
    /// * `execution_id` - The workflow execution ID to watch
    ///
    /// # Returns
    /// A stream of execution events
    pub async fn watch_execution(
        &mut self,
        execution_id: WorkflowExecutionId,
    ) -> Result<impl Stream<Item = ExecutionEvent> + Send, ClientError> {
        let mut client = self.get_service_client().await?;

        let request = WatchWorkflowRequest {
            workflow_id: execution_id.0.to_string(),
        };

        let mut stream = client
            .watch_workflow(Request::new(request))
            .await
            .map_err(|e| ClientError::Server(format!("gRPC error watching workflow: {}", e)))?
            .into_inner();

        // Convert proto stream to ExecutionEvent stream
        use tokio_stream::wrappers::ReceiverStream;
        let (tx, rx) = tokio::sync::mpsc::channel(128);

        tokio::spawn(async move {
            while let Some(update_result) = stream.message().await.transpose() {
                match update_result {
                    Ok(update) => {
                        match proto_workflow_update_to_execution_event(&update) {
                            Ok(event) => {
                                if tx.send(event).await.is_err() {
                                    // Receiver dropped, stop forwarding
                                    break;
                                }
                            }
                            Err(e) => {
                                // Log error but continue processing
                                eprintln!("Error converting workflow update: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error receiving workflow update: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(ReceiverStream::new(rx))
    }

    /// Get workflow status
    ///
    /// Alias for get_workflow_status for convenience
    pub async fn get_status(
        &self,
        execution_id: WorkflowExecutionId,
    ) -> Result<WorkflowStatus, ClientError> {
        self.get_workflow_status(execution_id).await
    }

    /// Get task result
    ///
    /// # Arguments
    /// * `execution_id` - The workflow execution ID
    /// * `task_id` - The task ID
    ///
    /// # Returns
    /// The task output if available
    pub async fn get_task_result(
        &self,
        execution_id: WorkflowExecutionId,
        task_id: TaskId,
    ) -> Result<Option<Value>, ClientError> {
        let mut client = self.get_service_client().await?;

        let request = GetTaskResultsRequest {
            workflow_id: execution_id.0.to_string(),
            task_id: task_id.to_string(),
        };

        let response = client
            .get_task_results(Request::new(request))
            .await
            .map_err(|e| ClientError::Server(format!("gRPC error: {}", e)))?
            .into_inner();

        // Parse outputs from JSON string
        if let Some(outputs_str) = response.outputs {
            serde_json::from_str(&outputs_str).map(Some).map_err(|e| {
                ClientError::InvalidInput(format!("Invalid JSON in task outputs: {}", e))
            })
        } else {
            Ok(None)
        }
    }

    /// Wait for workflow completion with timeout
    ///
    /// # Arguments
    /// * `execution_id` - The workflow execution ID
    /// * `timeout` - Maximum time to wait
    ///
    /// # Returns
    /// The workflow output when complete
    pub async fn wait_for_completion(
        &mut self,
        execution_id: WorkflowExecutionId,
        timeout: Duration,
    ) -> Result<Value, ClientError> {
        let original_timeout = self.timeout;
        self.timeout = timeout;
        let result = self.get_workflow_output(execution_id).await;
        self.timeout = original_timeout;
        result
    }

    /// Cancel a running workflow
    ///
    /// # Arguments
    /// * `execution_id` - The workflow execution ID
    pub async fn cancel_workflow(
        &self,
        execution_id: WorkflowExecutionId,
    ) -> Result<(), ClientError> {
        let mut client = self.get_service_client().await?;

        let request = CancelWorkflowRequest {
            workflow_id: execution_id.0.to_string(),
        };

        client
            .cancel_workflow(Request::new(request))
            .await
            .map_err(|e| ClientError::Server(format!("gRPC error: {}", e)))?;

        Ok(())
    }

    /// Pause a running workflow
    ///
    /// # Arguments
    /// * `execution_id` - The workflow execution ID
    pub async fn pause_workflow(
        &self,
        execution_id: WorkflowExecutionId,
    ) -> Result<WorkflowStatus, ClientError> {
        let mut client = self.get_service_client().await?;

        let request = PauseWorkflowRequest {
            workflow_id: execution_id.0.to_string(),
        };

        let response = client
            .pause_workflow(Request::new(request))
            .await
            .map_err(|e| ClientError::Server(format!("gRPC error: {}", e)))?
            .into_inner();

        proto_workflow_status_to_client(&response)
    }

    /// Resume a paused workflow
    ///
    /// # Arguments
    /// * `execution_id` - The workflow execution ID
    pub async fn resume_workflow(
        &self,
        execution_id: WorkflowExecutionId,
    ) -> Result<WorkflowStatus, ClientError> {
        let mut client = self.get_service_client().await?;

        let request = ResumeWorkflowRequest {
            workflow_id: execution_id.0.to_string(),
        };

        let response = client
            .resume_workflow(Request::new(request))
            .await
            .map_err(|e| ClientError::Server(format!("gRPC error: {}", e)))?
            .into_inner();

        proto_workflow_status_to_client(&response)
    }
}
