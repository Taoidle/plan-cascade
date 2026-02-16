//! Project Memory System
//!
//! Cross-session persistent memory for storing and retrieving project-level
//! knowledge (user preferences, project conventions, discovered patterns,
//! corrections, and facts).
//!
//! ## Module Structure
//!
//! - `store` — Core `ProjectMemoryStore` with CRUD operations
//! - `retrieval` — Search and 4-signal ranking algorithm
//! - `extraction` — LLM-driven memory extraction and explicit memory commands
//! - `maintenance` — Decay, pruning, and compaction operations

pub mod extraction;
pub mod maintenance;
pub mod retrieval;
pub mod store;

pub use extraction::{detect_memory_command, MemoryCommand, MemoryExtractor};
pub use maintenance::MemoryMaintenance;
pub use retrieval::compute_relevance_score;
pub use store::{
    MemoryCategory, MemoryEntry, MemorySearchRequest, MemorySearchResult, MemoryStats,
    MemoryUpdate, NewMemoryEntry, ProjectMemoryStore, UpsertResult,
};
