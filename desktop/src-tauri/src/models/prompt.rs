//! Prompt Template Models
//!
//! Data structures for the prompt library feature.

use serde::{Deserialize, Serialize};

/// A prompt template in the library
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: String,
    pub title: String,
    pub content: String,
    pub description: Option<String>,
    /// Category: "coding" | "writing" | "analysis" | "custom"
    pub category: String,
    /// Tags as a list of strings (stored as JSON in DB)
    pub tags: Vec<String>,
    /// Extracted {{variable}} names from content (stored as JSON in DB)
    pub variables: Vec<String>,
    /// Whether this is a built-in prompt (cannot be deleted)
    pub is_builtin: bool,
    /// Whether this prompt is pinned to the top
    pub is_pinned: bool,
    /// Number of times this prompt has been used
    pub use_count: u32,
    pub last_used_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Request to create a new prompt template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCreateRequest {
    pub title: String,
    pub content: String,
    pub description: Option<String>,
    pub category: String,
    pub tags: Vec<String>,
    pub is_pinned: bool,
}

/// Request to update an existing prompt template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptUpdateRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub is_pinned: Option<bool>,
}
