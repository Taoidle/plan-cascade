//! LLM Provider Module
//!
//! Provides a unified interface for interacting with multiple LLM providers:
//! - Anthropic Claude
//! - OpenAI (GPT-4, o1, o3)
//! - DeepSeek
//! - Ollama (local inference)

pub mod provider;
pub mod types;
pub mod anthropic;
pub mod openai;
pub mod deepseek;
pub mod ollama;

// Re-export main types
pub use provider::LlmProvider;
pub use types::*;
pub use anthropic::AnthropicProvider;
pub use openai::OpenAIProvider;
pub use deepseek::DeepSeekProvider;
pub use ollama::OllamaProvider;
