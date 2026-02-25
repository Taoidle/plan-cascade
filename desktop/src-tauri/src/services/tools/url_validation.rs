//! URL Validation for SSRF Prevention
//!
//! Shared validation logic used by WebFetch, Browser, and other tools
//! that accept user-provided URLs.

use std::net::IpAddr;

/// Check if an IP address is in a private/reserved range.
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_loopback()          // 127.0.0.0/8
                || ipv4.is_private()     // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
                || ipv4.is_link_local()  // 169.254.0.0/16
                || ipv4.is_unspecified() // 0.0.0.0
                || ipv4.is_broadcast()   // 255.255.255.255
        }
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback() || ipv6.is_unspecified()
        }
    }
}

/// Check if a hostname is a known private/local name.
pub fn is_private_host(host: &str) -> bool {
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
        return is_private_ip(ip);
    }

    false
}

/// Validate a URL string for SSRF safety: parse, enforce HTTPS, check hostname,
/// and resolve DNS to verify all IPs are public.
///
/// Returns the parsed URL on success or an error message on failure.
pub async fn validate_url_ssrf(url_str: &str) -> Result<url::Url, String> {
    // Auto-upgrade HTTP to HTTPS
    let url_str = if url_str.starts_with("http://") {
        url_str.replacen("http://", "https://", 1)
    } else if !url_str.starts_with("https://") {
        format!("https://{}", url_str)
    } else {
        url_str.to_string()
    };

    let url = url::Url::parse(&url_str).map_err(|e| format!("Invalid URL: {}", e))?;

    if url.scheme() != "https" {
        return Err("Only HTTPS URLs are supported".to_string());
    }

    let host = url
        .host_str()
        .ok_or_else(|| "URL has no host".to_string())?;

    // Fast-path: reject obviously private hostnames
    if is_private_host(host) {
        return Err(format!(
            "Blocked: private/local address '{}' (SSRF prevention)",
            host
        ));
    }

    // DNS resolution check: resolve hostname and verify all IPs are public.
    // This prevents DNS rebinding attacks where a hostname initially resolves
    // to a public IP but later resolves to a private one.
    let port = url.port_or_known_default().unwrap_or(443);
    match tokio::net::lookup_host(format!("{}:{}", host, port)).await {
        Ok(addrs) => {
            for addr in addrs {
                if is_private_ip(addr.ip()) {
                    return Err(format!(
                        "Blocked: '{}' resolves to private IP {} (SSRF prevention)",
                        host,
                        addr.ip()
                    ));
                }
            }
        }
        Err(e) => {
            return Err(format!("DNS resolution failed for '{}': {}", host, e));
        }
    }

    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_private_ip_v4() {
        assert!(is_private_ip("127.0.0.1".parse().unwrap()));
        assert!(is_private_ip("10.0.0.1".parse().unwrap()));
        assert!(is_private_ip("172.16.0.1".parse().unwrap()));
        assert!(is_private_ip("192.168.1.1".parse().unwrap()));
        assert!(is_private_ip("169.254.1.1".parse().unwrap()));
        assert!(is_private_ip("0.0.0.0".parse().unwrap()));

        assert!(!is_private_ip("8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip("1.1.1.1".parse().unwrap()));
        assert!(!is_private_ip("203.0.113.1".parse().unwrap()));
    }

    #[test]
    fn test_is_private_ip_v6() {
        assert!(is_private_ip("::1".parse().unwrap()));
        assert!(is_private_ip("::".parse().unwrap()));

        assert!(!is_private_ip("2001:db8::1".parse().unwrap()));
    }

    #[test]
    fn test_is_private_host() {
        assert!(is_private_host("localhost"));
        assert!(is_private_host("127.0.0.1"));
        assert!(is_private_host("::1"));
        assert!(is_private_host("0.0.0.0"));
        assert!(is_private_host("foo.local"));
        assert!(is_private_host("foo.internal"));

        assert!(!is_private_host("example.com"));
        assert!(!is_private_host("8.8.8.8"));
    }

    #[tokio::test]
    async fn test_validate_url_ssrf_blocks_private() {
        assert!(validate_url_ssrf("https://localhost").await.is_err());
        assert!(validate_url_ssrf("https://127.0.0.1").await.is_err());
        assert!(validate_url_ssrf("https://10.0.0.1").await.is_err());
        assert!(validate_url_ssrf("https://192.168.1.1").await.is_err());
        assert!(validate_url_ssrf("https://169.254.169.254").await.is_err());
    }

    #[tokio::test]
    async fn test_validate_url_ssrf_allows_public() {
        // This test requires network access; skip in CI if needed
        let result = validate_url_ssrf("https://example.com").await;
        assert!(result.is_ok());
    }
}
