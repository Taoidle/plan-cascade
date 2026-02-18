//! Proxy Configuration Types
//!
//! Data types for proxy configuration. These types are shared across LLM providers,
//! embedding providers, webhook channels, and other HTTP-using services.
//! The actual HTTP client factory is in the `plan-cascade-llm` crate.

use serde::{Deserialize, Serialize};

/// Proxy protocol type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProxyProtocol {
    Http,
    Https,
    Socks5,
}

impl ProxyProtocol {
    /// Return the URL scheme string for this protocol.
    pub fn scheme(&self) -> &'static str {
        match self {
            ProxyProtocol::Http => "http",
            ProxyProtocol::Https => "https",
            ProxyProtocol::Socks5 => "socks5",
        }
    }
}

/// Proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub protocol: ProxyProtocol,
    pub host: String,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Password — stored in keyring, only held in-memory here.
    /// Excluded from serialization to avoid accidental persistence.
    #[serde(skip_serializing, default)]
    pub password: Option<String>,
}

impl ProxyConfig {
    /// Build the proxy URL string (without auth).
    pub fn url(&self) -> String {
        format!("{}://{}:{}", self.protocol.scheme(), self.host, self.port)
    }

    /// Build the proxy URL string with embedded credentials (if any).
    pub fn url_with_auth(&self) -> String {
        match (&self.username, &self.password) {
            (Some(u), Some(p)) => {
                format!("{}://{}:{}@{}:{}", self.protocol.scheme(), u, p, self.host, self.port)
            }
            (Some(u), None) => {
                format!("{}://{}@{}:{}", self.protocol.scheme(), u, self.host, self.port)
            }
            _ => self.url(),
        }
    }
}

/// Per-provider proxy strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProxyStrategy {
    /// Use the global default proxy configuration.
    UseGlobal,
    /// Connect directly without any proxy.
    NoProxy,
    /// Use a provider-specific custom proxy configuration.
    Custom,
}

impl Default for ProxyStrategy {
    fn default() -> Self {
        Self::UseGlobal
    }
}

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
    fn test_proxy_url_with_auth() {
        let cfg = ProxyConfig {
            protocol: ProxyProtocol::Socks5,
            host: "proxy.example.com".to_string(),
            port: 1080,
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
        };
        assert_eq!(
            cfg.url_with_auth(),
            "socks5://user:pass@proxy.example.com:1080"
        );
    }

    #[test]
    fn test_proxy_strategy_default() {
        assert_eq!(ProxyStrategy::default(), ProxyStrategy::UseGlobal);
    }

    #[test]
    fn test_proxy_protocol_scheme() {
        assert_eq!(ProxyProtocol::Http.scheme(), "http");
        assert_eq!(ProxyProtocol::Https.scheme(), "https");
        assert_eq!(ProxyProtocol::Socks5.scheme(), "socks5");
    }

    #[test]
    fn test_proxy_config_serialization() {
        let cfg = ProxyConfig {
            protocol: ProxyProtocol::Socks5,
            host: "proxy.test".to_string(),
            port: 1080,
            username: Some("admin".to_string()),
            password: Some("secret".to_string()),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        // password should NOT be serialized (skip_serializing)
        assert!(!json.contains("secret"));
        assert!(json.contains("\"protocol\":\"socks5\""));
        assert!(json.contains("\"host\":\"proxy.test\""));
    }

    #[test]
    fn test_proxy_strategy_serialization() {
        let s = ProxyStrategy::Custom;
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"custom\"");

        let parsed: ProxyStrategy = serde_json::from_str("\"use_global\"").unwrap();
        assert_eq!(parsed, ProxyStrategy::UseGlobal);
    }
}
