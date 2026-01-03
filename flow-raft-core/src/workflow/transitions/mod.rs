//! Workflow state transitions with built-in validation
//!
//! This module contains state-based transition implementations for workflows.
//! Each file contains transitions from a specific source state, making it
//! easy to understand and test each transition in isolation.

mod from_draft;
mod from_paused;
mod from_running;
mod from_scheduled;

// Note: impl blocks are automatically available when the types are imported.
// No need to re-export them explicitly.
