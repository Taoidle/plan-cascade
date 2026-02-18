//! Pluggable Context Compaction
//!
//! Provides a trait-based abstraction for context compaction strategies.
//! This replaces the hardcoded compaction logic in the orchestrator with
//! pluggable implementations.
//!
//! Two built-in compactors are provided:
//!
//! - `SlidingWindowCompactor` - Fast, deterministic prefix-stable deletion.
//!   Preserves head and tail messages, removes middle. Best for providers
//!   with unreliable tool calling (Ollama, Qwen, DeepSeek, GLM).
//!
//! - `LlmSummaryCompactor` - Placeholder for LLM-based summarization.
//!   Wraps the concept of using an LLM to summarize compacted messages.
//!   The actual LLM call would be injected via a callback.
//!
//! The `ContextCompactor` trait allows custom strategies to be implemented
//! and plugged in without modifying orchestrator code.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::services::llm::types::{Message, MessageRole};
use crate::utils::error::{AppError, AppResult};

// ============================================================================
// CompactionStrategy Enum
// ============================================================================

/// Strategy for context compaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionStrategy {
    /// Use LLM to summarize compacted messages.
    LlmSummary,
    /// Use a sliding window with prefix-stable deletion.
    SlidingWindow,
    /// No compaction (keep all messages).
    None,
}

impl Default for CompactionStrategy {
    fn default() -> Self {
        Self::LlmSummary
    }
}

// ============================================================================
// CompactionConfig
// ============================================================================

/// Configuration for context compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionConfig {
    /// The compaction strategy to use.
    #[serde(default)]
    pub strategy: CompactionStrategy,
    /// Maximum number of messages before triggering compaction.
    #[serde(default = "default_max_messages")]
    pub max_messages: usize,
    /// Number of messages to preserve at the head (e.g., system prompt + first user message).
    #[serde(default = "default_preserve_head")]
    pub preserve_head: usize,
    /// Number of messages to preserve at the tail (recent context).
    #[serde(default = "default_preserve_tail")]
    pub preserve_tail: usize,
    /// Whether compaction is enabled at all.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_max_messages() -> usize {
    50
}

fn default_preserve_head() -> usize {
    2
}

fn default_preserve_tail() -> usize {
    6
}

fn default_enabled() -> bool {
    true
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            strategy: CompactionStrategy::default(),
            max_messages: default_max_messages(),
            preserve_head: default_preserve_head(),
            preserve_tail: default_preserve_tail(),
            enabled: default_enabled(),
        }
    }
}

impl CompactionConfig {
    /// Create a config for sliding window compaction.
    pub fn sliding_window() -> Self {
        Self {
            strategy: CompactionStrategy::SlidingWindow,
            ..Default::default()
        }
    }

    /// Create a config for LLM summary compaction.
    pub fn llm_summary() -> Self {
        Self {
            strategy: CompactionStrategy::LlmSummary,
            ..Default::default()
        }
    }

    /// Create a disabled compaction config.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Check if compaction should be triggered for the given message count.
    pub fn should_compact(&self, message_count: usize) -> bool {
        self.enabled && message_count > self.max_messages
    }

    /// Minimum number of messages needed for compaction to be meaningful.
    /// Need at least preserve_head + preserve_tail + 1 middle message.
    pub fn min_messages(&self) -> usize {
        self.preserve_head + self.preserve_tail + 1
    }
}

// ============================================================================
// CompactionResult
// ============================================================================

/// Result of a compaction operation, including metrics.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// The compacted messages.
    pub messages: Vec<Message>,
    /// Number of messages that were removed.
    pub messages_removed: usize,
    /// Number of messages that were preserved.
    pub messages_preserved: usize,
    /// Approximate tokens consumed by the compaction process itself (0 for non-LLM).
    pub compaction_tokens: u32,
}

// ============================================================================
// ContextCompactor Trait
// ============================================================================

/// Trait for pluggable context compaction strategies.
///
/// Implementations can use different approaches to reduce context size:
/// - LLM-based summarization (high quality, costly)
/// - Sliding window deletion (fast, preserves KV-cache prefix)
/// - Custom strategies
#[async_trait]
pub trait ContextCompactor: Send + Sync {
    /// Compact the given message history according to the configuration.
    ///
    /// Returns the compacted messages. The original slice is not modified.
    async fn compact(
        &self,
        messages: &[Message],
        config: &CompactionConfig,
    ) -> AppResult<CompactionResult>;

    /// Human-readable name for this compactor.
    fn name(&self) -> &str;
}

// ============================================================================
// SlidingWindowCompactor
// ============================================================================

/// Sliding window compactor that removes middle messages.
///
/// Preserves `preserve_head` messages at the start and `preserve_tail`
/// messages at the end, removing everything in between. Optionally inserts
/// a summary marker message at the splice point.
///
/// This mirrors the existing `compact_messages_prefix_stable` logic in the
/// orchestrator, extracted into a trait implementation for pluggability.
///
/// Advantages:
/// - Zero LLM cost
/// - Preserves KV-cache prefix stability
/// - Deterministic and fast
///
/// Disadvantages:
/// - Loses information from removed messages
/// - No summarization of removed content
pub struct SlidingWindowCompactor {
    /// Whether to insert a marker message at the splice point.
    insert_marker: bool,
}

impl SlidingWindowCompactor {
    /// Create a new SlidingWindowCompactor.
    pub fn new() -> Self {
        Self {
            insert_marker: true,
        }
    }

    /// Create without inserting a marker message.
    pub fn without_marker() -> Self {
        Self {
            insert_marker: false,
        }
    }
}

impl Default for SlidingWindowCompactor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContextCompactor for SlidingWindowCompactor {
    async fn compact(
        &self,
        messages: &[Message],
        config: &CompactionConfig,
    ) -> AppResult<CompactionResult> {
        if !config.enabled {
            return Ok(CompactionResult {
                messages: messages.to_vec(),
                messages_removed: 0,
                messages_preserved: messages.len(),
                compaction_tokens: 0,
            });
        }

        let min_required = config.min_messages();
        if messages.len() < min_required {
            return Ok(CompactionResult {
                messages: messages.to_vec(),
                messages_removed: 0,
                messages_preserved: messages.len(),
                compaction_tokens: 0,
            });
        }

        let head = &messages[..config.preserve_head];
        let tail_start = messages.len().saturating_sub(config.preserve_tail);
        let tail = &messages[tail_start..];
        let removed_count = messages.len() - config.preserve_head - config.preserve_tail;

        let mut result = Vec::with_capacity(config.preserve_head + config.preserve_tail + 1);
        result.extend_from_slice(head);

        if self.insert_marker && removed_count > 0 {
            result.push(Message::text(
                MessageRole::User,
                format!(
                    "[Context compacted: {} messages removed to stay within context limits. \
                     The conversation continues below with the most recent messages.]",
                    removed_count
                ),
            ));
        }

        result.extend_from_slice(tail);

        Ok(CompactionResult {
            messages: result,
            messages_removed: removed_count,
            messages_preserved: config.preserve_head + config.preserve_tail,
            compaction_tokens: 0,
        })
    }

    fn name(&self) -> &str {
        "SlidingWindowCompactor"
    }
}

// ============================================================================
// LlmSummaryCompactor
// ============================================================================

/// Type alias for the async summarization function.
///
/// The function receives the messages to summarize and should return
/// a summary string. This allows the compactor to be used without
/// directly depending on LLM provider types.
pub type SummarizeFn = Box<
    dyn Fn(Vec<Message>) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<String>> + Send>>
        + Send
        + Sync,
>;

/// LLM-based context compactor that uses a summarization function.
///
/// Preserves head and tail messages, summarizes the middle section
/// using a provided async function. The summary replaces the removed
/// messages as a single user message.
///
/// This mirrors the existing `compact_messages` LLM summarization logic,
/// extracted into a trait implementation for pluggability.
pub struct LlmSummaryCompactor {
    /// The summarization function.
    summarize: SummarizeFn,
}

impl LlmSummaryCompactor {
    /// Create a new LlmSummaryCompactor with the given summarization function.
    pub fn new<F>(summarize: F) -> Self
    where
        F: Fn(Vec<Message>) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<String>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        Self {
            summarize: Box::new(summarize),
        }
    }
}

#[async_trait]
impl ContextCompactor for LlmSummaryCompactor {
    async fn compact(
        &self,
        messages: &[Message],
        config: &CompactionConfig,
    ) -> AppResult<CompactionResult> {
        if !config.enabled {
            return Ok(CompactionResult {
                messages: messages.to_vec(),
                messages_removed: 0,
                messages_preserved: messages.len(),
                compaction_tokens: 0,
            });
        }

        let min_required = config.min_messages();
        if messages.len() < min_required {
            return Ok(CompactionResult {
                messages: messages.to_vec(),
                messages_removed: 0,
                messages_preserved: messages.len(),
                compaction_tokens: 0,
            });
        }

        let tail_start = messages.len().saturating_sub(config.preserve_tail);
        let head = &messages[..config.preserve_head];
        let middle = &messages[config.preserve_head..tail_start];
        let tail = &messages[tail_start..];
        let removed_count = middle.len();

        // Summarize the middle section
        let summary = (self.summarize)(middle.to_vec()).await?;

        let mut result = Vec::with_capacity(config.preserve_head + 1 + config.preserve_tail);
        result.extend_from_slice(head);
        result.push(Message::text(
            MessageRole::User,
            format!(
                "[Summary of {} compacted messages]\n{}",
                removed_count, summary
            ),
        ));
        result.extend_from_slice(tail);

        Ok(CompactionResult {
            messages: result,
            messages_removed: removed_count,
            // The summary message is added, so preserved = head + tail + 1 summary
            messages_preserved: config.preserve_head + config.preserve_tail + 1,
            compaction_tokens: 0, // actual tokens would be tracked by the caller
        })
    }

    fn name(&self) -> &str {
        "LlmSummaryCompactor"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::llm::types::{MessageContent, MessageRole};

    /// Extract the first text content from a message (test helper).
    fn extract_text(msg: &Message) -> Option<&str> {
        msg.content.iter().find_map(|c| {
            if let MessageContent::Text { text } = c {
                Some(text.as_str())
            } else {
                None
            }
        })
    }

    fn make_messages(count: usize) -> Vec<Message> {
        (0..count)
            .map(|i| {
                let role = if i % 2 == 0 {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                };
                Message::text(role, format!("Message {}", i))
            })
            .collect()
    }

    // ── CompactionStrategy tests ─────────────────────────────────────

    #[test]
    fn test_compaction_strategy_default() {
        assert_eq!(CompactionStrategy::default(), CompactionStrategy::LlmSummary);
    }

    #[test]
    fn test_compaction_strategy_serialization() {
        let json = serde_json::to_string(&CompactionStrategy::SlidingWindow).unwrap();
        assert_eq!(json, r#""sliding_window""#);

        let parsed: CompactionStrategy = serde_json::from_str(r#""llm_summary""#).unwrap();
        assert_eq!(parsed, CompactionStrategy::LlmSummary);

        let parsed: CompactionStrategy = serde_json::from_str(r#""none""#).unwrap();
        assert_eq!(parsed, CompactionStrategy::None);
    }

    // ── CompactionConfig tests ───────────────────────────────────────

    #[test]
    fn test_compaction_config_defaults() {
        let config = CompactionConfig::default();
        assert_eq!(config.strategy, CompactionStrategy::LlmSummary);
        assert_eq!(config.max_messages, 50);
        assert_eq!(config.preserve_head, 2);
        assert_eq!(config.preserve_tail, 6);
        assert!(config.enabled);
    }

    #[test]
    fn test_compaction_config_sliding_window() {
        let config = CompactionConfig::sliding_window();
        assert_eq!(config.strategy, CompactionStrategy::SlidingWindow);
        assert!(config.enabled);
    }

    #[test]
    fn test_compaction_config_llm_summary() {
        let config = CompactionConfig::llm_summary();
        assert_eq!(config.strategy, CompactionStrategy::LlmSummary);
    }

    #[test]
    fn test_compaction_config_disabled() {
        let config = CompactionConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_compaction_config_should_compact() {
        let config = CompactionConfig {
            max_messages: 10,
            enabled: true,
            ..Default::default()
        };
        assert!(!config.should_compact(5));
        assert!(!config.should_compact(10));
        assert!(config.should_compact(11));
    }

    #[test]
    fn test_compaction_config_should_compact_disabled() {
        let config = CompactionConfig {
            max_messages: 10,
            enabled: false,
            ..Default::default()
        };
        assert!(!config.should_compact(100));
    }

    #[test]
    fn test_compaction_config_min_messages() {
        let config = CompactionConfig {
            preserve_head: 2,
            preserve_tail: 6,
            ..Default::default()
        };
        assert_eq!(config.min_messages(), 9); // 2 + 6 + 1
    }

    #[test]
    fn test_compaction_config_serialization_roundtrip() {
        let config = CompactionConfig {
            strategy: CompactionStrategy::SlidingWindow,
            max_messages: 30,
            preserve_head: 3,
            preserve_tail: 4,
            enabled: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: CompactionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.strategy, CompactionStrategy::SlidingWindow);
        assert_eq!(parsed.max_messages, 30);
        assert_eq!(parsed.preserve_head, 3);
        assert_eq!(parsed.preserve_tail, 4);
        assert!(parsed.enabled);
    }

    // ── SlidingWindowCompactor tests ─────────────────────────────────

    #[tokio::test]
    async fn test_sliding_window_basic_compaction() {
        let compactor = SlidingWindowCompactor::new();
        let messages = make_messages(20);
        let config = CompactionConfig {
            preserve_head: 2,
            preserve_tail: 4,
            enabled: true,
            max_messages: 10,
            ..Default::default()
        };

        let result = compactor.compact(&messages, &config).await.unwrap();
        // 2 head + 1 marker + 4 tail = 7 messages
        assert_eq!(result.messages.len(), 7);
        assert_eq!(result.messages_removed, 14);
        assert_eq!(result.messages_preserved, 6);
        assert_eq!(result.compaction_tokens, 0);

        // Verify head preserved
        assert!(extract_text(&result.messages[0]).unwrap().contains("Message 0"));
        assert!(extract_text(&result.messages[1]).unwrap().contains("Message 1"));

        // Verify marker inserted
        let marker_text = extract_text(&result.messages[2]).unwrap();
        assert!(marker_text.contains("14 messages removed"));

        // Verify tail preserved
        assert!(extract_text(&result.messages[3]).unwrap().contains("Message 16"));
    }

    #[tokio::test]
    async fn test_sliding_window_without_marker() {
        let compactor = SlidingWindowCompactor::without_marker();
        let messages = make_messages(12);
        let config = CompactionConfig {
            preserve_head: 2,
            preserve_tail: 3,
            enabled: true,
            max_messages: 5,
            ..Default::default()
        };

        let result = compactor.compact(&messages, &config).await.unwrap();
        // 2 head + 3 tail = 5 messages (no marker)
        assert_eq!(result.messages.len(), 5);
        assert_eq!(result.messages_removed, 7);
    }

    #[tokio::test]
    async fn test_sliding_window_too_few_messages() {
        let compactor = SlidingWindowCompactor::new();
        let messages = make_messages(5);
        let config = CompactionConfig {
            preserve_head: 2,
            preserve_tail: 4,
            enabled: true,
            max_messages: 3,
            ..Default::default()
        };

        // 5 < min_messages (2 + 4 + 1 = 7), so no compaction
        let result = compactor.compact(&messages, &config).await.unwrap();
        assert_eq!(result.messages.len(), 5);
        assert_eq!(result.messages_removed, 0);
    }

    #[tokio::test]
    async fn test_sliding_window_disabled() {
        let compactor = SlidingWindowCompactor::new();
        let messages = make_messages(100);
        let config = CompactionConfig::disabled();

        let result = compactor.compact(&messages, &config).await.unwrap();
        assert_eq!(result.messages.len(), 100);
        assert_eq!(result.messages_removed, 0);
    }

    #[tokio::test]
    async fn test_sliding_window_exact_min_messages() {
        let compactor = SlidingWindowCompactor::new();
        // exactly min_messages = preserve_head + preserve_tail + 1 = 9
        let messages = make_messages(9);
        let config = CompactionConfig {
            preserve_head: 2,
            preserve_tail: 6,
            enabled: true,
            max_messages: 5,
            ..Default::default()
        };

        let result = compactor.compact(&messages, &config).await.unwrap();
        // Should compact: 2 head + 1 marker + 6 tail = 9, removing 1 message
        assert_eq!(result.messages_removed, 1);
    }

    #[test]
    fn test_sliding_window_name() {
        let compactor = SlidingWindowCompactor::new();
        assert_eq!(compactor.name(), "SlidingWindowCompactor");
    }

    // ── LlmSummaryCompactor tests ────────────────────────────────────

    #[tokio::test]
    async fn test_llm_summary_basic_compaction() {
        let compactor = LlmSummaryCompactor::new(|msgs| {
            Box::pin(async move {
                Ok(format!("Summary of {} messages", msgs.len()))
            })
        });

        let messages = make_messages(15);
        let config = CompactionConfig {
            preserve_head: 2,
            preserve_tail: 3,
            enabled: true,
            max_messages: 5,
            ..Default::default()
        };

        let result = compactor.compact(&messages, &config).await.unwrap();
        // 2 head + 1 summary + 3 tail = 6
        assert_eq!(result.messages.len(), 6);
        assert_eq!(result.messages_removed, 10);

        // Check summary message
        let summary_text = extract_text(&result.messages[2]).unwrap();
        assert!(summary_text.contains("Summary of 10 compacted messages"));
        assert!(summary_text.contains("Summary of 10 messages"));
    }

    #[tokio::test]
    async fn test_llm_summary_too_few_messages() {
        let compactor = LlmSummaryCompactor::new(|_msgs| {
            Box::pin(async move { Ok("summary".to_string()) })
        });

        let messages = make_messages(5);
        let config = CompactionConfig {
            preserve_head: 2,
            preserve_tail: 6,
            enabled: true,
            max_messages: 3,
            ..Default::default()
        };

        let result = compactor.compact(&messages, &config).await.unwrap();
        assert_eq!(result.messages.len(), 5);
        assert_eq!(result.messages_removed, 0);
    }

    #[tokio::test]
    async fn test_llm_summary_disabled() {
        let compactor = LlmSummaryCompactor::new(|_| {
            Box::pin(async { Ok("should not be called".to_string()) })
        });

        let messages = make_messages(100);
        let config = CompactionConfig::disabled();

        let result = compactor.compact(&messages, &config).await.unwrap();
        assert_eq!(result.messages.len(), 100);
    }

    #[tokio::test]
    async fn test_llm_summary_error_propagation() {
        let compactor = LlmSummaryCompactor::new(|_| {
            Box::pin(async { Err(AppError::internal("LLM call failed")) })
        });

        let messages = make_messages(20);
        let config = CompactionConfig {
            preserve_head: 2,
            preserve_tail: 3,
            enabled: true,
            max_messages: 5,
            ..Default::default()
        };

        let result = compactor.compact(&messages, &config).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("LLM call failed"));
    }

    #[test]
    fn test_llm_summary_name() {
        let compactor = LlmSummaryCompactor::new(|_| {
            Box::pin(async { Ok(String::new()) })
        });
        assert_eq!(compactor.name(), "LlmSummaryCompactor");
    }

    // ── CompactionResult tests ───────────────────────────────────────

    #[test]
    fn test_compaction_result_fields() {
        let result = CompactionResult {
            messages: vec![],
            messages_removed: 10,
            messages_preserved: 8,
            compaction_tokens: 500,
        };
        assert_eq!(result.messages_removed, 10);
        assert_eq!(result.messages_preserved, 8);
        assert_eq!(result.compaction_tokens, 500);
    }

    // ── Trait object tests ───────────────────────────────────────────

    #[tokio::test]
    async fn test_compactor_as_trait_object() {
        let compactor: Box<dyn ContextCompactor> = Box::new(SlidingWindowCompactor::new());
        let messages = make_messages(15);
        let config = CompactionConfig {
            preserve_head: 2,
            preserve_tail: 3,
            enabled: true,
            max_messages: 5,
            ..Default::default()
        };

        let result = compactor.compact(&messages, &config).await.unwrap();
        assert!(result.messages_removed > 0);
    }

    #[test]
    fn test_compactors_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SlidingWindowCompactor>();
        assert_send_sync::<LlmSummaryCompactor>();
    }
}
