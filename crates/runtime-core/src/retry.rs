use crate::config::RetryPolicy;
use crate::error::RuntimeError;

/// Retry decision returned by shared retry helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryDecision {
    pub should_retry: bool,
    pub next_delay_ms: Option<u64>,
}

impl RetryDecision {
    pub fn stop() -> Self {
        Self {
            should_retry: false,
            next_delay_ms: None,
        }
    }

    pub fn retry_after(delay_ms: u64) -> Self {
        Self {
            should_retry: true,
            next_delay_ms: Some(delay_ms),
        }
    }
}

impl RetryPolicy {
    /// Decide whether the runtime should retry after a failed attempt.
    pub fn classify_retry(&self, attempts_completed: u32, error: &RuntimeError) -> RetryDecision {
        if !error.is_retryable() {
            return RetryDecision::stop();
        }

        if attempts_completed >= self.max_attempts {
            return RetryDecision::stop();
        }

        let base_delay = self.delay_ms_for_attempt(attempts_completed.saturating_sub(1));
        let next_delay = self.apply_jitter(base_delay);
        RetryDecision::retry_after(next_delay)
    }

    /// Apply ±30 % jitter to `delay_ms` when `self.jitter` is enabled.
    fn apply_jitter(&self, delay_ms: u64) -> u64 {
        if !self.jitter || delay_ms == 0 {
            return delay_ms;
        }

        // Sub-nanosecond entropy from the system clock.
        let entropy = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(54321);

        // Range: [70, 130] inclusive → 61 values.
        let factor_offset = entropy % 61; // 0..=60
        let factor = 70 + factor_offset; // 70..=130

        let jittered = delay_ms.saturating_mul(factor) / 100;
        jittered.min(self.max_delay_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{RuntimeError, RuntimeErrorCode};

    #[test]
    fn non_retryable_errors_stop_immediately() {
        let policy = RetryPolicy::default();
        let err = RuntimeError::new(RuntimeErrorCode::InvalidConfig, "bad config");

        let decision = policy.classify_retry(1, &err);

        assert_eq!(
            decision,
            RetryDecision {
                should_retry: false,
                next_delay_ms: None
            }
        );
    }

    #[test]
    fn retryable_errors_retry_until_attempt_budget_is_exhausted() {
        let policy = RetryPolicy {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 1_000,
            jitter: false,
        };
        let err = RuntimeError::new(RuntimeErrorCode::Transport, "temporary network failure");

        let first = policy.classify_retry(1, &err);
        let second = policy.classify_retry(2, &err);
        let third = policy.classify_retry(3, &err);

        assert_eq!(
            first,
            RetryDecision {
                should_retry: true,
                next_delay_ms: Some(100)
            }
        );
        assert_eq!(
            second,
            RetryDecision {
                should_retry: true,
                next_delay_ms: Some(200)
            }
        );
        assert_eq!(
            third,
            RetryDecision {
                should_retry: false,
                next_delay_ms: None
            }
        );
    }

    #[test]
    fn delay_helper_caps_at_max_delay() {
        let policy = RetryPolicy {
            max_attempts: 10,
            initial_delay_ms: 250,
            max_delay_ms: 1_000,
            jitter: true,
        };

        assert_eq!(policy.delay_ms_for_attempt(0), 250);
        assert_eq!(policy.delay_ms_for_attempt(1), 500);
        assert_eq!(policy.delay_ms_for_attempt(2), 1_000);
        assert_eq!(policy.delay_ms_for_attempt(6), 1_000);
    }
}
