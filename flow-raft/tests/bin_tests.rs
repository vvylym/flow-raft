//! Binary tests: flowraft-node and flowraft (--help, minimal invoke, health).

use std::process::{Child, Command as StdCommand};
use std::time::Duration;

use assert_cmd::cargo::cargo_bin_cmd;
use portpicker::pick_unused_port;
use predicates::prelude::*;

#[test]
fn flowraft_node_help() {
    let mut c = cargo_bin_cmd!("flowraft-node");
    c.arg("--help");
    c.assert()
        .success()
        .stdout(predicate::str::contains("flowraft-node"));
}

#[test]
fn flowraft_node_serve_help() {
    let mut c = cargo_bin_cmd!("flowraft-node");
    c.args(["serve", "--help"]);
    c.assert()
        .success()
        .stdout(predicate::str::contains("--id"));
}

#[test]
fn flowraft_node_serve_fails_without_required() {
    let mut c = cargo_bin_cmd!("flowraft-node");
    c.args(["serve"]);
    c.assert().failure();
}

#[test]
fn flowraft_help() {
    let mut c = cargo_bin_cmd!("flowraft");
    c.arg("--help");
    c.assert()
        .success()
        .stdout(predicate::str::contains("flowraft"));
}

#[test]
fn flowraft_workflow_help() {
    let mut c = cargo_bin_cmd!("flowraft");
    c.args(["workflow", "--help"]);
    c.assert().success();
}

#[tokio::test]
async fn flowraft_node_serve_health_responds() {
    let http_port = pick_unused_port().expect("free port");
    let raft_port = pick_unused_port().expect("free port");
    let grpc_port = pick_unused_port().expect("free port");

    let bin = std::env::var("CARGO_BIN_EXE_flowraft_node").unwrap_or_else(|_| {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("..");
        p.push("target");
        p.push("debug");
        p.push("flowraft-node");
        p.to_string_lossy().into_owned()
    });
    let child = StdCommand::new(&bin)
        .args([
            "serve",
            "--id",
            "1",
            "--bootstrap",
            "--raft",
            &format!("127.0.0.1:{}", raft_port),
            "--grpc",
            &format!("127.0.0.1:{}", grpc_port),
            "--http",
            &format!("127.0.0.1:{}", http_port),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn flowraft-node");

    let _guard = KillOnDrop(child);

    tokio::time::sleep(Duration::from_secs(2)).await;

    let url = format!("http://127.0.0.1:{}/health", http_port);
    let resp = reqwest::get(&url).await.expect("GET /health");
    assert_eq!(resp.status(), 200, "GET /health should return 200");
}

struct KillOnDrop(Child);
impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}
