# Memory Skill Iteration Plan

**Version**: 1.0.0
**Created**: 2026-02-16
**Status**: Draft
**Reference**: [adk-rust](../../adk-rust/) architecture patterns

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current Architecture Baseline](#2-current-architecture-baseline)
3. [Target Architecture Vision](#3-target-architecture-vision)
4. [P0: Project Memory System](#4-p0-project-memory-system)
5. [P0: Skill System](#5-p0-skill-system)
6. [P0: Memory & Skill Management UI](#6-p0-memory--skill-management-ui)
7. [P1: Agentic Lifecycle Hooks](#7-p1-agentic-lifecycle-hooks)
8. [P1: State Scope Layering](#8-p1-state-scope-layering)
9. [P2: Tool Trait Abstraction](#9-p2-tool-trait-abstraction)
10. [P2: MCP Toolset Runtime Integration](#10-p2-mcp-toolset-runtime-integration)
11. [Cross-Cutting Concerns](#11-cross-cutting-concerns)
12. [Migration & Compatibility](#12-migration--compatibility)
13. [Testing Strategy](#13-testing-strategy)
14. [Appendix](#14-appendix)

---

## 1. Executive Summary

### 1.1 Background

Plan Cascade Desktop currently has strong codebase indexing (Tree-sitter + TF-IDF + SQLite) and session-level memory (`SessionMemoryManager`), but **lacks cross-session persistent memory and reusable skill capabilities**. Each new session starts from scratch — the agent doesn't remember user preferences, project conventions, or successful task patterns from previous sessions.

### 1.2 Goal

Introduce a Memory Skill system that enables the agent to **learn, remember, and reuse knowledge across sessions**, inspired by cognitive science memory models and production frameworks (Letta/MemGPT, Mem0, MemOS, adk-rust).

### 1.3 Key Terminology

| Term | Definition |
|------|-----------|
| **Semantic Memory** | Persistent facts about users and projects (preferences, conventions, patterns) |
| **Episodic Memory** | Records of specific past interactions (what happened, what worked/failed) |
| **Procedural Memory / Skill** | Reusable task execution templates (step-by-step instructions for common operations) |
| **Memory Extraction** | LLM-driven process of distilling session interactions into structured memory entries |
| **Skill Injection** | Matching and prepending relevant skills to user prompts before LLM calls |
| **Memory Decay** | Automatic reduction of importance for stale, unused memories |

### 1.4 Priority Overview

| Priority | Feature | Effort | Impact | Dependencies |
|----------|---------|--------|--------|-------------|
| **P0** | Project Memory System | Medium | Critical | None |
| **P0** | Skill System | Medium | Critical | None |
| **P0** | Memory & Skill Management UI | Low-Medium | High | P0 Memory + Skill |
| **P1** | Agentic Lifecycle Hooks | Medium-High | High | None (but enables P0 integration) |
| **P1** | State Scope Layering | Low | Medium | None |
| **P2** | Tool Trait Abstraction | High | Medium | P1 Hooks |
| **P2** | MCP Toolset Runtime Integration | Medium-High | Medium | P2 Tool Trait |

---

## 2. Current Architecture Baseline

### 2.1 What Exists Today

```
┌─────────────────────────────────────────────────────────────┐
│                     Context Management                       │
│                                                             │
│  Layer 1 (Stable)         → System prompt + Index + Tools   │
│  Layer 2 (Semi-stable)    → SessionMemory (single session)  │
│  Layer 3 (Volatile)       → Conversation messages           │
│                                                             │
│  Codebase Index           → Tree-sitter symbols + TF-IDF    │
│  File Read Dedup          → Content hash cache              │
│  Tool Result Truncation   → Bounded output injection        │
│  Session Persistence      → SQLite (resume capable)         │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Key Existing Components

| Component | File | Role |
|-----------|------|------|
| `SessionMemoryManager` | `services/orchestrator/service_helpers/session_state.rs` | Maintains in-session memory at messages[1] with `[SESSION_MEMORY_V1]` marker |
| `SessionMemory` | Same file | Tracks `files_read`, `key_findings`, `task_description`, `tool_usage_counts` |
| `EmbeddingService` | `services/orchestrator/embedding_service.rs` | TF-IDF vectorization with `Vocabulary` (8192 tokens), cosine similarity search |
| `IndexStore` | `services/orchestrator/index_store.rs` | SQLite tables: `file_index`, `file_symbols`, `file_embeddings`, `embedding_vocabulary` |
| `build_system_prompt()` | `services/tools/system_prompt.rs` | Assembles Layer 1 prompt with project summary injection |
| `ToolResult` / `ReadCacheEntry` | `services/tools/executor.rs` | Tool execution with dedup cache |
| `Database` | `storage/database.rs` | SQLite r2d2 pool with 12 existing tables |
| `ToolDefinition` | `services/tools/definitions.rs` | 14 tools with JSON Schema parameters |

### 2.3 What's Missing

| Gap | Impact |
|-----|--------|
| Session A's learnings are invisible to Session B | User must repeat preferences and conventions |
| No reusable task patterns | Agent reinvents the wheel for similar tasks |
| No learning from failures | Same mistakes can repeat across sessions |
| No lifecycle hooks | All logic hardcoded in `agentic_loop.rs` (2676 lines) |
| Tools hardcoded in `executor.rs` | Cannot dynamically add/remove tools |
| State has no scoping | All state has the same lifetime |

---

## 3. Target Architecture Vision

### 3.1 Memory Hierarchy

```
┌─────────────────────────────────────────────────────────────────┐
│                     Enhanced Context Management                  │
│                                                                 │
│  Layer 1 (Stable)                                               │
│  ├── System prompt                                              │
│  ├── Project index summary          (existing)                  │
│  ├── Tool definitions               (existing)                  │
│  ├── Project Memory (semantic)       (NEW - P0)                 │
│  └── Matched Skills (procedural)     (NEW - P0)                 │
│                                                                 │
│  Layer 2 (Semi-stable)                                          │
│  ├── SessionMemory                   (existing)                 │
│  └── Memory extraction trigger       (NEW - P1 hooks)           │
│                                                                 │
│  Layer 3 (Volatile)                                             │
│  └── Conversation messages           (existing)                 │
│                                                                 │
│  Persistent Storage (NEW)                                       │
│  ├── project_memories table          (NEW - P0)                 │
│  ├── skill_library table             (NEW - P0)                 │
│  └── episodic_records table          (NEW - P0)                 │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Data Flow

```
Session Start
    │
    ├─ Load project memories (search by project_path)
    ├─ Load skill index (.skills/ directory)
    ├─ Inject into Layer 1 system prompt
    │
    ▼
User Message Arrives
    │
    ├─ Match relevant skills (lexical scoring)
    ├─ Prepend matched skill to user content
    │
    ▼
Agentic Loop (existing)
    │
    ├─ LLM call → tool execution → iterate
    ├─ SessionMemoryManager accumulates findings
    │
    ▼
Session End / Compaction
    │
    ├─ Extract memories from session findings (LLM-driven)
    ├─ Upsert to project_memories (deduplicate + merge)
    ├─ Optionally generate skill from successful execution
    ├─ Update access counts and decay stale entries
    │
    ▼
Next Session Benefits
```

---

## 4. P0: Project Memory System

### 4.1 Overview

A cross-session persistent memory system that stores and retrieves project-level knowledge (user preferences, project conventions, discovered patterns, corrections).

### 4.2 Database Schema

Add to `storage/database.rs` `init_schema()`:

```sql
-- Cross-session project memory
CREATE TABLE IF NOT EXISTS project_memories (
    id TEXT PRIMARY KEY,
    project_path TEXT NOT NULL,
    category TEXT NOT NULL CHECK(category IN (
        'preference',    -- User preferences ("always use pnpm")
        'convention',    -- Project conventions ("API routes in src/routes/")
        'pattern',       -- Discovered patterns ("error handling uses Result<T, AppError>")
        'correction',    -- Learned corrections ("don't use npm, use pnpm in this project")
        'fact'           -- General facts ("this is a Tauri 2 + React 18 app")
    )),
    content TEXT NOT NULL,
    keywords TEXT NOT NULL DEFAULT '[]',         -- JSON array for fast filtering
    embedding BLOB,                              -- TF-IDF vector (reuse EmbeddingService)
    importance REAL NOT NULL DEFAULT 0.5,        -- 0.0-1.0, decays over time
    access_count INTEGER NOT NULL DEFAULT 0,     -- bumped on retrieval
    source_session_id TEXT,                      -- which session created this
    source_context TEXT,                          -- brief excerpt of original context
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_accessed_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(project_path, content)                -- prevent exact duplicates
);

CREATE INDEX IF NOT EXISTS idx_project_memories_project
    ON project_memories(project_path);
CREATE INDEX IF NOT EXISTS idx_project_memories_category
    ON project_memories(project_path, category);
CREATE INDEX IF NOT EXISTS idx_project_memories_importance
    ON project_memories(project_path, importance DESC);

-- Episodic records for learning from past interactions
CREATE TABLE IF NOT EXISTS episodic_records (
    id TEXT PRIMARY KEY,
    project_path TEXT NOT NULL,
    session_id TEXT NOT NULL,
    record_type TEXT NOT NULL CHECK(record_type IN (
        'success',      -- Successfully completed task pattern
        'failure',      -- Failed approach that should be avoided
        'discovery'     -- Important discovery during exploration
    )),
    task_summary TEXT NOT NULL,         -- What the user asked
    approach_summary TEXT NOT NULL,     -- What the agent did
    outcome_summary TEXT NOT NULL,      -- What happened
    tools_used TEXT NOT NULL DEFAULT '[]',  -- JSON array of tool names
    keywords TEXT NOT NULL DEFAULT '[]',
    embedding BLOB,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_episodic_records_project
    ON episodic_records(project_path);
```

### 4.3 Core Module: `ProjectMemoryStore`

**New file**: `src-tauri/src/services/memory/mod.rs`

```rust
// Module structure
// services/memory/
// ├── mod.rs           -- Module exports
// ├── store.rs         -- ProjectMemoryStore implementation
// ├── extraction.rs    -- LLM-driven memory extraction
// ├── retrieval.rs     -- Search and ranking
// └── maintenance.rs   -- Decay, compaction, cleanup
```

#### 4.3.1 Store Interface

**File**: `services/memory/store.rs`

```rust
use crate::storage::database::Database;
use crate::services::orchestrator::embedding_service::EmbeddingService;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Categories of project memory
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCategory {
    Preference,
    Convention,
    Pattern,
    Correction,
    Fact,
}

/// A single memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub project_path: String,
    pub category: MemoryCategory,
    pub content: String,
    pub keywords: Vec<String>,
    pub importance: f32,
    pub access_count: i64,
    pub source_session_id: Option<String>,
    pub source_context: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_accessed_at: String,
}

/// Request to search memories
pub struct MemorySearchRequest {
    pub project_path: String,
    pub query: String,
    pub categories: Option<Vec<MemoryCategory>>,  // filter by category
    pub top_k: usize,                              // max results (default: 10)
    pub min_importance: f32,                        // threshold (default: 0.1)
}

/// Search result with relevance score
pub struct MemorySearchResult {
    pub entry: MemoryEntry,
    pub relevance_score: f32,  // combined: embedding similarity + importance + recency
}

/// Core store implementation
pub struct ProjectMemoryStore {
    db: Arc<Database>,
    embedding_service: Arc<EmbeddingService>,
}

impl ProjectMemoryStore {
    pub fn new(db: Arc<Database>, embedding_service: Arc<EmbeddingService>) -> Self;

    // --- Write Operations ---

    /// Add a new memory entry (generates embedding, deduplicates)
    pub async fn add_memory(&self, entry: NewMemoryEntry) -> AppResult<MemoryEntry>;

    /// Batch add memories (from extraction results)
    pub async fn add_memories(&self, entries: Vec<NewMemoryEntry>) -> AppResult<Vec<MemoryEntry>>;

    /// Update an existing memory (merge content, refresh embedding)
    pub async fn update_memory(&self, id: &str, updates: MemoryUpdate) -> AppResult<MemoryEntry>;

    /// Upsert: if similar memory exists (cosine > 0.85), merge; otherwise insert
    pub async fn upsert_memory(&self, entry: NewMemoryEntry) -> AppResult<UpsertResult>;

    // --- Read Operations ---

    /// Search memories by semantic similarity + keyword match
    pub async fn search(&self, req: MemorySearchRequest) -> AppResult<Vec<MemorySearchResult>>;

    /// Get all memories for a project (for UI listing)
    pub async fn list_memories(
        &self,
        project_path: &str,
        category: Option<MemoryCategory>,
        offset: usize,
        limit: usize,
    ) -> AppResult<Vec<MemoryEntry>>;

    /// Get a single memory by ID
    pub async fn get_memory(&self, id: &str) -> AppResult<Option<MemoryEntry>>;

    /// Get memory count by project
    pub async fn count_memories(&self, project_path: &str) -> AppResult<usize>;

    // --- Delete Operations ---

    /// Delete a specific memory
    pub async fn delete_memory(&self, id: &str) -> AppResult<()>;

    /// Delete all memories for a project
    pub async fn clear_project_memories(&self, project_path: &str) -> AppResult<usize>;

    // --- Maintenance Operations ---

    /// Decay importance of memories not accessed in `days` days
    /// Formula: new_importance = importance * decay_factor
    /// Where decay_factor = 0.95^(days_since_last_access / 7)
    pub async fn decay_memories(&self, project_path: &str) -> AppResult<usize>;

    /// Remove memories with importance below threshold
    pub async fn prune_memories(
        &self,
        project_path: &str,
        min_importance: f32,
    ) -> AppResult<usize>;

    /// Compact: merge highly similar memories (cosine > 0.90)
    pub async fn compact_memories(&self, project_path: &str) -> AppResult<usize>;
}

/// Result of an upsert operation
pub enum UpsertResult {
    Inserted(MemoryEntry),
    Merged { original_id: String, merged: MemoryEntry },
    Skipped { reason: String },
}

/// Input for creating a new memory
pub struct NewMemoryEntry {
    pub project_path: String,
    pub category: MemoryCategory,
    pub content: String,
    pub keywords: Vec<String>,
    pub importance: f32,
    pub source_session_id: Option<String>,
    pub source_context: Option<String>,
}

/// Partial update fields
pub struct MemoryUpdate {
    pub content: Option<String>,
    pub category: Option<MemoryCategory>,
    pub importance: Option<f32>,
    pub keywords: Option<Vec<String>>,
}
```

#### 4.3.2 Search & Ranking Algorithm

**File**: `services/memory/retrieval.rs`

The search algorithm combines three signals:

```rust
/// Relevance scoring formula:
///
///   final_score = w1 * embedding_similarity
///               + w2 * keyword_overlap
///               + w3 * importance
///               + w4 * recency_score
///
/// Where:
///   w1 = 0.40  (semantic relevance)
///   w2 = 0.25  (keyword match)
///   w3 = 0.20  (importance weight)
///   w4 = 0.15  (recency bonus)
///
///   recency_score = 1.0 / (1.0 + days_since_last_access * 0.1)
///   keyword_overlap = |query_keywords ∩ memory_keywords| / |query_keywords ∪ memory_keywords|

pub fn compute_relevance_score(
    embedding_similarity: f32,
    keyword_overlap: f32,
    importance: f32,
    days_since_last_access: f64,
) -> f32 {
    let recency = 1.0 / (1.0 + days_since_last_access as f32 * 0.1);
    0.40 * embedding_similarity
        + 0.25 * keyword_overlap
        + 0.20 * importance
        + 0.15 * recency
}
```

**Search flow**:

1. Generate TF-IDF embedding for query (reuse `EmbeddingService`)
2. Retrieve candidate memories from SQLite (filter by `project_path`, optional `category`, `importance >= min_importance`)
3. Compute cosine similarity between query embedding and each candidate
4. Compute keyword overlap (Jaccard coefficient)
5. Apply combined scoring formula
6. Sort by final_score descending, return top_k
7. Bump `access_count` and `last_accessed_at` for returned entries

#### 4.3.3 Memory Extraction (LLM-Driven)

**File**: `services/memory/extraction.rs`

```rust
/// Extract memories from session content using the active LLM provider
pub struct MemoryExtractor;

impl MemoryExtractor {
    /// Extract structured memories from session memory content
    ///
    /// Called at session end or during compaction.
    /// Uses a lightweight LLM call with structured output.
    pub async fn extract_from_session(
        provider: &dyn LlmProvider,
        session_memory: &SessionMemory,
        conversation_summary: &str,
        existing_memories: &[MemoryEntry],  // avoid duplicates
    ) -> AppResult<Vec<NewMemoryEntry>>;
}
```

**Extraction prompt template**:

```
You are a memory extraction system. Analyze the following session data and extract
facts worth remembering for future sessions with this project.

## Session Task
{task_description}

## Files Read
{files_read_list}

## Key Findings
{key_findings_list}

## Conversation Summary
{conversation_summary}

## Already Known (DO NOT duplicate these)
{existing_memories_list}

---

Extract NEW facts in the following JSON format. Only extract information that is:
1. Stable (unlikely to change frequently)
2. Useful for future sessions
3. Not already known

Return a JSON array:
[
  {
    "category": "preference|convention|pattern|correction|fact",
    "content": "concise factual statement",
    "keywords": ["keyword1", "keyword2"],
    "importance": 0.0-1.0
  }
]

Rules:
- "preference": user explicitly stated preferences (e.g., "use pnpm not npm")
- "convention": project-specific conventions discovered (e.g., "tests in __tests__/ directories")
- "pattern": recurring code/architecture patterns (e.g., "all API routes return CommandResponse<T>")
- "correction": mistakes to avoid (e.g., "editing executor.rs requires cargo check due to type complexity")
- "fact": general project facts (e.g., "frontend uses Zustand for state management")
- importance: 0.9+ for explicit user instructions, 0.5-0.8 for discovered patterns, 0.3-0.5 for general facts
- Return empty array [] if nothing worth extracting
```

**Extraction triggers**:
1. **Session end**: When a session is explicitly closed or times out
2. **During compaction**: When Layer 3 messages are compacted (opportunity to persist findings)
3. **Explicit user command**: "Remember this: ..." pattern detection in user messages

#### 4.3.4 Explicit Memory Commands

Detect user intent to create/delete memories directly:

```rust
/// Patterns detected in user messages:
///
/// Create: "remember that...", "always use...", "never do...", "note that..."
/// Delete: "forget that...", "stop remembering...", "delete memory about..."
/// Query:  "what do you remember about...", "what are my preferences..."
///
/// These bypass LLM extraction and directly create/modify memory entries
/// with importance = 0.95 (explicit user instruction).

pub fn detect_memory_command(user_message: &str) -> Option<MemoryCommand> {
    // Pattern matching for explicit memory operations
    // Returns None if no memory command detected
}

pub enum MemoryCommand {
    Remember { content: String },
    Forget { query: String },
    Query { query: String },
}
```

#### 4.3.5 Integration with System Prompt

**Modify**: `services/tools/system_prompt.rs`

```rust
pub fn build_system_prompt(
    project_root: &Path,
    tools: &[ToolDefinition],
    project_summary: Option<&ProjectIndexSummary>,
    project_memories: Option<&[MemoryEntry]>,       // NEW
    matched_skills: Option<&[SkillMatch]>,           // NEW
    provider_name: &str,
    model_name: &str,
    language: &str,
) -> String {
    let mut prompt = String::new();

    // ... existing sections (identity, language, working directory) ...

    // NEW: Project Memory section
    if let Some(memories) = project_memories {
        if !memories.is_empty() {
            prompt += "\n## Project Memory\n";
            prompt += "The following facts were learned from previous sessions:\n\n";
            for memory in memories {
                let badge = match memory.category {
                    MemoryCategory::Preference => "[PREF]",
                    MemoryCategory::Convention => "[CONV]",
                    MemoryCategory::Pattern => "[PATN]",
                    MemoryCategory::Correction => "[WARN]",
                    MemoryCategory::Fact => "[FACT]",
                };
                prompt += &format!("- {} {}\n", badge, memory.content);
            }
            prompt += "\n";
        }
    }

    // ... existing sections (project summary, tools, decision tree, rules) ...

    prompt
}
```

**Memory injection budget**: Maximum 2000 characters for project memories in Layer 1 to avoid excessive token usage. Prioritize by combined importance + relevance score.

#### 4.3.6 Tauri Commands

**New file**: `src-tauri/src/commands/memory.rs`

```rust
#[tauri::command]
pub async fn search_project_memories(
    project_path: String,
    query: String,
    categories: Option<Vec<String>>,
    top_k: Option<usize>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<MemorySearchResult>>, String>;

#[tauri::command]
pub async fn list_project_memories(
    project_path: String,
    category: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<MemoryEntry>>, String>;

#[tauri::command]
pub async fn add_project_memory(
    project_path: String,
    category: String,
    content: String,
    keywords: Vec<String>,
    importance: Option<f32>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<MemoryEntry>, String>;

#[tauri::command]
pub async fn update_project_memory(
    id: String,
    content: Option<String>,
    category: Option<String>,
    importance: Option<f32>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<MemoryEntry>, String>;

#[tauri::command]
pub async fn delete_project_memory(
    id: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<()>, String>;

#[tauri::command]
pub async fn clear_project_memories(
    project_path: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<usize>, String>;

#[tauri::command]
pub async fn get_memory_stats(
    project_path: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<MemoryStats>, String>;
```

#### 4.3.7 App State Integration

**Modify**: `src-tauri/src/state.rs`

```rust
pub struct AppState {
    // ... existing fields ...
    pub memory_store: Arc<RwLock<Option<ProjectMemoryStore>>>,  // NEW
}
```

Initialize `ProjectMemoryStore` in `init_app` command, passing the existing `Database` and `EmbeddingService` instances.

---

## 5. P0: Skill System

### 5.1 Overview

A universal skill system that is **fully compatible with plan-cascade's SKILL.md format**, adk-rust's `.skills/` format, and Vercel/community skill ecosystems. Supports four skill sources with a three-tier priority system, auto-detection based on project files, and auto-generated skills from successful sessions.

### 5.2 Universal Compatibility: Supported Skill Formats

The parser must handle all three formats found in the ecosystem:

#### Format A: Plan Cascade SKILL.md (Full-featured)

Used by plan-cascade builtin skills, external framework skills (Vercel, Vue, Rust).

```markdown
---
name: hybrid-ralph
version: "3.2.0"
description: Hybrid architecture combining Ralph's PRD format...
user-invocable: true
allowed-tools:
  - Read
  - Write
  - Edit
  - Bash
license: MIT
metadata:
  author: vercel
  version: "1.0.0"
hooks:
  PreToolUse:
    - matcher: "Write|Edit|Bash"
      hooks:
        - type: command
          command: |
            # shell commands to run before tool use
  PostToolUse:
    - matcher: "Write|Edit"
      hooks:
        - type: command
          command: |
            # shell commands to run after tool use
---

# Skill Title

{Markdown body with instructions}
```

**Fields**:
- `name` (required): Unique skill identifier
- `description` (required): When to use this skill
- `version` (optional): Semantic version string
- `user-invocable` (optional): Whether directly callable by user (default: false)
- `allowed-tools` (optional): Restrict which tools the skill may use
- `license` (optional): License identifier
- `metadata` (optional): Arbitrary key-value pairs (author, version, etc.)
- `hooks` (optional): PreToolUse / PostToolUse / Stop shell hooks
- `tags` (optional): String array for categorization

#### Format B: adk-rust .skills/ (Lightweight)

Used by adk-rust and simple project-local skills.

```markdown
---
name: adk-rust-app-bootstrap
description: Bootstrap new ADK-Rust applications with correct crate/features...
tags: [bootstrap, setup]
---

# ADK Rust App Bootstrap

## Workflow
1. Choose dependency scope
2. Select provider feature flags
...
```

**Fields**: `name` (required), `description` (required), `tags` (optional). No hooks, no version.

#### Format C: Convention Files (No frontmatter required)

Files like `CLAUDE.md`, `AGENTS.md`, `SKILLS.md` discovered in project root and subdirectories.

```markdown
# CLAUDE.md

This file provides guidance to Claude Code...

## Build Commands
...
```

**No frontmatter required** — filename becomes name, first heading becomes description.

### 5.3 Four-Source Skill Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Skill Source Hierarchy                         │
│                                                                 │
│  Priority 1-50:   BUILTIN                                       │
│  │  Bundled with plan-cascade (python, go, java, typescript)    │
│  │  Source: builtin-skills/ directory                           │
│  │                                                              │
│  Priority 51-100: EXTERNAL / SUBMODULE                          │
│  │  Community skills (Vercel React, Vue, Rust)                  │
│  │  Source: external-skills/ git submodules or remote URLs       │
│  │                                                              │
│  Priority 101-200: USER                                         │
│  │  User-defined skills from local paths or URLs                │
│  │  Source: .plan-cascade/skills.json or ~/.plan-cascade/skills.json │
│  │                                                              │
│  Priority 201+:  PROJECT-LOCAL                                  │
│  │  Project .skills/ directory + convention files               │
│  │  Source: <project_root>/.skills/ + CLAUDE.md etc.            │
│  │  Always highest priority (project-specific overrides all)    │
│  │                                                              │
│  ↓ Higher priority overrides lower on name conflict             │
└─────────────────────────────────────────────────────────────────┘
```

### 5.4 Skill Configuration (external-skills.json compatible)

The desktop app reads the **same `external-skills.json` format** used by plan-cascade CLI:

```json
{
  "version": "1.1.0",
  "priority_ranges": {
    "builtin":   { "min": 1,   "max": 50  },
    "submodule": { "min": 51,  "max": 100 },
    "user":      { "min": 101, "max": 200 }
  },
  "sources": {
    "vercel": {
      "type": "submodule",
      "path": "external-skills/vercel",
      "repository": "https://github.com/vercel-labs/agent-skills"
    }
  },
  "skills": {
    "react-best-practices": {
      "source": "vercel",
      "skill_path": "skills/react-best-practices",
      "detect": {
        "files": ["package.json"],
        "patterns": ["\"react\"", "\"next\""]
      },
      "inject_into": ["planning", "implementation", "retry"],
      "priority": 100
    }
  },
  "settings": {
    "max_skills_per_story": 3,
    "max_content_lines": 200,
    "cache_enabled": true
  }
}
```

**Desktop-specific additions**: The desktop app stores a local config at `~/.plan-cascade-desktop/skills-config.json` that extends external-skills.json with UI state (enabled/disabled toggles, user notes).

### 5.5 Auto-Detection Logic

Skills are automatically activated based on project file analysis:

```rust
/// Detect which skills are applicable to the current project
pub struct SkillDetector;

impl SkillDetector {
    /// Scan project root for detection markers
    ///
    /// For each configured skill, check:
    /// 1. detect.files — do any of these files exist in project root?
    /// 2. detect.patterns — do any of these patterns appear in the detected files?
    ///
    /// Returns skills that match, sorted by priority (highest first)
    pub async fn detect_applicable_skills(
        project_root: &Path,
        config: &SkillsConfig,
    ) -> AppResult<Vec<DetectedSkill>>;
}

pub struct DetectedSkill {
    pub skill_id: String,
    pub name: String,
    pub source: SkillSource,
    pub priority: u32,
    pub matched_files: Vec<String>,       // Which detect.files matched
    pub matched_patterns: Vec<String>,    // Which detect.patterns matched
    pub inject_into: Vec<InjectionPhase>,
}

pub enum SkillSource {
    Builtin,
    External { source_name: String },
    User,
    ProjectLocal,
}

pub enum InjectionPhase {
    Planning,        // PRD/design generation
    Implementation,  // Code execution
    Retry,           // Error recovery
    Always,          // All phases (for project-local skills)
}
```

**Detection examples from actual external-skills.json**:

| Skill | Files | Patterns | Priority |
|-------|-------|----------|----------|
| react-best-practices | `package.json` | `"react"`, `"next"`, `"@react"` | 100 |
| vue-best-practices | `package.json` | `"vue"`, `"nuxt"`, `"@vue"` | 100 |
| rust-coding-guidelines | `Cargo.toml` | `[package]`, `[dependencies]` | 100 |
| typescript-best-practices | `tsconfig.json`, `package.json` | `"typescript"`, `"@types/"` | 36 |
| rust-concurrency | `Cargo.toml` | `tokio`, `async-std`, `rayon` | 80 |

### 5.6 Core Module: Skill System

**New directory**: `src-tauri/src/services/skills/`

```
services/skills/
├── mod.rs           -- Module exports
├── model.rs         -- SkillDocument, SkillIndex, SkillMatch, SkillsConfig types
├── config.rs        -- external-skills.json + user config loading and merging
├── discovery.rs     -- Filesystem scanning (4 sources) + auto-detection
├── parser.rs        -- Universal SKILL.md parser (Format A + B + C)
├── index.rs         -- SkillIndex construction with SHA-256 hashing
├── select.rs        -- Lexical scoring, detection-based selection, priority resolution
├── injector.rs      -- Skill injection into user content
└── generator.rs     -- Auto-generate skills from successful sessions
```

#### 5.6.1 Data Types

**File**: `services/skills/model.rs`

```rust
/// A parsed and indexed skill document (universal format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDocument {
    pub id: String,                     // normalized-name + first 12 chars of SHA-256
    pub name: String,
    pub description: String,
    pub version: Option<String>,        // Semantic version
    pub tags: Vec<String>,
    pub body: String,                   // Full skill text (markdown)
    pub path: PathBuf,                  // Source file path
    pub hash: String,                   // Full SHA-256 hex string
    pub last_modified: Option<i64>,     // Unix timestamp

    // --- Plan Cascade extensions ---
    pub user_invocable: bool,           // Whether directly callable (default: false)
    pub allowed_tools: Vec<String>,     // Restrict tool access (empty = all)
    pub license: Option<String>,
    pub metadata: HashMap<String, String>, // author, etc.
    pub hooks: Option<SkillHooks>,      // Pre/Post tool hooks

    // --- Source & priority ---
    pub source: SkillSource,
    pub priority: u32,                  // 1-200+ resolved priority

    // --- Detection (from config) ---
    pub detect: Option<SkillDetection>,
    pub inject_into: Vec<InjectionPhase>,

    // --- Runtime state (from desktop config) ---
    pub enabled: bool,                  // User can toggle in UI
}

/// Hooks for pre/post tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillHooks {
    pub pre_tool_use: Vec<ToolHookRule>,
    pub post_tool_use: Vec<ToolHookRule>,
    pub stop: Vec<HookAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolHookRule {
    pub matcher: String,              // Regex pattern matching tool names
    pub hooks: Vec<HookAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookAction {
    pub hook_type: String,            // "command"
    pub command: String,              // Shell command to execute
}

/// Detection rules (from external-skills.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDetection {
    pub files: Vec<String>,           // Files to check for existence
    pub patterns: Vec<String>,        // Content patterns to search for
}

/// Lightweight summary without body (for UI listings)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub tags: Vec<String>,
    pub source: SkillSource,
    pub priority: u32,
    pub enabled: bool,
    pub detected: bool,               // Whether auto-detection matched this project
    pub user_invocable: bool,
    pub has_hooks: bool,
    pub inject_into: Vec<InjectionPhase>,
    pub path: PathBuf,
}

/// Immutable collection of indexed skills
#[derive(Debug, Clone)]
pub struct SkillIndex {
    skills: Vec<SkillDocument>,
}

impl SkillIndex {
    pub fn is_empty(&self) -> bool;
    pub fn len(&self) -> usize;
    pub fn skills(&self) -> &[SkillDocument];
    pub fn summaries(&self) -> Vec<SkillSummary>;
    /// Get only auto-detected applicable skills for this project
    pub fn detected_skills(&self) -> Vec<&SkillDocument>;
    /// Get skills by source tier
    pub fn skills_by_source(&self, source: &SkillSource) -> Vec<&SkillDocument>;
}

/// A matched skill with relevance score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMatch {
    pub score: f32,
    pub match_reason: MatchReason,    // Why this skill was selected
    pub skill: SkillSummary,
}

pub enum MatchReason {
    AutoDetected,                     // Matched via detect.files + detect.patterns
    LexicalMatch { query: String },   // Matched via lexical scoring
    UserForced,                       // User explicitly enabled for this session
}

/// Policy for skill selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionPolicy {
    pub top_k: usize,                  // Max skills to return (default: 3)
    pub min_score: f32,                // Minimum score threshold (default: 1.0)
    pub include_tags: Vec<String>,     // Must match one of these (OR)
    pub exclude_tags: Vec<String>,     // Must not match any (AND NOT)
    pub max_content_lines: usize,      // Max lines per skill (default: 200)
}

impl Default for SelectionPolicy {
    fn default() -> Self {
        Self {
            top_k: 3,         // matches external-skills.json "max_skills_per_story": 3
            min_score: 1.0,
            include_tags: vec![],
            exclude_tags: vec![],
            max_content_lines: 200, // matches external-skills.json "max_content_lines": 200
        }
    }
}
```

#### 5.6.2 Universal Parser

**File**: `services/skills/parser.rs`

```rust
/// Parse any SKILL.md format (Plan Cascade full, adk-rust lightweight, convention file)
pub fn parse_skill_file(path: &Path, content: &str) -> AppResult<ParsedSkill> {
    // 1. Try to extract YAML frontmatter (between --- delimiters)
    // 2. If frontmatter found:
    //    a. Parse all known fields (name, description, version, tags,
    //       user-invocable, allowed-tools, license, metadata, hooks)
    //    b. Unknown fields are preserved in metadata HashMap
    //    c. Body is everything after closing ---
    // 3. If no frontmatter (convention file):
    //    a. name = filename without extension (e.g., "CLAUDE" from "CLAUDE.md")
    //    b. description = first heading text, or first non-empty line
    //    c. body = entire file content
    // 4. Validate: name and description must be non-empty
}

pub struct ParsedSkill {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub tags: Vec<String>,
    pub body: String,
    pub user_invocable: bool,
    pub allowed_tools: Vec<String>,
    pub license: Option<String>,
    pub metadata: HashMap<String, String>,
    pub hooks: Option<SkillHooks>,
}
```

**Key compatibility rules**:
- `allowed-tools` (YAML kebab-case) maps to `allowed_tools` (Rust snake_case)
- `user-invocable` (YAML) maps to `user_invocable` (Rust)
- `hooks.PreToolUse` (YAML PascalCase) maps to `hooks.pre_tool_use` (Rust)
- Unknown frontmatter fields are preserved in `metadata` (forward-compatible)
- Files without `.md` extension are ignored

#### 5.6.3 Four-Source Discovery

**File**: `services/skills/discovery.rs`

```rust
/// Discover all skills from all 4 sources and merge by priority
pub async fn discover_all_skills(
    project_root: &Path,
    skills_config: &SkillsConfig,
    user_config: Option<&UserSkillConfig>,
) -> AppResult<Vec<DiscoveredSkill>> {
    // 1. BUILTIN: Load from bundled skills directory (or embedded)
    //    Priority range: 1-50

    // 2. EXTERNAL: Load from external-skills.json sources
    //    For each source, resolve path relative to plan-cascade install dir
    //    Priority range: 51-100

    // 3. USER: Load from ~/.plan-cascade/skills.json + .plan-cascade/skills.json
    //    Support both local paths and remote URLs (with caching)
    //    Priority range: 101-200

    // 4. PROJECT-LOCAL: Scan <project_root>/.skills/ + convention files
    //    Always highest priority: 201+

    // 5. DEDUP: Same name → higher priority wins
    //    Sort by priority descending
}

/// Discover project-local skills only (fast path for session start)
pub fn discover_project_skills(project_root: &Path) -> AppResult<Vec<PathBuf>> {
    let mut files = Vec::new();

    // .skills/ directory (recursive, SKILL.md files)
    let skills_dir = project_root.join(".skills");
    if skills_dir.is_dir() {
        walk_skill_directory(&skills_dir, &mut files);
    }

    // Convention files in root + subdirectories
    discover_convention_files(project_root, &mut files);

    Ok(files)
}

/// Convention file names to discover
const CONVENTION_FILES: &[&str] = &[
    "CLAUDE.md", "AGENTS.md", "AGENT.md", "SKILLS.md",
    "COPILOT.md", "GEMINI.md", "SOUL.md",
];

/// Directories to skip during recursive walk
const IGNORED_DIRS: &[&str] = &[
    ".git", ".hg", ".svn", "target", "node_modules",
    ".next", "dist", "build", "coverage", "__pycache__",
    ".plan-cascade",
];
```

#### 5.6.4 Selection: Detection + Lexical Hybrid

**File**: `services/skills/select.rs`

Selection uses a two-phase approach:

```rust
/// Phase 1: Auto-detection (at session start)
///
/// For each configured skill with detect rules:
///   1. Check detect.files → do any exist in project root?
///   2. Check detect.patterns → do patterns appear in those files?
///   3. If matched, add to active skills with match_reason = AutoDetected
///
/// Phase 2: Lexical matching (per user message)
///
/// For project-local skills (no detect rules):
///   Same algorithm as adk-rust:
///   - Name match:        +4.0
///   - Description match: +2.5
///   - Tags match:        +2.0
///   - Body match:        +1.0
///   - Normalize by sqrt(body_token_count)
///
/// Final: Merge Phase 1 + Phase 2 results
///   - Auto-detected skills always included (up to max_skills_per_story)
///   - Lexical matches fill remaining slots
///   - Priority breaks ties

pub fn select_skills_for_session(
    index: &SkillIndex,
    project_root: &Path,
    user_message: &str,
    phase: InjectionPhase,
    policy: &SelectionPolicy,
) -> Vec<SkillMatch>;
```

#### 5.6.5 Injection

**File**: `services/skills/injector.rs`

```rust
/// Inject matched skills into system prompt or user content
pub fn inject_skills(
    matched_skills: &[SkillMatch],
    policy: &SelectionPolicy,
) -> String {
    // Format matching plan-cascade's injection format:
    //
    // ## Framework-Specific Best Practices
    //
    // The following guidelines apply based on detected frameworks:
    //
    // ### {Skill Name}
    // *Source: {source_name} ({source_type}) | Priority: {priority}*
    //
    // {skill body, truncated to max_content_lines}
    //
    // ---
}
```

### 5.7 Skill Library Database (Auto-Generated Skills)

In addition to file-based skills, support auto-generated skills from successful sessions:

```sql
-- Auto-generated skills from successful task executions
CREATE TABLE IF NOT EXISTS skill_library (
    id TEXT PRIMARY KEY,
    project_path TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    tags TEXT NOT NULL DEFAULT '[]',           -- JSON array
    body TEXT NOT NULL,                        -- Markdown instruction text (SKILL.md format)
    source_type TEXT NOT NULL CHECK(source_type IN (
        'generated',    -- Auto-generated from successful session
        'refined'       -- Auto-refined from multiple similar sessions
    )),
    source_session_ids TEXT NOT NULL DEFAULT '[]',  -- JSON array of session IDs
    usage_count INTEGER NOT NULL DEFAULT 0,
    success_rate REAL NOT NULL DEFAULT 1.0,    -- 0.0-1.0, updated on use
    keywords TEXT NOT NULL DEFAULT '[]',
    embedding BLOB,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_skill_library_project
    ON skill_library(project_path);
CREATE INDEX IF NOT EXISTS idx_skill_library_enabled
    ON skill_library(project_path, enabled);
```

### 5.8 Skill Generation from Sessions

```rust
/// Generate a reusable skill from a successful session execution
pub struct SkillGenerator;

impl SkillGenerator {
    /// Analyze a completed session and optionally generate a skill template
    ///
    /// Criteria for skill generation:
    /// 1. Session completed successfully (no terminal errors)
    /// 2. Task involved 3+ tool calls (non-trivial)
    /// 3. No similar skill already exists (cosine similarity < 0.80)
    pub async fn maybe_generate_skill(
        provider: &dyn LlmProvider,
        session_summary: &str,
        tool_calls: &[(String, String)],  // (tool_name, brief_description)
        existing_skills: &[SkillSummary],
    ) -> AppResult<Option<GeneratedSkill>>;
}

pub struct GeneratedSkill {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub body: String,                     // Valid SKILL.md format body
    pub source_session_ids: Vec<String>,
}
```

### 5.9 Tauri Commands

**New file**: `src-tauri/src/commands/skills.rs`

```rust
/// List all skills from all sources for a project
/// Returns: builtin + external + user + project-local + generated
/// Each entry includes source, priority, enabled state, and detection status
#[tauri::command]
pub async fn list_skills(
    project_path: String,
    source_filter: Option<String>,    // "builtin" | "external" | "user" | "project" | "generated"
    include_disabled: Option<bool>,   // default: true (show all in UI)
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<SkillSummary>>, String>;

/// Get full skill content
#[tauri::command]
pub async fn get_skill(
    id: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SkillDocument>, String>;

/// Search skills by query (lexical matching)
#[tauri::command]
pub async fn search_skills(
    project_path: String,
    query: String,
    top_k: Option<usize>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<SkillMatch>>, String>;

/// Run auto-detection to find applicable skills
#[tauri::command]
pub async fn detect_applicable_skills(
    project_path: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<DetectedSkill>>, String>;

/// Toggle a skill's enabled state (persisted in desktop config)
#[tauri::command]
pub async fn toggle_skill(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<CommandResponse<()>, String>;

/// Create a new project-local skill file in .skills/
#[tauri::command]
pub async fn create_skill_file(
    project_path: String,
    name: String,
    description: String,
    tags: Vec<String>,
    body: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SkillDocument>, String>;

/// Delete a skill (file-based: delete file; generated: delete DB row)
#[tauri::command]
pub async fn delete_skill(
    id: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<()>, String>;

/// Toggle a generated skill's enabled state
#[tauri::command]
pub async fn toggle_generated_skill(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<CommandResponse<()>, String>;

/// Refresh the skill index (re-scan all sources)
#[tauri::command]
pub async fn refresh_skill_index(
    project_path: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SkillIndexStats>, String>;

/// Get skills config overview (sources, counts, detection results)
#[tauri::command]
pub async fn get_skills_overview(
    project_path: String,
    state: State<'_, AppState>,
) -> Result<CommandResponse<SkillsOverview>, String>;
```

---

## 6. P0: Memory & Skill Management UI

### 6.1 Overview

Integrate skill and memory management directly into the existing SimpleMode sidebar, activating the currently disabled "Skills" button at `WorkspaceTreeSidebar.tsx:166-175`. The UI is split into two surfaces:

1. **Sidebar Panel** (lightweight) — Quick skill/memory overview within the existing sidebar flow
2. **Full Management Dialog** (detailed) — Modal for detailed viewing, editing, and configuration

### 6.2 Activating the Existing Skill Button

**Current state** (`WorkspaceTreeSidebar.tsx` lines 166-175):

```tsx
// Currently disabled
<button
  disabled
  className={clsx(
    'flex-1 flex items-center justify-center gap-1 px-2 py-1.5 rounded-md text-xs transition-colors',
    'text-gray-400 dark:text-gray-600 cursor-not-allowed opacity-50'
  )}
  title={t('sidebar.skills')}
>
  {t('sidebar.skills')}
</button>
```

**Target state** — Enable the button and toggle a skill panel below the toolbar:

```tsx
<button
  onClick={() => setShowSkillPanel(!showSkillPanel)}
  className={clsx(
    'flex-1 flex items-center justify-center gap-1 px-2 py-1.5 rounded-md text-xs transition-colors',
    showSkillPanel
      ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
      : 'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800'
  )}
  title={t('sidebar.skills')}
>
  <SparklesIcon className="w-3.5 h-3.5" />
  <span>{t('sidebar.skills')}</span>
  {detectedSkillCount > 0 && (
    <span className="ml-0.5 px-1 py-0.5 rounded-full bg-primary-500 text-white text-2xs">
      {detectedSkillCount}
    </span>
  )}
</button>
```

### 6.3 Sidebar Skill Panel Design

When the Skills button is clicked, a collapsible panel appears between the toolbar and the session tree:

```
┌─────────────────────────────────────────┐
│  [New Task]                             │  ← Existing toolbar
│  [+ Directory] [★ Skills] [◈ Plugins]  │  ← Skills button now active
├─────────────────────────────────────────┤
│  Skills & Memory for this project    ⚙  │  ← Panel header (⚙ opens full dialog)
│                                         │
│  ▸ Auto-Detected (3)                    │  ← Collapsible section
│    ✓ React Best Practices    vercel     │  ← Green check = active
│    ✓ TypeScript Guidelines   builtin    │
│    ✓ Web Design Guidelines   vercel     │
│                                         │
│  ▸ Project Skills (2)                   │  ← .skills/ + CLAUDE.md
│    ✓ CLAUDE.md               local      │
│    ✓ Add Tauri Command       .skills/   │
│                                         │
│  ▸ Memories (12)                        │  ← Collapsible, shows count
│    ● Use pnpm not npm        [PREF]     │
│    ● API uses CommandResponse [PATN]    │
│    ● Tests in Vitest         [CONV]     │
│    ... +9 more                          │
│                                         │
│  [Manage All...]                        │  ← Opens full dialog
├─────────────────────────────────────────┤
│  Current: Adding user auth...           │  ← Existing current task
├─────────────────────────────────────────┤
│  ▸ ~/projects/my-app (5 sessions)       │  ← Existing session tree
│  ▸ Other (2)                            │
└─────────────────────────────────────────┘
```

#### 6.3.1 Sidebar Panel Component

**New file**: `src/components/SimpleMode/SkillMemoryPanel.tsx`

```tsx
interface SkillMemoryPanelProps {
  projectPath: string;
  onOpenFullDialog: () => void;
}

export function SkillMemoryPanel({ projectPath, onOpenFullDialog }: SkillMemoryPanelProps) {
  // Three collapsible sections:
  // 1. Auto-Detected Skills (from external-skills.json detection)
  // 2. Project Skills (from .skills/ + convention files)
  // 3. Memories (top N from ProjectMemoryStore)

  // Each skill row shows:
  //   [toggle] name                source_badge
  // Toggle enables/disables injection for this session

  // Each memory row shows:
  //   ● brief_content             [category_badge]
  // Click expands to show full content + edit/delete
}
```

#### 6.3.2 Skill Row Component

**New file**: `src/components/SimpleMode/SkillRow.tsx`

```tsx
interface SkillRowProps {
  skill: SkillSummary;
  onToggle: (id: string, enabled: boolean) => void;
  onClick: (id: string) => void;
}

// Renders as a compact row:
//
//   [✓] react-best-practices          vercel
//   │   React and Next.js performance...  P:100
//
// - Checkbox toggles enabled/disabled
// - Source badge: "builtin" (gray), "vercel" (blue), "local" (green), "generated" (purple)
// - Priority shown as small P:N badge
// - Click row to expand/preview skill content
// - Hover shows full description tooltip
```

#### 6.3.3 Inline Skill Toggle Behavior

When a user toggles a skill on/off in the sidebar:

1. **Immediate**: Update UI state (checkbox)
2. **Persist**: Save to `~/.plan-cascade-desktop/skills-config.json`
3. **Next LLM call**: Skill injection includes/excludes based on toggle
4. **No session restart needed**: Takes effect on next message

### 6.4 Full Management Dialog

Opened via "Manage All..." button or the ⚙ icon. A modal dialog with two tabs:

```
┌─────────────────────────────────────────────────────────────────┐
│  Skills & Memory Manager                              [×]       │
├──────────────────────┬──────────────────────────────────────────┤
│                      │                                          │
│  [Skills] [Memory]   │                                          │
│                      │                                          │
├──────────────────────┤                                          │
│                      │                                          │
│  Source: [All ▼]     │   Skill Detail                           │
│  Search: [________]  │                                          │
│                      │   Name: react-best-practices             │
│  ┌─ Auto-Detected ─┐│   Source: vercel (submodule)              │
│  │ ✓ react-best-   ││   Priority: 100                          │
│  │   practices     ▶││   Version: 1.0.0                        │
│  │ ✓ typescript-   ││   License: MIT                           │
│  │   guidelines    ││   Author: vercel                          │
│  │ ✓ web-design    ││   Detected via: package.json → "react"   │
│  └──────────────────┘│   Inject into: planning, implementation  │
│                      │                                          │
│  ┌─ Project ────────┐│   Preview:                               │
│  │ ✓ CLAUDE.md     ││   ┌──────────────────────────────────┐   │
│  │ ✓ Add Tauri Cmd ││   │ # Vercel React Best Practices    │   │
│  │ ○ Fix CI (gen.) ││   │                                  │   │
│  └──────────────────┘│   │ Comprehensive performance        │   │
│                      │   │ optimization guide for React...  │   │
│  ┌─ Available ──────┐│   │                                  │   │
│  │ ○ vue-best-     ││   │ ## When to Apply                 │   │
│  │   practices     ││   │ - Writing new React components   │   │
│  │ ○ rust-coding   ││   │ - Implementing data fetching     │   │
│  │   guidelines    ││   │ ...                              │   │
│  └──────────────────┘│   └──────────────────────────────────┘   │
│                      │                                          │
│  [+ New Skill]       │   [Open File] [Toggle] [Delete]          │
│  [↻ Refresh Index]   │                                          │
├──────────────────────┴──────────────────────────────────────────┤
│  14 skills total │ 3 active │ 2 generated │ Last scan: 2m ago   │
└─────────────────────────────────────────────────────────────────┘
```

#### 6.4.1 Skills Tab Sections

Skills are grouped into three visual sections:

| Section | Contents | Visual |
|---------|----------|--------|
| **Auto-Detected** | Skills that matched this project's files/patterns | Green highlight, checked by default |
| **Project** | `.skills/` files + convention files + generated skills | Normal, checked by default |
| **Available** | All other configured skills (didn't match detection) | Dimmed, unchecked by default |

#### 6.4.2 Memory Tab

```
┌─────────────────────────────────────────────────────────────────┐
│  [Skills] [Memory]                                              │
├──────────────────────┬──────────────────────────────────────────┤
│                      │                                          │
│  Category: [All ▼]   │   Memory Detail                          │
│  Search: [________]  │                                          │
│                      │   Category: Preference                   │
│  ● Use pnpm not npm  │   Content:                               │
│    [PREF] ★★★★☆      │   "This project uses pnpm as the        │
│                      │    package manager. Always use pnpm       │
│  ● API returns       │    instead of npm for install, add,      │
│    CommandResponse<T> │    and run commands."                    │
│    [PATN] ★★★☆☆      │                                          │
│                      │   Importance: ████████░░ 0.85            │
│  ● Tests in Vitest   │   Keywords: pnpm, npm, package-manager  │
│    [CONV] ★★★☆☆      │   Accessed: 12 times                    │
│                      │   Created: 2026-02-10                    │
│  ● Don't edit        │   Last used: 2026-02-15                  │
│    executor.rs w/o   │   Source: Session #abc123                │
│    cargo check       │                                          │
│    [WARN] ★★★★★      │   [Edit Content] [Adjust Importance]    │
│                      │   [Delete]                               │
│  ... (paginated)     │                                          │
├──────────────────────┴──────────────────────────────────────────┤
│  23 memories │ 5 pref │ 8 conv │ 6 patn │ 3 warn │ 1 fact     │
│  [+ Add Memory] [Prune Stale] [Clear All]                       │
└─────────────────────────────────────────────────────────────────┘
```

**Category filter options**: All, Preferences, Conventions, Patterns, Corrections, Facts

**Category badges**:
- `[PREF]` — purple badge (user preferences)
- `[CONV]` — blue badge (project conventions)
- `[PATN]` — green badge (discovered patterns)
- `[WARN]` — orange badge (corrections/warnings)
- `[FACT]` — gray badge (general facts)

### 6.5 Frontend Components

**New directories**:

```
src/components/SimpleMode/
├── SkillMemoryPanel.tsx          -- Inline sidebar panel (lightweight)
├── SkillRow.tsx                  -- Compact skill row for sidebar
├── MemoryRow.tsx                 -- Compact memory row for sidebar

src/components/SkillMemory/
├── SkillMemoryDialog.tsx         -- Full management modal
├── SkillsTab.tsx                 -- Skills tab content
├── SkillDetail.tsx               -- Right-side skill detail view
├── SkillPreview.tsx              -- Markdown preview of skill body
├── SkillEditor.tsx               -- Create/edit skill dialog
├── SkillSourceBadge.tsx          -- Source indicator (builtin/vercel/local/generated)
├── MemoryTab.tsx                 -- Memory tab content
├── MemoryDetail.tsx              -- Right-side memory detail view
├── MemoryEditor.tsx              -- Edit memory content/importance
├── CategoryBadge.tsx             -- PREF/CONV/PATN/WARN/FACT badges
├── ImportanceBar.tsx             -- Visual importance indicator
└── EmptyState.tsx                -- Onboarding when no skills/memories
```

### 6.6 Zustand Store

**New file**: `src/store/skillMemory.ts`

```typescript
// --- Skill Types ---

type SkillSource = 'builtin' | 'external' | 'user' | 'project' | 'generated';
type InjectionPhase = 'planning' | 'implementation' | 'retry' | 'always';

interface SkillSummary {
  id: string;
  name: string;
  description: string;
  version?: string;
  tags: string[];
  source: SkillSource;
  sourceName?: string;        // e.g., "vercel", "vue", "rust"
  priority: number;
  enabled: boolean;
  detected: boolean;          // Whether auto-detection matched
  userInvocable: boolean;
  hasHooks: boolean;
  injectInto: InjectionPhase[];
  path: string;
}

interface SkillDocument extends SkillSummary {
  body: string;               // Full markdown content
  hash: string;
  allowedTools: string[];
  license?: string;
  metadata: Record<string, string>;
}

interface DetectedSkill {
  skillId: string;
  name: string;
  matchedFiles: string[];
  matchedPatterns: string[];
}

// --- Memory Types ---

type MemoryCategory = 'preference' | 'convention' | 'pattern' | 'correction' | 'fact';

interface MemoryEntry {
  id: string;
  projectPath: string;
  category: MemoryCategory;
  content: string;
  keywords: string[];
  importance: number;
  accessCount: number;
  sourceSessionId?: string;
  createdAt: string;
  updatedAt: string;
  lastAccessedAt: string;
}

// --- Store ---

interface SkillMemoryState {
  // Skill state
  skills: SkillSummary[];
  selectedSkillId: string | null;
  selectedSkillDetail: SkillDocument | null;
  detectedSkills: DetectedSkill[];
  skillsLoading: boolean;

  // Memory state
  memories: MemoryEntry[];
  selectedMemoryId: string | null;
  memoriesLoading: boolean;
  memorySearchQuery: string;
  memoryCategoryFilter: MemoryCategory | null;
  memoryStats: {
    total: number;
    byCategory: Record<MemoryCategory, number>;
  } | null;

  // UI state
  showSkillPanel: boolean;       // sidebar panel toggle
  showFullDialog: boolean;       // full dialog toggle
  activeTab: 'skills' | 'memory';
  skillSourceFilter: SkillSource | null;

  // Skill actions
  loadSkills: (projectPath: string) => Promise<void>;
  detectSkills: (projectPath: string) => Promise<void>;
  toggleSkill: (id: string, enabled: boolean) => Promise<void>;
  getSkillDetail: (id: string) => Promise<void>;
  createSkill: (projectPath: string, skill: NewSkillInput) => Promise<void>;
  deleteSkill: (id: string) => Promise<void>;
  refreshSkillIndex: (projectPath: string) => Promise<void>;

  // Memory actions
  loadMemories: (projectPath: string) => Promise<void>;
  searchMemories: (projectPath: string, query: string) => Promise<void>;
  addMemory: (entry: NewMemoryInput) => Promise<void>;
  updateMemory: (id: string, updates: Partial<MemoryEntry>) => Promise<void>;
  deleteMemory: (id: string) => Promise<void>;
  clearAllMemories: (projectPath: string) => Promise<void>;
  pruneStaleMemories: (projectPath: string) => Promise<void>;

  // UI actions
  setShowSkillPanel: (show: boolean) => void;
  setShowFullDialog: (show: boolean) => void;
  setActiveTab: (tab: 'skills' | 'memory') => void;
}
```

### 6.7 i18n Extensions

**Add to** `src/i18n/locales/zh/simpleMode.json`:

```json
"skillPanel": {
  "title": "技能与记忆",
  "autoDetected": "自动检测",
  "projectSkills": "项目技能",
  "available": "可用技能",
  "memories": "记忆",
  "manageAll": "管理全部...",
  "noSkills": "未发现技能",
  "noMemories": "暂无记忆",
  "detected": "已检测",
  "enabled": "已启用",
  "disabled": "已禁用",
  "generated": "自动生成",
  "refresh": "刷新索引",
  "newSkill": "新建技能",
  "pruneStale": "清理过期",
  "clearAll": "清除全部",
  "addMemory": "添加记忆",
  "source": "来源",
  "priority": "优先级",
  "version": "版本",
  "injectInto": "注入阶段",
  "detectedVia": "检测方式",
  "importance": "重要性",
  "accessed": "访问次数",
  "lastUsed": "最后使用",
  "category": "分类",
  "categories": {
    "all": "全部",
    "preference": "偏好",
    "convention": "约定",
    "pattern": "模式",
    "correction": "纠正",
    "fact": "事实"
  }
}
```

Equivalent keys for `en/simpleMode.json` and `ja/simpleMode.json`.

### 6.8 Integration with SimpleMode Layout

**Modify**: `src/components/SimpleMode/WorkspaceTreeSidebar.tsx`

```tsx
// Add state and imports
import { SkillMemoryPanel } from './SkillMemoryPanel';
import { useSkillMemoryStore } from '../../store/skillMemory';

// In WorkspaceTreeSidebar component:
const { showSkillPanel, setShowSkillPanel, skills, detectedSkills } = useSkillMemoryStore();

// In the JSX, between SidebarToolbar and session tree:
{showSkillPanel && workspacePath && (
  <SkillMemoryPanel
    projectPath={workspacePath}
    onOpenFullDialog={() => setShowFullDialog(true)}
  />
)}
```

### 6.9 Session Active Skills Indicator

When skills are injected into a session, show a subtle indicator in the chat area:

```tsx
// In the chat message area header, show active skills:
{activeSkills.length > 0 && (
  <div className="flex items-center gap-1 px-3 py-1 text-2xs text-gray-500">
    <SparklesIcon className="w-3 h-3" />
    <span>Active: {activeSkills.map(s => s.name).join(', ')}</span>
    {injectedMemoryCount > 0 && (
      <span>| {injectedMemoryCount} memories</span>
    )}
  </div>
)}
```

### 6.10 Notification Integration

- **New memories extracted**: Toast notification after session end: "Learned 3 new facts about this project"
- **Skills auto-detected**: Toast on first project open: "Detected React + TypeScript. 3 skills activated."
- **Skill generation**: Toast after successful session: "Generated skill: 'Add API Endpoint' from this session"

---

## 7. P1: Agentic Lifecycle Hooks

### 7.1 Overview

A plugin-style hook system that decouples cross-cutting concerns (memory extraction, skill injection, analytics, logging) from the core agentic loop. Inspired by adk-rust's `adk-plugin` crate.

### 7.2 Hook Points

```
┌─────────────────────────────────────────────────────────────┐
│                      Agentic Loop                            │
│                                                             │
│  ①  on_session_start(ctx)                                   │
│      ↓                                                      │
│  ②  on_user_message(ctx, message) → Option<modified_msg>    │
│      ↓                                                      │
│  ┌─ Loop ──────────────────────────────────────────────┐    │
│  │  ③  on_before_llm(ctx, request) → Option<mod_req>   │    │
│  │      ↓                                               │    │
│  │      LLM Call                                        │    │
│  │      ↓                                               │    │
│  │  ④  on_after_llm(ctx, response) → Option<mod_resp>   │    │
│  │      ↓                                               │    │
│  │  ⑤  on_before_tool(ctx, name, args) → Option<skip>   │    │
│  │      ↓                                               │    │
│  │      Tool Execution                                  │    │
│  │      ↓                                               │    │
│  │  ⑥  on_after_tool(ctx, name, result)                 │    │
│  │      ↓                                               │    │
│  │      Continue or break                               │    │
│  └──────────────────────────────────────────────────────┘    │
│      ↓                                                      │
│  ⑦  on_session_end(ctx, summary)                            │
│      ↓                                                      │
│  ⑧  on_compaction(ctx, compacted_messages)                  │
└─────────────────────────────────────────────────────────────┘
```

### 7.3 Data Types

**New file**: `src-tauri/src/services/orchestrator/hooks.rs`

```rust
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Context available to all hooks
pub struct HookContext {
    pub session_id: String,
    pub project_path: String,
    pub provider_name: String,
    pub model_name: String,
}

/// Type aliases for hook callbacks
pub type SessionStartHook = Arc<
    dyn Fn(&HookContext) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send>>
        + Send + Sync
>;

pub type UserMessageHook = Arc<
    dyn Fn(&HookContext, &str) -> Pin<Box<dyn Future<Output = AppResult<Option<String>>> + Send>>
        + Send + Sync
>;

pub type BeforeLlmHook = Arc<
    dyn Fn(&HookContext, &mut Vec<Message>) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send>>
        + Send + Sync
>;

pub type AfterLlmHook = Arc<
    dyn Fn(&HookContext, &LlmResponse) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send>>
        + Send + Sync
>;

pub type BeforeToolHook = Arc<
    dyn Fn(&HookContext, &str, &serde_json::Value)
        -> Pin<Box<dyn Future<Output = AppResult<Option<ToolResult>>> + Send>>
        + Send + Sync
>;

pub type AfterToolHook = Arc<
    dyn Fn(&HookContext, &str, &ToolResult) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send>>
        + Send + Sync
>;

pub type SessionEndHook = Arc<
    dyn Fn(&HookContext, &SessionSummary) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send>>
        + Send + Sync
>;

pub type CompactionHook = Arc<
    dyn Fn(&HookContext, &[Message]) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send>>
        + Send + Sync
>;

/// Registry of all hooks
pub struct AgenticHooks {
    pub on_session_start: Vec<SessionStartHook>,
    pub on_user_message: Vec<UserMessageHook>,
    pub on_before_llm: Vec<BeforeLlmHook>,
    pub on_after_llm: Vec<AfterLlmHook>,
    pub on_before_tool: Vec<BeforeToolHook>,
    pub on_after_tool: Vec<AfterToolHook>,
    pub on_session_end: Vec<SessionEndHook>,
    pub on_compaction: Vec<CompactionHook>,
}

impl AgenticHooks {
    pub fn new() -> Self;

    /// Execute all hooks of a given type sequentially
    pub async fn fire_session_start(&self, ctx: &HookContext) -> AppResult<()>;
    pub async fn fire_user_message(&self, ctx: &HookContext, msg: &str) -> AppResult<Option<String>>;
    pub async fn fire_before_llm(&self, ctx: &HookContext, messages: &mut Vec<Message>) -> AppResult<()>;
    pub async fn fire_after_llm(&self, ctx: &HookContext, response: &LlmResponse) -> AppResult<()>;
    pub async fn fire_before_tool(&self, ctx: &HookContext, name: &str, args: &Value) -> AppResult<Option<ToolResult>>;
    pub async fn fire_after_tool(&self, ctx: &HookContext, name: &str, result: &ToolResult) -> AppResult<()>;
    pub async fn fire_session_end(&self, ctx: &HookContext, summary: &SessionSummary) -> AppResult<()>;
    pub async fn fire_compaction(&self, ctx: &HookContext, messages: &[Message]) -> AppResult<()>;
}

/// Summary of a completed session for hooks
pub struct SessionSummary {
    pub session_id: String,
    pub task_description: String,
    pub files_read: Vec<(String, usize, u64)>,
    pub key_findings: Vec<String>,
    pub tool_usage: HashMap<String, usize>,
    pub total_turns: usize,
    pub success: bool,
}
```

### 7.4 Hook Registration (Built-in Hooks)

```rust
/// Build default hooks with memory and skill integration
pub fn build_default_hooks(
    memory_store: Arc<ProjectMemoryStore>,
    skill_index: Arc<RwLock<Option<SkillIndex>>>,
    extraction_provider: Arc<dyn LlmProvider>,
) -> AgenticHooks {
    let mut hooks = AgenticHooks::new();

    // Hook 1: Load project memories at session start
    hooks.on_session_start.push(Arc::new(move |ctx| {
        let store = memory_store.clone();
        Box::pin(async move {
            // Pre-load top memories for this project
            // Store in HookContext for later injection
            Ok(())
        })
    }));

    // Hook 2: Skill matching on user message
    hooks.on_user_message.push(Arc::new(move |ctx, msg| {
        let index = skill_index.clone();
        Box::pin(async move {
            // Match skills and inject into message
            // Return modified message or None
            Ok(None)
        })
    }));

    // Hook 3: Memory extraction at session end
    hooks.on_session_end.push(Arc::new(move |ctx, summary| {
        let store = memory_store.clone();
        let provider = extraction_provider.clone();
        Box::pin(async move {
            // Extract memories from session summary
            // Upsert to ProjectMemoryStore
            Ok(())
        })
    }));

    // Hook 4: Memory extraction during compaction
    hooks.on_compaction.push(Arc::new(move |ctx, messages| {
        let store = memory_store.clone();
        Box::pin(async move {
            // Extract memories from compacted messages before they're lost
            Ok(())
        })
    }));

    hooks
}
```

### 7.5 Integration with Agentic Loop

**Modify**: `services/orchestrator/service_helpers/agentic_loop.rs`

The key change is inserting hook fire points into the existing loop without restructuring it:

```rust
// In the main loop function:

// 1. Fire session start (before first LLM call)
if is_first_turn {
    hooks.fire_session_start(&hook_ctx).await?;
}

// 2. Fire user message (when new user input arrives)
if let Some(modified) = hooks.fire_user_message(&hook_ctx, &user_message).await? {
    user_message = modified;
}

// 3. Fire before LLM (before each provider call)
hooks.fire_before_llm(&hook_ctx, &mut messages).await?;

// 4. [Existing LLM call]

// 5. Fire after LLM (after each provider response)
hooks.fire_after_llm(&hook_ctx, &response).await?;

// 6. Fire before tool (before each tool execution)
if let Some(intercepted) = hooks.fire_before_tool(&hook_ctx, &tool_name, &args).await? {
    tool_result = intercepted;  // Hook intercepted the tool call
} else {
    // 7. [Existing tool execution]
}

// 8. Fire after tool
hooks.fire_after_tool(&hook_ctx, &tool_name, &tool_result).await?;

// 9. Fire session end (when loop terminates)
hooks.fire_session_end(&hook_ctx, &session_summary).await?;
```

---

## 8. P1: State Scope Layering

### 8.1 Overview

Introduce semantic state scopes (inspired by adk-rust's `user:`, `app:`, `temp:` prefixes) to differentiate state lifetime in `SessionMemoryManager`.

### 8.2 Scope Definitions

```rust
/// State scope determines lifetime and persistence behavior
pub enum StateScope {
    /// Temporary: cleared after each tool call round
    /// Use for: intermediate computation results, current file context
    Temp,

    /// Session: persists within the current session
    /// Use for: accumulated findings, files read, task context
    /// This is the current SessionMemory behavior
    Session,

    /// Project: persists across sessions (stored in project_memories)
    /// Use for: user preferences, project conventions, patterns
    Project,
}

/// Key prefix convention
pub const KEY_PREFIX_TEMP: &str = "temp:";
pub const KEY_PREFIX_SESSION: &str = "session:";
pub const KEY_PREFIX_PROJECT: &str = "project:";
```

### 8.3 Modifications to SessionMemory

**Modify**: `services/orchestrator/service_helpers/session_state.rs`

```rust
pub(super) struct SessionMemory {
    pub(super) files_read: Vec<(String, usize, u64)>,
    pub(super) key_findings: Vec<String>,
    pub(super) task_description: String,
    pub(super) tool_usage_counts: HashMap<String, usize>,
    // NEW: Scoped key-value state
    pub(super) scoped_state: HashMap<String, serde_json::Value>,
}

impl SessionMemory {
    /// Set a scoped value
    pub fn set_state(&mut self, key: &str, value: serde_json::Value) {
        self.scoped_state.insert(key.to_string(), value);
    }

    /// Get a scoped value
    pub fn get_state(&self, key: &str) -> Option<&serde_json::Value> {
        self.scoped_state.get(key)
    }

    /// Clear all temp: prefixed state (called after each tool round)
    pub fn clear_temp_state(&mut self) {
        self.scoped_state.retain(|k, _| !k.starts_with(KEY_PREFIX_TEMP));
    }

    /// Extract project: prefixed state for persistence
    pub fn extract_project_state(&self) -> HashMap<String, serde_json::Value> {
        self.scoped_state
            .iter()
            .filter(|(k, _)| k.starts_with(KEY_PREFIX_PROJECT))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}
```

### 8.4 Benefits

- `temp:current_file` — auto-cleared, no memory bloat
- `session:key_finding_xxx` — survives within session, extracted at end
- `project:user_prefers_pnpm` — immediately persisted to `ProjectMemoryStore`

---

## 9. P2: Tool Trait Abstraction

### 9.1 Overview

Refactor the hardcoded tool system in `executor.rs` (3415 lines, 14 tools in a match statement) into a trait-based, registry-driven architecture. Inspired by adk-rust's `Tool` trait + `Toolset` pattern.

### 9.2 Tool Trait

**New file**: `src-tauri/src/services/tools/trait_def.rs`

```rust
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

/// Unified tool interface
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name (must match LLM function call name)
    fn name(&self) -> &str;

    /// Human-readable description for system prompt
    fn description(&self) -> &str;

    /// JSON Schema for input parameters
    fn parameters_schema(&self) -> Value;

    /// Whether this tool might take a long time
    fn is_long_running(&self) -> bool { false }

    /// Execute the tool with the given arguments
    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult;
}

/// Context available during tool execution
pub struct ToolExecutionContext {
    pub session_id: String,
    pub project_root: PathBuf,
    pub working_directory: PathBuf,
    pub read_cache: Arc<Mutex<HashMap<(PathBuf, usize, usize), ReadCacheEntry>>>,
    pub cancellation_token: CancellationToken,
}

/// Collection of tools
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self;

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn Tool>);

    /// Unregister a tool by name
    pub fn unregister(&mut self, name: &str) -> Option<Arc<dyn Tool>>;

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>>;

    /// Get all tool definitions for system prompt
    pub fn definitions(&self) -> Vec<ToolDefinition>;

    /// Get all tool names
    pub fn names(&self) -> Vec<String>;

    /// Execute a tool by name
    pub async fn execute(
        &self,
        name: &str,
        ctx: &ToolExecutionContext,
        args: Value,
    ) -> AppResult<ToolResult>;
}
```

### 9.3 Migration Strategy

**Phase 1**: Define `Tool` trait alongside existing `executor.rs` match statement.

**Phase 2**: Implement each existing tool as a struct implementing `Tool`:

```rust
// Example: ReadTool
pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str { "Read" }

    fn description(&self) -> &str {
        "Read a file from the local filesystem. Supports text files, images, PDFs, and Jupyter notebooks."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string", "description": "Absolute path to the file" },
                "offset": { "type": "integer", "description": "Line number to start reading from" },
                "limit": { "type": "integer", "description": "Number of lines to read" },
                "pages": { "type": "string", "description": "Page range for PDF files" }
            },
            "required": ["file_path"]
        })
    }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        // Move existing Read implementation from executor.rs
    }
}
```

**Phase 3**: Replace the match statement in `executor.rs` with `ToolRegistry::execute()`.

**Phase 4**: Remove the old match branches.

### 9.4 Individual Tool Structs

Each of the 14 existing tools becomes its own struct:

| Current match branch | New struct | File |
|---------------------|-----------|------|
| `"Read"` | `ReadTool` | `services/tools/impls/read.rs` |
| `"Write"` | `WriteTool` | `services/tools/impls/write.rs` |
| `"Edit"` | `EditTool` | `services/tools/impls/edit.rs` |
| `"Bash"` | `BashTool` | `services/tools/impls/bash.rs` |
| `"Glob"` | `GlobTool` | `services/tools/impls/glob.rs` |
| `"Grep"` | `GrepTool` | `services/tools/impls/grep.rs` |
| `"LS"` | `LsTool` | `services/tools/impls/ls.rs` |
| `"Cwd"` | `CwdTool` | `services/tools/impls/cwd.rs` |
| `"Analyze"` | `AnalyzeTool` | `services/tools/impls/analyze.rs` |
| `"Task"` | `TaskTool` | `services/tools/impls/task.rs` |
| `"WebFetch"` | `WebFetchTool` | `services/tools/impls/web_fetch.rs` |
| `"WebSearch"` | `WebSearchTool` | `services/tools/impls/web_search.rs` |
| `"NotebookEdit"` | `NotebookEditTool` | `services/tools/impls/notebook_edit.rs` |
| `"CodebaseSearch"` | `CodebaseSearchTool` | `services/tools/impls/codebase_search.rs` |

### 9.5 Benefits

- Dynamic tool enable/disable (user settings, agent profiles)
- Clean per-tool unit testing
- Auto-generated tool definitions from trait (no manual sync with `definitions.rs`)
- Foundation for MCP tool integration (P2)
- Each tool file is ~100-300 lines instead of one 3415-line file

---

## 10. P2: MCP Toolset Runtime Integration

### 10.1 Overview

Extend the Tool Registry to support dynamically loaded tools from MCP (Model Context Protocol) servers at runtime. Currently MCP is UI-only (`src/components/MCP/`); this adds actual tool execution.

### 10.2 Architecture

```
┌─────────────────────────────────────────────────┐
│                  ToolRegistry                    │
│                                                 │
│  Built-in Tools (14)                            │
│  ├── ReadTool, WriteTool, EditTool, ...         │
│                                                 │
│  MCP Tool Adapters (dynamic)                    │
│  ├── mcp_server_1::tool_a (via stdio)           │
│  ├── mcp_server_1::tool_b (via stdio)           │
│  └── mcp_server_2::tool_c (via HTTP/SSE)        │
└─────────────────────────────────────────────────┘
```

### 10.3 MCP Tool Adapter

**New file**: `src-tauri/src/services/tools/mcp_adapter.rs`

```rust
/// Wraps an MCP server tool as an ADK-style Tool
pub struct McpToolAdapter {
    server_name: String,
    tool_name: String,
    description: String,
    parameters_schema: Value,
    client: Arc<McpClient>,
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        // Namespaced: "mcp:{server_name}:{tool_name}"
        &self.qualified_name
    }

    fn description(&self) -> &str { &self.description }

    fn parameters_schema(&self) -> Value { self.parameters_schema.clone() }

    async fn execute(&self, ctx: &ToolExecutionContext, args: Value) -> ToolResult {
        // Proxy execution to MCP server via client
        match self.client.call_tool(&self.tool_name, args).await {
            Ok(response) => ToolResult::ok(response.to_string()),
            Err(e) => ToolResult::err(format!("MCP tool error: {}", e)),
        }
    }
}
```

### 10.4 MCP Client

```rust
/// MCP client supporting stdio and HTTP/SSE transports
pub struct McpClient {
    transport: McpTransport,
    server_info: McpServerInfo,
}

pub enum McpTransport {
    Stdio {
        process: Child,
        stdin: ChildStdin,
        stdout: BufReader<ChildStdout>,
    },
    Http {
        base_url: String,
        client: reqwest::Client,
        headers: HashMap<String, String>,
    },
}

impl McpClient {
    /// Connect to an MCP server
    pub async fn connect(config: &McpServerConfig) -> AppResult<Self>;

    /// Discover available tools
    pub async fn list_tools(&self) -> AppResult<Vec<McpToolInfo>>;

    /// Call a tool on the MCP server
    pub async fn call_tool(&self, name: &str, args: Value) -> AppResult<Value>;

    /// Disconnect and cleanup
    pub async fn disconnect(&mut self) -> AppResult<()>;
}
```

### 10.5 Integration with Existing MCP UI

**Modify**: `src-tauri/src/commands/mcp.rs` (existing)

Add new commands:

```rust
/// When user enables an MCP server, connect and register its tools
#[tauri::command]
pub async fn connect_mcp_server(
    server_id: String,
    state: State<'_, AppState>,
    standalone: State<'_, StandaloneState>,
) -> Result<CommandResponse<Vec<McpToolInfo>>, String>;

/// When user disables an MCP server, unregister its tools
#[tauri::command]
pub async fn disconnect_mcp_server(
    server_id: String,
    state: State<'_, AppState>,
    standalone: State<'_, StandaloneState>,
) -> Result<CommandResponse<()>, String>;
```

### 10.6 Schema Sanitization

MCP tool schemas may contain JSON Schema features that LLMs don't understand well. Sanitize before registration:

```rust
/// Remove unsupported JSON Schema features for LLM compatibility
/// Following adk-rust's approach (adk-tool/src/mcp/toolset.rs)
pub fn sanitize_schema(schema: &mut Value) {
    // Remove: $schema, $ref, $id, definitions, $defs
    // Flatten allOf/anyOf/oneOf where possible
    // Convert complex types to simpler descriptions
}
```

---

## 11. Cross-Cutting Concerns

### 11.1 Token Budget Management

All new content injected into the system prompt must respect token budgets:

| Component | Budget | Priority |
|-----------|--------|----------|
| System prompt base | ~500 tokens | Fixed |
| Project index summary | ~1000 tokens | Fixed |
| Tool definitions | ~2000 tokens | Fixed |
| **Project memories** | **~500 tokens** | **Ranked by relevance** |
| **Matched skills** | **~1000 tokens** | **Top 1-2 skills** |
| Session memory | ~500 tokens | Fixed |

**Total Layer 1 budget**: ~5500 tokens (well within all providers' limits).

If budget is exceeded, apply this priority order for trimming:
1. Reduce matched skills to 1 (or skip if low relevance)
2. Reduce project memories to top 5
3. Truncate skill body

### 11.2 Provider Compatibility

Memory and skill injection must work across all 7 LLM providers:

| Provider | Consideration |
|----------|--------------|
| Anthropic | Leverage `cache_control: ephemeral` on Layer 1 — memories/skills benefit from prompt caching |
| OpenAI | Standard system message injection |
| DeepSeek | Standard system message injection |
| Qwen | Standard system message injection |
| GLM | Standard system message injection |
| MiniMax | Standard system message injection (Anthropic protocol) |
| Ollama | Minimize injection size (smaller context windows) |

### 11.3 Privacy & Security

- Memories are stored per-project in the local SQLite database
- No memories are sent to external services (beyond the LLM API call itself)
- Users can view, edit, and delete all memories through the UI
- Memory extraction prompts must NOT extract secrets (API keys, passwords, tokens)
- Add explicit instruction in extraction prompt: "Do NOT extract secrets, credentials, API keys, or passwords"

### 11.4 Performance Considerations

| Operation | Target Latency | Strategy |
|-----------|---------------|----------|
| Memory search at session start | < 50ms | SQLite index + pre-computed embeddings |
| Skill index load | < 100ms | File-based, cached in memory |
| Skill matching | < 10ms | Lexical scoring (no LLM call) |
| Memory extraction at session end | < 5s | Single lightweight LLM call |
| Memory decay/prune | < 100ms | Batch SQL update |

### 11.5 Telemetry

Track memory/skill usage metrics in the existing analytics system:

```sql
-- New analytics event types
-- 'memory_extracted': count of memories extracted from session
-- 'memory_injected': count of memories injected into session
-- 'skill_matched': which skill was matched and its score
-- 'skill_used': whether the matched skill actually helped (heuristic: fewer iterations than average)
```

---

## 12. Migration & Compatibility

### 12.1 Database Migration

Add migration logic in `Database::init_schema()`:

```rust
// Check if new tables exist; create if not
// This is additive — no existing tables are modified
// Existing sessions, analytics, agents data are unaffected
```

### 12.2 Backward Compatibility

- All new features are **additive** — no existing functionality is removed or changed
- Sessions created before memory system exists will simply have no memories to inject
- The agentic loop continues to work identically if no hooks are registered
- Tool trait abstraction (P2) preserves exact same behavior; only internal structure changes

### 12.3 Feature Flags

Consider gating new features behind settings for gradual rollout:

```json
{
  "memory_enabled": true,
  "memory_auto_extract": true,
  "memory_max_injected": 10,
  "skills_enabled": true,
  "skills_auto_generate": false,
  "skills_max_injected_chars": 4000
}
```

---

## 13. Testing Strategy

### 13.1 Backend Unit Tests

| Module | Test File | Key Tests |
|--------|-----------|-----------|
| `ProjectMemoryStore` | `tests/memory_store_tests.rs` | CRUD, search, upsert dedup, decay, prune, compact |
| `MemoryExtractor` | `tests/memory_extraction_tests.rs` | Prompt construction, response parsing, dedup against existing |
| `SkillIndex` | `tests/skill_index_tests.rs` | Discovery, parsing, frontmatter validation, hash consistency |
| `select_skills` | `tests/skill_select_tests.rs` | Scoring accuracy, tag filtering, tie-breaking, edge cases |
| `inject_skills_into_content` | `tests/skill_injector_tests.rs` | Format, truncation, multiple skills, empty cases |
| `AgenticHooks` | `tests/hooks_tests.rs` | Registration, fire order, error propagation, hook intercept |
| `ToolRegistry` | `tests/tool_registry_tests.rs` | Register, unregister, execute, unknown tool error |

### 13.2 Frontend Tests

| Component | Test File | Key Tests |
|-----------|-----------|-----------|
| `MemoryPanel` | `MemoryPanel.test.tsx` | Render, tab switching, empty state |
| `MemoryList` | `MemoryList.test.tsx` | List rendering, category filter, pagination |
| `MemoryCard` | `MemoryCard.test.tsx` | Edit mode, delete confirmation, importance display |
| `SkillList` | `SkillList.test.tsx` | File vs generated, enable/disable toggle |
| `memory store` | `memory.test.ts` | Store actions, IPC mock, state updates |

### 13.3 Integration Tests

| Scenario | Description |
|----------|-------------|
| Memory round-trip | Create session → extract memories → new session → verify injection |
| Skill matching | Create .skills/ files → start session → verify skill injected in prompt |
| Memory decay | Create memories → advance time → run decay → verify importance reduced |
| Hook chain | Register 3 hooks → fire session_end → verify all executed in order |
| Cross-provider | Run memory injection with Anthropic vs Ollama → verify both work |

---

## 14. Appendix

### 14.1 File Change Summary

| Priority | New Files | Modified Files |
|----------|-----------|---------------|
| **P0** | `services/memory/mod.rs` | `storage/database.rs` (add tables) |
| | `services/memory/store.rs` | `services/tools/system_prompt.rs` (add memory/skill sections) |
| | `services/memory/extraction.rs` | `state.rs` (add memory_store field) |
| | `services/memory/retrieval.rs` | `main.rs` (register new commands) |
| | `services/memory/maintenance.rs` | `commands/standalone.rs` (integrate memory load/save) |
| | `services/skills/mod.rs` | |
| | `services/skills/model.rs` | |
| | `services/skills/discovery.rs` | |
| | `services/skills/parser.rs` | |
| | `services/skills/index.rs` | |
| | `services/skills/select.rs` | |
| | `services/skills/injector.rs` | |
| | `commands/memory.rs` | |
| | `commands/skills.rs` | |
| | `src/components/Memory/*.tsx` (8 files) | |
| | `src/store/memory.ts` | |
| **P1** | `services/orchestrator/hooks.rs` | `services/orchestrator/service_helpers/agentic_loop.rs` (insert hook fire points) |
| | | `services/orchestrator/service_helpers/session_state.rs` (add scoped state) |
| **P2** | `services/tools/trait_def.rs` | `services/tools/executor.rs` (replace match with registry) |
| | `services/tools/registry.rs` | `services/tools/definitions.rs` (derive from trait) |
| | `services/tools/impls/*.rs` (14 files) | `commands/mcp.rs` (add connect/disconnect) |
| | `services/tools/mcp_adapter.rs` | |
| | `services/tools/mcp_client.rs` | |

### 14.2 adk-rust Reference Mapping

| adk-rust Module | Concept Borrowed | Plan Cascade Target |
|----------------|-----------------|-------------------|
| `adk-skill/src/discovery.rs` | File scanning with ignored dirs | `services/skills/discovery.rs` |
| `adk-skill/src/parser.rs` | Frontmatter + convention file parsing | `services/skills/parser.rs` |
| `adk-skill/src/select.rs` | Lexical scoring (4.0/2.5/2.0/1.0 weights) | `services/skills/select.rs` |
| `adk-skill/src/injector.rs` | `[skill:name]...[/skill]` injection format | `services/skills/injector.rs` |
| `adk-skill/src/model.rs` | SkillDocument, SkillIndex, SkillMatch types | `services/skills/model.rs` |
| `adk-memory/src/service.rs` | MemoryService trait design | `services/memory/store.rs` |
| `adk-plugin` | Lifecycle hooks (before/after model/tool) | `services/orchestrator/hooks.rs` |
| `adk-core/src/event.rs` | State key prefixes (user:/app:/temp:) | `session_state.rs` scoped state |
| `adk-core/src/tool.rs` | Tool trait + Toolset pattern | `services/tools/trait_def.rs` |
| `adk-tool/src/mcp/toolset.rs` | McpToolset + schema sanitization | `services/tools/mcp_adapter.rs` |

### 14.3 Cognitive Science Memory Model Reference

```
┌──────────────────────────────────────────────────────────┐
│                    Human Memory Model                     │
│                                                          │
│  Sensory Memory      → Context Window (existing)         │
│  Working Memory       → SessionMemory (existing)          │
│  Long-Term Memory                                         │
│  ├── Semantic         → project_memories table (P0)       │
│  ├── Episodic         → episodic_records table (P0)       │
│  └── Procedural       → skill_library + .skills/ (P0)    │
│                                                          │
│  Memory Consolidation → Memory Extraction (P0)            │
│  Memory Retrieval     → Search + Injection (P0)           │
│  Memory Decay         → Importance decay + prune (P0)     │
│  Reflection           → Skill generation (P0)             │
└──────────────────────────────────────────────────────────┘
```

### 14.4 Research References

| System | Key Contribution | Applicable Here |
|--------|-----------------|----------------|
| [Letta/MemGPT](https://docs.letta.com/concepts/memgpt/) | Self-editing memory, LLM OS metaphor | Agent-driven memory management |
| [Mem0](https://mem0.ai/) | Hybrid datastore (vector + graph + KV) | Retrieval scoring formula |
| [A-MEM](https://arxiv.org/abs/2502.12110) | Zettelkasten-style self-organizing memory | Memory linking and evolution |
| [MemOS](https://github.com/MemTensor/MemOS) | Skill as executable behavioral capability | Skill definition and reuse |
| [Voyager](https://voyager.minedojo.org/) | Code-based skill library for lifelong learning | Skill generation from sessions |
| [LangMem](https://blog.langchain.com/langmem-sdk-launch/) | Three memory types (semantic/episodic/procedural) | Memory categorization |
| [Generative Agents](https://arxiv.org/abs/2304.03442) | Memory stream + reflection + planning | Relevance-recency-importance scoring |
| [adk-rust](https://github.com/anthropics/adk-rust) | Skill discovery, parsing, injection; Plugin hooks | Direct implementation reference |

---

*This document will be updated as implementation progresses. Track changes via git history.*
