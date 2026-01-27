//! Shared workflow definitions used by examples and benchmarks.
//!
//! Each workflow has a clear input/output contract and test cases that assert
//! different inputs produce the expected outputs. Use these in examples and
//! benches instead of ad-hoc inline workflows.

pub mod bench_workflows;
pub mod order_complex;
pub mod order_conditional;
pub mod order_parallel;
pub mod order_pipeline;

pub use bench_workflows::{conditional_nop_graph, linear_nop_graph, parallel_nop_graph};
pub use order_complex::{
    MergeResult, OrderInput, RejectResult, merge_result_from_snapshot, order_complex_cases,
    order_complex_graph, reject_result_from_snapshot,
};
pub use order_conditional::{
    OrderValid, ProcessedOrder, RejectedOrder, order_conditional_cases, order_conditional_graph,
    processed_from_snapshot, rejected_from_snapshot,
};
pub use order_parallel::{
    OrderItems, OrderResult, order_parallel_cases, order_parallel_graph, order_result_from_snapshot,
};
pub use order_pipeline::{
    Order, Receipt, order_pipeline_cases, order_pipeline_graph, receipt_from_snapshot,
};
