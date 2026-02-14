//! Provider-Specific Stream Adapters
//!
//! Each adapter handles the unique streaming format of its provider.

pub mod claude_api;
pub mod claude_code;
pub mod deepseek;
pub mod glm;
pub mod minimax;
pub mod ollama;
pub mod openai;
pub mod qwen;

pub use claude_api::ClaudeApiAdapter;
pub use claude_code::ClaudeCodeAdapter;
pub use deepseek::DeepSeekAdapter;
pub use glm::GlmAdapter;
pub use minimax::MinimaxAdapter;
pub use ollama::OllamaAdapter;
pub use openai::OpenAIAdapter;
pub use qwen::QwenAdapter;
