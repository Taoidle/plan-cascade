//! Shared sanitization helpers for webhook errors and payload fragments.

use regex::Regex;
use std::sync::OnceLock;

const MAX_ERROR_CHARS: usize = 512;
const MAX_RESPONSE_BODY_CHARS: usize = 8192;

static SENSITIVE_KV_RE: OnceLock<Regex> = OnceLock::new();
static BEARER_RE: OnceLock<Regex> = OnceLock::new();
static URL_RE: OnceLock<Regex> = OnceLock::new();

fn sensitive_kv_re() -> &'static Regex {
    SENSITIVE_KV_RE.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(token|secret|password|api[_-]?key|access[_-]?token)\b(\s*[:=]\s*)([^\s,&"']+)"#,
        )
        .expect("valid sensitive kv regex")
    })
}

fn bearer_re() -> &'static Regex {
    BEARER_RE.get_or_init(|| {
        Regex::new(r"(?i)\bbearer\s+([A-Za-z0-9._~+-]{8,})").expect("valid bearer regex")
    })
}

fn url_re() -> &'static Regex {
    URL_RE.get_or_init(|| Regex::new(r#"https?://[^\s"'<>]+"#).expect("valid url regex"))
}

pub fn sanitize_for_user(message: &str) -> String {
    sanitize_and_truncate(message, MAX_ERROR_CHARS)
}

pub fn sanitize_response_body_for_storage(body: &str) -> String {
    sanitize_and_truncate(body, MAX_RESPONSE_BODY_CHARS)
}

fn sanitize_and_truncate(input: &str, max_chars: usize) -> String {
    let mut sanitized = input.to_string();
    sanitized = sensitive_kv_re()
        .replace_all(&sanitized, "$1$2[REDACTED]")
        .to_string();
    sanitized = bearer_re()
        .replace_all(&sanitized, "Bearer [REDACTED]")
        .to_string();

    let mut url_matches: Vec<String> = Vec::new();
    for capture in url_re().find_iter(&sanitized) {
        url_matches.push(capture.as_str().to_string());
    }
    for original in url_matches {
        let replacement = sanitize_url_string(&original);
        sanitized = sanitized.replace(&original, &replacement);
    }

    truncate_chars(&sanitized, max_chars)
}

fn sanitize_url_string(url_str: &str) -> String {
    let Ok(mut url) = url::Url::parse(url_str) else {
        return url_str.to_string();
    };

    if let Some(password) = url.password() {
        if !password.is_empty() {
            let _ = url.set_password(Some("[REDACTED]"));
        }
    }

    if !url.username().is_empty() && url.password().is_none() {
        let _ = url.set_username("[REDACTED]");
    }

    if let Some(query) = url.query() {
        let mut serializer = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
            let should_redact = is_sensitive_key(key.as_ref());
            serializer.append_pair(&key, if should_redact { "[REDACTED]" } else { &value });
        }
        let serialized = serializer.finish();
        url.set_query(if serialized.is_empty() {
            None
        } else {
            Some(&serialized)
        });
    }

    if let Some(segments) = url.path_segments() {
        let sanitized_segments = segments
            .map(|segment| {
                if is_sensitive_path_segment(segment) {
                    "[REDACTED]".to_string()
                } else {
                    segment.to_string()
                }
            })
            .collect::<Vec<_>>();
        let mut path = String::new();
        for segment in sanitized_segments {
            path.push('/');
            path.push_str(&segment);
        }
        if path.is_empty() {
            path.push('/');
        }
        url.set_path(&path);
    }

    url.to_string()
}

fn is_sensitive_key(key: &str) -> bool {
    let key_lc = key.to_ascii_lowercase();
    key_lc.contains("token")
        || key_lc.contains("secret")
        || key_lc.contains("password")
        || key_lc.contains("api_key")
        || key_lc.contains("apikey")
        || key_lc.contains("access_token")
}

fn is_sensitive_path_segment(segment: &str) -> bool {
    if segment.len() < 24 {
        return false;
    }
    segment
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.')
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut truncated = input
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_kv_and_bearer() {
        let value = "token=abc123 password: qwe Bearer abcdefghijklmn";
        let sanitized = sanitize_for_user(value);
        assert!(sanitized.contains("token=[REDACTED]"));
        assert!(sanitized.contains("password: [REDACTED]"));
        assert!(sanitized.contains("Bearer [REDACTED]"));
    }

    #[test]
    fn test_sanitize_url_query_and_path() {
        let value = "https://example.com/hook/abcdefghijklmnopqrstuvwxyz?token=abc&safe=ok";
        let sanitized = sanitize_for_user(value);
        assert!(sanitized.contains("token=%5BREDACTED%5D"));
        assert!(sanitized.contains("/hook/[REDACTED]"));
    }
}
