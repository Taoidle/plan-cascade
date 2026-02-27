//! Knowledge System
//!
//! Provides the RAG (Retrieval-Augmented Generation) pipeline components:
//! - `chunker`: Document chunking strategies (paragraph, token, semantic)
//! - `reranker`: Result reranking strategies
//! - `pipeline`: RagPipeline orchestrating ingest and query
//! - `context_provider`: Auto-retrieval of relevant knowledge for agent context

pub mod chunker;
pub mod code_chunker;
pub mod context_provider;
pub mod docs_indexer;
pub mod pipeline;
pub mod reranker;
