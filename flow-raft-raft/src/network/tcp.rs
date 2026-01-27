//! TCP-based Raft network transport for production deployments.
//!
//! Provides a real network implementation that sends AppendEntries, Vote, and
//! InstallSnapshot RPCs over TCP with length-delimited rkyv framing. OpenRaft
//! types are serialized as JSON (serde) and the resulting bytes are rkyv-encoded.
//! Use [TcpNetworkFactory] when building nodes and [TcpRaftRpcServer] to accept
//! incoming RPCs on each node.

use std::collections::BTreeMap;
use std::io;
use std::net::SocketAddr;

use rkyv::{from_bytes, to_bytes};

use openraft::BasicNode;
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
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

use crate::types::{NodeId, TypeConfig};

const TAG_APPEND_ENTRIES: u8 = 1;
const TAG_VOTE: u8 = 2;
const TAG_INSTALL_SNAPSHOT: u8 = 3;

const STATUS_OK: u8 = 0;
const STATUS_ERR: u8 = 1;

/// Wire response for TCP RPC: success variants or error string.
#[derive(serde::Serialize, serde::Deserialize)]
enum WireResponse {
    AppendEntries(AppendEntriesResponse<NodeId>),
    Vote(VoteResponse<NodeId>),
    InstallSnapshot(InstallSnapshotResponse<NodeId>),
    Err(String),
}

/// TCP-based Raft network factory for production.
///
/// Uses [BasicNode::addr] from the membership when creating a client to each peer.
/// Each node must run [TcpRaftRpcServer] bound to its advertised address.
#[derive(Debug, Clone, Default)]
pub struct TcpNetworkFactory;

impl TcpNetworkFactory {
    /// Create a new TCP network factory.
    pub fn new() -> Self {
        Self
    }
}

impl RaftNetworkFactory<TypeConfig> for TcpNetworkFactory {
    type Network = TcpNetwork;

    async fn new_client(&mut self, _target: NodeId, node: &BasicNode) -> Self::Network {
        TcpNetwork {
            addr: node.addr.clone(),
        }
    }
}

/// TCP network connection to a single peer.
#[derive(Debug)]
pub struct TcpNetwork {
    addr: String,
}

impl TcpNetwork {
    async fn connect(&self) -> io::Result<TcpStream> {
        let stream = TcpStream::connect(&self.addr).await?;
        stream.set_nodelay(true)?;
        Ok(stream)
    }

    async fn rpc<Req, Res, F>(
        &mut self,
        tag: u8,
        req: Req,
        _option: RPCOption,
        decode: F,
    ) -> Result<Res, RPCError<NodeId, BasicNode, RaftError<NodeId>>>
    where
        Req: serde::Serialize,
        F: FnOnce(WireResponse) -> Option<Res>,
    {
        let json = serde_json::to_vec(&req).map_err(|e| {
            RPCError::Unreachable(Unreachable::new(&io::Error::new(
                io::ErrorKind::InvalidData,
                e,
            )))
        })?;
        let body = to_bytes::<rancor::Error>(&json).map_err(|e| {
            RPCError::Unreachable(Unreachable::new(&io::Error::new(
                io::ErrorKind::InvalidData,
                e,
            )))
        })?;
        let len = body.len() as u32;

        let mut stream = self
            .connect()
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;

        stream
            .write_all(&[tag])
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;
        stream
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;
        stream
            .write_all(&body)
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;
        stream
            .flush()
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;

        let mut status = [0u8; 1];
        stream
            .read_exact(&mut status)
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut body = vec![0u8; len];
        stream
            .read_exact(&mut body)
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;

        let json: Vec<u8> = from_bytes::<Vec<u8>, rancor::Error>(&body).map_err(|e| {
            RPCError::Unreachable(Unreachable::new(&io::Error::new(
                io::ErrorKind::InvalidData,
                e,
            )))
        })?;
        let resp: WireResponse = serde_json::from_slice(&json).map_err(|e| {
            RPCError::Unreachable(Unreachable::new(&io::Error::new(
                io::ErrorKind::InvalidData,
                e,
            )))
        })?;

        if status[0] == STATUS_ERR {
            let msg = match resp {
                WireResponse::Err(s) => s,
                _ => "remote error".to_string(),
            };
            return Err(RPCError::Unreachable(Unreachable::new(&io::Error::other(
                msg,
            ))));
        }

        decode(resp).ok_or_else(|| {
            RPCError::Unreachable(Unreachable::new(&io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected response type",
            )))
        })
    }
}

impl openraft::RaftNetwork<TypeConfig> for TcpNetwork {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<TypeConfig>,
        option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        self.rpc(TAG_APPEND_ENTRIES, rpc, option, |w| {
            if let WireResponse::AppendEntries(r) = w {
                Some(r)
            } else {
                None
            }
        })
        .await
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<NodeId>,
        option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        self.rpc(TAG_VOTE, rpc, option, |w| {
            if let WireResponse::Vote(r) = w {
                Some(r)
            } else {
                None
            }
        })
        .await
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<NodeId>,
        RPCError<NodeId, BasicNode, RaftError<NodeId, openraft::error::InstallSnapshotError>>,
    > {
        let json = serde_json::to_vec(&rpc).map_err(|e| {
            RPCError::Unreachable(Unreachable::new(&io::Error::new(
                io::ErrorKind::InvalidData,
                e,
            )))
        })?;
        let body = to_bytes::<rancor::Error>(&json).map_err(|e| {
            RPCError::Unreachable(Unreachable::new(&io::Error::new(
                io::ErrorKind::InvalidData,
                e,
            )))
        })?;
        let len = body.len() as u32;
        let mut stream = self
            .connect()
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;
        stream
            .write_all(&[TAG_INSTALL_SNAPSHOT])
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;
        stream
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;
        stream
            .write_all(&body)
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;
        stream
            .flush()
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;
        let mut status = [0u8; 1];
        stream
            .read_exact(&mut status)
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut body = vec![0u8; len];
        stream
            .read_exact(&mut body)
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;
        let json: Vec<u8> = from_bytes::<Vec<u8>, rancor::Error>(&body).map_err(|e| {
            RPCError::Unreachable(Unreachable::new(&io::Error::new(
                io::ErrorKind::InvalidData,
                e,
            )))
        })?;
        let resp: WireResponse = serde_json::from_slice(&json).map_err(|e| {
            RPCError::Unreachable(Unreachable::new(&io::Error::new(
                io::ErrorKind::InvalidData,
                e,
            )))
        })?;
        if status[0] == STATUS_ERR {
            let msg = match resp {
                WireResponse::Err(s) => s,
                _ => "remote error".to_string(),
            };
            return Err(RPCError::Unreachable(Unreachable::new(&io::Error::other(
                msg,
            ))));
        }
        match resp {
            WireResponse::InstallSnapshot(r) => Ok(r),
            _ => Err(RPCError::Unreachable(Unreachable::new(&io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected response type",
            )))),
        }
    }
}

/// RPC server that accepts TCP connections and dispatches Raft RPCs to a local [openraft::Raft].
///
/// Run [TcpRaftRpcServer::run] on each node, bound to the same address used in the
/// membership [BasicNode::addr] for that node.
pub struct TcpRaftRpcServer {
    raft: std::sync::Arc<openraft::Raft<TypeConfig>>,
    bind: SocketAddr,
}

impl TcpRaftRpcServer {
    /// Create a new TCP Raft RPC server.
    pub fn new(raft: std::sync::Arc<openraft::Raft<TypeConfig>>, bind: SocketAddr) -> Self {
        Self { raft, bind }
    }

    /// Run the accept loop. Returns when the listener is closed or an unrecoverable error occurs.
    pub async fn run(self) -> io::Result<()> {
        let listener = TcpListener::bind(self.bind).await?;
        tracing::info!(addr = %self.bind, "Raft RPC server listening");
        loop {
            let (stream, _) = listener.accept().await?;
            let raft = std::sync::Arc::clone(&self.raft);
            tokio::spawn(async move {
                if let Err(e) = serve_connection(raft, stream).await {
                    tracing::debug!(error = %e, "Raft RPC connection error");
                }
            });
        }
    }

    /// Spawn the server in the background and return a handle that aborts when dropped.
    /// For a long-lived server, keep the handle or call [TcpRaftRpcServer::run] instead.
    pub fn spawn(self) -> tokio::task::JoinHandle<io::Result<()>> {
        tokio::spawn(self.run())
    }
}

async fn serve_connection(
    raft: std::sync::Arc<openraft::Raft<TypeConfig>>,
    stream: TcpStream,
) -> io::Result<()> {
    stream.set_nodelay(true)?;
    let (mut rd, mut wr) = stream.into_split();

    loop {
        let mut tag = [0u8; 1];
        if rd.read_exact(&mut tag).await.is_err() {
            break;
        }
        let mut len_buf = [0u8; 4];
        if rd.read_exact(&mut len_buf).await.is_err() {
            break;
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut body = vec![0u8; len];
        if rd.read_exact(&mut body).await.is_err() {
            break;
        }

        let (status, resp): (u8, WireResponse) = match tag[0] {
            TAG_APPEND_ENTRIES => {
                let json: Vec<u8> = match from_bytes::<Vec<u8>, rancor::Error>(&body) {
                    Ok(t) => t,
                    Err(e) => {
                        let _ =
                            write_response(&mut wr, STATUS_ERR, &WireResponse::Err(e.to_string()))
                                .await;
                        continue;
                    }
                };
                let req: AppendEntriesRequest<TypeConfig> = match serde_json::from_slice(&json) {
                    Ok(t) => t,
                    Err(e) => {
                        let _ =
                            write_response(&mut wr, STATUS_ERR, &WireResponse::Err(e.to_string()))
                                .await;
                        continue;
                    }
                };
                match raft.append_entries(req).await {
                    Ok(r) => (STATUS_OK, WireResponse::AppendEntries(r)),
                    Err(e) => (STATUS_ERR, WireResponse::Err(e.to_string())),
                }
            }
            TAG_VOTE => {
                let json: Vec<u8> = match from_bytes::<Vec<u8>, rancor::Error>(&body) {
                    Ok(t) => t,
                    Err(e) => {
                        let _ =
                            write_response(&mut wr, STATUS_ERR, &WireResponse::Err(e.to_string()))
                                .await;
                        continue;
                    }
                };
                let req: VoteRequest<NodeId> = match serde_json::from_slice(&json) {
                    Ok(t) => t,
                    Err(e) => {
                        let _ =
                            write_response(&mut wr, STATUS_ERR, &WireResponse::Err(e.to_string()))
                                .await;
                        continue;
                    }
                };
                match raft.vote(req).await {
                    Ok(r) => (STATUS_OK, WireResponse::Vote(r)),
                    Err(e) => (STATUS_ERR, WireResponse::Err(e.to_string())),
                }
            }
            TAG_INSTALL_SNAPSHOT => {
                let json: Vec<u8> = match from_bytes::<Vec<u8>, rancor::Error>(&body) {
                    Ok(t) => t,
                    Err(e) => {
                        let _ =
                            write_response(&mut wr, STATUS_ERR, &WireResponse::Err(e.to_string()))
                                .await;
                        continue;
                    }
                };
                let req: InstallSnapshotRequest<TypeConfig> = match serde_json::from_slice(&json) {
                    Ok(t) => t,
                    Err(e) => {
                        let _ =
                            write_response(&mut wr, STATUS_ERR, &WireResponse::Err(e.to_string()))
                                .await;
                        continue;
                    }
                };
                match raft.install_snapshot(req).await {
                    Ok(r) => (STATUS_OK, WireResponse::InstallSnapshot(r)),
                    Err(e) => (STATUS_ERR, WireResponse::Err(e.to_string())),
                }
            }
            _ => {
                let _ = write_response(
                    &mut wr,
                    STATUS_ERR,
                    &WireResponse::Err("unknown tag".to_string()),
                )
                .await;
                continue;
            }
        };

        if write_response(&mut wr, status, &resp).await.is_err() {
            break;
        }
    }
    Ok(())
}

async fn write_response(
    wr: &mut tokio::net::tcp::OwnedWriteHalf,
    status: u8,
    resp: &WireResponse,
) -> io::Result<()> {
    let json =
        serde_json::to_vec(resp).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let body = to_bytes::<rancor::Error>(&json)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let len = body.len() as u32;
    wr.write_all(&[status]).await?;
    wr.write_all(&len.to_be_bytes()).await?;
    wr.write_all(&body).await?;
    wr.flush().await?;
    Ok(())
}

/// Helper to build a [BTreeMap] of [NodeId] to [BasicNode] for [openraft::Raft::initialize]
/// when using TCP. Each entry’s [BasicNode::addr] must be the "host:port" where that node’s
/// [TcpRaftRpcServer] is listening.
pub fn tcp_nodes(
    entries: impl IntoIterator<Item = (NodeId, String)>,
) -> BTreeMap<NodeId, BasicNode> {
    entries
        .into_iter()
        .map(|(id, addr)| (id, BasicNode { addr }))
        .collect()
}
