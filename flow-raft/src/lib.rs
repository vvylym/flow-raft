//! FlowRaft
//!
//! A distributed, stateful workflow engine optimized for correctness,
//! fault tolerance, and deterministic execution.
//!
//! This is the main facade crate that re-exports all public APIs from
//! the FlowRaft workspace crates.

// Re-export core types
pub use flow_raft_core::*;

// Re-export raft types (with specific exports to avoid conflicts)
pub use flow_raft_raft::{
    FlowRaftApp, FlowRaftNode, RaftConfig, TypeConfig,
    config::default_config,
    executor::{TaskHandler, WorkflowExecutor},
    network::MemoryNetworkFactory,
    storage::{LogStore, StateMachineStore},
    types::Request,
};

// Re-export API types
pub use flow_raft_api::{
    client::{FlowRaftClient, WorkflowExecutionId, WorkflowStatus},
    graph::builder::{ConditionObject, MergeObject, SplitObject},
    graph::{GraphBuilder, NodeName, graph_to_workflow},
    workflow::{WorkflowBuilder, WorkflowDef},
};

#[doc = "Graph builder API module"]
pub mod graph {
    pub use flow_raft_api::graph::*;
}

#[doc = "Handler registry and executor module"]
pub mod handlers {
    pub use flow_raft_server::handlers::*;
}

// Re-export observability types
pub use flow_raft_observability::*;

// Re-export server types (with specific exports to avoid conflicts)
pub use flow_raft_server::{
    cluster::{ClusterNode, ClusterStatus, NodeRole},
    handlers::{HandlerExecutor, HandlerRegistry},
    node::launcher::{launch_cluster_node, launch_single_node, start_metrics_server},
    node::{NodeConfig, NodeMode},
};

/// Prelude module for convenient imports
///
/// Use `use flow_raft::prelude::*;` to import all commonly used types.
pub mod prelude {
    // Core types
    pub use flow_raft_core::{
        RetryConfig, TaskExecution, TaskId, TaskState, WorkflowId, WorkflowSnapshot, WorkflowState,
    };

    // Graph building
    pub use flow_raft_api::graph::builder::{
        ConditionObject, MergeObject, NodeFunction, SplitObject, wrap_function,
    };
    pub use flow_raft_api::graph::{GraphBuilder, NodeName, graph_to_workflow};

    // Workflow definition
    pub use flow_raft_api::workflow::{WorkflowBuilder, WorkflowDef};

    // Client API
    pub use flow_raft_api::client::{
        ClientError, FlowRaftClient, WorkflowExecutionId, WorkflowStatus,
    };

    // Raft/App
    pub use flow_raft_raft::app::builder::{AppBuilderError, FlowRaftAppBuilder};
    pub use flow_raft_raft::config::default_config;
    pub use flow_raft_raft::{FlowRaftApp, FlowRaftNode, RaftConfig};

    // Executor
    pub use flow_raft_raft::executor::{TaskHandler, WorkflowExecutor};

    // Cluster/Node
    pub use flow_raft_server::cluster::{ClusterNode, ClusterStatus, NodeRole};
    pub use flow_raft_server::node::launcher::{
        launch_cluster, launch_cluster_node, launch_single_node,
    };
    pub use flow_raft_server::node::{NodeConfig, NodeMode};

    // Handlers
    pub use flow_raft_server::handlers::{HandlerExecutor, HandlerRegistry};

    // Storage
    pub use flow_raft_raft::storage::{LogStore, StateMachineStore};

    // Network
    pub use flow_raft_raft::network::MemoryNetworkFactory;

    // Observability
    pub use flow_raft_observability::*;

    // Common async/error types
    pub use serde_json::Value;
    pub use std::sync::Arc;
}
