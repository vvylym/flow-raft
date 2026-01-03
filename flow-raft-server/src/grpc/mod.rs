//! gRPC service for FlowRaft
//!
//! Provides gRPC API endpoints for workflow management and observability.

pub mod service;
pub mod types;

pub use flow_raft_proto::proto::flow_raft_service_server::{
    FlowRaftService, FlowRaftServiceServer,
};
pub use service::FlowRaftServiceImpl;
