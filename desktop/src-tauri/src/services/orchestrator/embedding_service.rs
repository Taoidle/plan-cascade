//! TF-IDF Embedding Service
//!
//! Provides local, offline text vectorization using TF-IDF (Term Frequency —
//! Inverse Document Frequency).  Designed as a drop-in replacement surface for
//! heavier ML-based embeddings (e.g. fastembed/ONNX) — the public API
//! (`embed_text`, `embed_batch`, `cosine_similarity`) stays the same regardless
//! of the backend.
//!
//! ## Design Decisions (ADR-002 / ADR-003)
//!
//! * **No external ML dependency** — pure Rust, zero ONNX overhead.
//! * **Fixed-size vocabulary** — built from the first `embed_batch` call.
//!   Subsequent calls reuse the same vocabulary for consistency.
//! * **Thread-safe** via `Arc<Mutex<...>>` so it can be shared across the
//!   background indexer and tool executor.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Maximum vocabulary size (number of unique tokens tracked).
const MAX_VOCAB_SIZE: usize = 8192;

/// Minimum document frequency — tokens appearing in fewer documents are pruned.
const MIN_DOC_FREQUENCY: usize = 1;

/// Result of a semantic search query.
#[derive(Debug, Clone)]
pub struct SemanticSearchResult {
    pub file_path: String,
    pub chunk_index: i64,
    pub chunk_text: String,
    pub similarity: f32,
}

/// Internal vocabulary learned from a corpus.
#[derive(Debug, Clone)]
struct Vocabulary {
    /// Map from token → column index in the TF-IDF vector.
    token_to_idx: HashMap<String, usize>,
    /// Inverse document frequency for each token (same order as `token_to_idx` values).
    idf: Vec<f32>,
    /// Total number of documents the vocabulary was built from.
    num_docs: usize,
}

/// Thread-safe TF-IDF embedding service.
///
/// Lazily initializes its vocabulary on the first `embed_batch` call.
/// After that the vocabulary is frozen and reused for all subsequent calls.
#[derive(Debug, Clone)]
pub struct EmbeddingService {
    inner: Arc<Mutex<EmbeddingServiceInner>>,
}

#[derive(Debug)]
struct EmbeddingServiceInner {
    vocab: Option<Vocabulary>,
}

impl EmbeddingService {
    /// Create a new, uninitialised embedding service.
    ///
    /// The vocabulary will be built lazily on the first `embed_batch` call.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(EmbeddingServiceInner { vocab: None })),
        }
    }

    /// Return the dimensionality of the embedding vectors.
    ///
    /// Returns 0 if the vocabulary has not been built yet.
    pub fn dimension(&self) -> usize {
        let guard = self.inner.lock().unwrap();
        guard.vocab.as_ref().map_or(0, |v| v.idf.len())
    }

    /// Build (or rebuild) the vocabulary from a corpus of documents.
    ///
    /// Each entry in `corpus` is the full text of one document.  After this
    /// call, `embed_text` and `embed_batch` will produce vectors of the same
    /// dimensionality as the vocabulary.
    pub fn build_vocabulary(&self, corpus: &[&str]) {
        let vocab = build_vocab(corpus);
        let mut guard = self.inner.lock().unwrap();
        guard.vocab = Some(vocab);
    }

    /// Embed a single text string into a TF-IDF vector.
    ///
    /// Returns an empty vector if the vocabulary has not been built yet.
    pub fn embed_text(&self, text: &str) -> Vec<f32> {
        let guard = self.inner.lock().unwrap();
        match &guard.vocab {
            Some(vocab) => tfidf_vector(text, vocab),
            None => Vec::new(),
        }
    }

    /// Embed a batch of text strings.
    ///
    /// If the vocabulary has not been built yet, it is built from the provided
    /// texts (treating them as the corpus).  Subsequent calls reuse the
    /// existing vocabulary.
    pub fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        {
            let mut guard = self.inner.lock().unwrap();
            if guard.vocab.is_none() {
                guard.vocab = Some(build_vocab(texts));
            }
        }
        // Re-acquire read access (we could keep the guard, but releasing and
        // re-locking is fine for the batch sizes we expect).
        let guard = self.inner.lock().unwrap();
        let vocab = guard.vocab.as_ref().unwrap();
        texts.iter().map(|t| tfidf_vector(t, vocab)).collect()
    }

    /// Check whether the vocabulary has been initialised.
    pub fn is_ready(&self) -> bool {
        self.inner.lock().unwrap().vocab.is_some()
    }
}

// ---------------------------------------------------------------------------
// Cosine similarity
// ---------------------------------------------------------------------------

/// Compute the cosine similarity between two vectors.
///
/// Returns 0.0 when either vector has zero magnitude.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut mag_a = 0.0f32;
    let mut mag_b = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        mag_a += x * x;
        mag_b += y * y;
    }

    let denom = mag_a.sqrt() * mag_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

// ---------------------------------------------------------------------------
// Tokenisation helpers
// ---------------------------------------------------------------------------

/// Simple tokeniser: lowercase, split on non-alphanumeric, filter short tokens.
///
/// Also splits camelCase and snake_case identifiers into sub-tokens for better
/// code search results.
fn tokenize(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut tokens: Vec<String> = Vec::new();

    for word in lower.split(|c: char| !c.is_alphanumeric() && c != '_') {
        let word = word.trim_matches('_');
        if word.is_empty() || word.len() < 2 {
            continue;
        }
        // Split camelCase
        let parts = split_camel_case(word);
        for part in &parts {
            if part.len() >= 2 {
                tokens.push(part.clone());
            }
        }
        // Also push the full word for exact-match boost
        if parts.len() > 1 && word.len() >= 2 {
            tokens.push(word.to_string());
        }
    }

    tokens
}

/// Split a camelCase or snake_case string into parts.
fn split_camel_case(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();

    for ch in s.chars() {
        if ch == '_' {
            if !current.is_empty() {
                parts.push(current.clone());
                current.clear();
            }
        } else if ch.is_uppercase() && !current.is_empty() {
            parts.push(current.clone());
            current.clear();
            current.push(ch.to_lowercase().next().unwrap_or(ch));
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    if parts.is_empty() {
        parts.push(s.to_string());
    }

    parts
}

// ---------------------------------------------------------------------------
// Vocabulary building
// ---------------------------------------------------------------------------

/// Build a `Vocabulary` from a set of documents.
fn build_vocab(corpus: &[&str]) -> Vocabulary {
    let num_docs = corpus.len().max(1);

    // Count document frequency for each token.
    let mut doc_freq: HashMap<String, usize> = HashMap::new();

    for doc in corpus {
        let tokens = tokenize(doc);
        // De-duplicate tokens within this document.
        let unique: std::collections::HashSet<&str> =
            tokens.iter().map(|s| s.as_str()).collect();
        for tok in unique {
            *doc_freq.entry(tok.to_string()).or_insert(0) += 1;
        }
    }

    // Filter by minimum document frequency and sort by frequency descending
    // (most common tokens get lower indices, which is irrelevant for correctness
    // but helps with debugging).
    let mut entries: Vec<(String, usize)> = doc_freq
        .into_iter()
        .filter(|(_, freq)| *freq >= MIN_DOC_FREQUENCY)
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    entries.truncate(MAX_VOCAB_SIZE);

    let mut token_to_idx = HashMap::with_capacity(entries.len());
    let mut idf = Vec::with_capacity(entries.len());

    for (idx, (token, freq)) in entries.into_iter().enumerate() {
        token_to_idx.insert(token, idx);
        // Standard IDF formula: log(N / df) + 1 (the +1 prevents zero IDF).
        let idf_val = ((num_docs as f32) / (freq as f32)).ln() + 1.0;
        idf.push(idf_val);
    }

    Vocabulary {
        token_to_idx,
        idf,
        num_docs,
    }
}

// ---------------------------------------------------------------------------
// TF-IDF vector computation
// ---------------------------------------------------------------------------

/// Compute a normalised TF-IDF vector for `text` using the given vocabulary.
fn tfidf_vector(text: &str, vocab: &Vocabulary) -> Vec<f32> {
    let dim = vocab.idf.len();
    if dim == 0 {
        return Vec::new();
    }

    let tokens = tokenize(text);
    let total_tokens = tokens.len().max(1) as f32;

    // Term frequency (normalised by document length)
    let mut tf = vec![0.0f32; dim];
    for tok in &tokens {
        if let Some(&idx) = vocab.token_to_idx.get(tok.as_str()) {
            tf[idx] += 1.0 / total_tokens;
        }
    }

    // Multiply by IDF
    for (i, idf_val) in vocab.idf.iter().enumerate() {
        tf[i] *= idf_val;
    }

    // L2 normalise
    let mag: f32 = tf.iter().map(|v| v * v).sum::<f32>().sqrt();
    if mag > 0.0 {
        for v in &mut tf {
            *v /= mag;
        }
    }

    tf
}

/// Serialize an f32 vector to bytes (little-endian) for BLOB storage.
pub fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(embedding.len() * 4);
    for val in embedding {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

/// Deserialize bytes (little-endian) back to an f32 vector.
pub fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
            f32::from_le_bytes(arr)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Tokenization tests
    // =========================================================================

    #[test]
    fn tokenize_basic() {
        let tokens = tokenize("hello world");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
    }

    #[test]
    fn tokenize_camel_case() {
        let tokens = tokenize("processUserData");
        assert!(tokens.contains(&"process".to_string()));
        assert!(tokens.contains(&"user".to_string()));
        assert!(tokens.contains(&"data".to_string()));
        // Full token also present
        assert!(tokens.contains(&"processuserdata".to_string()));
    }

    #[test]
    fn tokenize_snake_case() {
        let tokens = tokenize("get_user_name");
        assert!(tokens.contains(&"get".to_string()));
        assert!(tokens.contains(&"user".to_string()));
        assert!(tokens.contains(&"name".to_string()));
    }

    #[test]
    fn tokenize_filters_short_tokens() {
        let tokens = tokenize("a b cd ef");
        assert!(!tokens.contains(&"a".to_string()));
        assert!(!tokens.contains(&"b".to_string()));
        assert!(tokens.contains(&"cd".to_string()));
        assert!(tokens.contains(&"ef".to_string()));
    }

    // =========================================================================
    // Vocabulary building tests
    // =========================================================================

    #[test]
    fn build_vocab_basic() {
        let corpus = vec!["hello world", "hello rust", "world rust programming"];
        let vocab = build_vocab(&corpus);
        assert!(!vocab.token_to_idx.is_empty());
        assert!(vocab.token_to_idx.contains_key("hello"));
        assert!(vocab.token_to_idx.contains_key("world"));
        assert!(vocab.token_to_idx.contains_key("rust"));
        assert_eq!(vocab.num_docs, 3);
    }

    #[test]
    fn build_vocab_respects_max_size() {
        // Generate a corpus with many unique tokens
        let docs: Vec<String> = (0..100)
            .map(|i| format!("token_{i} common_word another_common"))
            .collect();
        let refs: Vec<&str> = docs.iter().map(|s| s.as_str()).collect();
        let vocab = build_vocab(&refs);
        assert!(vocab.idf.len() <= MAX_VOCAB_SIZE);
    }

    // =========================================================================
    // TF-IDF vector tests
    // =========================================================================

    #[test]
    fn tfidf_vector_has_correct_dimension() {
        let corpus = vec!["fn main() { println!(\"hello\"); }", "struct Config { name: String }"];
        let vocab = build_vocab(&corpus);
        let vec = tfidf_vector("fn main() { }", &vocab);
        assert_eq!(vec.len(), vocab.idf.len());
    }

    #[test]
    fn tfidf_vector_is_normalised() {
        let corpus = vec!["hello world", "foo bar baz"];
        let vocab = build_vocab(&corpus);
        let vec = tfidf_vector("hello world", &vocab);
        let mag: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
        // Should be approximately 1.0
        assert!((mag - 1.0).abs() < 0.01, "magnitude should be ~1.0, got {}", mag);
    }

    #[test]
    fn tfidf_empty_text_returns_zero_vector() {
        let corpus = vec!["hello world"];
        let vocab = build_vocab(&corpus);
        let vec = tfidf_vector("", &vocab);
        assert!(vec.iter().all(|&v| v == 0.0));
    }

    // =========================================================================
    // Cosine similarity tests
    // =========================================================================

    #[test]
    fn cosine_identical_vectors() {
        let a = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn cosine_orthogonal_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn cosine_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_different_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_empty_vectors() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    // =========================================================================
    // EmbeddingService integration tests
    // =========================================================================

    #[test]
    fn embedding_service_lazy_init() {
        let svc = EmbeddingService::new();
        assert!(!svc.is_ready());
        assert_eq!(svc.dimension(), 0);

        let texts = vec!["fn main() { }", "struct Config { }"];
        let embeddings = svc.embed_batch(&texts);
        assert!(svc.is_ready());
        assert_eq!(embeddings.len(), 2);
        assert!(svc.dimension() > 0);
    }

    #[test]
    fn embedding_service_embed_text_before_vocab() {
        let svc = EmbeddingService::new();
        let vec = svc.embed_text("hello world");
        assert!(vec.is_empty());
    }

    #[test]
    fn embedding_service_embed_text_after_vocab() {
        let svc = EmbeddingService::new();
        svc.build_vocabulary(&["hello world", "foo bar"]);
        let vec = svc.embed_text("hello world");
        assert!(!vec.is_empty());
    }

    #[test]
    fn similar_texts_have_high_similarity() {
        let svc = EmbeddingService::new();
        let corpus = vec![
            "fn process_user_data(user: &User) -> Result<Data>",
            "fn handle_request(req: Request) -> Response",
            "struct UserData { name: String, age: u32 }",
            "fn main() { println!(\"hello\"); }",
        ];
        svc.build_vocabulary(&corpus);

        let v1 = svc.embed_text("fn process_user_data(user: &User) -> Result<Data>");
        let v2 = svc.embed_text("fn handle_user_data(user: &User) -> Data");
        let v3 = svc.embed_text("struct Config { debug: bool }");

        let sim_similar = cosine_similarity(&v1, &v2);
        let sim_different = cosine_similarity(&v1, &v3);

        assert!(
            sim_similar > sim_different,
            "similar texts should score higher: {} vs {}",
            sim_similar,
            sim_different
        );
    }

    // =========================================================================
    // Serialization roundtrip tests
    // =========================================================================

    #[test]
    fn embedding_bytes_roundtrip() {
        let original = vec![1.0f32, -2.5, 0.0, 3.14159, f32::MAX, f32::MIN];
        let bytes = embedding_to_bytes(&original);
        let decoded = bytes_to_embedding(&bytes);
        assert_eq!(original, decoded);
    }

    #[test]
    fn embedding_bytes_empty() {
        let original: Vec<f32> = vec![];
        let bytes = embedding_to_bytes(&original);
        assert!(bytes.is_empty());
        let decoded = bytes_to_embedding(&bytes);
        assert!(decoded.is_empty());
    }

    // =========================================================================
    // Thread safety test
    // =========================================================================

    #[test]
    fn embedding_service_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<EmbeddingService>();
    }
}
