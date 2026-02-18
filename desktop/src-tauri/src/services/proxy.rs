//! Proxy Configuration & HTTP Client Factory (re-export shim)
//!
//! Proxy types are defined in `plan-cascade-core::proxy`.
//! The HTTP client factory is defined in `plan-cascade-llm::http_client`.
//! This module re-exports both for backward compatibility.

// Re-export proxy types from core
pub use plan_cascade_core::proxy::*;

// Re-export build_http_client from LLM crate
pub use plan_cascade_llm::http_client::build_http_client;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_url_no_auth() {
        let cfg = ProxyConfig {
            protocol: ProxyProtocol::Http,
            host: "127.0.0.1".to_string(),
            port: 8080,
            username: None,
            password: None,
        };
        assert_eq!(cfg.url(), "http://127.0.0.1:8080");
    }

    #[test]
    fn test_build_http_client_no_proxy() {
        let _client = build_http_client(None);
    }

    #[test]
    fn test_build_http_client_with_proxy() {
        let cfg = ProxyConfig {
            protocol: ProxyProtocol::Http,
            host: "127.0.0.1".to_string(),
            port: 8080,
            username: None,
            password: None,
        };
        let _client = build_http_client(Some(&cfg));
    }
}
