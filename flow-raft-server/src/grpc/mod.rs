//! gRPC service for FlowRaft
//!
//! Provides gRPC API endpoints for workflow management and observability.

pub mod run_on_cluster;
pub mod service;
pub mod types;

pub use flow_raft_proto::proto::flow_raft_service_server::{
    FlowRaftService, FlowRaftServiceServer,
};
pub use run_on_cluster::{RunGrpcOnClusterError, run_grpc_on_cluster};
pub use service::FlowRaftServiceImpl;
