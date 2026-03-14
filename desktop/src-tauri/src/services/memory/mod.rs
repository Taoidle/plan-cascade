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
pub mod query_policy_v2;
pub mod query_v2;
pub mod retrieval;
pub mod store;

pub use extraction::{detect_memory_command, MemoryCommand, MemoryExtractor};
pub use maintenance::MemoryMaintenance;
pub use query_policy_v2::{
    memory_query_tuning_v2, tuning_for_context_envelope_v2, tuning_for_task_context_v2,
    MemoryQueryPresetV2, MemoryQueryTuningV2, DEFAULT_MIN_IMPORTANCE_V2,
    DEFAULT_PER_SCOPE_BUDGET_V2, DEFAULT_TOP_K_TOTAL_V2,
};
pub use query_v2::{
    list_memory_entries_v2, list_pending_memory_candidates_v2, memory_stats_v2, purge_memories_v2,
    query_memory_entries_v2, restore_deleted_memories_v2, review_memory_candidates_v2,
    set_memory_status_v2, MemoryReviewCandidateV2, MemoryReviewDecisionV2, MemoryReviewSummaryV2,
    MemoryScopeV2, MemoryStatusV2, RiskTierV2, UnifiedMemoryQueryRequestV2,
    UnifiedMemoryQueryResultV2,
};
pub use retrieval::compute_relevance_score;
pub use store::{
    MemoryCategory, MemoryEntry, MemorySearchRequest, MemorySearchResult, MemoryStats,
    MemoryUpdate, NewMemoryEntry, ProjectMemoryStore, UpsertResult,
};
