//! Plan Cascade LLM
//!
//! Provides a unified interface for interacting with multiple LLM providers:
//! - Anthropic Claude
//! - OpenAI (GPT-4, o1, o3)
//! - DeepSeek
//! - GLM (ZhipuAI)
//! - MiniMax
//! - Qwen (DashScope)
//! - Ollama (local inference)
//!
//! Also includes provider-specific streaming adapters and the HTTP client factory.

pub mod anthropic;
pub mod deepseek;
pub mod glm;
pub mod http_client;
pub mod minimax;
pub mod ollama;
pub mod openai;
pub mod provider;
pub mod qwen;
pub mod streaming_adapters;
pub mod types;

// Re-export main types
pub use anthropic::AnthropicProvider;
pub use deepseek::DeepSeekProvider;
pub use glm::GlmProvider;
pub use http_client::build_http_client;
pub use minimax::MinimaxProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAIProvider;
pub use provider::LlmProvider;
pub use qwen::QwenProvider;
pub use types::*;

// Re-export streaming adapters
pub use streaming_adapters::{
    ClaudeApiAdapter, DeepSeekAdapter, GlmAdapter, MinimaxAdapter, OllamaAdapter, OpenAIAdapter,
    QwenAdapter,
};
