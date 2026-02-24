//! Code Chunker
//!
//! A `Chunker` implementation that delegates to the orchestrator's
//! `chunk_file_content` function, which uses tree-sitter symbol boundaries
//! for supported languages and a fixed-size window fallback otherwise.
//!
//! This bridges the knowledge pipeline's `Chunker` trait with the
//! codebase indexing infrastructure, enabling tree-sitter-aware chunking
//! for RAG ingestion of source code files.

use std::collections::HashMap;

use crate::services::orchestrator::background_indexer::chunk_file_content;
use crate::utils::error::AppResult;

use super::chunker::{Chunk, Chunker, Document};

/// A `Chunker` that uses tree-sitter symbol-aware chunking for source code.
///
/// Delegates to `chunk_file_content(content, language)` from the orchestrator
/// module.  The language can be set explicitly or inferred from the document's
/// `metadata["language"]` key.
pub struct CodeChunker {
    /// Programming language hint (e.g. "rust", "python", "typescript").
    /// When `None`, the chunker attempts to read `document.metadata["language"]`.
    language: Option<String>,
}

impl CodeChunker {
    /// Create a `CodeChunker` that infers language from document metadata.
    pub fn new() -> Self {
        Self { language: None }
    }

    /// Create a `CodeChunker` with an explicit language hint.
    pub fn with_language(language: impl Into<String>) -> Self {
        Self {
            language: Some(language.into()),
        }
    }

    /// Resolve the language for a given document.
    fn resolve_language(&self, document: &Document) -> String {
        if let Some(ref lang) = self.language {
            return lang.clone();
        }
        document
            .metadata
            .get("language")
            .cloned()
            .unwrap_or_default()
    }
}

impl Chunker for CodeChunker {
    fn chunk(&self, document: &Document) -> AppResult<Vec<Chunk>> {
        let content = &document.content;
        if content.is_empty() {
            return Ok(Vec::new());
        }

        let lang = self.resolve_language(document);
        let file_chunks = chunk_file_content(content, &lang);

        let chunks = file_chunks
            .into_iter()
            .enumerate()
            .map(|(i, fc)| {
                let mut metadata: HashMap<String, String> = document.metadata.clone();
                metadata.insert("chunker".to_string(), "code".to_string());
                if !lang.is_empty() {
                    metadata.insert("language".to_string(), lang.clone());
                }

                Chunk {
                    chunk_id: format!("{}:{}", document.id, i),
                    document_id: document.id.clone(),
                    content: fc.text,
                    index: i,
                    char_offset: 0, // FileChunk doesn't track char offsets
                    metadata,
                }
            })
            .collect();

        Ok(chunks)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_chunker_with_language_rust() {
        let chunker = CodeChunker::with_language("rust");
        let doc = Document::new(
            "test-rs",
            "fn main() {\n    println!(\"hello\");\n}\n\nfn helper() -> i32 {\n    42\n}\n",
        );
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(
            !chunks.is_empty(),
            "Should produce non-empty chunks for Rust code"
        );
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.chunk_id, format!("test-rs:{}", i));
            assert_eq!(chunk.document_id, "test-rs");
            assert_eq!(chunk.metadata.get("chunker"), Some(&"code".to_string()));
            assert_eq!(chunk.metadata.get("language"), Some(&"rust".to_string()));
        }
    }

    #[test]
    fn code_chunker_infers_language_from_metadata() {
        let chunker = CodeChunker::new();
        let mut doc = Document::new(
            "test-py",
            "def foo():\n    pass\n\ndef bar():\n    return 1\n",
        );
        doc.metadata
            .insert("language".to_string(), "python".to_string());
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(!chunks.is_empty());
        assert_eq!(
            chunks[0].metadata.get("language"),
            Some(&"python".to_string())
        );
    }

    #[test]
    fn code_chunker_empty_content() {
        let chunker = CodeChunker::with_language("rust");
        let doc = Document::new("empty", "");
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn code_chunker_fallback_for_unknown_language() {
        let chunker = CodeChunker::with_language("brainfuck");
        let doc = Document::new(
            "bf",
            "some random content\nmore lines\n".repeat(50).as_str(),
        );
        let chunks = chunker.chunk(&doc).unwrap();
        // Should still produce chunks via the fixed-window fallback
        assert!(!chunks.is_empty());
    }
}
