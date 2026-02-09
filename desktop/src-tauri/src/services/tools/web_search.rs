//! WebSearch Service
//!
//! Pluggable web search with support for Tavily, Brave Search, and DuckDuckGo providers.

use async_trait::async_trait;

/// A search result entry
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Trait for pluggable search providers
#[async_trait]
pub trait SearchProvider: Send + Sync {
    /// Provider name for display
    fn name(&self) -> &str;

    /// Execute a search query
    async fn search(&self, query: &str, max_results: u32) -> Result<Vec<SearchResult>, String>;
}

/// Tavily search provider (requires API key)
struct TavilyProvider {
    client: reqwest::Client,
    api_key: String,
}

#[async_trait]
impl SearchProvider for TavilyProvider {
    fn name(&self) -> &str {
        "Tavily"
    }

    async fn search(&self, query: &str, max_results: u32) -> Result<Vec<SearchResult>, String> {
        let body = serde_json::json!({
            "api_key": self.api_key,
            "query": query,
            "max_results": max_results,
            "include_answer": false,
        });

        let response = self
            .client
            .post("https://api.tavily.com/search")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Tavily request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let err_body = response.text().await.unwrap_or_default();
            return Err(format!("Tavily API error ({}): {}", status.as_u16(), err_body));
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Tavily response: {}", e))?;

        let results = data
            .get("results")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|item| SearchResult {
                        title: item.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string(),
                        url: item.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string(),
                        snippet: item.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }
}

/// Brave Search provider (requires API key)
struct BraveSearchProvider {
    client: reqwest::Client,
    api_key: String,
}

#[async_trait]
impl SearchProvider for BraveSearchProvider {
    fn name(&self) -> &str {
        "Brave Search"
    }

    async fn search(&self, query: &str, max_results: u32) -> Result<Vec<SearchResult>, String> {
        let response = self
            .client
            .get("https://api.search.brave.com/res/v1/web/search")
            .header("X-Subscription-Token", &self.api_key)
            .header("Accept", "application/json")
            .query(&[("q", query), ("count", &max_results.to_string())])
            .send()
            .await
            .map_err(|e| format!("Brave Search request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let err_body = response.text().await.unwrap_or_default();
            return Err(format!("Brave Search API error ({}): {}", status.as_u16(), err_body));
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Brave Search response: {}", e))?;

        let results = data
            .get("web")
            .and_then(|w| w.get("results"))
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|item| SearchResult {
                        title: item.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string(),
                        url: item.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string(),
                        snippet: item.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }
}

/// DuckDuckGo instant answer provider (no API key required, limited results)
struct DuckDuckGoProvider {
    client: reqwest::Client,
}

#[async_trait]
impl SearchProvider for DuckDuckGoProvider {
    fn name(&self) -> &str {
        "DuckDuckGo"
    }

    async fn search(&self, query: &str, max_results: u32) -> Result<Vec<SearchResult>, String> {
        let response = self
            .client
            .get("https://api.duckduckgo.com/")
            .query(&[("q", query), ("format", "json"), ("no_html", "1")])
            .send()
            .await
            .map_err(|e| format!("DuckDuckGo request failed: {}", e))?;

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse DuckDuckGo response: {}", e))?;

        let mut results = Vec::new();

        // Abstract (main result)
        if let Some(abstract_text) = data.get("AbstractText").and_then(|t| t.as_str()) {
            if !abstract_text.is_empty() {
                results.push(SearchResult {
                    title: data
                        .get("Heading")
                        .and_then(|h| h.as_str())
                        .unwrap_or("Result")
                        .to_string(),
                    url: data
                        .get("AbstractURL")
                        .and_then(|u| u.as_str())
                        .unwrap_or("")
                        .to_string(),
                    snippet: abstract_text.to_string(),
                });
            }
        }

        // Related topics
        if let Some(topics) = data.get("RelatedTopics").and_then(|r| r.as_array()) {
            for topic in topics {
                if results.len() >= max_results as usize {
                    break;
                }
                if let Some(text) = topic.get("Text").and_then(|t| t.as_str()) {
                    results.push(SearchResult {
                        title: text.chars().take(80).collect::<String>(),
                        url: topic
                            .get("FirstURL")
                            .and_then(|u| u.as_str())
                            .unwrap_or("")
                            .to_string(),
                        snippet: text.to_string(),
                    });
                }
            }
        }

        Ok(results)
    }
}

/// WebSearch service with pluggable provider
pub struct WebSearchService {
    provider: Box<dyn SearchProvider>,
}

impl WebSearchService {
    /// Create a new WebSearch service with the specified provider.
    ///
    /// - `"tavily"` requires an API key
    /// - `"brave"` requires an API key
    /// - `"duckduckgo"` works without an API key (limited results)
    pub fn new(provider_name: &str, api_key: Option<&str>) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("PlanCascade/1.0 (Desktop)")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let provider: Box<dyn SearchProvider> = match provider_name.to_lowercase().as_str() {
            "tavily" => {
                let key = api_key
                    .filter(|k| !k.is_empty())
                    .ok_or_else(|| "Tavily requires an API key. Configure it in Settings > LLM Backend.".to_string())?;
                Box::new(TavilyProvider {
                    client,
                    api_key: key.to_string(),
                })
            }
            "brave" | "brave_search" => {
                let key = api_key
                    .filter(|k| !k.is_empty())
                    .ok_or_else(|| "Brave Search requires an API key. Configure it in Settings > LLM Backend.".to_string())?;
                Box::new(BraveSearchProvider {
                    client,
                    api_key: key.to_string(),
                })
            }
            "duckduckgo" | "" => Box::new(DuckDuckGoProvider { client }),
            other => return Err(format!(
                "Unknown search provider: '{}'. Supported: tavily, brave, duckduckgo",
                other
            )),
        };

        Ok(Self { provider })
    }

    /// Execute a web search and format results as markdown.
    pub async fn search(&self, query: &str, max_results: Option<u32>) -> Result<String, String> {
        let max_results = max_results.unwrap_or(5).min(10);

        // Sanitize query: strip control chars
        let query: String = query
            .chars()
            .filter(|c| !c.is_control() || *c == ' ')
            .collect();

        if query.trim().is_empty() {
            return Err("Search query cannot be empty".to_string());
        }

        let results = self.provider.search(&query, max_results).await?;

        if results.is_empty() {
            return Ok(format!(
                "## Search Results for: \"{}\"\n\nNo results found.",
                query
            ));
        }

        let mut output = format!(
            "## Search Results for: \"{}\" (via {})\n\n",
            query,
            self.provider.name()
        );

        for (i, result) in results.iter().enumerate() {
            output.push_str(&format!(
                "{}. **{}** - {}\n   {}\n\n",
                i + 1,
                result.title,
                result.url,
                result.snippet
            ));
        }

        Ok(output)
    }

    /// Get the name of the underlying search provider
    pub fn provider_name(&self) -> &str {
        self.provider.name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_duckduckgo_provider() {
        let service = WebSearchService::new("duckduckgo", None);
        assert!(service.is_ok());
        assert_eq!(service.unwrap().provider_name(), "DuckDuckGo");
    }

    #[test]
    fn test_create_default_provider() {
        let service = WebSearchService::new("", None);
        assert!(service.is_ok());
    }

    #[test]
    fn test_tavily_requires_key() {
        let service = WebSearchService::new("tavily", None);
        assert!(service.is_err());
    }

    #[test]
    fn test_brave_requires_key() {
        let service = WebSearchService::new("brave", None);
        assert!(service.is_err());
    }

    #[test]
    fn test_unknown_provider() {
        let service = WebSearchService::new("unknown", None);
        assert!(service.is_err());
    }
}
