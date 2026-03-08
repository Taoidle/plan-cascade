# Plan Cascade Desktop Memory and Skill System Design Document

**Version**: 1.0.0
**Date**: 2026-03-08
**Scope**: Memory system architecture, skill system, V2 alignment

---

## Table of Contents

1. [Memory and Skill Architecture V2 Overview](#1-memory-and-skill-architecture-v2-overview)
2. [ADR: Memory Scope Model V2](#2-adr-memory-scope-model-v2)
3. [V2 Alignment Notes](#3-v2-alignment-notes)
4. [Key Terminology Definitions](#4-key-terminology-definitions)

---

## 1. Memory and Skill Architecture V2 Overview

> Source: First half of `memory-skill-iteration-plan.md`

### 1.1 V2 Alignment Notes

- Unified memory read paths are `query_memory_entries_v2` / `list_memory_entries_v2` / `memory_stats_v2`
- Canonical storage is `memory_entries_v2`, with explicit `scope/status/risk_tier/conflict_flag`
- Review governance uses `list_pending_memory_candidates_v2` and `review_memory_candidates_v2`
- Workflow phase truth source is `Workflow Kernel modeSnapshots`; frontend phase backfill has been removed
- ContextOps adds memory-specific SLI fields (`memory_query_p95_ms`, `empty_hit_rate`, `candidate_count`, `review_backlog`, `approve_rate`, `reject_rate`)

### 1.2 Architecture Vision

#### Memory Hierarchy

```
┌─────────────────────────────────────────────────────────────────┐
│                     Enhanced Context Management                  │
│                                                                 │
│  Layer 1 (Stable)                                               │
│  ├── System prompt                                              │
│  ├── Project index summary          (existing)                  │
│  ├── Tool definitions               (existing)                   │
│  ├── Project Memory (semantic)       (NEW - P0)                │
│  └── Matched Skills (procedural)     (NEW - P0)               │
│                                                                 │
│  Layer 2 (Semi-stable)                                          │
│  ├── SessionMemory                   (existing)                 │
│  └── Memory extraction trigger       (NEW - P1 hooks)           │
│                                                                 │
│  Layer 3 (Volatile)                                             │
│  └── Conversation messages           (existing)                 │
│                                                                 │
│  Persistent Storage (NEW)                                        │
│  ├── project_memories table          (NEW - P0)                │
│  ├── skill_library table             (NEW - P0)                 │
│  └── episodic_records table          (NEW - P0)                 │
└─────────────────────────────────────────────────────────────────┘
```

### 1.3 Data Flow

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

### 1.4 Key Terminology

| Term | Definition |
|------|------------|
| **Semantic Memory** | Persistent facts about users and projects (preferences, conventions, patterns) |
| **Episodic Memory** | Records of specific past interactions (what happened, what succeeded/failed) |
| **Procedural Memory / Skill** | Reusable task execution templates (step-by-step instructions for common operations) |
| **Memory Extraction | LLM-driven process of extracting structured memory entries from session interactions |
| **Skill Injection** | Matching and prepending relevant skills to user prompts before LLM calls |
| **Memory Decay** | Automatic reduction of importance for outdated, unused memories |

### 1.5 P0: Project Memory System

#### 1.5.1 Database Schema

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
    keywords TEXT NOT NULL DEFAULT '[]',
    embedding BLOB,
    importance REAL NOT NULL DEFAULT 0.5,
    access_count INTEGER NOT NULL DEFAULT 0,
    source_session_id TEXT,
    source_context TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_accessed_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(project_path, content)
);
```

#### 1.5.2 Search and Ranking Algorithm

Relevance scoring formula:

```
final_score = w1 * embedding_similarity
            + w2 * keyword_overlap
            + w3 * importance
            + w4 * recency_score

Where:
  w1 = 0.40  (semantic relevance)
  w2 = 0.25  (keyword matching)
  w3 = 0.20  (importance weight)
  w4 = 0.15  (recency bonus)
```

### 1.6 P0: Skill System

#### 1.6.1 Supported Skill Formats

| Format | Source | Characteristics |
|--------|--------|----------------|
| Plan Cascade SKILL.md | builtin-skills/ | Full functionality |
| adk-rust .skills/ | External framework | Lightweight |
| Convention Files | CLAUDE.md, AGENTS.md | No frontmatter |

#### 1.6.2 Four-Source Skill Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Skill Source Hierarchy                         │
│                                                                 │
│  Priority 1-50:   BUILTIN                                       │
│  │  Bundled with plan-cascade (python, go, java, typescript)   │
│  │                                                                │
│  Priority 51-100:  PROJECT                                       │
│  │  .skills/ directory in current project                       │
│  │                                                                │
│  Priority 101-150: USER                                          │
│  │  User-defined skills (~/.plan-cascade/skills/)               │
│  │                                                                │
│  Priority 151-200: COMMUNITY                                     │
│  │  Downloaded skill packages                                    │
└─────────────────────────────────────────────────────────────────┘
```

#### 1.6.3 Skill Matching Algorithm

Skills are matched using a multi-stage pipeline:

1. **Lexical matching**: Keywords and skill metadata
2. **Embedding similarity**: Semantic relevance (optional, expensive)
3. **Priority boost**: Higher priority skills score higher
4. **Diversity**: Avoid returning multiple skills for the same task

---

## 2. ADR: Memory Scope Model V2

> Source: `ADR-memory-scope-model-v2.md`

### 2.1 Decision

Memory entries have explicit **scope** that determines visibility and persistence:

| Scope | Visibility | Persistence | Use Case |
|-------|------------|-------------|----------|
| `session` | Current session only | Until session ends | Temporary working context |
| `project` | Project-scoped | Permanent until deleted | Project-specific knowledge |
| `global` | All projects | Permanent until deleted | User preferences, universal patterns |

### 2.2 Memory Status Lifecycle

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  candidate   │────>│   active    │────>│   archived   │
│  (pending    │     │  (in use)   │     │ (superseded) │
│   review)    │     │              │     │              │
└──────────────┘     └──────────────┘     └──────────────┘
       ▲                    │                    │
       │                    │                    │
       └────────────────────┴────────────────────┘
                    Reactivation
```

### 2.3 Risk Tier Classification

| Tier | Description | Example |
|------|-------------|---------|
| **Low** | Safe to use automatically | "User prefers dark mode" |
| **Medium** | Review recommended | "API key format changed" |
| **High** | Manual approval required | "Breaking change to build process" |

### 2.4 Conflict Detection

Memories can be flagged for conflicts:

- **Direct contradiction**: Same fact stated differently
- **Outdated**: Newer information supersedes old
- **Scope mismatch**: Project vs global confusion

---

## 3. V2 Alignment Notes

> Source: Second half of `memory-skill-iteration-plan.md`

### 3.1 Read Path Unification

All memory reads go through V2 commands:

```rust
// V2 read commands
pub async fn query_memory_entries_v2(...) -> Result<Vec<MemoryEntry>>;
pub async fn list_memory_entries_v2(...) -> Result<Vec<MemoryEntry>>;
pub async fn memory_stats_v2(...) -> Result<MemoryStats>;
```

### 3.2 Write Path Unification

Memory creation follows a candidate workflow:

```
User Interaction
       │
       ▼
LLM Extraction (session insights)
       │
       ▼
Create Memory Candidate
       │
       ▼
Pending Review Queue
       │
       ▼
[Auto-approve Low Risk] or [Human Review]
       │
       ▼
Active Memory Entry
```

### 3.3 ContextOps Integration

Memory system exposes SLI metrics:

```rust
struct MemoryMetrics {
    memory_query_p95_ms: f64,      // Query latency
    empty_hit_rate: f64,           // % of queries returning no results
    candidate_count: usize,        // Pending review count
    review_backlog: usize,         // Items needing review
    approve_rate: f64,             // Approval rate
    reject_rate: f64,              // Rejection rate
}
```

---

## 4. Key Terminology Definitions

### 4.1 Memory Types

| Type | Description | Example |
|------|-------------|---------|
| Semantic | Factual knowledge | "This project uses pnpm" |
| Episodic | Past events | "Yesterday's build failed due to X" |
| Procedural | How-to knowledge | Skill definitions |

### 4.2 Skill Types

| Type | Description |
|------|-------------|
| **Builtin** | Bundled with application |
| **Project** | Project-specific (.skills/) |
| **User** | User-defined (~/.plan-cascade/) |
| **Community** | Downloaded packages |

### 4.3 Query Concepts

| Concept | Definition |
|---------|------------|
| **Relevance Score** | Composite score from embedding, keywords, importance, recency |
| **Hit Rate** | Percentage of queries returning results |
| **Empty Hit** | Query with no matching memories |
| **Candidate** | Memory entry pending review |

---

## Appendix: Database Schema Reference

### Full Schema

```sql
-- Project memories table
CREATE TABLE project_memories (
    id TEXT PRIMARY KEY,
    project_path TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT 'project' CHECK(scope IN ('session', 'project', 'global')),
    status TEXT NOT NULL DEFAULT 'candidate' CHECK(status IN ('candidate', 'active', 'archived')),
    risk_tier TEXT NOT NULL DEFAULT 'low' CHECK(risk_tier IN ('low', 'medium', 'high')),
    conflict_flag BOOLEAN DEFAULT FALSE,
    category TEXT NOT NULL,
    content TEXT NOT NULL,
    keywords TEXT NOT NULL DEFAULT '[]',
    embedding BLOB,
    importance REAL NOT NULL DEFAULT 0.5,
    access_count INTEGER NOT NULL DEFAULT 0,
    source_session_id TEXT,
    source_context TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_accessed_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(project_path, content)
);

-- Skill library table
CREATE TABLE skill_library (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    source TEXT NOT NULL CHECK(source IN ('builtin', 'project', 'user', 'community')),
    priority INTEGER NOT NULL DEFAULT 100,
    format TEXT NOT NULL CHECK(format IN ('skill_md', 'adk_rust', 'convention')),
    content TEXT NOT NULL,
    metadata TEXT NOT NULL DEFAULT '{}',
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(source, name)
);

-- Episodic records table
CREATE TABLE episodic_records (
    id TEXT PRIMARY KEY,
    project_path TEXT NOT NULL,
    session_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    summary TEXT NOT NULL,
    details TEXT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    INDEX idx_project_session (project_path, session_id)
);
```
