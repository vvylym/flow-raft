//! Raft network layer
//!
//! Provides network implementations for Raft protocol communication.

pub mod memory;

pub use memory::MemoryNetworkFactory;
