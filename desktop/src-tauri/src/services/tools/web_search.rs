//! WebSearch Service
//!
//! Pluggable web search with support for Tavily, Brave Search, DuckDuckGo, and SearXNG providers.

use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Maximum search requests per minute
const MAX_SEARCHES_PER_MIN: u32 = 20;

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
            return Err(format!(
                "Tavily API error ({}): {}",
                status.as_u16(),
                err_body
            ));
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
                        title: item
                            .get("title")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_string(),
                        url: item
                            .get("url")
                            .and_then(|u| u.as_str())
                            .unwrap_or("")
                            .to_string(),
                        snippet: item
                            .get("content")
                            .and_then(|c| c.as_str())
                            .unwrap_or("")
                            .to_string(),
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
            return Err(format!(
                "Brave Search API error ({}): {}",
                status.as_u16(),
                err_body
            ));
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
                        title: item
                            .get("title")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_string(),
                        url: item
                            .get("url")
                            .and_then(|u| u.as_str())
                            .unwrap_or("")
                            .to_string(),
                        snippet: item
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }
}

/// DuckDuckGo search provider (no API key required, scrapes HTML results)
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
            .post("https://html.duckduckgo.com/html/")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("q={}", urlencoding::encode(query)))
            .send()
            .await
            .map_err(|e| format!("DuckDuckGo request failed: {}", e))?;

        let html = response
            .text()
            .await
            .map_err(|e| format!("Failed to read DuckDuckGo response: {}", e))?;

        let mut results = Vec::new();
        // Parse result blocks: each result is in a div with class "result"
        // Links are in <a class="result__a" href="...">Title</a>
        // Snippets are in <a class="result__snippet" ...>text</a>
        let mut pos = 0;
        while results.len() < max_results as usize {
            // Find next result link
            let link_marker = "class=\"result__a\"";
            let link_start = match html[pos..].find(link_marker) {
                Some(i) => pos + i,
                None => break,
            };

            // Extract href from the <a> tag
            let href_start = match html[..link_start].rfind("href=\"") {
                Some(i) => i + 6,
                None => {
                    pos = link_start + link_marker.len();
                    continue;
                }
            };
            let href_end = match html[href_start..].find('"') {
                Some(i) => href_start + i,
                None => {
                    pos = link_start + link_marker.len();
                    continue;
                }
            };
            let raw_url = &html[href_start..href_end];

            // DuckDuckGo wraps URLs in a redirect: extract the actual URL
            let url = if raw_url.contains("uddg=") {
                raw_url
                    .split("uddg=")
                    .nth(1)
                    .and_then(|u| u.split('&').next())
                    .map(|u| urlencoding::decode(u).unwrap_or_default().to_string())
                    .unwrap_or_else(|| raw_url.to_string())
            } else {
                raw_url.to_string()
            };

            // Extract title: text between > and </a> after the link_marker
            let title_start = match html[link_start..].find('>') {
                Some(i) => link_start + i + 1,
                None => {
                    pos = link_start + link_marker.len();
                    continue;
                }
            };
            let title_end = match html[title_start..].find("</a>") {
                Some(i) => title_start + i,
                None => {
                    pos = link_start + link_marker.len();
                    continue;
                }
            };
            let title = strip_html_tags(&html[title_start..title_end]);

            // Extract snippet
            pos = title_end;
            let snippet_marker = "class=\"result__snippet\"";
            let snippet = if let Some(snippet_pos) = html[pos..].find(snippet_marker) {
                let snippet_abs = pos + snippet_pos;
                if let Some(snippet_content_start) = html[snippet_abs..].find('>') {
                    let s_start = snippet_abs + snippet_content_start + 1;
                    if let Some(s_end) = html[s_start..]
                        .find("</a>")
                        .or_else(|| html[s_start..].find("</span>"))
                    {
                        strip_html_tags(&html[s_start..s_start + s_end])
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            if !url.is_empty() && !title.is_empty() {
                results.push(SearchResult {
                    title,
                    url,
                    snippet,
                });
            }

            pos = title_end + 1;
        }

        Ok(results)
    }
}

/// Strip HTML tags and decode common HTML entities from a string.
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    // Decode common HTML entities
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

/// SearXNG self-hosted search provider.
///
/// Queries a SearXNG instance's JSON API. The base URL is configured via
/// the `api_key` field (e.g., "https://searxng.example.com").
struct SearxngProvider {
    client: reqwest::Client,
    base_url: String,
}

#[async_trait]
impl SearchProvider for SearxngProvider {
    fn name(&self) -> &str {
        "SearXNG"
    }

    async fn search(&self, query: &str, max_results: u32) -> Result<Vec<SearchResult>, String> {
        let url = format!("{}/search", self.base_url.trim_end_matches('/'));
        let response = self
            .client
            .get(&url)
            .query(&[("q", query), ("format", "json"), ("categories", "general")])
            .send()
            .await
            .map_err(|e| format!("SearXNG request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let err_body = response.text().await.unwrap_or_default();
            return Err(format!(
                "SearXNG API error ({}): {}",
                status.as_u16(),
                err_body
            ));
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse SearXNG response: {}", e))?;

        let results = data
            .get("results")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .take(max_results as usize)
                    .map(|item| SearchResult {
                        title: item
                            .get("title")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_string(),
                        url: item
                            .get("url")
                            .and_then(|u| u.as_str())
                            .unwrap_or("")
                            .to_string(),
                        snippet: item
                            .get("content")
                            .and_then(|c| c.as_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }
}

/// WebSearch service with pluggable provider
pub struct WebSearchService {
    provider: Box<dyn SearchProvider>,
    request_count: AtomicU32,
    window_start: Mutex<Instant>,
}

impl WebSearchService {
    /// Create a new WebSearch service with the specified provider.
    ///
    /// - `"tavily"` requires an API key
    /// - `"brave"` requires an API key
    /// - `"searxng"` requires a base URL (passed via the `api_key` parameter)
    /// - `"duckduckgo"` works without an API key (scrapes HTML results)
    pub fn new(provider_name: &str, api_key: Option<&str>) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("PlanCascade/1.0 (Desktop)")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let provider: Box<dyn SearchProvider> = match provider_name.to_lowercase().as_str() {
            "tavily" => {
                let key = api_key.filter(|k| !k.is_empty()).ok_or_else(|| {
                    "Tavily requires an API key. Configure it in Settings > LLM Backend."
                        .to_string()
                })?;
                Box::new(TavilyProvider {
                    client,
                    api_key: key.to_string(),
                })
            }
            "brave" | "brave_search" => {
                let key = api_key.filter(|k| !k.is_empty()).ok_or_else(|| {
                    "Brave Search requires an API key. Configure it in Settings > LLM Backend."
                        .to_string()
                })?;
                Box::new(BraveSearchProvider {
                    client,
                    api_key: key.to_string(),
                })
            }
            "searxng" => {
                let base_url = api_key.filter(|k| !k.is_empty()).ok_or_else(|| {
                    "SearXNG requires a base URL. Configure it in Settings > LLM Backend (use the API key field for the base URL).".to_string()
                })?;
                Box::new(SearxngProvider {
                    client,
                    base_url: base_url.to_string(),
                })
            }
            "duckduckgo" | "" => Box::new(DuckDuckGoProvider { client }),
            other => {
                return Err(format!(
                    "Unknown search provider: '{}'. Supported: tavily, brave, duckduckgo, searxng",
                    other
                ))
            }
        };

        Ok(Self {
            provider,
            request_count: AtomicU32::new(0),
            window_start: Mutex::new(Instant::now()),
        })
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

        // Rate limit check
        {
            let mut start = self.window_start.lock().unwrap();
            if start.elapsed() >= std::time::Duration::from_secs(60) {
                // Reset window
                *start = Instant::now();
                self.request_count.store(0, Ordering::Relaxed);
            }
        }
        let count = self.request_count.fetch_add(1, Ordering::Relaxed);
        if count >= MAX_SEARCHES_PER_MIN {
            return Err(format!(
                "Rate limited: too many search requests (max {} per minute)",
                MAX_SEARCHES_PER_MIN
            ));
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

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("<b>hello</b>"), "hello");
        assert_eq!(strip_html_tags("a &amp; b"), "a & b");
        assert_eq!(strip_html_tags("<a href=\"x\">link</a>"), "link");
        assert_eq!(strip_html_tags("&lt;tag&gt;"), "<tag>");
        assert_eq!(strip_html_tags("hello &nbsp; world"), "hello   world");
    }

    #[test]
    fn test_create_searxng_provider() {
        let service = WebSearchService::new("searxng", Some("https://searx.example.com"));
        assert!(service.is_ok());
        assert_eq!(service.unwrap().provider_name(), "SearXNG");
    }

    #[test]
    fn test_searxng_requires_base_url() {
        let service = WebSearchService::new("searxng", None);
        assert!(service.is_err());
    }
}
