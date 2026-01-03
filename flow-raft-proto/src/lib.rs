//! Protocol buffer definitions for FlowRaft gRPC service
//!
//! This crate contains the generated Rust code from the FlowRaft protocol buffer
//! definitions. Both the server and client crates depend on this crate to share
//! the same proto definitions.

#[allow(missing_docs)]
pub mod proto {
    #![allow(clippy::all, missing_docs)]
    // Include the generated proto code
    include!(concat!(env!("OUT_DIR"), "/flowraft.rs"));
}
