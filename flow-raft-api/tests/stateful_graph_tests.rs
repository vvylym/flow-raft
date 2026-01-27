//! Tests for StatefulGraphBuilder and stateful workflow graphs.

use flow_raft_api::graph::stateful::StatefulGraphBuilder;
use flow_raft_core::RetryConfig;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Counter {
    total: u64,
}

#[test]
fn stateful_builder_simple() {
    let mut b = StatefulGraphBuilder::<Counter>::new("stateful_simple");
    b.add_node("inc", |_: (), _s: &Counter| Ok::<(), String>(()), None)
        .set_root("inc");
    let g = b.build();
    assert!(g.is_ok());
    let g = g.unwrap();
    let wd = g.workflow_def("stateful_simple").unwrap();
    assert_eq!(wd.name(), "stateful_simple");
    assert_eq!(g.handlers().len(), 1);
    assert!(g.handlers().contains_key("fn_inc"));
}

#[test]
fn stateful_builder_with_condition() {
    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    struct S {
        flag: bool,
    }

    let mut b = StatefulGraphBuilder::<S>::new("stateful_cond");
    b.add_node("start", |_: (), _s: &S| Ok::<bool, String>(true), None)
        .add_node("then_n", |_: bool, _s: &S| Ok::<(), String>(()), None)
        .add_node("else_n", |_: bool, _s: &S| Ok::<(), String>(()), None)
        .add_conditional_edge("start", |_x: bool, s: &S| s.flag, "then_n", "else_n")
        .set_root("start");
    let g = b.build();
    assert!(g.is_ok());
    let g = g.unwrap();
    assert_eq!(g.handlers().len(), 3);
}

#[test]
fn stateful_builder_with_state_and_state_access() {
    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    struct S {
        n: u32,
    }
    let state = Arc::new(RwLock::new(S { n: 42 }));
    let mut b = StatefulGraphBuilder::<S>::with_name_and_state("named", state);
    let _handle = b.state();
    b.add_node("n", |_: (), s: &S| Ok::<u32, String>(s.n), None)
        .set_root("n");
    let g = b.build();
    assert!(g.is_ok());
    let g = g.unwrap();
    assert_eq!(g.handlers().len(), 1);
}

#[test]
fn stateful_builder_with_retry_config() {
    let mut b = StatefulGraphBuilder::<Counter>::new("retry_workflow")
        .with_retry_config(RetryConfig::new(3));
    b.add_node("inc", |_: (), _s: &Counter| Ok::<(), String>(()), None)
        .set_root("inc");
    let g = b.build();
    assert!(g.is_ok());
}

#[test]
fn stateful_builder_split_edge() {
    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    struct S {}
    let mut b = StatefulGraphBuilder::<S>::new("split_workflow");
    b.add_node("start", |_: (), _s: &S| Ok::<(), String>(()), None)
        .add_node("b1", |_: (), _s: &S| Ok::<(), String>(()), None)
        .add_node("b2", |_: (), _s: &S| Ok::<(), String>(()), None)
        .add_split_edge(
            "start",
            |_: (), _s: &S| Ok(vec!["b1".to_string(), "b2".to_string()]),
            vec!["b1", "b2"],
        )
        .set_root("start");
    let g = b.build();
    assert!(g.is_ok());
    let g = g.unwrap();
    assert_eq!(g.handlers().len(), 3);
}

#[test]
fn stateful_builder_merge_edge() {
    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    struct S {}
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct Out {
        n: usize,
    }
    let mut b = StatefulGraphBuilder::<S>::new("merge_workflow");
    b.add_node("start", |_: (), _s: &S| Ok::<(), String>(()), None)
        .add_node("a", |_: (), _s: &S| Ok::<(), String>(()), None)
        .add_node("b", |_: (), _s: &S| Ok::<(), String>(()), None)
        .add_node("merged", |_: Out, _s: &S| Ok::<(), String>(()), None)
        .add_simple_edge("start", "a")
        .add_simple_edge("start", "b")
        .add_merge_edge(
            vec!["a", "b"],
            |inputs: Vec<()>, _s: &S| Ok::<Out, String>(Out { n: inputs.len() }),
            "merged",
        )
        .set_root("start");
    let g = b.build();
    assert!(g.is_ok());
    let g = g.unwrap();
    assert_eq!(g.handlers().len(), 4);
}

#[test]
fn stateful_builder_switch_edge() {
    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    struct S {}
    let mut b = StatefulGraphBuilder::<S>::new("switch_workflow");
    b.add_node("start", |_: (), _s: &S| Ok::<i32, String>(0), None)
        .add_node("left", |_: i32, _s: &S| Ok::<(), String>(()), None)
        .add_node("right", |_: i32, _s: &S| Ok::<(), String>(()), None)
        .add_switch_edge(
            "start",
            |x: i32, _s: &S| {
                if x >= 0 {
                    "left".to_string()
                } else {
                    "right".to_string()
                }
            },
            vec!["left", "right"],
        )
        .set_root("start");
    let g = b.build();
    assert!(g.is_ok());
    let g = g.unwrap();
    assert_eq!(g.handlers().len(), 3);
}
