//! Builder for FlowRaft client
//!
//! Provides a fluent API for configuring FlowRaft clients.

use std::time::Duration;

use crate::client::FlowRaftClient;

/// Builder for FlowRaft client
pub struct FlowRaftClientBuilder {
    endpoint: Option<String>,
    timeout: Option<Duration>,
}

impl FlowRaftClientBuilder {
    /// Create a new client builder
    pub fn new() -> Self {
        Self {
            endpoint: None,
            timeout: None,
        }
    }

    /// Set the server endpoint
    ///
    /// # Arguments
    /// * `endpoint` - Server endpoint (e.g., "http://localhost:50051")
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Set the request timeout
    ///
    /// # Arguments
    /// * `timeout` - Request timeout (default: 5 minutes)
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Build the client
    ///
    /// # Returns
    /// The configured FlowRaft client
    pub fn build(self) -> FlowRaftClient {
        let endpoint = self
            .endpoint
            .unwrap_or_else(|| "http://localhost:50051".to_string());
        let timeout = self.timeout.unwrap_or(Duration::from_secs(300));

        FlowRaftClient::new(endpoint).with_timeout(timeout)
    }
}

impl Default for FlowRaftClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
