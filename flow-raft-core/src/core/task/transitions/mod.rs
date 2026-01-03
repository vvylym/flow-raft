//! Task state transitions with built-in validation
//!
//! This module contains state-based transition implementations for tasks.
//! Each file contains transitions from a specific source state, making it
//! easy to understand and test each transition in isolation.

mod from_failed;
mod from_pending;
mod from_running;
mod from_scheduled;

// Note: impl blocks are automatically available when the types are imported.
// No need to re-export them explicitly.
