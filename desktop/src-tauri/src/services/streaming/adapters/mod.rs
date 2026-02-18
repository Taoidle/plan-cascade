//! Provider-Specific Stream Adapters (re-export shim)
//!
//! LLM-specific adapters are now in `plan-cascade-llm::streaming_adapters`.
//! ClaudeCodeAdapter remains local (it's a CLI adapter, not an LLM API adapter).

// ClaudeCodeAdapter stays in the main crate
pub mod claude_code;
pub use claude_code::ClaudeCodeAdapter;

// Re-export LLM provider adapters from the llm crate
pub use plan_cascade_llm::streaming_adapters::ClaudeApiAdapter;
pub use plan_cascade_llm::streaming_adapters::DeepSeekAdapter;
pub use plan_cascade_llm::streaming_adapters::GlmAdapter;
pub use plan_cascade_llm::streaming_adapters::MinimaxAdapter;
pub use plan_cascade_llm::streaming_adapters::OllamaAdapter;
pub use plan_cascade_llm::streaming_adapters::OpenAIAdapter;
pub use plan_cascade_llm::streaming_adapters::QwenAdapter;
