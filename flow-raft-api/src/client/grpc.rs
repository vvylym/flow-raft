//! gRPC client implementation for FlowRaft
//!
//! Provides low-level gRPC client for connecting to FlowRaft servers.

use std::time::Duration;

use tonic::transport::{Channel, Endpoint};

use crate::client::ClientError;

/// gRPC client for FlowRaft service
pub struct GrpcClient {
    /// Tonic channel to the server
    channel: Channel,
    /// Request timeout
    timeout: Duration,
}

impl GrpcClient {
    /// Create a new gRPC client
    ///
    /// # Arguments
    /// * `endpoint` - Server endpoint (e.g., "http://localhost:50051")
    /// * `timeout` - Request timeout
    pub async fn new(endpoint: impl Into<String>, timeout: Duration) -> Result<Self, ClientError> {
        let endpoint_str = endpoint.into();
        let endpoint = Endpoint::from_shared(endpoint_str.clone())
            .map_err(|e| {
                ClientError::Connection(format!("Invalid endpoint '{}': {}", endpoint_str, e))
            })?
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(5));

        let channel = endpoint.connect().await.map_err(|e| {
            ClientError::Connection(format!("Failed to connect to '{}': {}", endpoint_str, e))
        })?;

        Ok(Self { channel, timeout })
    }

    /// Get the underlying channel
    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    /// Get the timeout
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}
