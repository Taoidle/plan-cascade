//! Reranker
//!
//! Defines the `Reranker` trait and implementations for reranking
//! search results based on relevance to a query.
//!
//! - `NoopReranker`: pass-through, preserves original order
//! - `LlmReranker`: uses keyword-overlap scoring to reorder results
//!   (In production, this would use an LlmProvider for scoring)

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message};
use crate::utils::error::AppResult;

/// A search result to be reranked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Chunk text content.
    pub chunk_text: String,
    /// Source document ID.
    pub document_id: String,
    /// Collection name this result came from.
    pub collection_name: String,
    /// Relevance score (0.0 to 1.0).
    pub score: f32,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

/// Trait for reranking search results.
#[async_trait]
pub trait Reranker: Send + Sync {
    /// Rerank results based on the query, returning reordered results.
    async fn rerank(&self, query: &str, results: Vec<SearchResult>) -> AppResult<Vec<SearchResult>>;
}

/// No-op reranker that preserves original order.
pub struct NoopReranker;

#[async_trait]
impl Reranker for NoopReranker {
    async fn rerank(&self, _query: &str, results: Vec<SearchResult>) -> AppResult<Vec<SearchResult>> {
        Ok(results)
    }
}

/// LLM-based reranker that scores relevance of each result against the query.
///
/// When an `LlmProvider` is supplied, sends a batch scoring prompt to the LLM
/// for semantic relevance assessment. Falls back to keyword-overlap scoring
/// when no provider is available or the LLM call fails.
pub struct LlmReranker {
    provider: Option<Arc<dyn LlmProvider>>,
}

impl LlmReranker {
    /// Create a new LLM reranker without a provider (keyword-overlap only).
    pub fn new() -> Self {
        Self { provider: None }
    }

    /// Create a new LLM reranker with the given provider.
    pub fn with_provider(provider: Arc<dyn LlmProvider>) -> Self {
        Self {
            provider: Some(provider),
        }
    }

    /// Score relevance of a chunk against a query using keyword overlap.
    fn score_relevance_heuristic(query: &str, chunk_text: &str) -> f32 {
        let query_words: std::collections::HashSet<String> = query
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        let chunk_words: std::collections::HashSet<String> = chunk_text
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        if query_words.is_empty() {
            return 0.0;
        }

        let overlap = query_words.intersection(&chunk_words).count() as f32;
        let score = overlap / query_words.len() as f32;
        score.min(1.0)
    }

    /// Build a batch relevance scoring prompt.
    fn build_rerank_prompt(query: &str, results: &[SearchResult]) -> String {
        let mut prompt = format!(
            r#"You are a relevance scoring engine. Score how relevant each document chunk is to the query.

Query: "{}"

For each chunk below, output a JSON array of scores (0.0 to 1.0):
"#,
            query
        );

        for (i, result) in results.iter().enumerate() {
            // Truncate to avoid overly long prompts
            let text: String = result.chunk_text.chars().take(500).collect();
            prompt.push_str(&format!("\nChunk {}: \"{}\"\n", i, text));
        }

        prompt.push_str(
            r#"
Respond with ONLY a JSON array of numbers, one per chunk:
[0.85, 0.42, ...]"#,
        );

        prompt
    }

    /// Parse the LLM response into a vector of scores.
    fn parse_scores(response: &str, expected_count: usize) -> Option<Vec<f32>> {
        // Find the JSON array in the response
        let start = response.find('[')?;
        let end = response.rfind(']')?;
        let json_str = &response[start..=end];
        let scores: Vec<f32> = serde_json::from_str(json_str).ok()?;

        if scores.len() == expected_count {
            Some(scores)
        } else {
            None
        }
    }

    /// Apply heuristic-based reranking (keyword overlap).
    fn rerank_heuristic(query: &str, results: &mut [SearchResult]) {
        for result in results.iter_mut() {
            let relevance = Self::score_relevance_heuristic(query, &result.chunk_text);
            result.score = (result.score + relevance) / 2.0;
        }
    }
}

impl Default for LlmReranker {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Reranker for LlmReranker {
    async fn rerank(&self, query: &str, mut results: Vec<SearchResult>) -> AppResult<Vec<SearchResult>> {
        if results.is_empty() {
            return Ok(results);
        }

        // Attempt LLM-based reranking when provider is available
        if let Some(provider) = &self.provider {
            let prompt = Self::build_rerank_prompt(query, &results);
            let messages = vec![Message::user(prompt)];
            let request_options = LlmRequestOptions {
                temperature_override: Some(0.0),
                ..Default::default()
            };

            match provider
                .send_message(messages, None, vec![], request_options)
                .await
            {
                Ok(response) => {
                    if let Some(content) = &response.content {
                        if let Some(scores) = Self::parse_scores(content, results.len()) {
                            for (result, llm_score) in results.iter_mut().zip(scores.iter()) {
                                // Blend original vector score with LLM relevance score
                                result.score = (result.score + llm_score) / 2.0;
                            }

                            results.sort_by(|a, b| {
                                b.score
                                    .partial_cmp(&a.score)
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            });
                            return Ok(results);
                        }
                    }
                    tracing::warn!("Failed to parse LLM reranking response, falling back to heuristic");
                }
                Err(e) => {
                    tracing::warn!("LLM reranking call failed, falling back to heuristic: {}", e);
                }
            }
        }

        // Fallback: keyword-overlap heuristic
        Self::rerank_heuristic(query, &mut results);
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(text: &str, score: f32) -> SearchResult {
        SearchResult {
            chunk_text: text.to_string(),
            document_id: "d1".to_string(),
            collection_name: "col".to_string(),
            score,
            metadata: Default::default(),
        }
    }

    #[tokio::test]
    async fn noop_reranker_preserves_order() {
        let reranker = NoopReranker;
        let results = vec![
            make_result("first", 0.9),
            make_result("second", 0.8),
        ];

        let reranked = reranker.rerank("query", results).await.unwrap();
        assert_eq!(reranked.len(), 2);
        assert_eq!(reranked[0].chunk_text, "first");
        assert_eq!(reranked[1].chunk_text, "second");
    }

    #[tokio::test]
    async fn noop_reranker_preserves_scores() {
        let reranker = NoopReranker;
        let results = vec![make_result("test", 0.75)];
        let reranked = reranker.rerank("q", results).await.unwrap();
        assert!((reranked[0].score - 0.75).abs() < 0.001);
    }

    #[tokio::test]
    async fn llm_reranker_scores_and_reorders() {
        let reranker = LlmReranker::new();
        let results = vec![
            make_result("The weather is nice today", 0.5),
            make_result("Rust programming language is fast", 0.5),
            make_result("Rust ownership model prevents bugs", 0.5),
        ];

        let reranked = reranker
            .rerank("Rust programming language", results)
            .await
            .unwrap();

        // The Rust-related results should be ranked higher
        assert_eq!(reranked.len(), 3);
        // First result should be the one about "Rust programming language" (most keyword overlap)
        assert!(
            reranked[0].chunk_text.contains("Rust programming"),
            "Expected top result about Rust programming, got: {}",
            reranked[0].chunk_text
        );
    }

    #[tokio::test]
    async fn llm_reranker_handles_empty_results() {
        let reranker = LlmReranker::new();
        let results = Vec::new();
        let reranked = reranker.rerank("query", results).await.unwrap();
        assert!(reranked.is_empty());
    }

    #[test]
    fn score_relevance_full_overlap() {
        let score = LlmReranker::score_relevance_heuristic("hello world", "hello world foo");
        assert!(score > 0.9, "Full overlap should score high: {}", score);
    }

    #[test]
    fn score_relevance_no_overlap() {
        let score = LlmReranker::score_relevance_heuristic("hello world", "foo bar baz");
        assert!(score < 0.01, "No overlap should score near zero: {}", score);
    }

    #[test]
    fn score_relevance_partial_overlap() {
        let score = LlmReranker::score_relevance_heuristic("hello world", "hello foo bar");
        assert!(score > 0.0 && score < 1.0, "Partial overlap: {}", score);
    }

    #[test]
    fn score_relevance_empty_query() {
        let score = LlmReranker::score_relevance_heuristic("", "some text");
        assert_eq!(score, 0.0);
    }
}
