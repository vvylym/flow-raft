//! Tests for TypedGraphBuilder and type-checked edges.

use flow_raft_api::graph::{TypedGraphBuilder, condition, node, node_ok, switch};
use flow_raft_core::RetryConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct A {
    x: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct B {
    y: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct C {
    z: bool,
}

fn a_to_b(a: A) -> Result<B, String> {
    Ok(B {
        y: format!("{}", a.x),
    })
}

fn b_to_c(b: B) -> Result<C, String> {
    Ok(C {
        z: b.y.parse::<i32>().is_ok(),
    })
}

fn b_to_c_wrong_input(_c: C) -> Result<C, String> {
    Ok(C { z: true })
}

#[test]
fn typed_builder_simple_edges_ok() {
    let mut b = TypedGraphBuilder::new("t");
    b.add_node("n1", node(a_to_b), None)
        .add_node("n2", node(b_to_c), None)
        .add_simple_edge("n1", "n2")
        .set_root("n1");
    let g = b.build();
    assert!(
        g.is_ok(),
        "build should succeed when output type matches input type"
    );
}

#[test]
fn typed_builder_edge_type_mismatch_fails() {
    let mut b = TypedGraphBuilder::new("t");
    b.add_node("n1", node(a_to_b), None)
        .add_node("n2", node(b_to_c_wrong_input), None)
        .add_simple_edge("n1", "n2")
        .set_root("n1");
    let g = b.build();
    assert!(
        g.is_err(),
        "build should fail when output type of source != input type of target"
    );
    let err = match g {
        Err(e) => e,
        Ok(_) => panic!("expected build to fail"),
    };
    assert!(
        err.contains("does not match") || err.contains("output type"),
        "error should mention type mismatch: {}",
        err
    );
}

#[test]
fn typed_builder_node_ok_infallible() {
    fn id(b: B) -> B {
        b
    }
    let mut b = TypedGraphBuilder::new("t");
    b.add_node("n1", node(a_to_b), None)
        .add_node("n2", node_ok(id), None)
        .add_simple_edge("n1", "n2")
        .set_root("n1");
    let g = b.build();
    assert!(g.is_ok());
}

#[test]
fn typed_builder_conditional_edge_ok() {
    fn cond(b: B) -> bool {
        !b.y.is_empty()
    }
    fn branch_then(_b: B) -> Result<C, String> {
        Ok(C { z: true })
    }
    fn branch_else(_b: B) -> Result<C, String> {
        Ok(C { z: false })
    }
    let mut b = TypedGraphBuilder::new("t");
    b.add_node("src", node(a_to_b), None)
        .add_node("then_b", node(branch_then), None)
        .add_node("else_b", node(branch_else), None)
        .add_conditional_edge("src", condition(cond), "then_b", "else_b")
        .set_root("src");
    let g = b.build();
    assert!(
        g.is_ok(),
        "conditional edge with matching types should build"
    );
}

#[test]
fn typed_graph_workflow_def_and_handlers() {
    let mut b = TypedGraphBuilder::new("t");
    b.add_node("n1", node(a_to_b), None)
        .add_node("n2", node(b_to_c), None)
        .add_simple_edge("n1", "n2")
        .set_root("n1");
    let tg = b.build().unwrap();
    let wd = tg.workflow_def("my_workflow").unwrap();
    assert_eq!(wd.name(), "my_workflow");
    let handlers = tg.handlers();
    assert_eq!(handlers.len(), 2);
    assert!(handlers.contains_key("fn_n1"));
    assert!(handlers.contains_key("fn_n2"));
}

#[test]
fn typed_builder_with_retry_config() {
    let mut b = TypedGraphBuilder::new("t").with_retry_config(RetryConfig::new(5));
    b.add_node("n1", node(a_to_b), None)
        .add_node("n2", node(b_to_c), None)
        .add_simple_edge("n1", "n2")
        .set_root("n1");
    let g = b.build();
    assert!(g.is_ok());
}

#[test]
fn typed_builder_switch_edge() {
    fn branch_left(_b: B) -> Result<C, String> {
        Ok(C { z: true })
    }
    fn branch_right(_b: B) -> Result<C, String> {
        Ok(C { z: false })
    }
    let mut b = TypedGraphBuilder::new("t");
    b.add_node("src", node(a_to_b), None)
        .add_node("left", node(branch_left), None)
        .add_node("right", node(branch_right), None)
        .add_switch_edge(
            "src",
            switch(|b: B| {
                if b.y.is_empty() {
                    "right".to_string()
                } else {
                    "left".to_string()
                }
            }),
            vec!["left", "right"],
        )
        .set_root("src");
    let g = b.build();
    assert!(g.is_ok());
    let tg = g.unwrap();
    assert_eq!(tg.handlers().len(), 3);
}
