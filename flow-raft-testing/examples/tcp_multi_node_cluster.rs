//! Multi-node Raft cluster over TCP
//!
//! Demonstrates production-style deployment: 3 nodes communicating via
//! TCP instead of in-memory transport. Each node runs [TcpRaftRpcServer]
//! and uses [TcpNetworkFactory]; [tcp_nodes] supplies peer addresses for
//! [initialize_cluster_with_nodes].
//!
//! **Note:** This example forms a cluster and prints the first node's Raft metrics
//! (leader, state). It does not assert a particular leader or run workflows.
//! Correctness: "cluster initializes and nodes reach a defined state."

use flow_raft::{
    FlowRaftNode, LogStore, StateMachineStore, TcpNetworkFactory, TcpRaftRpcServer, default_config,
    tcp_nodes,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let addrs = ["127.0.0.1:5010", "127.0.0.1:5011", "127.0.0.1:5012"];
    let bind_addrs: Vec<std::net::SocketAddr> = [
        "0.0.0.0:5010".parse()?,
        "0.0.0.0:5011".parse()?,
        "0.0.0.0:5012".parse()?,
    ]
    .into();

    let config = default_config();
    let mut rafts = Vec::new();
    for (i, id) in (1u64..=3).enumerate() {
        let log_store = LogStore::default();
        let state_machine = StateMachineStore::default();
        let network = TcpNetworkFactory::new();
        let node = FlowRaftNode::new(id, config.clone(), network, log_store, state_machine).await?;
        let server = TcpRaftRpcServer::new(node.raft.clone(), bind_addrs[i]);
        server.spawn();
        rafts.push((id, node));
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    let nodes_map = tcp_nodes([
        (1, addrs[0].into()),
        (2, addrs[1].into()),
        (3, addrs[2].into()),
    ]);
    rafts[0].1.initialize_cluster_with_nodes(nodes_map).await?;

    tokio::time::sleep(Duration::from_secs(2)).await;
    if let Some((id, n)) = rafts.first() {
        let metrics = n.raft.metrics();
        let m = metrics.borrow();
        println!(
            "Node {}: leader={:?} state={:?}",
            id, m.current_leader, m.state
        );
    }
    println!("TCP multi-node cluster example completed.");
    Ok(())
}
