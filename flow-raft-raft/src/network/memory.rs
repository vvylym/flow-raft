//! In-memory network implementation for testing and single-node mode.
//!
//! Forwards append_entries, vote, and install_snapshot to the target node's
//! Raft. Use [MemoryNetworkFactory::register_raft] after creating each node
//! so the network can route RPCs.

use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use futures::lock::Mutex;
use openraft::BasicNode;
use openraft::Raft;
use openraft::RaftTypeConfig;
use openraft::error::RPCError;
use openraft::error::RaftError;
use openraft::error::Unreachable;
use openraft::network::RPCOption;
use openraft::network::RaftNetworkFactory;
use openraft::raft::AppendEntriesRequest;
use openraft::raft::AppendEntriesResponse;
use openraft::raft::InstallSnapshotRequest;
use openraft::raft::InstallSnapshotResponse;
use openraft::raft::VoteRequest;
use openraft::raft::VoteResponse;

use crate::types::{NodeId, TypeConfig};

/// In-memory network for multi-node tests.
///
/// Register each node's Raft with [Self::register_raft] after creating the
/// node and before the leader sends append_entries.
#[derive(Debug, Clone, Default)]
pub struct MemoryNetworkFactory {
    nodes: Arc<Mutex<HashMap<NodeId, Arc<Raft<TypeConfig>>>>>,
}

impl MemoryNetworkFactory {
    /// Create a new memory network factory.
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a node's Raft so RPCs to this node can be forwarded.
    /// Call this after creating each [FlowRaftNode](crate::node::FlowRaftNode)
    /// and before the leader sends heartbeats.
    pub async fn register_raft(&self, id: NodeId, raft: Arc<Raft<TypeConfig>>) {
        let mut nodes = self.nodes.lock().await;
        nodes.insert(id, raft);
    }

    /// Remove a node from the network. Call when simulating node failure so
    /// RPCs to this node fail and the cluster can elect a new leader.
    pub async fn unregister_raft(&self, id: NodeId) {
        let mut nodes = self.nodes.lock().await;
        nodes.remove(&id);
    }
}

/// In-memory network connection to a target node.
#[derive(Debug)]
pub struct MemoryNetwork {
    pub(crate) target: NodeId,
    nodes: Arc<Mutex<HashMap<NodeId, Arc<Raft<TypeConfig>>>>>,
}

impl RaftNetworkFactory<TypeConfig> for MemoryNetworkFactory {
    type Network = MemoryNetwork;

    async fn new_client(&mut self, target: NodeId, _node: &BasicNode) -> Self::Network {
        MemoryNetwork {
            target,
            nodes: self.nodes.clone(),
        }
    }
}

fn to_rpc_error<E: std::fmt::Display>(
    e: E,
) -> RPCError<NodeId, openraft::BasicNode, RaftError<NodeId>> {
    let err = io::Error::other(e.to_string());
    RPCError::Unreachable(Unreachable::new(&err))
}

impl openraft::RaftNetwork<TypeConfig> for MemoryNetwork {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<
        AppendEntriesResponse<<TypeConfig as RaftTypeConfig>::NodeId>,
        RPCError<
            <TypeConfig as RaftTypeConfig>::NodeId,
            <TypeConfig as RaftTypeConfig>::Node,
            RaftError<<TypeConfig as RaftTypeConfig>::NodeId>,
        >,
    > {
        let raft = {
            let guard = self.nodes.lock().await;
            guard.get(&self.target).cloned()
        };
        let raft = raft.ok_or_else(|| to_rpc_error("target node not registered"))?;
        raft.append_entries(rpc).await.map_err(to_rpc_error)
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<<TypeConfig as RaftTypeConfig>::NodeId>,
        _option: RPCOption,
    ) -> Result<
        VoteResponse<<TypeConfig as RaftTypeConfig>::NodeId>,
        RPCError<
            <TypeConfig as RaftTypeConfig>::NodeId,
            <TypeConfig as RaftTypeConfig>::Node,
            RaftError<<TypeConfig as RaftTypeConfig>::NodeId>,
        >,
    > {
        let raft = {
            let guard = self.nodes.lock().await;
            guard.get(&self.target).cloned()
        };
        let raft = raft.ok_or_else(|| to_rpc_error("target node not registered"))?;
        raft.vote(rpc).await.map_err(to_rpc_error)
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<<TypeConfig as RaftTypeConfig>::NodeId>,
        RPCError<
            <TypeConfig as RaftTypeConfig>::NodeId,
            <TypeConfig as RaftTypeConfig>::Node,
            RaftError<
                <TypeConfig as RaftTypeConfig>::NodeId,
                openraft::error::InstallSnapshotError,
            >,
        >,
    > {
        let raft = {
            let guard = self.nodes.lock().await;
            guard.get(&self.target).cloned()
        };
        let err = io::Error::other("target node not registered");
        let raft = raft.ok_or_else(|| RPCError::Unreachable(Unreachable::new(&err)))?;
        raft.install_snapshot(rpc).await.map_err(|e| {
            let err = io::Error::other(e.to_string());
            RPCError::Unreachable(Unreachable::new(&err))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_network_client() {
        let mut factory = MemoryNetworkFactory::new();
        let node = BasicNode {
            addr: "".to_string(),
        };
        let client = RaftNetworkFactory::new_client(&mut factory, 1, &node).await;
        assert_eq!(client.target, 1);
    }
}
