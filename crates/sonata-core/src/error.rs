//! Typed public error categories for `sonata-core` boundaries with automatic redaction.
//!
//! 1. **Stable Categories**: `CoreError` separates authentication, rate-limiting, offline, timeout,
//!    cancellation, not-found, malformed-response, external-tool, cache, I/O, and internal failures.
//! 2. **Redaction Guarantee**: All constructors scrub dynamic text through `crate::redaction::sensitive_text`
//!    so cookies, authorization headers, and signed stream URLs never leak through `Display`.

use crate::redaction::sensitive_text;
use std::time::Duration;
use thiserror::Error;

/// Classifies failures returned by external runtime tools such as `yt-dlp`, `ffmpeg`, or JS runtimes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalToolErrorKind {
    NotFound,
    Timeout,
    NonZeroExit,
    InvalidOutput,
    SpawnFailed,
}

/// Typed public error returned by moved and newly contracted `sonata-core` boundaries.
///
/// # Examples
///
/// ```
/// use sonata_core::error::CoreError;
/// use std::time::Duration;
///
/// let err = CoreError::rate_limited(Some(Duration::from_secs(60)), "HTTP 429 from YouTube Music");
/// assert!(matches!(err, CoreError::RateLimited { retry_after: Some(_), .. }));
/// assert_eq!(err.to_string(), "HTTP 429 from YouTube Music");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    #[error("{detail}")]
    AuthRequired { detail: String },

    #[error("{detail}")]
    RateLimited {
        retry_after: Option<Duration>,
        detail: String,
    },

    #[error("{detail}")]
    Offline { detail: String },

    #[error("{detail}")]
    Timeout { detail: String },

    #[error("{detail}")]
    Cancelled { detail: String },

    #[error("{detail}")]
    NotFound { detail: String },

    #[error("{detail}")]
    InvalidResponse { provider: String, detail: String },

    #[error("{detail}")]
    ExternalTool {
        tool: String,
        kind: ExternalToolErrorKind,
        detail: String,
    },

    #[error("{detail}")]
    Cache { detail: String },

    #[error("{detail}")]
    Io { detail: String },

    #[error("{detail}")]
    Internal { detail: String },
}

impl CoreError {
    pub fn auth_required(detail: impl AsRef<str>) -> Self {
        Self::AuthRequired {
            detail: sensitive_text(detail.as_ref()),
        }
    }

    pub fn rate_limited(retry_after: Option<Duration>, detail: impl AsRef<str>) -> Self {
        Self::RateLimited {
            retry_after,
            detail: sensitive_text(detail.as_ref()),
        }
    }

    pub fn offline(detail: impl AsRef<str>) -> Self {
        Self::Offline {
            detail: sensitive_text(detail.as_ref()),
        }
    }

    pub fn timeout(detail: impl AsRef<str>) -> Self {
        Self::Timeout {
            detail: sensitive_text(detail.as_ref()),
        }
    }

    pub fn cancelled(detail: impl AsRef<str>) -> Self {
        Self::Cancelled {
            detail: sensitive_text(detail.as_ref()),
        }
    }

    pub fn not_found(detail: impl AsRef<str>) -> Self {
        Self::NotFound {
            detail: sensitive_text(detail.as_ref()),
        }
    }

    pub fn invalid_response(provider: impl Into<String>, detail: impl AsRef<str>) -> Self {
        Self::InvalidResponse {
            provider: provider.into(),
            detail: sensitive_text(detail.as_ref()),
        }
    }

    pub fn external_tool(
        tool: impl Into<String>,
        kind: ExternalToolErrorKind,
        detail: impl AsRef<str>,
    ) -> Self {
        Self::ExternalTool {
            tool: tool.into(),
            kind,
            detail: sensitive_text(detail.as_ref()),
        }
    }

    pub fn cache(detail: impl AsRef<str>) -> Self {
        Self::Cache {
            detail: sensitive_text(detail.as_ref()),
        }
    }

    pub fn io(detail: impl AsRef<str>) -> Self {
        Self::Io {
            detail: sensitive_text(detail.as_ref()),
        }
    }

    pub fn internal(detail: impl AsRef<str>) -> Self {
        Self::Internal {
            detail: sensitive_text(detail.as_ref()),
        }
    }

    /// Maps a `reqwest::Error` into a redacted `CoreError` category.
    pub fn from_reqwest(provider: impl Into<String>, err: &reqwest::Error) -> Self {
        let provider = provider.into();
        let detail = sensitive_text(&err.to_string());
        if err.is_timeout() {
            return Self::Timeout { detail };
        }
        if err.is_connect() {
            return Self::Offline { detail };
        }
        if let Some(status) = err.status() {
            return match status.as_u16() {
                401 | 403 => Self::AuthRequired { detail },
                404 => Self::NotFound { detail },
                429 => Self::RateLimited {
                    retry_after: None,
                    detail,
                },
                _ => Self::InvalidResponse { provider, detail },
            };
        }
        if err.is_decode() {
            return Self::InvalidResponse { provider, detail };
        }
        Self::Internal { detail }
    }

    /// Classifies an `anyhow::Error` from legacy internal paths into a redacted `CoreError`.
    pub fn classify_anyhow(err: &anyhow::Error) -> Self {
        if let Some(core_err) = err.downcast_ref::<Self>() {
            return core_err.clone();
        }
        if let Some(req_err) = err.downcast_ref::<reqwest::Error>() {
            return Self::from_reqwest("provider", req_err);
        }
        if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
            return match io_err.kind() {
                std::io::ErrorKind::TimedOut => Self::timeout(io_err.to_string()),
                std::io::ErrorKind::NotFound => Self::not_found(io_err.to_string()),
                _ => Self::io(io_err.to_string()),
            };
        }

        let raw = err.to_string();
        let lower = raw.to_ascii_lowercase();
        if lower.contains("429")
            || lower.contains("too many requests")
            || lower.contains("rate limit")
        {
            return Self::rate_limited(None, raw);
        }
        if lower.contains("401")
            || lower.contains("403")
            || lower.contains("not signed in")
            || lower.contains("login_required")
            || lower.contains("auth")
            || lower.contains("cookie")
        {
            return Self::auth_required(raw);
        }
        if lower.contains("timed out") || lower.contains("timeout") {
            return Self::timeout(raw);
        }
        if lower.contains("cancel") {
            return Self::cancelled(raw);
        }
        if lower.contains("dns error")
            || lower.contains("failed to lookup address")
            || lower.contains("connection refused")
            || lower.contains("network is unreachable")
            || lower.contains("offline")
        {
            return Self::offline(raw);
        }
        if lower.contains("not found")
            || lower.contains("failed to resolve")
            || lower.contains("failed to recover")
        {
            return Self::not_found(raw);
        }
        if lower.contains("yt-dlp") {
            return Self::external_tool("yt-dlp", ExternalToolErrorKind::NonZeroExit, raw);
        }
        if lower.contains("invalid") || lower.contains("malformed") || lower.contains("parse") {
            return Self::invalid_response("provider", raw);
        }
        Self::internal(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_sensitive_credentials_and_urls_across_all_error_variants() {
        let dirty = "Cookie: SAPISID=sapi-secret-123; sp_dc=spdc-secret-456\nAuthorization: Bearer auth-secret-789\nhttps://user:pass@rr1.googlevideo.com/videoplayback?sig=url-secret-000";
        let variants = [
            CoreError::auth_required(dirty),
            CoreError::rate_limited(Some(Duration::from_secs(30)), dirty),
            CoreError::offline(dirty),
            CoreError::timeout(dirty),
            CoreError::cancelled(dirty),
            CoreError::not_found(dirty),
            CoreError::invalid_response("ytmusic", dirty),
            CoreError::external_tool("yt-dlp", ExternalToolErrorKind::NonZeroExit, dirty),
            CoreError::cache(dirty),
            CoreError::io(dirty),
            CoreError::internal(dirty),
        ];

        for err in variants {
            let rendered = err.to_string();
            for secret in [
                "sapi-secret-123",
                "spdc-secret-456",
                "auth-secret-789",
                "url-secret-000",
                "pass@",
            ] {
                assert!(
                    !rendered.contains(secret),
                    "error leaked secret `{secret}` in `{rendered}`"
                );
            }
            assert!(rendered.contains("[redacted-sensitive-value]"));
            assert!(rendered.contains("[redacted-stream-url]"));
        }
    }

    #[test]
    fn classifies_anyhow_errors_into_stable_categories() {
        assert!(matches!(
            CoreError::classify_anyhow(&anyhow::anyhow!(
                "YouTube Music HTTP 429 Too Many Requests"
            )),
            CoreError::RateLimited { .. }
        ));
        assert!(matches!(
            CoreError::classify_anyhow(&anyhow::anyhow!("yt-dlp timed out while resolving stream")),
            CoreError::Timeout { .. }
        ));
        assert!(matches!(
            CoreError::classify_anyhow(&anyhow::anyhow!("Failed to resolve stream for \"Song\"")),
            CoreError::NotFound { .. }
        ));
        assert!(matches!(
            CoreError::classify_anyhow(&anyhow::anyhow!("Request failed: HTTP 401 unauthorized")),
            CoreError::AuthRequired { .. }
        ));
        assert!(matches!(
            CoreError::classify_anyhow(&anyhow::anyhow!("dns error: failed to lookup address")),
            CoreError::Offline { .. }
        ));
        assert!(matches!(
            CoreError::classify_anyhow(&anyhow::anyhow!("request cancelled by caller")),
            CoreError::Cancelled { .. }
        ));
    }
}
