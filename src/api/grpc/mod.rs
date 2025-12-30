//! gRPC service for FlowRaft
//!
//! Provides gRPC API endpoints for workflow management and observability.

pub mod service;
pub mod types;

pub use service::FlowRaftServiceImpl;
pub use types::proto::flow_raft_service_server::{FlowRaftService, FlowRaftServiceServer};
