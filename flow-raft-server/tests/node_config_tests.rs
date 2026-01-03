//! Tests for node configuration

use flow_raft_server::node::config::{NetworkConfig, NodeConfig, NodeMode};

#[test]
fn test_node_config_new() {
    let config = NodeConfig::new(1, NodeMode::Leader);
    // Verify config is created
    assert_eq!(config.node_id, 1);
    assert_eq!(config.mode, NodeMode::Leader);
}

#[test]
fn test_node_mode_variants() {
    let _leader = NodeMode::Leader;
    let _follower = NodeMode::Follower;
    let _auto = NodeMode::Auto;
    // Verify all variants can be created
}

#[test]
fn test_node_config_with_raft_config() {
    let config = NodeConfig::new(1, NodeMode::Leader)
        .with_raft_config(flow_raft_raft::config::default_config());
    assert_eq!(config.node_id, 1);
}

#[test]
fn test_node_config_with_network_config() {
    let network_config = NetworkConfig {
        address: Some("127.0.0.1".to_string()),
        port: Some(8080),
    };
    let config = NodeConfig::new(1, NodeMode::Leader).with_network_config(network_config);
    assert!(config.network_config.address.is_some());
}
