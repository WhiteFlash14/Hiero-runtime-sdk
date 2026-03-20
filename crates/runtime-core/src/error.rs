use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::fmt;

/// Stable machine readable runtime error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuntimeErrorCode {
    InvalidConfig,
    Transport,
    MirrorHttp,
    Consensus,
    Schedule,
    Timeout,
    RateLimited,
    NotFound,
    Serialization,
    Unsupported,
    Internal,
}

/// Shared runtime error payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeError {
    pub code: RuntimeErrorCode,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl RuntimeError {
    pub fn new(code: RuntimeErrorCode, message: impl Into<String>) -> Self {
        let code_value = code;
        Self {
            code,
            message: message.into(),
            retryable: Self::default_retryable(code_value),
            details: None,
        }
    }

    pub fn with_details(
        code: RuntimeErrorCode,
        message: impl Into<String>,
        details: Value,
    ) -> Self {
        let code_value = code;
        Self {
            code,
            message: message.into(),
            retryable: Self::default_retryable(code_value),
            details: Some(details),
        }
    }

    pub fn with_retryable(
        code: RuntimeErrorCode,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
            details: None,
        }
    }

    pub fn with_retryable_and_details(
        code: RuntimeErrorCode,
        message: impl Into<String>,
        retryable: bool,
        details: Value,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
            details: Some(details),
        }
    }

    pub fn is_retryable(&self) -> bool {
        self.retryable
    }

    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self::new(RuntimeErrorCode::InvalidConfig, message)
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(RuntimeErrorCode::Timeout, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(RuntimeErrorCode::NotFound, message)
    }

    pub fn transport(message: impl Into<String>) -> Self {
        Self::new(RuntimeErrorCode::Transport, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(RuntimeErrorCode::Internal, message)
    }

    fn default_retryable(code: RuntimeErrorCode) -> bool {
        matches!(
            code,
            RuntimeErrorCode::Transport | RuntimeErrorCode::Timeout | RuntimeErrorCode::RateLimited
        )
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({:?})", self.message, self.code)
    }
}

impl Error for RuntimeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_classification_is_stable() {
        assert!(RuntimeError::new(RuntimeErrorCode::Transport, "network issue").is_retryable());
        assert!(RuntimeError::new(RuntimeErrorCode::Timeout, "timed out").is_retryable());
        assert!(RuntimeError::new(RuntimeErrorCode::RateLimited, "back off").is_retryable());

        assert!(!RuntimeError::new(RuntimeErrorCode::InvalidConfig, "bad config").is_retryable());
        assert!(!RuntimeError::new(RuntimeErrorCode::NotFound, "missing").is_retryable());
        assert!(!RuntimeError::new(RuntimeErrorCode::Internal, "bug").is_retryable());
    }

    #[test]
    fn error_serialization_uses_stable_shape() {
        let err = RuntimeError::with_details(
            RuntimeErrorCode::MirrorHttp,
            "mirror request failed",
            serde_json::json!({
                "status": 503,
                "endpoint": "/api/v1/transactions"
            }),
        );

        let json = serde_json::to_value(&err).expect("error should serialize");

        assert_eq!(json["code"], "MIRROR_HTTP");
        assert_eq!(json["message"], "mirror request failed");
        assert_eq!(json["retryable"], false);
        assert_eq!(json["details"]["status"], 503);
        assert_eq!(json["details"]["endpoint"], "/api/v1/transactions");
    }

    #[test]
    fn display_contains_message_and_code() {
        let err = RuntimeError::timeout("mirror polling exceeded timeout");
        let display = err.to_string();

        assert!(display.contains("mirror polling exceeded timeout"));
        assert!(display.contains("Timeout"));
    }

    #[test]
    fn details_are_omitted_when_absent() {
        let err = RuntimeError::invalid_config("network.mirrorBaseUrl must not be empty");
        let json = serde_json::to_string(&err).expect("error should serialize");

        assert!(json.contains("\"code\":\"INVALID_CONFIG\""));
        assert!(json.contains("\"message\":\"network.mirrorBaseUrl must not be empty\""));
        assert!(json.contains("\"retryable\":false"));
        assert!(!json.contains("\"details\""));
    }
}
