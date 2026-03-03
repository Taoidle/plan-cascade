//! Webhook HTTP client factory.
//!
//! Webhook delivery requires bounded latency and predictable redirect behavior.
//! This client is intentionally separate from generic LLM HTTP clients.

use std::time::Duration;

use crate::services::proxy::ProxyConfig;

const WEBHOOK_CONNECT_TIMEOUT_SECS: u64 = 5;
const WEBHOOK_REQUEST_TIMEOUT_SECS: u64 = 15;
const WEBHOOK_TCP_KEEPALIVE_SECS: u64 = 30;

pub fn build_webhook_http_client(proxy: Option<&ProxyConfig>) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(WEBHOOK_CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(WEBHOOK_REQUEST_TIMEOUT_SECS))
        .tcp_keepalive(Duration::from_secs(WEBHOOK_TCP_KEEPALIVE_SECS))
        .redirect(reqwest::redirect::Policy::none());

    match proxy {
        Some(cfg) => {
            let url = cfg.url();
            let mut reqwest_proxy = reqwest::Proxy::all(&url).expect("valid proxy URL");
            if let (Some(username), Some(password)) = (&cfg.username, &cfg.password) {
                reqwest_proxy = reqwest_proxy.basic_auth(username, password);
            }
            builder = builder.proxy(reqwest_proxy);
        }
        None => {
            builder = builder.no_proxy();
        }
    }

    builder
        .build()
        .expect("failed to build webhook reqwest client")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::proxy::ProxyProtocol;

    #[test]
    fn test_build_webhook_http_client_without_proxy() {
        let _client = build_webhook_http_client(None);
    }

    #[test]
    fn test_build_webhook_http_client_with_proxy() {
        let proxy = ProxyConfig {
            protocol: ProxyProtocol::Http,
            host: "127.0.0.1".to_string(),
            port: 8080,
            username: None,
            password: None,
        };
        let _client = build_webhook_http_client(Some(&proxy));
    }
}
