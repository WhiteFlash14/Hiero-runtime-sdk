use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NetworkKind {
    Mainnet,
    Testnet,
    Previewnet,
    Custom,
}

/// A single consensus node endpoint used for custom network configurations.

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusNodeConfig {
    /// Host and port of the consensus node, e.g. `"34.94.106.61:50211"`.
    pub url: String,
    /// Hedera account ID of the node, e.g. `"0.0.3"`.
    pub account_id: String,
}

/// Network level runtime configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfig {
    pub kind: NetworkKind,
    pub mirror_base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consensus_nodes: Option<Vec<ConsensusNodeConfig>>,
}

/// Optional operator identity used by write oriented runtimes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperatorConfig {
    pub account_id: String,
    pub private_key: String,
}

/// Exponential retry policy shared across runtime subsystems.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 8,
            initial_delay_ms: 250,
            max_delay_ms: 3_000,
            jitter: true,
        }
    }
}

impl RetryPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_attempts == 0 {
            return Err("retry.maxAttempts must be greater than zero".to_string());
        }

        if self.initial_delay_ms == 0 {
            return Err("retry.initialDelayMs must be greater than zero".to_string());
        }

        if self.max_delay_ms == 0 {
            return Err("retry.maxDelayMs must be greater than zero".to_string());
        }

        if self.initial_delay_ms > self.max_delay_ms {
            return Err(
                "retry.initialDelayMs must be less than or equal to retry.maxDelayMs".to_string(),
            );
        }

        Ok(())
    }

    /// Deterministic exponential backoff delay for a 0 based attempt index.
    pub fn delay_ms_for_attempt(&self, attempt_index: u32) -> u64 {
        let factor = 1u64.checked_shl(attempt_index.min(63)).unwrap_or(u64::MAX);
        let raw = self.initial_delay_ms.saturating_mul(factor);
        raw.min(self.max_delay_ms)
    }
}

/// Finality and polling behavior used by tx/schedule runtimes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalityPolicy {
    pub receipt_timeout_ms: u64,
    pub mirror_timeout_ms: u64,
    pub poll_interval_ms: u64,
}

impl Default for FinalityPolicy {
    fn default() -> Self {
        Self {
            receipt_timeout_ms: 15_000,
            mirror_timeout_ms: 20_000,
            poll_interval_ms: 500,
        }
    }
}

impl FinalityPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.receipt_timeout_ms == 0 {
            return Err("finality.receiptTimeoutMs must be greater than zero".to_string());
        }

        if self.mirror_timeout_ms == 0 {
            return Err("finality.mirrorTimeoutMs must be greater than zero".to_string());
        }

        if self.poll_interval_ms == 0 {
            return Err("finality.pollIntervalMs must be greater than zero".to_string());
        }

        Ok(())
    }
}

/// Top level runtime config shared by native and TS facing layers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfig {
    pub network: NetworkConfig,
    pub operator: Option<OperatorConfig>,
    pub retry: RetryPolicy,
    pub finality: FinalityPolicy,
}

impl RuntimeConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.network.mirror_base_url.trim().is_empty() {
            return Err("network.mirrorBaseUrl must not be empty".to_string());
        }

        self.retry.validate()?;
        self.finality.validate()?;

        if let Some(operator) = &self.operator {
            if operator.account_id.trim().is_empty() {
                return Err("operator.accountId must not be empty".to_string());
            }

            if operator.private_key.trim().is_empty() {
                return Err("operator.privateKey must not be empty".to_string());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_config_serializes_to_camel_case() {
        let config = RuntimeConfig {
            network: NetworkConfig {
                kind: NetworkKind::Testnet,
                mirror_base_url: "https://testnet.mirrornode.hedera.com".to_string(),
                consensus_nodes: None,
            },
            operator: Some(OperatorConfig {
                account_id: "0.0.1001".to_string(),
                private_key: "302e...".to_string(),
            }),
            retry: RetryPolicy::default(),
            finality: FinalityPolicy::default(),
        };

        let json = serde_json::to_value(&config).expect("config should serialize");

        assert_eq!(json["network"]["kind"], "testnet");
        assert_eq!(
            json["network"]["mirrorBaseUrl"],
            "https://testnet.mirrornode.hedera.com"
        );
        assert_eq!(json["operator"]["accountId"], "0.0.1001");
        assert_eq!(json["operator"]["privateKey"], "302e...");
        assert_eq!(json["retry"]["maxAttempts"], 8);
        assert_eq!(json["finality"]["receiptTimeoutMs"], 15_000);
    }

    #[test]
    fn runtime_config_validation_rejects_empty_mirror_url() {
        let config = RuntimeConfig {
            network: NetworkConfig {
                kind: NetworkKind::Custom,
                mirror_base_url: "".to_string(),
                consensus_nodes: None,
            },
            operator: None,
            retry: RetryPolicy::default(),
            finality: FinalityPolicy::default(),
        };

        let err = config.validate().expect_err("empty mirror URL must fail");
        assert_eq!(err, "network.mirrorBaseUrl must not be empty");
    }

    #[test]
    fn retry_policy_validation_rejects_bad_bounds() {
        let policy = RetryPolicy {
            max_attempts: 3,
            initial_delay_ms: 2_000,
            max_delay_ms: 1_000,
            jitter: false,
        };

        let err = policy.validate().expect_err("initial > max must fail");
        assert_eq!(
            err,
            "retry.initialDelayMs must be less than or equal to retry.maxDelayMs"
        );
    }

    #[test]
    fn finality_policy_validation_rejects_zero_poll_interval() {
        let policy = FinalityPolicy {
            receipt_timeout_ms: 1_000,
            mirror_timeout_ms: 1_000,
            poll_interval_ms: 0,
        };

        let err = policy.validate().expect_err("zero poll interval must fail");
        assert_eq!(err, "finality.pollIntervalMs must be greater than zero");
    }

    #[test]
    fn delay_ms_for_attempt_is_exponential_and_capped() {
        let policy = RetryPolicy {
            max_attempts: 8,
            initial_delay_ms: 100,
            max_delay_ms: 500,
            jitter: true,
        };

        assert_eq!(policy.delay_ms_for_attempt(0), 100);
        assert_eq!(policy.delay_ms_for_attempt(1), 200);
        assert_eq!(policy.delay_ms_for_attempt(2), 400);
        assert_eq!(policy.delay_ms_for_attempt(3), 500);
        assert_eq!(policy.delay_ms_for_attempt(10), 500);
    }
}
