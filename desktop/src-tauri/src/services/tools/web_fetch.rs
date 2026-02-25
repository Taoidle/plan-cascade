//! WebFetch Service
//!
//! Fetches web pages, converts HTML to markdown, with caching and SSRF protection.

use mini_moka::sync::Cache;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// Maximum requests per domain per minute
const MAX_REQUESTS_PER_DOMAIN_PER_MIN: u32 = 10;

/// Rate limit window (60 seconds)
const RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// Maximum download size (10MB)
const MAX_DOWNLOAD_SIZE: usize = 10 * 1024 * 1024;

/// Maximum output size (100KB)
const MAX_OUTPUT_SIZE: usize = 100 * 1024;

/// Default timeout in seconds
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Maximum timeout in seconds
const MAX_TIMEOUT_SECS: u64 = 60;

/// Cache TTL (15 minutes)
const CACHE_TTL_SECS: u64 = 15 * 60;

/// Maximum cache entries
const MAX_CACHE_ENTRIES: u64 = 100;

/// Maximum number of redirects to follow
const MAX_REDIRECTS: usize = 5;

/// WebFetch service with persistent client and in-memory cache
pub struct WebFetchService {
    client: reqwest::Client,
    cache: Cache<String, String>,
    domain_counters: Cache<String, Arc<AtomicU32>>,
}

impl WebFetchService {
    /// Create a new WebFetch service
    pub fn new() -> Self {
        // Disable automatic redirects — we follow them manually with SSRF checks
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .user_agent("PlanCascade/1.0 (Desktop)")
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let cache = Cache::builder()
            .max_capacity(MAX_CACHE_ENTRIES)
            .time_to_live(Duration::from_secs(CACHE_TTL_SECS))
            .build();

        let domain_counters = Cache::builder()
            .max_capacity(1000)
            .time_to_live(Duration::from_secs(RATE_LIMIT_WINDOW_SECS))
            .build();

        Self { client, cache, domain_counters }
    }

    /// Fetch a URL and return its content as markdown.
    pub async fn fetch(&self, url_str: &str, timeout_secs: Option<u64>) -> Result<String, String> {
        // Validate URL with async DNS resolution (prevents DNS rebinding SSRF)
        let url = super::url_validation::validate_url_ssrf(url_str).await?;
        let url_string = url.to_string();

        // Rate limit check: max requests per domain per minute
        let domain = url.host_str().unwrap_or("unknown").to_string();
        let counter = match self.domain_counters.get(&domain) {
            Some(c) => c,
            None => {
                let c = Arc::new(AtomicU32::new(0));
                self.domain_counters.insert(domain.clone(), c.clone());
                c
            }
        };
        let count = counter.fetch_add(1, Ordering::Relaxed);
        if count >= MAX_REQUESTS_PER_DOMAIN_PER_MIN {
            return Err(format!(
                "Rate limited: too many requests to '{}' (max {} per minute)",
                domain, MAX_REQUESTS_PER_DOMAIN_PER_MIN
            ));
        }

        // Check cache
        if let Some(cached) = self.cache.get(&url_string) {
            return Ok(cached);
        }

        // Build request with optional timeout override
        let timeout = Duration::from_secs(
            timeout_secs
                .unwrap_or(DEFAULT_TIMEOUT_SECS)
                .min(MAX_TIMEOUT_SECS),
        );

        // Follow redirects manually with SSRF validation on each hop
        let mut current_url = url;
        let mut redirects = 0;

        let response = loop {
            let resp = self
                .client
                .get(current_url.as_str())
                .timeout(timeout)
                .send()
                .await
                .map_err(|e| format!("Failed to fetch URL: {}", e))?;

            let status = resp.status();
            if status.is_redirection() {
                redirects += 1;
                if redirects > MAX_REDIRECTS {
                    return Err(format!("Too many redirects (max {})", MAX_REDIRECTS));
                }

                let location = resp
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .ok_or_else(|| "Redirect without Location header".to_string())?;

                // Resolve relative redirects against current URL
                let redirect_url = current_url
                    .join(location)
                    .map_err(|e| format!("Invalid redirect URL '{}': {}", location, e))?;

                // Validate redirect target for SSRF (async DNS check)
                current_url =
                    super::url_validation::validate_url_ssrf(redirect_url.as_str()).await?;
                continue;
            }

            break resp;
        };

        let status = response.status();
        if !status.is_success() {
            return Err(format!(
                "HTTP error: {} {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown")
            ));
        }

        // Check content length header
        if let Some(content_length) = response.content_length() {
            if content_length > MAX_DOWNLOAD_SIZE as u64 {
                return Err(format!(
                    "Content too large: {:.1} MB (max {:.1} MB)",
                    content_length as f64 / (1024.0 * 1024.0),
                    MAX_DOWNLOAD_SIZE as f64 / (1024.0 * 1024.0)
                ));
            }
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        // Read body with size limit
        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read response body: {}", e))?;

        if bytes.len() > MAX_DOWNLOAD_SIZE {
            return Err(format!(
                "Response too large: {:.1} MB (max {:.1} MB)",
                bytes.len() as f64 / (1024.0 * 1024.0),
                MAX_DOWNLOAD_SIZE as f64 / (1024.0 * 1024.0)
            ));
        }

        let body = String::from_utf8_lossy(&bytes).to_string();

        // Convert to markdown if HTML
        let result =
            if content_type.contains("text/html") || content_type.contains("application/xhtml") {
                html2md::parse_html(&body)
            } else {
                body
            };

        // Truncate to max output size using char_indices for UTF-8 safety
        let result = if result.len() > MAX_OUTPUT_SIZE {
            let truncate_at = result
                .char_indices()
                .take_while(|(idx, _)| *idx <= MAX_OUTPUT_SIZE)
                .last()
                .map(|(idx, ch)| idx + ch.len_utf8())
                .unwrap_or(MAX_OUTPUT_SIZE.min(result.len()));
            let mut truncated = result[..truncate_at].to_string();
            truncated.push_str("\n\n... (content truncated)");
            truncated
        } else {
            result
        };

        // Cache the result
        self.cache.insert(url_string, result.clone());

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf8_safe_truncation() {
        // Create a string with multi-byte characters near the truncation boundary
        let content = "a".repeat(MAX_OUTPUT_SIZE - 2) + "日本語";
        assert!(content.len() > MAX_OUTPUT_SIZE);

        // The truncation should not panic and should produce valid UTF-8
        let truncate_at = content
            .char_indices()
            .take_while(|(idx, _)| *idx <= MAX_OUTPUT_SIZE)
            .last()
            .map(|(idx, ch)| idx + ch.len_utf8())
            .unwrap_or(MAX_OUTPUT_SIZE.min(content.len()));

        let truncated = &content[..truncate_at];
        assert!(truncated.is_char_boundary(truncated.len()));
        // Verify it's valid UTF-8
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }

    #[test]
    fn test_utf8_safe_truncation_ascii_only() {
        let content = "a".repeat(MAX_OUTPUT_SIZE + 100);
        let truncate_at = content
            .char_indices()
            .take_while(|(idx, _)| *idx <= MAX_OUTPUT_SIZE)
            .last()
            .map(|(idx, ch)| idx + ch.len_utf8())
            .unwrap_or(MAX_OUTPUT_SIZE.min(content.len()));
        assert!(truncate_at <= MAX_OUTPUT_SIZE + 4); // at most one char past
    }

    #[tokio::test]
    async fn test_validate_blocks_private() {
        let service = WebFetchService::new();
        // These should fail at the SSRF validation step
        assert!(service.fetch("https://localhost", None).await.is_err());
        assert!(service.fetch("https://127.0.0.1", None).await.is_err());
        assert!(service.fetch("https://10.0.0.1", None).await.is_err());
        assert!(service.fetch("https://192.168.1.1", None).await.is_err());
    }
}
