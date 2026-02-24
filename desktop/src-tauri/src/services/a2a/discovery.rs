//! A2A Agent Discovery
//!
//! Discovers remote A2A agents by fetching their agent card from the
//! well-known endpoint: `GET {base_url}/.well-known/agent.json`.

use super::types::{A2aError, AgentCard};

/// The well-known path for agent discovery, as specified by the A2A protocol.
pub const WELL_KNOWN_PATH: &str = "/.well-known/agent.json";

/// Discovers a remote A2A agent by fetching its agent card.
///
/// Sends a GET request to `{base_url}/.well-known/agent.json`, parses the
/// response as an `AgentCard`, and validates required fields.
///
/// # Arguments
/// * `client` - The HTTP client to use for the request
/// * `base_url` - The base URL of the remote agent (e.g., "https://agent.example.com")
///
/// # Errors
/// Returns `A2aError` if the request fails, the response is not valid JSON,
/// or the agent card fails validation.
pub async fn discover_agent(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<AgentCard, A2aError> {
    let url = format_discovery_url(base_url);

    let response = client.get(&url).send().await?;
    let status = response.status().as_u16();

    if status != 200 {
        let body = response.text().await.unwrap_or_default();
        return Err(A2aError::HttpError { status, body });
    }

    let card: AgentCard = response
        .json()
        .await
        .map_err(|e| A2aError::InvalidResponse(format!("Failed to parse agent card: {}", e)))?;

    card.validate()?;

    Ok(card)
}

/// Constructs the full discovery URL from a base URL.
///
/// Handles trailing slashes to avoid double-slash issues.
fn format_discovery_url(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    format!("{}{}", base, WELL_KNOWN_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_discovery_url_no_trailing_slash() {
        let url = format_discovery_url("https://agent.example.com");
        assert_eq!(url, "https://agent.example.com/.well-known/agent.json");
    }

    #[test]
    fn test_format_discovery_url_with_trailing_slash() {
        let url = format_discovery_url("https://agent.example.com/");
        assert_eq!(url, "https://agent.example.com/.well-known/agent.json");
    }

    #[test]
    fn test_format_discovery_url_with_multiple_trailing_slashes() {
        let url = format_discovery_url("https://agent.example.com///");
        assert_eq!(url, "https://agent.example.com/.well-known/agent.json");
    }

    #[test]
    fn test_format_discovery_url_with_port() {
        let url = format_discovery_url("http://localhost:8080");
        assert_eq!(url, "http://localhost:8080/.well-known/agent.json");
    }

    #[test]
    fn test_format_discovery_url_with_path() {
        let url = format_discovery_url("https://example.com/agents/v1");
        assert_eq!(url, "https://example.com/agents/v1/.well-known/agent.json");
    }
}
