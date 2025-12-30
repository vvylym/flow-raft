//! Node configuration for FlowRaft
//!
//! Provides configuration structures for launching nodes.

use std::path::PathBuf;

use crate::raft::config::RaftConfig;
use crate::raft::types::NodeId;

/// Node operation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeMode {
    /// Leader node (can accept writes)
    Leader,
    /// Follower node (read-only, replicates from leader)
    Follower,
    /// Auto mode (determined by Raft consensus)
    Auto,
}

/// Network configuration
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Node address (for future network implementation)
    pub address: Option<String>,
    /// Port (for future network implementation)
    pub port: Option<u16>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            address: None,
            port: None,
        }
    }
}

/// Node configuration
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Node ID
    pub node_id: NodeId,
    /// Node operation mode
    pub mode: NodeMode,
    /// Raft configuration
    pub raft_config: RaftConfig,
    /// Network configuration
    pub network_config: NetworkConfig,
    /// Storage path (None for in-memory)
    pub storage_path: Option<PathBuf>,
}

impl NodeConfig {
    /// Creates a new node configuration
    pub fn new(node_id: NodeId, mode: NodeMode) -> Self {
        Self {
            node_id,
            mode,
            raft_config: RaftConfig::default(),
            network_config: NetworkConfig::default(),
            storage_path: None,
        }
    }

    /// Sets the Raft configuration
    pub fn with_raft_config(mut self, config: RaftConfig) -> Self {
        self.raft_config = config;
        self
    }

    /// Sets the network configuration
    pub fn with_network_config(mut self, config: NetworkConfig) -> Self {
        self.network_config = config;
        self
    }

    /// Sets the storage path
    pub fn with_storage_path(mut self, path: PathBuf) -> Self {
        self.storage_path = Some(path);
        self
    }
}
