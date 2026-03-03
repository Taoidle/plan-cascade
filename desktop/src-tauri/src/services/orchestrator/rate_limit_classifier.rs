//! Rate limit signal classifier for embedding providers.
//!
//! Different vendors use different combinations of HTTP status, custom
//! error codes, response headers, and message text to indicate throttling.
//! This module centralizes detection so provider adapters can map those
//! signals into `EmbeddingError::RateLimited` consistently.

use reqwest::header::HeaderMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Structured result for a detected rate-limit signal.
#[derive(Debug, Clone)]
pub struct RateLimitSignal {
    pub retry_after_secs: Option<u32>,
    pub provider_code: Option<String>,
    pub reason: String,
}

/// Classify whether a provider response indicates rate limiting.
///
/// Signals are considered in priority order:
/// 1) HTTP status code
/// 2) retry headers
/// 3) provider error code
/// 4) error message text
pub fn classify_rate_limit(
    status: Option<u16>,
    headers: Option<&HeaderMap>,
    provider_code: Option<&str>,
    message: Option<&str>,
) -> Option<RateLimitSignal> {
    let retry_after = parse_retry_after_secs(headers);
    let normalized_code = provider_code.map(|c| c.trim().to_ascii_lowercase());
    let normalized_msg = message.map(|m| m.trim().to_ascii_lowercase());

    if status == Some(429) {
        return Some(RateLimitSignal {
            retry_after_secs: retry_after,
            provider_code: provider_code.map(ToString::to_string),
            reason: "http_429".to_string(),
        });
    }

    if retry_after.is_some() {
        return Some(RateLimitSignal {
            retry_after_secs: retry_after,
            provider_code: provider_code.map(ToString::to_string),
            reason: "retry_after_header".to_string(),
        });
    }

    if let Some(code) = normalized_code.as_deref() {
        if code_looks_rate_limited(code) {
            return Some(RateLimitSignal {
                retry_after_secs: retry_after,
                provider_code: provider_code.map(ToString::to_string),
                reason: "provider_code".to_string(),
            });
        }
    }

    if let Some(msg) = normalized_msg.as_deref() {
        if message_looks_rate_limited(msg) {
            return Some(RateLimitSignal {
                retry_after_secs: retry_after,
                provider_code: provider_code.map(ToString::to_string),
                reason: "message_pattern".to_string(),
            });
        }
    }

    None
}

fn parse_retry_after_secs(headers: Option<&HeaderMap>) -> Option<u32> {
    let headers = headers?;

    if let Some(raw) = headers.get("retry-after") {
        if let Ok(text) = raw.to_str() {
            if let Ok(secs) = text.trim().parse::<u32>() {
                return Some(secs.max(1));
            }
        }
    }

    if let Some(raw) = headers.get("x-retry-after-ms") {
        if let Ok(text) = raw.to_str() {
            if let Ok(ms) = text.trim().parse::<u64>() {
                return Some(((ms + 999) / 1000).max(1) as u32);
            }
        }
    }

    if let Some(raw) = headers.get("x-ratelimit-reset") {
        if let Ok(text) = raw.to_str() {
            if let Ok(value) = text.trim().parse::<u64>() {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                // Some providers return epoch timestamp; others return delta.
                let delta = if value > now { value - now } else { value };
                if delta > 0 {
                    return Some(delta.min(u32::MAX as u64) as u32);
                }
            }
        }
    }

    None
}

fn code_looks_rate_limited(code: &str) -> bool {
    const CODE_PATTERNS: &[&str] = &[
        "throttling",
        "rate_limit",
        "ratelimit",
        "quotaexhausted",
        "quota_exhausted",
        "too_many_requests",
        "request_limit",
        "qps_limit",
        "frequency_limit",
        "1302", // commonly used by some providers for rate limiting
    ];

    CODE_PATTERNS.iter().any(|p| code.contains(p))
}

fn message_looks_rate_limited(msg: &str) -> bool {
    const MSG_PATTERNS: &[&str] = &[
        "rate limit",
        "too many requests",
        "throttle",
        "quota exceeded",
        "quota exhausted",
        "qps",
        "rpm",
        "tpm",
        "限流",
        "频率",
        "配额",
    ];

    MSG_PATTERNS.iter().any(|p| msg.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_by_status_429() {
        let signal = classify_rate_limit(Some(429), None, None, Some("Too many requests"));
        assert!(signal.is_some());
        assert_eq!(signal.unwrap().reason, "http_429");
    }

    #[test]
    fn classify_by_provider_code_without_429() {
        let signal = classify_rate_limit(Some(400), None, Some("Throttling.RateQuota"), None);
        assert!(signal.is_some());
        let signal = signal.unwrap();
        assert_eq!(signal.reason, "provider_code");
        assert_eq!(signal.provider_code.as_deref(), Some("Throttling.RateQuota"));
    }

    #[test]
    fn classify_by_message_without_known_code() {
        let signal = classify_rate_limit(Some(403), None, None, Some("request frequency exceeded"));
        assert!(signal.is_some());
        assert_eq!(signal.unwrap().reason, "message_pattern");
    }
}
