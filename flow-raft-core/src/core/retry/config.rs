//! Retry configuration and failure kind
//!
//! Defines how tasks should be retried and what types of failures are retryable.

use crate::MaxRetriesExceededError;

/// Default maximum attempts
const RETRY_CONFIG_DEFAULT_MAX_ATTEMPTS: u8 = 3;

/// Default backoff factor
const RETRY_CONFIG_DEFAULT_BACKOFF_FACTOR: f64 = 2.0;

/// Default initial defay (milliseconds)
const RETRY_CONFIG_DEFAULT_INITIAL_DELAY_MS: u64 = 1000;

/// Classification of failure types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FailureKind {
    /// Retryable failure - the operation can be attempted again
    Retryable,
    /// Terminal failure - the operation should not be retried
    Terminal,
}

/// Retry configuration for a task
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u8,
    /// Current attempt number (0-indexed)
    pub current_attempt: u8,
    /// Last failure kind encountered
    pub last_failure_kind: Option<FailureKind>,
    /// Backoff factor for exponential backoff (default: 2.0)
    pub backoff_factor: f64,
    /// Initial delay in milliseconds (default: 1000)
    pub initial_delay_ms: u64,
}

impl RetryConfig {
    /// Creates a new retry configuration
    ///
    /// # Arguments
    /// * `max_attempts` - Maximum number of retry attempts
    ///
    /// # Example
    /// ```rust
    /// use flow_raft::core::RetryConfig;
    ///
    /// let config = RetryConfig::new(3);
    /// assert_eq!(config.max_attempts, 3);
    /// assert_eq!(config.current_attempt, 0);
    /// ```
    #[inline]
    pub fn new(max_attempts: u8) -> Self {
        Self {
            max_attempts,
            current_attempt: 0,
            last_failure_kind: None,
            backoff_factor: RETRY_CONFIG_DEFAULT_BACKOFF_FACTOR,
            initial_delay_ms: RETRY_CONFIG_DEFAULT_INITIAL_DELAY_MS,
        }
    }

    /// Creates a new retry configuration with custom backoff settings
    ///
    /// # Arguments
    /// * `max_attempts` - Maximum number of retry attempts
    /// * `backoff_factor` - Exponential backoff factor
    /// * `initial_delay_ms` - Initial delay in milliseconds
    #[inline]
    pub fn with_backoff(max_attempts: u8, backoff_factor: f64, initial_delay_ms: u64) -> Self {
        Self {
            max_attempts,
            current_attempt: 0,
            last_failure_kind: None,
            backoff_factor,
            initial_delay_ms,
        }
    }

    /// Returns whether a retry is possible
    ///
    /// A retry is possible if:
    /// - The last failure was retryable (or no failure yet)
    /// - The current attempt is less than max attempts
    ///
    /// # Example
    /// ```rust
    /// use flow_raft::core::{RetryConfig, FailureKind};
    ///
    /// let mut config = RetryConfig::new(3);
    /// assert!(config.can_retry());
    ///
    /// config.last_failure_kind = Some(FailureKind::Terminal);
    /// assert!(!config.can_retry());
    /// ```
    #[inline]
    pub fn can_retry(&self) -> bool {
        if let Some(failure) = self.last_failure_kind
            && failure == FailureKind::Terminal
        {
            return false;
        }
        self.current_attempt < self.max_attempts
    }

    /// Increments the retry attempt counter
    ///
    /// Returns an error if retry is not possible (max attempts exceeded or terminal failure).
    ///
    /// # Example
    /// ```rust
    /// use flow_raft::core::{RetryConfig, RetryError};
    ///
    /// let mut config = RetryConfig::new(3);
    /// assert!(config.increment().is_ok());
    /// assert_eq!(config.current_attempt, 1);
    /// ```
    pub fn increment(&mut self) -> Result<(), MaxRetriesExceededError> {
        if !self.can_retry() {
            return Err(MaxRetriesExceededError::new(
                self.max_attempts,
                self.current_attempt,
            ));
        }
        self.current_attempt += 1;
        Ok(())
    }

    /// Calculates the delay for the next retry attempt using exponential backoff
    ///
    /// # Example
    /// ```rust
    /// use flow_raft::core::RetryConfig;
    ///
    /// let config = RetryConfig::with_backoff(3, 2.0, 1000);
    /// // First retry: 1000ms, second: 2000ms, third: 4000ms
    /// ```
    #[inline]
    pub fn calculate_delay(&self) -> u64 {
        if self.current_attempt == 0 {
            return self.initial_delay_ms;
        }
        (self.initial_delay_ms as f64 * self.backoff_factor.powi(self.current_attempt as i32 - 1))
            as u64
    }

    /// Resets the retry configuration to initial state
    #[inline]
    pub fn reset(&mut self) {
        self.current_attempt = 0;
        self.last_failure_kind = None;
    }
}

impl Default for RetryConfig {
    #[inline]
    fn default() -> Self {
        Self::new(RETRY_CONFIG_DEFAULT_MAX_ATTEMPTS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn test_retry_config_new() {
        let config = RetryConfig::new(5);
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.current_attempt, 0);
        assert_eq!(config.last_failure_kind, None);
        assert_eq!(config.backoff_factor, 2.0);
        assert_eq!(config.initial_delay_ms, 1000);
    }

    #[test]
    fn test_retry_config_with_backoff() {
        let config = RetryConfig::with_backoff(3, 1.5, 500);
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.current_attempt, 0);
        assert_eq!(config.backoff_factor, 1.5);
        assert_eq!(config.initial_delay_ms, 500);
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, RETRY_CONFIG_DEFAULT_MAX_ATTEMPTS);
        assert_eq!(config.current_attempt, 0);
    }

    #[rstest]
    #[case::initial(0, None, true)]
    #[case::after_increment(1, None, true)]
    #[case::max_attempts(3, None, false)]
    #[case::terminal_failure(0, Some(FailureKind::Terminal), false)]
    #[case::retryable_failure(1, Some(FailureKind::Retryable), true)]
    fn test_can_retry(
        #[case] current_attempt: u8,
        #[case] last_failure_kind: Option<FailureKind>,
        #[case] expected: bool,
    ) {
        let mut config = RetryConfig::new(3);
        config.current_attempt = current_attempt;
        config.last_failure_kind = last_failure_kind;
        assert_eq!(config.can_retry(), expected);
    }

    #[rstest]
    #[case::success(0, None, true, 1)]
    #[case::max_attempts_exceeded(3, None, false, 3)]
    #[case::terminal_failure(0, Some(FailureKind::Terminal), false, 0)]
    fn test_increment(
        #[case] initial_attempt: u8,
        #[case] last_failure_kind: Option<FailureKind>,
        #[case] should_succeed: bool,
        #[case] expected_attempt: u8,
    ) {
        let mut config = RetryConfig::new(3);
        config.current_attempt = initial_attempt;
        config.last_failure_kind = last_failure_kind;
        let result = config.increment();
        assert_eq!(result.is_ok(), should_succeed);
        assert_eq!(config.current_attempt, expected_attempt);
    }

    #[rstest]
    #[case(0, 1000)]
    #[case(1, 1000)]
    #[case(2, 2000)]
    #[case(3, 4000)]
    fn test_calculate_delay(#[case] attempt: u8, #[case] expected_delay: u64) {
        let mut config = RetryConfig::with_backoff(5, 2.0, 1000);
        config.current_attempt = attempt;
        assert_eq!(config.calculate_delay(), expected_delay);
    }

    #[test]
    fn test_calculate_delay_custom_backoff() {
        let mut config = RetryConfig::with_backoff(5, 1.5, 500);
        config.current_attempt = 2;
        assert_eq!(config.calculate_delay(), 750); // 500 * 1.5^1
    }

    #[test]
    fn test_reset() {
        let mut config = RetryConfig::new(3);
        config.current_attempt = 2;
        config.last_failure_kind = Some(FailureKind::Retryable);
        config.reset();
        assert_eq!(config.current_attempt, 0);
        assert_eq!(config.last_failure_kind, None);
    }
}
