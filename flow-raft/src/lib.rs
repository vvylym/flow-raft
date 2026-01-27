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
    network::{MemoryNetworkFactory, TcpNetworkFactory, TcpRaftRpcServer, tcp_nodes},
    storage::{LogStore, StateMachineStore},
    types::Request,
};

// Re-export API types (single GraphBuilder, builder pattern; use node/condition/split/merge/switch)
pub use flow_raft_api::{
    client::{FlowRaftClient, WorkflowExecutionId, WorkflowStatus},
    graph::builder::{ConditionObject, MergeObject, SplitObject},
    graph::{GraphBuilder, NodeName, graph_to_workflow},
    workflow::WorkflowDef,
};

#[doc = "Graph builder API module"]
pub mod graph {
    pub use flow_raft_api::graph::*;
}

mod typed_handlers;
pub use typed_handlers::register_typed_graph_handlers;

#[doc = "Handler registry and executor module"]
pub mod handlers {
    pub use flow_raft_server::handlers::*;
}

// Re-export observability types
pub use flow_raft_observability::*;

// Re-export server types (with specific exports to avoid conflicts)
pub use flow_raft_server::handlers::{HandlerExecutor, HandlerRegistry};

/// Prelude module for convenient imports
///
/// Use `use flow_raft::prelude::*;` to import all commonly used types.
pub mod prelude {
    // Core types
    pub use flow_raft_core::{
        RetryConfig, TaskExecution, TaskId, TaskState, WorkflowId, WorkflowSnapshot, WorkflowState,
    };

    // Graph building: single GraphBuilder, builder pattern; wrap fns with node/condition/split/merge/switch
    pub use crate::register_typed_graph_handlers;
    pub use flow_raft_api::graph::builder::{
        ConditionObject, MergeObject, NodeFunction, SplitObject,
    };
    pub use flow_raft_api::graph::{
        GraphBuilder, NodeName, TypedCondition, TypedGraph, TypedGraphBuilder, TypedMerge,
        TypedNodeFunction, TypedSplit, TypedSwitch, condition, graph_to_workflow, merge, node,
        node_async, node_async_ok, node_ok, split, switch,
    };

    // Workflow definition
    pub use flow_raft_api::workflow::WorkflowDef;

    // Client API
    pub use flow_raft_api::client::{
        ClientError, FlowRaftClient, FlowRaftClientBuilder, WorkflowExecutionId, WorkflowStatus,
    };

    // Raft/App
    pub use flow_raft_raft::config::default_config;
    pub use flow_raft_raft::{AppBuilderError, FlowRaftAppBuilder};
    pub use flow_raft_raft::{FlowRaftApp, FlowRaftNode, RaftConfig};

    // Executor
    pub use flow_raft_raft::executor::{TaskHandler, WorkflowExecutor};

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
