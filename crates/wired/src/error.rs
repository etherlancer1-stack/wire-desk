//! Error types for Wire Desk.
//!
//! Every error carries a stable machine-readable code used by the API envelope
//! (spec #194) and by the CLI `--json` mode (spec #215). No silent failures.

use thiserror::Error;
use serde::Serialize;

#[derive(Error, Debug)]
pub enum WireError {
    #[error("config: {0}")]
    Config(String),
    #[error("database: {0}")]
    Database(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("validation: {0}")]
    Validation(String),
    #[error("scope violation: {0}")]
    ScopeViolation(String),
    #[error("quota exceeded: {0}")]
    QuotaExceeded(String),
    #[error("rate limited")]
    RateLimited,
    #[error("internal: {0}")]
    Internal(String),
    #[error("external: {0}")]
    External(String),
}

pub type WireResult<T> = Result<T, WireError>;

/// Stable API error codes (spec #295).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    Unauthorized,
    Forbidden,
    NotFound,
    ValidationError,
    ScopeViolation,
    QuotaExceeded,
    RateLimited,
    InternalError,
    PaymentRequired,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::Unauthorized => "UNAUTHORIZED",
            ErrorCode::Forbidden => "FORBIDDEN",
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::ValidationError => "VALIDATION_ERROR",
            ErrorCode::ScopeViolation => "SCOPE_VIOLATION",
            ErrorCode::QuotaExceeded => "QUOTA_EXCEEDED",
            ErrorCode::RateLimited => "RATE_LIMITED",
            ErrorCode::InternalError => "INTERNAL_ERROR",
            ErrorCode::PaymentRequired => "PAYMENT_REQUIRED",
        }
    }
}

/// Structured API error body (spec #194 error envelope).
#[derive(Debug, Clone, Serialize)]
pub struct ApiErrorBody {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl From<&WireError> for ApiErrorBody {
    fn from(err: &WireError) -> Self {
        let (code, message, details) = match err {
            WireError::Config(msg) => (ErrorCode::InternalError, msg.clone(), None),
            WireError::Database(msg) => (ErrorCode::InternalError, msg.clone(), None),
            WireError::NotFound(msg) => (ErrorCode::NotFound, msg.clone(), None),
            WireError::Unauthorized => (ErrorCode::Unauthorized, "unauthorized".into(), None),
            WireError::Forbidden(msg) => (ErrorCode::Forbidden, msg.clone(), None),
            WireError::Validation(msg) => (ErrorCode::ValidationError, msg.clone(), None),
            WireError::ScopeViolation(msg) => (ErrorCode::ScopeViolation, msg.clone(), None),
            WireError::QuotaExceeded(msg) => (ErrorCode::QuotaExceeded, msg.clone(), None),
            WireError::RateLimited => (ErrorCode::RateLimited, "rate limited".into(), None),
            WireError::Internal(msg) => (ErrorCode::InternalError, msg.clone(), None),
            WireError::External(msg) => (ErrorCode::InternalError, msg.clone(), None),
        };
        Self { code, message, details: None }
    }
}

impl WireError {
    pub fn status_code(&self) -> axum::http::StatusCode {
        match self {
            WireError::Unauthorized => axum::http::StatusCode::UNAUTHORIZED,
            WireError::Forbidden(_) => axum::http::StatusCode::FORBIDDEN,
            WireError::NotFound(_) => axum::http::StatusCode::NOT_FOUND,
            WireError::Validation(_) | WireError::ScopeViolation(_) => {
                axum::http::StatusCode::UNPROCESSABLE_ENTITY
            }
            WireError::QuotaExceeded(_) => axum::http::StatusCode::PAYMENT_REQUIRED,
            WireError::RateLimited => axum::http::StatusCode::TOO_MANY_REQUESTS,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
