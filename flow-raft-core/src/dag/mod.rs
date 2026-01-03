//! DAG operations module
//!
//! This module provides shared DAG (Directed Acyclic Graph) operations used by
//! both task and workflow modules. It includes dependency tracking and DAG
//! validation utilities.

mod dependencies;
mod utils;

pub use dependencies::TaskDependencies;
pub use utils::{ready_tasks, topological_order, validate_dag};
