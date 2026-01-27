//! Task routing for distributed execution.
//!
//! Lets the leader decide whether to run a task locally or route it to another node
//! that has the workflow and handlers registered.

use std::collections::HashMap;

use flow_raft_core::WorkflowId;

/// Resolves (workflow_id, handler_name) to an optional gRPC endpoint.
/// None means "run locally". Used by the leader to route tasks to nodes that have the handler.
pub trait TaskRouter: Send + Sync {
    /// Returns the gRPC endpoint (e.g. "http://127.0.0.1:50052") for the node that should run
    /// this task, or None to run locally.
    fn route(&self, workflow_id: &WorkflowId, handler_name: &str) -> Option<String>;
}

/// Task router that always runs locally (no routing).
#[derive(Debug, Clone, Default)]
pub struct LocalOnlyTaskRouter;

impl TaskRouter for LocalOnlyTaskRouter {
    fn route(&self, _workflow_id: &WorkflowId, _handler_name: &str) -> Option<String> {
        None
    }
}

/// Task router that uses a fixed map from (workflow_id, handler_name) to endpoint.
/// Supports "workflow + handlers registered per node" by configuring which node (endpoint)
/// runs which (workflow_id, handler_name).
#[derive(Debug, Clone)]
pub struct MapTaskRouter {
    /// (workflow_id string, handler_name) -> endpoint
    map: HashMap<(String, String), String>,
}

impl MapTaskRouter {
    /// Creates an empty map router.
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Associates (workflow_id, handler_name) with an endpoint.
    pub fn add_route(
        &mut self,
        workflow_id: impl std::fmt::Display,
        handler_name: impl Into<String>,
        endpoint: impl Into<String>,
    ) {
        self.map.insert(
            (workflow_id.to_string(), handler_name.into()),
            endpoint.into(),
        );
    }

    /// Builds from a list of (workflow_id, handler_name, endpoint).
    pub fn from_routes(routes: impl IntoIterator<Item = (WorkflowId, String, String)>) -> Self {
        let mut m = HashMap::new();
        for (wf, name, ep) in routes {
            m.insert((wf.as_ref().to_string(), name), ep);
        }
        Self { map: m }
    }
}

impl Default for MapTaskRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskRouter for MapTaskRouter {
    fn route(&self, workflow_id: &WorkflowId, handler_name: &str) -> Option<String> {
        self.map
            .get(&(workflow_id.as_ref().to_string(), handler_name.to_string()))
            .cloned()
    }
}

/// Caller that can run a task on a remote endpoint (e.g. via RunTask gRPC).
/// The executor uses this when the router returns a different endpoint.
#[tonic::async_trait]
pub trait RunTaskCaller: Send + Sync {
    /// Runs the task on the given endpoint and returns the task output or error.
    /// The caller (leader) is responsible for applying the result to the Raft state.
    async fn run_task_on(
        &self,
        endpoint: &str,
        workflow_id: WorkflowId,
        task_id: flow_raft_core::TaskId,
        handler_name: &str,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
}

/// [RunTaskCaller] implementation using [flow_raft_api::client::FlowRaftClient].
#[derive(Clone)]
pub struct ClientRunTaskCaller {
    timeout: std::time::Duration,
}

impl ClientRunTaskCaller {
    /// Creates a caller with the given request timeout.
    pub fn new(timeout: std::time::Duration) -> Self {
        Self { timeout }
    }

    /// Creates a caller with a 60-second timeout.
    pub fn default_timeout() -> Self {
        Self::new(std::time::Duration::from_secs(60))
    }
}

#[tonic::async_trait]
impl RunTaskCaller for ClientRunTaskCaller {
    async fn run_task_on(
        &self,
        endpoint: &str,
        workflow_id: WorkflowId,
        task_id: flow_raft_core::TaskId,
        handler_name: &str,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let client =
            flow_raft_api::client::FlowRaftClient::new(endpoint).with_timeout(self.timeout);
        client
            .run_task_on(endpoint, workflow_id, task_id, handler_name, inputs)
            .await
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_only_router_returns_none() {
        let r = LocalOnlyTaskRouter;
        let wf = WorkflowId::default();
        assert!(r.route(&wf, "h1").is_none());
        assert!(r.route(&wf, "any").is_none());
    }

    #[test]
    fn map_router_empty_returns_none() {
        let r = MapTaskRouter::new();
        let wf = WorkflowId::default();
        assert!(r.route(&wf, "h1").is_none());
    }

    #[test]
    fn map_router_add_route_and_route() {
        let wf = WorkflowId::default();
        let mut m = MapTaskRouter::new();
        m.add_route(wf.as_ref(), "h1", "http://127.0.0.1:50052");
        assert_eq!(
            m.route(&wf, "h1").as_deref(),
            Some("http://127.0.0.1:50052")
        );
        assert!(m.route(&wf, "h2").is_none());
    }

    #[test]
    fn map_router_from_routes() {
        let wf = WorkflowId::default();
        let m = MapTaskRouter::from_routes([(wf, "h1".to_string(), "http://a".to_string())]);
        assert_eq!(m.route(&wf, "h1").as_deref(), Some("http://a"));
    }

    #[test]
    fn client_run_task_caller_default_timeout() {
        let _ = ClientRunTaskCaller::default_timeout();
    }

    #[test]
    fn client_run_task_caller_new() {
        let _ = ClientRunTaskCaller::new(std::time::Duration::from_secs(30));
    }
}
