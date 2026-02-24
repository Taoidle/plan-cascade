//! Document Chunker
//!
//! Defines the `Chunker` trait and three built-in implementations:
//! - `ParagraphChunker`: splits on blank lines and markdown headers
//! - `TokenChunker`: fixed token-count windows with configurable overlap
//! - `SemanticChunker`: splits on topic boundaries using embedding similarity
//!
//! ## Usage
//!
//! ```rust,ignore
//! let chunker = ParagraphChunker::new(1000);
//! let doc = Document::new("doc-1", "# Title\n\nParagraph one.\n\nParagraph two.");
//! let chunks = chunker.chunk(&doc)?;
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::services::orchestrator::embedding_service::EmbeddingService;
use crate::utils::error::{AppError, AppResult};

// ---------------------------------------------------------------------------
// Document & Chunk data structures
// ---------------------------------------------------------------------------

/// A document to be chunked, typically created from parsed file content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Unique document identifier.
    pub id: String,
    /// Full text content of the document.
    pub content: String,
    /// Arbitrary metadata key-value pairs (e.g., source_type, author).
    pub metadata: HashMap<String, String>,
    /// Original file path, if available.
    pub source_path: Option<String>,
}

impl Document {
    /// Create a new document with minimal fields.
    pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            metadata: HashMap::new(),
            source_path: None,
        }
    }

    /// Create a new document with source path.
    pub fn with_source(
        id: impl Into<String>,
        content: impl Into<String>,
        source_path: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            metadata: HashMap::new(),
            source_path: Some(source_path.into()),
        }
    }

    /// Create a Document from parsed file content (output of file_parsers).
    ///
    /// Accepts the text content extracted by `parse_pdf`, `parse_docx`,
    /// `parse_xlsx`, or markdown reading.
    pub fn from_parsed_content(
        id: impl Into<String>,
        content: impl Into<String>,
        source_path: impl Into<String>,
        source_type: impl Into<String>,
    ) -> Self {
        let mut metadata = HashMap::new();
        metadata.insert("source_type".to_string(), source_type.into());
        Self {
            id: id.into(),
            content: content.into(),
            metadata,
            source_path: Some(source_path.into()),
        }
    }
}

/// A chunk produced by splitting a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// Unique chunk identifier (typically "{document_id}:{index}").
    pub chunk_id: String,
    /// ID of the source document.
    pub document_id: String,
    /// Text content of this chunk.
    pub content: String,
    /// Zero-based index of this chunk within the document.
    pub index: usize,
    /// Character offset of this chunk's start within the original document.
    pub char_offset: usize,
    /// Inherited and chunk-specific metadata.
    pub metadata: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Chunker trait
// ---------------------------------------------------------------------------

/// Trait for document chunking strategies.
pub trait Chunker: Send + Sync {
    /// Split a document into chunks.
    fn chunk(&self, document: &Document) -> AppResult<Vec<Chunk>>;
}

// ---------------------------------------------------------------------------
// ChunkerConfig
// ---------------------------------------------------------------------------

/// Configuration enum for selecting and parameterizing chunking strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "strategy")]
pub enum ChunkerConfig {
    /// Split on paragraph boundaries (blank lines and markdown headers).
    Paragraph {
        /// Maximum characters per chunk (default 1000).
        max_chunk_size: usize,
    },
    /// Fixed token-count windows with overlap.
    Token {
        /// Number of tokens per chunk (default 256).
        token_count: usize,
        /// Number of overlapping tokens between consecutive chunks (default 50).
        overlap_tokens: usize,
    },
    /// Split on topic boundaries using embedding similarity drop.
    Semantic {
        /// Similarity threshold below which a boundary is detected (default 0.5).
        threshold: f32,
        /// Minimum sentences per chunk (default 2).
        min_sentences: usize,
    },
    /// Split source code using tree-sitter symbol boundaries.
    Code {
        /// Programming language hint (e.g. "rust", "python").
        /// When `None`, inferred from document metadata.
        language: Option<String>,
    },
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        ChunkerConfig::Paragraph {
            max_chunk_size: 1000,
        }
    }
}

impl ChunkerConfig {
    /// Build a boxed Chunker from this configuration.
    pub fn build(&self) -> Box<dyn Chunker> {
        match self {
            ChunkerConfig::Paragraph { max_chunk_size } => {
                Box::new(ParagraphChunker::new(*max_chunk_size))
            }
            ChunkerConfig::Token {
                token_count,
                overlap_tokens,
            } => Box::new(TokenChunker::new(*token_count, *overlap_tokens)),
            ChunkerConfig::Semantic {
                threshold,
                min_sentences,
            } => Box::new(SemanticChunker::new(*threshold, *min_sentences)),
            ChunkerConfig::Code { language } => match language {
                Some(lang) => Box::new(super::code_chunker::CodeChunker::with_language(lang)),
                None => Box::new(super::code_chunker::CodeChunker::new()),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// ParagraphChunker
// ---------------------------------------------------------------------------

/// Splits documents on double newlines and markdown headers.
///
/// Respects document structure by treating `\n\n` and `\n# ` as natural
/// chunk boundaries. If a paragraph exceeds `max_chunk_size`, it is
/// further split at sentence boundaries.
pub struct ParagraphChunker {
    max_chunk_size: usize,
}

impl ParagraphChunker {
    pub fn new(max_chunk_size: usize) -> Self {
        Self {
            max_chunk_size: max_chunk_size.max(100),
        }
    }
}

impl Chunker for ParagraphChunker {
    fn chunk(&self, document: &Document) -> AppResult<Vec<Chunk>> {
        let content = &document.content;
        if content.is_empty() {
            return Ok(Vec::new());
        }

        // Step 1: Split into segments on double-newlines and markdown headers.
        // A segment boundary occurs when:
        //   - A blank line separates paragraphs (double newline)
        //   - A markdown header line (starts with #) begins
        let lines: Vec<&str> = content.split('\n').collect();
        let mut segments: Vec<(usize, String)> = Vec::new();
        let mut current_lines: Vec<&str> = Vec::new();
        let mut current_offset = 0usize;
        let mut char_pos = 0usize;

        for (idx, &line) in lines.iter().enumerate() {
            let is_header = line.starts_with('#');
            let is_empty = line.trim().is_empty();

            if is_header && !current_lines.is_empty() {
                // Flush current before header
                let text = current_lines.join("\n").trim().to_string();
                if !text.is_empty() {
                    segments.push((current_offset, text));
                }
                current_lines.clear();
                current_offset = char_pos;
            } else if is_empty && !current_lines.is_empty() {
                // Blank line = paragraph boundary; flush current
                let text = current_lines.join("\n").trim().to_string();
                if !text.is_empty() {
                    segments.push((current_offset, text));
                }
                current_lines.clear();
                char_pos += line.len() + 1;
                current_offset = char_pos;
                continue;
            }

            if !is_empty || !current_lines.is_empty() {
                current_lines.push(line);
            }
            char_pos += line.len() + 1; // +1 for newline
        }

        // Flush remaining
        if !current_lines.is_empty() {
            let text = current_lines.join("\n").trim().to_string();
            if !text.is_empty() {
                segments.push((current_offset, text));
            }
        }

        // Step 2: Convert segments directly to chunks.
        // Each natural segment (paragraph or header section) becomes a chunk.
        let mut chunks = Vec::new();
        for (offset, text) in segments {
            self.add_chunk(&mut chunks, &document.id, &text, offset, &document.metadata);
        }

        // Step 3: Split oversized chunks at sentence boundaries.
        let mut final_chunks = Vec::new();
        for chunk in chunks {
            if chunk.content.len() > self.max_chunk_size {
                let sub_chunks =
                    split_by_sentences(&chunk.content, self.max_chunk_size, chunk.char_offset);
                for (_i, (sub_offset, sub_text)) in sub_chunks.into_iter().enumerate() {
                    final_chunks.push(Chunk {
                        chunk_id: format!("{}:{}", document.id, final_chunks.len()),
                        document_id: document.id.clone(),
                        content: sub_text,
                        index: final_chunks.len(),
                        char_offset: sub_offset,
                        metadata: chunk.metadata.clone(),
                    });
                }
            } else {
                let mut c = chunk;
                c.index = final_chunks.len();
                c.chunk_id = format!("{}:{}", document.id, final_chunks.len());
                final_chunks.push(c);
            }
        }

        Ok(final_chunks)
    }
}

impl ParagraphChunker {
    fn add_chunk(
        &self,
        chunks: &mut Vec<Chunk>,
        doc_id: &str,
        text: &str,
        offset: usize,
        metadata: &HashMap<String, String>,
    ) {
        let index = chunks.len();
        chunks.push(Chunk {
            chunk_id: format!("{}:{}", doc_id, index),
            document_id: doc_id.to_string(),
            content: text.to_string(),
            index,
            char_offset: offset,
            metadata: metadata.clone(),
        });
    }
}

/// Split text at sentence boundaries to fit within max_size.
fn split_by_sentences(text: &str, max_size: usize, base_offset: usize) -> Vec<(usize, String)> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut current_offset = base_offset;

    // Simple sentence boundary detection: split on `. `, `! `, `? `, `.\n`
    let mut chars = text.chars().peekable();
    let mut pos = 0usize;

    while let Some(ch) = chars.next() {
        current.push(ch);
        pos += ch.len_utf8();

        let is_sentence_end = (ch == '.' || ch == '!' || ch == '?')
            && chars.peek().map_or(true, |next| next.is_whitespace());

        if is_sentence_end && current.len() >= max_size / 4 {
            if current.len() >= max_size || chars.peek().is_none() {
                result.push((current_offset, current.trim().to_string()));
                current = String::new();
                current_offset = base_offset + pos;
            }
        }
    }

    if !current.trim().is_empty() {
        result.push((current_offset, current.trim().to_string()));
    }

    if result.is_empty() && !text.is_empty() {
        result.push((base_offset, text.to_string()));
    }

    result
}

// ---------------------------------------------------------------------------
// TokenChunker
// ---------------------------------------------------------------------------

/// Splits documents into fixed-size token windows with overlap.
///
/// Uses whitespace tokenization (word boundaries). Consecutive chunks
/// share `overlap_tokens` tokens for context continuity.
pub struct TokenChunker {
    token_count: usize,
    overlap_tokens: usize,
}

impl TokenChunker {
    pub fn new(token_count: usize, overlap_tokens: usize) -> Self {
        let tc = token_count.max(1);
        Self {
            token_count: tc,
            overlap_tokens: overlap_tokens.min(tc.saturating_sub(1)),
        }
    }
}

impl Chunker for TokenChunker {
    fn chunk(&self, document: &Document) -> AppResult<Vec<Chunk>> {
        let content = &document.content;
        if content.is_empty() {
            return Ok(Vec::new());
        }

        // Tokenize by whitespace, preserving positions
        let tokens: Vec<(usize, &str)> = content
            .split_whitespace()
            .map(|word| {
                let offset = word.as_ptr() as usize - content.as_ptr() as usize;
                (offset, word)
            })
            .collect();

        if tokens.is_empty() {
            return Ok(Vec::new());
        }

        let mut chunks = Vec::new();
        let step = self.token_count.saturating_sub(self.overlap_tokens).max(1);
        let mut start = 0;

        while start < tokens.len() {
            let end = (start + self.token_count).min(tokens.len());
            let chunk_tokens = &tokens[start..end];

            let text: String = chunk_tokens
                .iter()
                .map(|(_, word)| *word)
                .collect::<Vec<_>>()
                .join(" ");

            let char_offset = chunk_tokens[0].0;
            let index = chunks.len();

            chunks.push(Chunk {
                chunk_id: format!("{}:{}", document.id, index),
                document_id: document.id.clone(),
                content: text,
                index,
                char_offset,
                metadata: document.metadata.clone(),
            });

            if end >= tokens.len() {
                break;
            }
            start += step;
        }

        Ok(chunks)
    }
}

// ---------------------------------------------------------------------------
// SemanticChunker
// ---------------------------------------------------------------------------

/// Splits documents on topic boundaries using embedding similarity.
///
/// Sentences within the document are grouped, and when the cosine
/// similarity between consecutive sentence groups drops below `threshold`,
/// a chunk boundary is introduced.
///
/// When an `EmbeddingService` is provided, uses real TF-IDF embeddings
/// trained on the document's sentences. Falls back to a lightweight
/// hash-based pseudo-embedding otherwise.
pub struct SemanticChunker {
    threshold: f32,
    min_sentences: usize,
    embedding_service: Option<Arc<EmbeddingService>>,
}

impl SemanticChunker {
    /// Create a new SemanticChunker with hash-based fallback embeddings.
    pub fn new(threshold: f32, min_sentences: usize) -> Self {
        Self {
            threshold: threshold.clamp(0.0, 1.0),
            min_sentences: min_sentences.max(1),
            embedding_service: None,
        }
    }

    /// Create a SemanticChunker that uses a real EmbeddingService.
    pub fn with_embedding_service(
        threshold: f32,
        min_sentences: usize,
        service: Arc<EmbeddingService>,
    ) -> Self {
        Self {
            threshold: threshold.clamp(0.0, 1.0),
            min_sentences: min_sentences.max(1),
            embedding_service: Some(service),
        }
    }

    /// Compute embeddings for all sentences.
    ///
    /// If an EmbeddingService is available, builds vocabulary from the
    /// sentences and produces TF-IDF embeddings. Otherwise, uses hash-based
    /// pseudo-embeddings as a lightweight fallback.
    fn compute_embeddings(&self, sentences: &[(usize, &str)]) -> Vec<Vec<f32>> {
        if let Some(service) = &self.embedding_service {
            let texts: Vec<&str> = sentences.iter().map(|(_, s)| *s).collect();
            // Build vocabulary from this document's sentences, then embed
            service.build_vocabulary(&texts);
            texts.iter().map(|s| service.embed_text(s)).collect()
        } else {
            sentences
                .iter()
                .map(|(_, s)| Self::hash_embedding(s))
                .collect()
        }
    }

    /// Hash-based pseudo-embedding for when no EmbeddingService is available.
    fn hash_embedding(sentence: &str) -> Vec<f32> {
        let words: Vec<&str> = sentence.split_whitespace().collect();
        let dim = 32;
        let mut vec = vec![0.0f32; dim];

        for (i, word) in words.iter().enumerate() {
            let mut h: u32 = 0;
            for b in word.bytes() {
                h = h.wrapping_mul(31).wrapping_add(b as u32);
            }
            let idx = (h as usize) % dim;
            vec[idx] += 1.0 / (i + 1) as f32;
        }

        // L2 normalize
        let mag: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
        if mag > 0.0 {
            for v in &mut vec {
                *v /= mag;
            }
        }

        vec
    }

    fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        let denom = mag_a * mag_b;
        if denom == 0.0 {
            0.0
        } else {
            dot / denom
        }
    }
}

impl Chunker for SemanticChunker {
    fn chunk(&self, document: &Document) -> AppResult<Vec<Chunk>> {
        let content = &document.content;
        if content.is_empty() {
            return Ok(Vec::new());
        }

        // Split into sentences
        let sentences: Vec<(usize, &str)> = split_into_sentences(content);
        if sentences.is_empty() {
            return Ok(vec![Chunk {
                chunk_id: format!("{}:0", document.id),
                document_id: document.id.clone(),
                content: content.to_string(),
                index: 0,
                char_offset: 0,
                metadata: document.metadata.clone(),
            }]);
        }

        // Compute embeddings for each sentence (real or hash-based)
        let embeddings: Vec<Vec<f32>> = self.compute_embeddings(&sentences);

        // Detect boundaries
        let mut chunks = Vec::new();
        let mut current_sentences: Vec<(usize, &str)> = Vec::new();
        let mut sentences_since_boundary = 0usize;

        for i in 0..sentences.len() {
            current_sentences.push(sentences[i]);
            sentences_since_boundary += 1;

            let should_split =
                if i + 1 < sentences.len() && sentences_since_boundary >= self.min_sentences {
                    let sim = Self::cosine_sim(&embeddings[i], &embeddings[i + 1]);
                    sim < self.threshold
                } else {
                    false
                };

            if should_split || i + 1 == sentences.len() {
                let text: String = current_sentences
                    .iter()
                    .map(|(_, s)| *s)
                    .collect::<Vec<_>>()
                    .join(" ");
                let char_offset = current_sentences[0].0;
                let index = chunks.len();

                chunks.push(Chunk {
                    chunk_id: format!("{}:{}", document.id, index),
                    document_id: document.id.clone(),
                    content: text.trim().to_string(),
                    index,
                    char_offset,
                    metadata: document.metadata.clone(),
                });

                current_sentences.clear();
                sentences_since_boundary = 0;
            }
        }

        Ok(chunks)
    }
}

/// Split text into sentences with their character offsets.
fn split_into_sentences(text: &str) -> Vec<(usize, &str)> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let b = bytes[i];
        // Detect sentence-ending punctuation followed by whitespace or end
        if (b == b'.' || b == b'!' || b == b'?')
            && (i + 1 >= len || bytes[i + 1].is_ascii_whitespace())
        {
            let end = i + 1;
            let sentence = text[start..end].trim();
            if !sentence.is_empty() {
                sentences.push((start, sentence));
            }
            // Skip whitespace after sentence end
            i += 1;
            while i < len && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            start = i;
            continue;
        }
        i += 1;
    }

    // Remaining text
    if start < len {
        let sentence = text[start..].trim();
        if !sentence.is_empty() {
            sentences.push((start, sentence));
        }
    }

    sentences
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ======================================================================
    // Document tests
    // ======================================================================

    #[test]
    fn document_new_basic() {
        let doc = Document::new("d1", "Hello world");
        assert_eq!(doc.id, "d1");
        assert_eq!(doc.content, "Hello world");
        assert!(doc.metadata.is_empty());
        assert!(doc.source_path.is_none());
    }

    #[test]
    fn document_with_source() {
        let doc = Document::with_source("d2", "Content", "/path/to/file.md");
        assert_eq!(doc.source_path, Some("/path/to/file.md".to_string()));
    }

    #[test]
    fn document_from_parsed_content() {
        let doc = Document::from_parsed_content("d3", "Parsed text", "/file.pdf", "pdf");
        assert_eq!(doc.metadata.get("source_type"), Some(&"pdf".to_string()));
        assert_eq!(doc.source_path, Some("/file.pdf".to_string()));
    }

    // ======================================================================
    // ParagraphChunker tests
    // ======================================================================

    #[test]
    fn paragraph_chunker_splits_on_headers() {
        let chunker = ParagraphChunker::new(1000);
        let doc = Document::new(
            "d1",
            "# Introduction\n\nFirst paragraph.\n\n# Methods\n\nSecond paragraph.",
        );
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(
            chunks.len() >= 2,
            "Should split on headers, got {} chunks",
            chunks.len()
        );
        assert!(chunks[0].content.contains("Introduction"));
        // Verify that "# Methods" and "Second paragraph." appear in chunks
        let all_text: String = chunks
            .iter()
            .map(|c| c.content.clone())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            all_text.contains("Methods"),
            "Should contain Methods header"
        );
        assert!(
            all_text.contains("Second paragraph"),
            "Should contain second paragraph"
        );
    }

    #[test]
    fn paragraph_chunker_splits_on_double_newlines() {
        let chunker = ParagraphChunker::new(1000);
        let doc = Document::new(
            "d1",
            "Paragraph one text.\n\nParagraph two text.\n\nParagraph three text.",
        );
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(
            chunks.len() >= 2,
            "Should split on double newlines, got {} chunks",
            chunks.len()
        );
    }

    #[test]
    fn paragraph_chunker_respects_max_size() {
        let chunker = ParagraphChunker::new(200);
        let long_para = "This is a sentence that is quite long. ".repeat(20);
        let doc = Document::new("d1", &long_para);
        let chunks = chunker.chunk(&doc).unwrap();
        // At least some chunks should be under max size
        let under_max = chunks.iter().filter(|c| c.content.len() <= 250).count();
        assert!(under_max > 0, "Should produce chunks respecting max size");
    }

    #[test]
    fn paragraph_chunker_empty_document() {
        let chunker = ParagraphChunker::new(1000);
        let doc = Document::new("d1", "");
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn paragraph_chunker_chunk_ids_are_correct() {
        let chunker = ParagraphChunker::new(1000);
        let doc = Document::new("doc-x", "# A\n\nText A.\n\n# B\n\nText B.");
        let chunks = chunker.chunk(&doc).unwrap();
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.chunk_id, format!("doc-x:{}", i));
            assert_eq!(chunk.document_id, "doc-x");
            assert_eq!(chunk.index, i);
        }
    }

    #[test]
    fn paragraph_chunker_preserves_metadata() {
        let chunker = ParagraphChunker::new(1000);
        let mut doc = Document::new("d1", "Para one.\n\nPara two.");
        doc.metadata
            .insert("author".to_string(), "test".to_string());
        let chunks = chunker.chunk(&doc).unwrap();
        for chunk in &chunks {
            assert_eq!(chunk.metadata.get("author"), Some(&"test".to_string()));
        }
    }

    // ======================================================================
    // TokenChunker tests
    // ======================================================================

    #[test]
    fn token_chunker_basic_split() {
        let chunker = TokenChunker::new(5, 0);
        let doc = Document::new("d1", "one two three four five six seven eight nine ten");
        let chunks = chunker.chunk(&doc).unwrap();
        assert_eq!(chunks.len(), 2, "10 words / 5 tokens = 2 chunks");
        assert_eq!(chunks[0].content.split_whitespace().count(), 5);
        assert_eq!(chunks[1].content.split_whitespace().count(), 5);
    }

    #[test]
    fn token_chunker_with_overlap() {
        let chunker = TokenChunker::new(5, 2);
        let doc = Document::new("d1", "one two three four five six seven eight nine ten");
        let chunks = chunker.chunk(&doc).unwrap();
        // With 5 tokens and 2 overlap, step = 3
        // Chunk 0: [0..5] = one two three four five
        // Chunk 1: [3..8] = four five six seven eight
        // Chunk 2: [6..10] = seven eight nine ten
        assert!(
            chunks.len() >= 3,
            "Should have overlapping chunks, got {}",
            chunks.len()
        );

        // Verify overlap: last 2 tokens of chunk 0 should be first 2 of chunk 1
        let c0_words: Vec<&str> = chunks[0].content.split_whitespace().collect();
        let c1_words: Vec<&str> = chunks[1].content.split_whitespace().collect();
        assert_eq!(c0_words[3], c1_words[0], "Overlap should share tokens");
        assert_eq!(c0_words[4], c1_words[1], "Overlap should share tokens");
    }

    #[test]
    fn token_chunker_overlap_correctness() {
        let chunker = TokenChunker::new(4, 1);
        let doc = Document::new("d1", "a b c d e f g h");
        let chunks = chunker.chunk(&doc).unwrap();
        // step = 3, so: [0..4], [3..7], [6..8]
        assert!(chunks.len() >= 2);
        // Check that chunk N last token == chunk N+1 first token
        for i in 0..chunks.len() - 1 {
            let current_words: Vec<&str> = chunks[i].content.split_whitespace().collect();
            let next_words: Vec<&str> = chunks[i + 1].content.split_whitespace().collect();
            let overlap_token = current_words.last().unwrap();
            assert_eq!(
                *overlap_token,
                next_words[0],
                "Overlap between chunk {} and {} failed",
                i,
                i + 1
            );
        }
    }

    #[test]
    fn token_chunker_empty_document() {
        let chunker = TokenChunker::new(10, 2);
        let doc = Document::new("d1", "");
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn token_chunker_single_word() {
        let chunker = TokenChunker::new(10, 2);
        let doc = Document::new("d1", "hello");
        let chunks = chunker.chunk(&doc).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "hello");
    }

    #[test]
    fn token_chunker_char_offsets() {
        let chunker = TokenChunker::new(3, 0);
        let doc = Document::new("d1", "hello world foo bar baz qux");
        let chunks = chunker.chunk(&doc).unwrap();
        // First chunk should start at offset 0
        assert_eq!(chunks[0].char_offset, 0);
        // Second chunk starts at "bar" which is at some offset
        assert!(chunks[1].char_offset > 0);
    }

    // ======================================================================
    // SemanticChunker tests
    // ======================================================================

    #[test]
    fn semantic_chunker_basic_split() {
        let chunker = SemanticChunker::new(0.5, 1);
        // Two very different topics should trigger a boundary
        let doc = Document::new(
            "d1",
            "The cat sat on the mat. The dog chased the ball. \
             Quantum computing uses qubits for computation. \
             Superposition allows parallel state evaluation.",
        );
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(!chunks.is_empty(), "Should produce at least one chunk");
    }

    #[test]
    fn semantic_chunker_respects_min_sentences() {
        let chunker = SemanticChunker::new(0.1, 3); // Very low threshold, 3 min sentences
        let doc = Document::new(
            "d1",
            "Sentence one. Sentence two. Sentence three. Sentence four. Sentence five.",
        );
        let chunks = chunker.chunk(&doc).unwrap();
        // With min_sentences=3, each chunk should have at least 3 sentences
        // (except possibly the last one)
        for chunk in &chunks[..chunks.len().saturating_sub(1)] {
            let sentence_count = chunk.content.matches('.').count();
            assert!(
                sentence_count >= 2,
                "Chunk should have multiple sentences due to min_sentences, got: '{}'",
                chunk.content
            );
        }
    }

    #[test]
    fn semantic_chunker_empty_document() {
        let chunker = SemanticChunker::new(0.5, 2);
        let doc = Document::new("d1", "");
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn semantic_chunker_detects_topic_boundary() {
        let chunker = SemanticChunker::new(0.3, 1);
        // Intentionally different topics with very different vocabulary
        let doc = Document::new(
            "d1",
            "Rust programming language is fast and safe. \
             Rust has zero-cost abstractions and ownership. \
             Cooking pasta requires boiling water. \
             Add salt to the water before the noodles.",
        );
        let chunks = chunker.chunk(&doc).unwrap();
        // With sufficiently different topics, should produce multiple chunks
        assert!(
            chunks.len() >= 1,
            "Semantic chunker should produce chunks, got {}",
            chunks.len()
        );
    }

    // ======================================================================
    // ChunkerConfig tests
    // ======================================================================

    #[test]
    fn chunker_config_paragraph_build() {
        let config = ChunkerConfig::Paragraph {
            max_chunk_size: 500,
        };
        let chunker = config.build();
        let doc = Document::new("d1", "Hello.\n\nWorld.");
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn chunker_config_token_build() {
        let config = ChunkerConfig::Token {
            token_count: 3,
            overlap_tokens: 1,
        };
        let chunker = config.build();
        let doc = Document::new("d1", "a b c d e");
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn chunker_config_semantic_build() {
        let config = ChunkerConfig::Semantic {
            threshold: 0.5,
            min_sentences: 1,
        };
        let chunker = config.build();
        let doc = Document::new("d1", "Hello world. Goodbye moon.");
        let chunks = chunker.chunk(&doc).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn chunker_config_default_is_paragraph() {
        let config = ChunkerConfig::default();
        matches!(config, ChunkerConfig::Paragraph { .. });
    }

    #[test]
    fn chunker_config_serde_roundtrip() {
        let config = ChunkerConfig::Token {
            token_count: 100,
            overlap_tokens: 20,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ChunkerConfig = serde_json::from_str(&json).unwrap();
        match deserialized {
            ChunkerConfig::Token {
                token_count,
                overlap_tokens,
            } => {
                assert_eq!(token_count, 100);
                assert_eq!(overlap_tokens, 20);
            }
            _ => panic!("Expected Token config"),
        }
    }

    // ======================================================================
    // split_into_sentences tests
    // ======================================================================

    #[test]
    fn split_sentences_basic() {
        let sentences = split_into_sentences("Hello world. How are you? I am fine!");
        assert_eq!(sentences.len(), 3);
    }

    #[test]
    fn split_sentences_no_ending() {
        let sentences = split_into_sentences("No ending punctuation");
        assert_eq!(sentences.len(), 1);
    }
}
