//! HTTP Client Factory
//!
//! Provides a factory function for building reqwest clients with proxy support.

use plan_cascade_core::proxy::ProxyConfig;

/// Build a `reqwest::Client` with the resolved proxy configuration.
///
/// - `Some(proxy)` -> configure proxy on the client
/// - `None` -> explicitly disable proxy (`no_proxy`), ignoring env vars
pub fn build_http_client(proxy: Option<&ProxyConfig>) -> reqwest::Client {
    let mut builder = reqwest::Client::builder();
    match proxy {
        Some(cfg) => {
            let url = cfg.url();
            let mut p = reqwest::Proxy::all(&url).expect("valid proxy URL");
            if let (Some(u), Some(pw)) = (&cfg.username, &cfg.password) {
                p = p.basic_auth(u, pw);
            }
            builder = builder.proxy(p);
        }
        None => {
            builder = builder.no_proxy();
        }
    }
    builder.build().expect("failed to build reqwest client")
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_cascade_core::proxy::{ProxyProtocol};

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
