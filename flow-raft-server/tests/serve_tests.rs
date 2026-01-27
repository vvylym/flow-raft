//! Tests for serve: build_components and run_bootstrap.

use std::net::SocketAddr;

use flow_raft_server::serve::{ServeConfigBuilder, build_components, run_bootstrap};

#[tokio::test]
async fn build_components_single_node_config() {
    let grpc: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let http: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let raft: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServeConfigBuilder::new()
        .with_node_id(1)
        .with_grpc_bind(grpc)
        .with_http_bind(http)
        .with_raft_bind(raft)
        .with_peers(vec![])
        .with_bootstrap(true)
        .build()
        .unwrap();

    let components = build_components(&config).await.unwrap();
    run_bootstrap(&config, &components.node).await.unwrap();

    components.raft_rpc_handle.abort();
    let _ = components.raft_rpc_handle.await;
}
