//! Retry policy and failure handling module
//!
//! This module provides retry configuration and failure kind classification
//! for tasks that may need to be retried.

mod config;
mod error;

pub use config::{FailureKind, RetryConfig};
pub use error::MaxRetriesExceededError;
