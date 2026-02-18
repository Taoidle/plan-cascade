//! LLM Provider Module (re-export shim)
//!
//! This module re-exports from the `plan-cascade-llm` workspace crate.
//! All types and implementations now live in `crates/llm/`.

pub mod anthropic {
    pub use plan_cascade_llm::anthropic::*;
}
pub mod deepseek {
    pub use plan_cascade_llm::deepseek::*;
}
pub mod glm {
    pub use plan_cascade_llm::glm::*;
}
pub mod minimax {
    pub use plan_cascade_llm::minimax::*;
}
pub mod ollama {
    pub use plan_cascade_llm::ollama::*;
}
pub mod openai {
    pub use plan_cascade_llm::openai::*;
}
pub mod provider {
    pub use plan_cascade_llm::provider::*;
}
pub mod qwen {
    pub use plan_cascade_llm::qwen::*;
}
pub mod types {
    pub use plan_cascade_llm::types::*;
}

// Re-export main types (backward-compatible with the original mod.rs)
pub use plan_cascade_llm::AnthropicProvider;
pub use plan_cascade_llm::DeepSeekProvider;
pub use plan_cascade_llm::GlmProvider;
pub use plan_cascade_llm::LlmProvider;
pub use plan_cascade_llm::MinimaxProvider;
pub use plan_cascade_llm::OllamaProvider;
pub use plan_cascade_llm::OpenAIProvider;
pub use plan_cascade_llm::QwenProvider;
pub use plan_cascade_llm::types::*;
