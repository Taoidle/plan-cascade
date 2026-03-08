# Plan Cascade Desktop Codebase Index Design Document

**Version**: 1.0.0
**Date**: 2026-03-08
**Scope**: Codebase indexing, search pipeline, hybrid search architecture

---

## Table of Contents

1. [Current State of Codebase Indexing](#1-current-state-of-codebase-indexing)
2. [Architecture Diagram](#2-architecture-diagram)
3. [Phase 1: HNSW Vector Index Design](#3-phase-1-hnsw-vector-index-design)
4. [Phase 2: FTS5 Full-Text Search Design](#4-phase-2-fts5-full-text-search-design)
5. [Phase 3: LSP Enhancement Layer Design](#5-phase-3-lsp-enhancement-layer-design)
6. [Search Pipeline Description](#6-search-pipeline-description)

---

## 1. Current State of Codebase Indexing

> Source: `codebase-index-iteration-plan.md`

### 1.1 Implemented Features

The codebase indexing feature currently includes:

1. **HNSW ANN Semantic Search** - Using SQLite fallback
2. **FTS5 BM25 Search** - Symbol and file path channels, with LIKE fallback
3. **LSP Enrichment Metadata** - Parsing types + reference counts for search output

The remaining gap is productization and UX consistency (multi-root, context handoff, and operation-level observability), not core search primitives.

### 1.2 Core Components

| Component | Location | Role |
|-----------|----------|------|
| `EmbeddingService` | `services/orchestrator/embedding_service.rs` | TF-IDF vectorization, lexical search |
| `IndexStore` | `services/orchestrator/index_store.rs` | SQLite table management |
| `HybridSearchEngine` | `services/orchestrator/hybrid_search.rs` | Multi-channel search fusion |
| `BackgroundIndexer` | `services/orchestrator/background_indexer.rs` | Background index building |

---

## 2. Architecture Diagram

### 2.1 Overall Architecture After Three Phases

```
Project Opened
    │
    ├─ Tree-sitter fast parse → file_index, file_symbols, symbol_fts (FTS5)
    │
    ├─ Embedding generation   → file_embeddings (SQLite BLOB) + HNSW index (disk file)
    │
    └─ LSP enrichment (optional, async)
          → symbol_resolved_type, reference_count, definition_location
          → cross_references table

Hybrid Search Query
    │
    ├─ Channel 1: FTS5 BM25 symbol search    (replaces LIKE)
    ├─ Channel 2: FTS5 BM25 filepath search  (replaces LIKE)
    └─ Channel 3: HNSW ANN semantic search   (replaces brute-force)
          │
          └─ RRF Fusion → ranked results
```

---

## 3. Phase 1: HNSW Vector Index Design

### 3.1 Goals

Replace O(n) brute-force cosine similarity scanning with O(log n) approximate nearest neighbor search using `hnsw_rs`. Target: **<1ms** per query on 100K vectors (currently ~50ms).

### 3.2 Storage Model: SQLite + HNSW Sidecar

```
~/.local/share/plan-cascade/
  └─ app.db                           ← SQLite (file_embeddings table unchanged)
  └─ hnsw_indexes/
       └─ <project-hash>/
            ├─ embeddings.hnsw.graph  ← HNSW topology
            └─ embeddings.hnsw.data   ← vector data
```

- **SQLite remains the source of truth** for all embedding data (CRUD, backup, migration)
- **HNSW files are derived caches** — can be deleted anytime and rebuilt from SQLite
- Project hash = SHA-256 of project_path, truncated to 16 hex chars

### 3.3 HNSW Parameters

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `max_nb_connection` | 24 | Good recall/speed balance for <100K vectors |
| `max_layer` | 16 | Standard default value |
| `ef_construction` | 200 | Higher = better index quality, one-time cost |
| `ef_search` | 64 | Adjustable at query time; 64 gives >95% recall |
| Distance | `DistCosine` | Matches current `cosine_similarity` function |

### 3.4 Lifecycle Flow

```
App Start (init_app)
    │
    ├─ IndexManager::new(pool)
    │   └─ index_store = IndexStore::new(pool)
    │
    ├─ For each known project:
    │   ├─ HnswIndex::new(index_dir, dimension)
    │   ├─ hnsw_index.load_from_disk()
    │   │   ├─ Ok(true)  → ready, skip rebuild
    │   │   └─ Ok(false) → rebuild_from_store(index_store, project_path)
    │   └─ Store in hnsw_indexes map
    │
    └─ ensure_indexed(project_path)
        └─ BackgroundIndexer runs:
            ├─ Phase 1: Tree-sitter parse → SQLite
            ├─ Phase 1b: Embedding generation → SQLite + hnsw_index.insert()
            └─ Phase 1c: hnsw_index.save_to_disk()

Query (CodebaseSearch / semantic_search command)
    │
    ├─ hnsw_index.search(query_embedding, top_k)
    │   → returns Vec<(embedding_id, distance)>
    │
    ├─ index_store.get_chunks_by_ids(ids)
    │   → returns Vec<(file_path, chunk_text)>
    │
    └─ Combine into SemanticSearchResult
```

### 3.5 Deletion Considerations

`hnsw_rs` **does NOT** support point deletion. For incremental file updates:

1. **Soft deletion**: Maintain a `HashSet<usize>` to mark stale IDs; filter from search results
2. **Periodic rebuild**: When stale IDs exceed 10% of total, rebuild from SQLite
3. **Re-index**: Always build fresh

---

## 4. Phase 2: FTS5 Full-Text Search Design

### 4.1 Goals

Replace `WHERE name LIKE '%query%'` with SQLite FTS5 ranked full-text search. Gain BM25 relevance scoring, boolean queries, prefix matching, and phrase search.

### 4.2 Schema Changes

```sql
-- FTS5 virtual table for symbol full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS symbol_fts USING fts5(
    symbol_name,
    file_path,
    symbol_kind,
    doc_comment,
    signature,
    content='',
    contentless_delete=1,
    tokenize='unicode61 remove_diacritics 2 tokenchars _'
);

-- FTS5 virtual table for file path search
CREATE VIRTUAL TABLE IF NOT EXISTS filepath_fts USING fts5(
    file_path,
    component,
    language,
    content='',
    contentless_delete=1,
    tokenize='unicode61 tokenchars _/.'
);
```

### 4.3 Key Tokenizer Selection

- `unicode61`: Handles non-ASCII (CJK, accented characters)
- `tokenchars _`: Treats underscores as part of tokens (matches Rust/Python naming)
- `tokenchars _/.`: For file paths, also treats `/` and `.` as token characters

### 4.4 FTS Query Sanitization

FTS5 has special syntax (`AND`, `OR`, `NOT`, `*`, `"`, `NEAR`). User queries need sanitization:

```rust
fn sanitize_fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .map(|token| {
            let escaped = token.replace('"', "\"\"");
            format!("\"{}\"*", escaped)
        })
        .collect::<Vec<_>>()
        .join(" ")
}
// "user controller" → "\"user\"* \"controller\"*"
```

---

## 5. Phase 3: LSP Enhancement Layer Design

### 5.1 Goals

Add type resolution and cross-reference information to search results using Language Server Protocol.

### 5.2 Enrichment Data

| Field | Description |
|-------|-------------|
| `symbol_resolved_type` | Fully qualified type for symbols |
| `reference_count` | Number of references to this symbol |
| `definition_location` | File and line of definition |
| `cross_references` | List of all reference locations |

### 5.3 Async LSP Processing

LSP enrichment runs asynchronously:

1. Indexing triggers LSP analysis in background
2. Results stored in `symbol_enrichment` table
3. Search results joined with enrichment data
4. Graceful degradation if LSP unavailable

### 5.4 Schema

```sql
CREATE TABLE IF NOT EXISTS symbol_enrichment (
    symbol_id TEXT PRIMARY KEY,
    resolved_type TEXT,
    reference_count INTEGER DEFAULT 0,
    definition_file TEXT,
    definition_line INTEGER,
    cross_references TEXT,  -- JSON array of {file, line, column}
    enriched_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS cross_references (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_symbol_id TEXT NOT NULL,
    to_symbol_id TEXT NOT NULL,
    file_path TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    column_start INTEGER,
    column_end INTEGER,
    FOREIGN KEY (from_symbol_id) REFERENCES file_symbols(id),
    FOREIGN KEY (to_symbol_id) REFERENCES file_symbols(id)
);
```

---

## 6. Search Pipeline Description

### 6.1 Unified Search API

```typescript
interface CodebaseSearchRequest {
  query: string;
  mode: 'hybrid' | 'symbol' | 'path' | 'semantic';
  filters?: {
    language?: string;
    file_path_prefix?: string;
    component?: string;
  };
  limit?: number;
  offset?: number;
}
```

### 6.2 Channel Fusion: RRF

**Reciprocal Rank Fusion** combines results from multiple channels:

```
RRF_score(d) = Σ 1 / (k + rank_i(d))

Where:
  k = 60 (constant)
  rank_i(d) = rank of document d in channel i
```

### 6.3 Query Flow

```
User Query
    │
    ▼
Query Analysis
    ├─ Determine search mode (hybrid/symbol/path/semantic)
    ├─ Extract filters
    └─ Sanitize FTS queries
    │
    ▼
Parallel Channel Search
    ├─ Channel 1: FTS5 symbol search
    ├─ Channel 2: FTS5 filepath search
    └─ Channel 3: HNSW semantic search (if mode includes semantic)
    │
    ▼
RRF Fusion
    ├─ Combine ranked lists
    ├─ Apply filters
    └─ Paginate results
    │
    ▼
Enrichment (optional)
    ├─ Join with LSP enrichment data
    └─ Add resolved types, references
    │
    ▼
Return Results
```

### 6.4 Performance Considerations

| Optimization | Impact |
|--------------|--------|
| HNSW O(log n) search | 50x faster than brute-force |
| FTS5 indexing | 100x faster than LIKE |
| RRF fusion | O(n) where n = total results |
| Background indexing | No blocking on query |
| Cached embeddings | Sub-ms retrieval |

---

## Appendix: Configuration

### Index Settings

```json
{
  "indexing": {
    "enabled": true,
    "auto_index_on_open": true,
    "background_indexing": true,
    "max_file_size_mb": 10,
    "exclude_patterns": ["node_modules", ".git", "target", "dist"]
  },
  "search": {
    "default_mode": "hybrid",
    "max_results": 100,
    "semantic_weight": 0.5,
    "symbol_weight": 0.3,
    "path_weight": 0.2
  },
  "hnsw": {
    "ef_search": 64,
    "max_connections": 24,
    "build_threads": 4
  }
}
```
