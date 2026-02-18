//! Knowledge System
//!
//! Provides the RAG (Retrieval-Augmented Generation) pipeline components:
//! - `chunker`: Document chunking strategies (paragraph, token, semantic)
//! - `reranker`: Result reranking strategies
//! - `pipeline`: RagPipeline orchestrating ingest and query
//! - `context_provider`: Auto-retrieval of relevant knowledge for agent context

pub mod chunker;
pub mod reranker;
pub mod pipeline;
pub mod context_provider;
