//! Raft configuration
//!
//! Provides configuration for Raft cluster settings.

use openraft::Config;

/// Raft configuration for FlowRaft
pub type RaftConfig = Config;

/// Create default Raft configuration
pub fn default_config() -> RaftConfig {
    Config {
        heartbeat_interval: 500,
        election_timeout_min: 1500,
        election_timeout_max: 3000,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = default_config();
        assert_eq!(config.heartbeat_interval, 500);
        assert_eq!(config.election_timeout_min, 1500);
        assert_eq!(config.election_timeout_max, 3000);
    }
}
