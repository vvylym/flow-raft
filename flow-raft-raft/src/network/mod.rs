//! Raft network layer
//!
//! Provides network implementations for Raft protocol communication.
//! - [MemoryNetworkFactory]: in-memory transport for single-node and tests.
//! - [TcpNetworkFactory] and [TcpRaftRpcServer]: TCP transport for production.

pub mod memory;
pub mod tcp;

pub use memory::MemoryNetworkFactory;
pub use tcp::{TcpNetworkFactory, TcpRaftRpcServer, tcp_nodes};
