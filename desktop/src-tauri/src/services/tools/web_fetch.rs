//! WebFetch Service
//!
//! Fetches web pages, converts HTML to markdown, with caching and SSRF protection.

use mini_moka::sync::Cache;
use std::net::IpAddr;
use std::time::Duration;

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

/// WebFetch service with persistent client and in-memory cache
pub struct WebFetchService {
    client: reqwest::Client,
    cache: Cache<String, String>,
}

impl WebFetchService {
    /// Create a new WebFetch service
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .user_agent("PlanCascade/1.0 (Desktop)")
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let cache = Cache::builder()
            .max_capacity(MAX_CACHE_ENTRIES)
            .time_to_live(Duration::from_secs(CACHE_TTL_SECS))
            .build();

        Self { client, cache }
    }

    /// Fetch a URL and return its content as markdown.
    pub async fn fetch(&self, url_str: &str, timeout_secs: Option<u64>) -> Result<String, String> {
        // Validate and parse URL
        let url = self.validate_url(url_str)?;
        let url_string = url.to_string();

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

        let response = self
            .client
            .get(url.as_str())
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch URL: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!("HTTP error: {} {}", status.as_u16(), status.canonical_reason().unwrap_or("Unknown")));
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
        let result = if content_type.contains("text/html") || content_type.contains("application/xhtml") {
            html2md::parse_html(&body)
        } else {
            body
        };

        // Truncate to max output size
        let result = if result.len() > MAX_OUTPUT_SIZE {
            let mut truncated = result[..MAX_OUTPUT_SIZE].to_string();
            truncated.push_str("\n\n... (content truncated)");
            truncated
        } else {
            result
        };

        // Cache the result
        self.cache.insert(url_string, result.clone());

        Ok(result)
    }

    /// Validate a URL: parse, enforce HTTPS, block private IPs
    fn validate_url(&self, url_str: &str) -> Result<url::Url, String> {
        // Auto-upgrade HTTP to HTTPS
        let url_str = if url_str.starts_with("http://") {
            url_str.replacen("http://", "https://", 1)
        } else if !url_str.starts_with("https://") {
            format!("https://{}", url_str)
        } else {
            url_str.to_string()
        };

        let url = url::Url::parse(&url_str)
            .map_err(|e| format!("Invalid URL: {}", e))?;

        // Must be HTTPS
        if url.scheme() != "https" {
            return Err("Only HTTPS URLs are supported".to_string());
        }

        // Check for private/local addresses
        let host = url
            .host_str()
            .ok_or_else(|| "URL has no host".to_string())?;

        if is_private_host(host) {
            return Err(format!(
                "Blocked: private/local address '{}' (SSRF prevention)",
                host
            ));
        }

        Ok(url)
    }
}

/// Check if a hostname resolves to a private/local IP address
fn is_private_host(host: &str) -> bool {
    // Block obvious hostnames
    let lower = host.to_lowercase();
    if lower == "localhost"
        || lower == "127.0.0.1"
        || lower == "::1"
        || lower == "0.0.0.0"
        || lower.ends_with(".local")
        || lower.ends_with(".internal")
    {
        return true;
    }

    // Check if host is an IP address in private ranges
    if let Ok(ip) = host.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(ipv4) => {
                ipv4.is_loopback()           // 127.0.0.0/8
                    || ipv4.is_private()      // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
                    || ipv4.is_link_local()   // 169.254.0.0/16
                    || ipv4.is_unspecified()  // 0.0.0.0
                    || ipv4.is_broadcast()    // 255.255.255.255
            }
            IpAddr::V6(ipv6) => {
                ipv6.is_loopback() || ipv6.is_unspecified()
            }
        };
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_private_host() {
        assert!(is_private_host("localhost"));
        assert!(is_private_host("127.0.0.1"));
        assert!(is_private_host("::1"));
        assert!(is_private_host("0.0.0.0"));
        assert!(is_private_host("10.0.0.1"));
        assert!(is_private_host("172.16.0.1"));
        assert!(is_private_host("192.168.1.1"));
        assert!(is_private_host("169.254.1.1"));
        assert!(is_private_host("foo.local"));
        assert!(is_private_host("foo.internal"));

        assert!(!is_private_host("example.com"));
        assert!(!is_private_host("8.8.8.8"));
        assert!(!is_private_host("1.1.1.1"));
    }

    #[test]
    fn test_validate_url_https() {
        let service = WebFetchService::new();
        let url = service.validate_url("https://example.com").unwrap();
        assert_eq!(url.scheme(), "https");
    }

    #[test]
    fn test_validate_url_auto_upgrade() {
        let service = WebFetchService::new();
        let url = service.validate_url("http://example.com").unwrap();
        assert_eq!(url.scheme(), "https");
    }

    #[test]
    fn test_validate_url_blocks_private() {
        let service = WebFetchService::new();
        assert!(service.validate_url("https://localhost").is_err());
        assert!(service.validate_url("https://127.0.0.1").is_err());
        assert!(service.validate_url("https://10.0.0.1").is_err());
        assert!(service.validate_url("https://192.168.1.1").is_err());
    }

    #[test]
    fn test_validate_url_allows_public() {
        let service = WebFetchService::new();
        assert!(service.validate_url("https://example.com").is_ok());
        assert!(service.validate_url("https://docs.rs").is_ok());
    }
}
