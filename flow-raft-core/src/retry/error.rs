//! Error types for the retry module

use thiserror::Error;

/// Maximum retries exceeded
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("max retries exceeded: {current_attempt}/{max_attempts}")]
pub struct MaxRetriesExceededError {
    /// Maximum number of attempts allowed
    max_attempts: u8,
    /// Current attempt number
    current_attempt: u8,
}

impl MaxRetriesExceededError {
    /// Constructor
    pub fn new(max_attempts: u8, current_attempt: u8) -> Self {
        Self {
            max_attempts,
            current_attempt,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_error_max_retries_exceeded() {
        let error = MaxRetriesExceededError::new(3, 3);
        let message = format!("{}", error);
        assert!(message.contains("max retries exceeded"));
        assert!(message.contains("3/3"));
    }
}
