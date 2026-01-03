//! Macros for reducing code repetition in the core module
//!
//! This module provides declarative macros for generating:
//! - State types (marker types + enum + From impls)
//! - ID types (newtype wrappers with all trait impls)
//! - Transition implementations (optional, may be manual for clarity)
//!
//! Note: Macros are exported at the crate root using `#[macro_export]`,
//! so they should be used as `crate::define_id_type!` or `flow_raft::define_id_type!`.

mod id_types;
mod state_types;

// Macros are exported at crate root, so we don't re-export them here
// They can be used as crate::define_id_type! or flow_raft::define_id_type!
