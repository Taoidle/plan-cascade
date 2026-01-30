//! Provider-Specific Stream Adapters
//!
//! Each adapter handles the unique streaming format of its provider.

pub mod claude_code;
pub mod claude_api;
pub mod openai;
pub mod deepseek;
pub mod ollama;

pub use claude_code::ClaudeCodeAdapter;
pub use claude_api::ClaudeApiAdapter;
pub use openai::OpenAIAdapter;
pub use deepseek::DeepSeekAdapter;
pub use ollama::OllamaAdapter;
