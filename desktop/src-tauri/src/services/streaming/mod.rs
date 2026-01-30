//! Unified Streaming Abstraction Layer
//!
//! Provides a common interface for processing real-time LLM responses from multiple providers:
//! - Claude Code CLI (stream-json format)
//! - Claude API (SSE format)
//! - OpenAI API (SSE with reasoning_content)
//! - DeepSeek API (SSE with <think> tags)
//! - Ollama (JSON with model-dependent thinking)

pub mod unified;
pub mod adapter;
pub mod adapters;
pub mod factory;
pub mod service;

// Re-export main types
pub use unified::{UnifiedStreamEvent, AdapterError};
pub use adapter::StreamAdapter;
pub use factory::AdapterFactory;
pub use service::UnifiedStreamingService;
