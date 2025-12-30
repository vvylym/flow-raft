//! Raft storage implementations
//!
//! Provides log storage and state machine implementations for OpenRaft.

pub mod log_store;
pub mod state_machine;

pub use log_store::LogStore;
pub use state_machine::{StateMachineData, StateMachineStore};
