//! Unified Streaming Abstraction Layer
//!
//! Core types (UnifiedStreamEvent, AdapterError, StreamAdapter) are defined in
//! `plan-cascade-core::streaming`. Provider-specific adapters are in
//! `plan-cascade-llm::streaming_adapters`. This module re-exports both and
//! provides the AdapterFactory and streaming service.

pub mod adapter;
pub mod adapters;
pub mod factory;
pub mod service;
pub mod unified;

// Re-export main types (from core, for backward compatibility)
pub use plan_cascade_core::streaming::StreamAdapter;
pub use plan_cascade_core::streaming::{AdapterError, UnifiedStreamEvent};
pub use factory::AdapterFactory;
pub use service::UnifiedStreamingService;
