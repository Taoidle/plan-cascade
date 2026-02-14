//! Adapter Factory
//!
//! Creates appropriate stream adapters based on provider and model.

use super::adapter::StreamAdapter;
use super::adapters::{
    ClaudeApiAdapter, ClaudeCodeAdapter, DeepSeekAdapter, GlmAdapter, MinimaxAdapter,
    OllamaAdapter, OpenAIAdapter, QwenAdapter,
};

/// Factory for creating stream adapters based on provider and model.
pub struct AdapterFactory;

impl AdapterFactory {
    /// Create an appropriate adapter for the given provider and model.
    ///
    /// # Arguments
    /// * `provider` - Provider name (claude-code, claude-api, openai, deepseek, ollama)
    /// * `model` - Model identifier (used for thinking capability detection)
    ///
    /// # Returns
    /// A boxed adapter implementing StreamAdapter trait.
    pub fn create(provider: &str, model: &str) -> Box<dyn StreamAdapter> {
        match provider.to_lowercase().as_str() {
            "claude-code" | "claude_code" => Box::new(ClaudeCodeAdapter::new()),

            "claude-api" | "claude_api" | "claude" | "anthropic" => {
                Box::new(ClaudeApiAdapter::new())
            }

            "openai" | "openai-api" | "gpt" => Box::new(OpenAIAdapter::new(model)),

            "deepseek" | "deepseek-api" => Box::new(DeepSeekAdapter::new(model)),

            "glm" | "glm-api" | "zhipu" | "zhipuai" => Box::new(GlmAdapter::new(model)),

            "qwen" | "qwen-api" | "dashscope" | "alibaba" | "aliyun" => {
                Box::new(QwenAdapter::new(model))
            }

            "minimax" | "minimax-api" => Box::new(MinimaxAdapter::new(model)),

            "ollama" | "ollama-api" => Box::new(OllamaAdapter::new(model)),

            // Default to OpenAI adapter as it's the most compatible format
            _ => {
                eprintln!(
                    "Warning: Unknown provider '{}', defaulting to OpenAI adapter",
                    provider
                );
                Box::new(OpenAIAdapter::new(model))
            }
        }
    }

    /// Get a list of supported provider names.
    pub fn supported_providers() -> &'static [&'static str] {
        &[
            "claude-code",
            "claude-api",
            "openai",
            "deepseek",
            "glm",
            "qwen",
            "minimax",
            "ollama",
        ]
    }

    /// Check if a provider is supported.
    pub fn is_supported(provider: &str) -> bool {
        let provider_lower = provider.to_lowercase();
        matches!(
            provider_lower.as_str(),
            "claude-code"
                | "claude_code"
                | "claude-api"
                | "claude_api"
                | "claude"
                | "anthropic"
                | "openai"
                | "openai-api"
                | "gpt"
                | "deepseek"
                | "deepseek-api"
                | "glm"
                | "glm-api"
                | "zhipu"
                | "zhipuai"
                | "qwen"
                | "qwen-api"
                | "dashscope"
                | "alibaba"
                | "aliyun"
                | "minimax"
                | "minimax-api"
                | "ollama"
                | "ollama-api"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_code_adapter() {
        let adapter = AdapterFactory::create("claude-code", "claude-3-opus");
        assert_eq!(adapter.provider_name(), "claude-code");
        assert!(adapter.supports_thinking());
    }

    #[test]
    fn test_claude_api_adapter() {
        let adapter = AdapterFactory::create("claude-api", "claude-3-opus");
        assert_eq!(adapter.provider_name(), "claude-api");
        assert!(adapter.supports_thinking());

        // Test alias
        let adapter = AdapterFactory::create("anthropic", "claude-3-opus");
        assert_eq!(adapter.provider_name(), "claude-api");
    }

    #[test]
    fn test_openai_adapter() {
        let adapter = AdapterFactory::create("openai", "gpt-4");
        assert_eq!(adapter.provider_name(), "openai");
        assert!(!adapter.supports_thinking());

        // Test with o1 model
        let adapter = AdapterFactory::create("openai", "o1-preview");
        assert!(adapter.supports_thinking());
    }

    #[test]
    fn test_deepseek_adapter() {
        let adapter = AdapterFactory::create("deepseek", "deepseek-chat");
        assert_eq!(adapter.provider_name(), "deepseek");
        assert!(!adapter.supports_thinking());

        // Test with R1 model
        let adapter = AdapterFactory::create("deepseek", "deepseek-r1");
        assert!(adapter.supports_thinking());
    }

    #[test]
    fn test_ollama_adapter() {
        let adapter = AdapterFactory::create("ollama", "llama3.2");
        assert_eq!(adapter.provider_name(), "ollama");
        assert!(!adapter.supports_thinking());

        // Test with thinking model
        let adapter = AdapterFactory::create("ollama", "deepseek-r1:14b");
        assert!(adapter.supports_thinking());
    }

    #[test]
    fn test_unknown_provider_fallback() {
        let adapter = AdapterFactory::create("unknown-provider", "some-model");
        assert_eq!(adapter.provider_name(), "openai");
    }

    #[test]
    fn test_supported_providers() {
        let providers = AdapterFactory::supported_providers();
        assert!(providers.contains(&"claude-code"));
        assert!(providers.contains(&"openai"));
        assert!(providers.contains(&"ollama"));
    }

    #[test]
    fn test_is_supported() {
        assert!(AdapterFactory::is_supported("claude-code"));
        assert!(AdapterFactory::is_supported("Claude-API"));
        assert!(AdapterFactory::is_supported("OPENAI"));
        assert!(!AdapterFactory::is_supported("unknown"));
    }
}
