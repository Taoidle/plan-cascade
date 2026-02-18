# Codebase Index Iteration Plan

> Phase 1: HNSW Vector Index · Phase 2: FTS5 Text Search · Phase 3: LSP Enhancement

## Current State Summary

The Codebase Index feature is functional but has three key bottlenecks:

1. **Vector search is O(n) brute-force** — `index_store.rs:semantic_search_filtered` loads all embeddings from SQLite into memory and computes cosine similarity one by one.
2. **Text search is naive SQL LIKE** — `hybrid_search.rs` symbol/filepath channels use `WHERE name LIKE '%query%'`, no ranking, no relevance scoring.
3. **Symbol extraction is syntax-only** — Tree-sitter extracts names/signatures but has no type information, cross-references, or call graphs.

### Architecture After All Three Phases

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

## Phase 1: HNSW Vector Index (hnsw_rs)

### Goal

Replace O(n) brute-force cosine similarity scan with O(log n) approximate nearest neighbor search using `hnsw_rs`. Target: **<1ms** per query at 100K vectors (currently ~50ms).

### Dependency

```toml
# Cargo.toml — add under [dependencies]
hnsw_rs = "0.3"
```

Pure Rust. No C/C++ compiler needed beyond what `rusqlite bundled` already requires. Adds `rayon`, `parking_lot`, `anndists` as transitive deps.

### Design

#### Storage Model: SQLite + HNSW Sidecar

```
~/.local/share/plan-cascade/         (or platform equivalent)
  └─ app.db                           ← SQLite (file_embeddings table unchanged)
  └─ hnsw_indexes/
       └─ <project-hash>/
            ├─ embeddings.hnsw.graph  ← HNSW topology
            └─ embeddings.hnsw.data   ← vector data
```

- **SQLite remains the source of truth** for all embedding data (CRUD, backup, migration).
- **HNSW files are a derived cache** — can be deleted and rebuilt from SQLite at any time.
- Project hash = SHA-256 of the project_path, truncated to 16 hex chars.

#### ID Mapping

`hnsw_rs` requires `usize` IDs for insertion. The `file_embeddings.id` column (INTEGER PRIMARY KEY) maps directly:

```
hnsw.insert((&embedding_vec, file_embeddings_id as usize))
```

Reverse lookup on search results:

```sql
SELECT file_path, chunk_index, chunk_text FROM file_embeddings WHERE id = ?1
```

#### HNSW Parameters

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `max_nb_connection` | 24 | Good recall/speed balance for <100K vectors |
| `max_layer` | 16 | Standard default |
| `ef_construction` | 200 | Higher = better index quality, one-time cost |
| `ef_search` | 64 | Tunable at query time; 64 gives >95% recall |
| Distance | `DistCosine` | Matches current `cosine_similarity` function |

#### Thread Safety

`Hnsw<f32, DistCosine>` is `Send + Sync`. Wrap in `Arc` and share across tokio tasks. Use `tokio::task::spawn_blocking` for `parallel_insert` and `parallel_search` (they use rayon internally).

### New Module: `hnsw_index.rs`

Location: `src-tauri/src/services/orchestrator/hnsw_index.rs`

```rust
/// Manages an in-process HNSW index backed by hnsw_rs.
///
/// Lifecycle:
///   1. On startup: try load from disk → if missing, rebuild from SQLite
///   2. On insert: insert into HNSW + periodic auto-save
///   3. On search: query HNSW → return (id, distance) pairs
///   4. On shutdown / reindex: save to disk
pub struct HnswIndex {
    /// The HNSW graph. None when no embeddings exist yet.
    inner: RwLock<Option<Arc<Hnsw<'static, f32, DistCosine>>>>,
    /// Directory for persistence files.
    index_dir: PathBuf,
    /// Embedding dimension (must match provider config).
    dimension: usize,
    /// Number of vectors currently in the index.
    count: AtomicUsize,
}
```

Public API:

```rust
impl HnswIndex {
    /// Create or open an HNSW index for a project.
    pub fn new(index_dir: PathBuf, dimension: usize) -> Self;

    /// Try to load a previously saved index from disk.
    /// Returns Ok(true) if loaded, Ok(false) if no files found.
    pub fn load_from_disk(&self) -> anyhow::Result<bool>;

    /// Rebuild the HNSW index from all embeddings in SQLite.
    pub async fn rebuild_from_store(&self, store: &IndexStore, project_path: &str) -> anyhow::Result<()>;

    /// Insert a single vector. Called during indexing.
    pub fn insert(&self, id: usize, embedding: &[f32]);

    /// Batch insert. Called during full reindex.
    pub fn batch_insert(&self, items: &[(&[f32], usize)]);

    /// Search for top_k nearest neighbors. Returns (id, distance) pairs.
    pub async fn search(&self, query: &[f32], top_k: usize) -> Vec<(usize, f32)>;

    /// Remove a vector by ID. For incremental updates, we rebuild
    /// the section (hnsw_rs does not support deletion — see Caveats).
    pub fn mark_stale(&self, id: usize);

    /// Persist current index state to disk.
    pub fn save_to_disk(&self) -> anyhow::Result<()>;

    /// Returns true if the index has been loaded/built.
    pub fn is_ready(&self) -> bool;
}
```

#### Deletion Caveat

`hnsw_rs` does **not** support point deletion. For incremental file updates:

1. **Soft-delete**: maintain a `HashSet<usize>` of stale IDs; filter them from search results.
2. **Periodic rebuild**: when stale IDs exceed 10% of total, rebuild from SQLite.
3. **On reindex**: always build fresh.

This is the standard approach used by all HNSW libraries without native deletion.

### Files to Modify

| File | Change |
|------|--------|
| `Cargo.toml` | Add `hnsw_rs = "0.3"` |
| `orchestrator/mod.rs` | Add `pub mod hnsw_index;` |
| **`orchestrator/hnsw_index.rs`** | **New file** — HnswIndex implementation |
| `orchestrator/index_manager.rs` | Add `hnsw_indexes: RwLock<HashMap<String, Arc<HnswIndex>>>` field. On `start_indexing` and `ensure_indexed`, create/load HnswIndex. Pass it to BackgroundIndexer. |
| `orchestrator/background_indexer.rs` | In `run_embedding_pass` / `run_embedding_pass_managed`: after storing embeddings in SQLite, also call `hnsw_index.insert()`. At end of pass, call `hnsw_index.save_to_disk()`. |
| `orchestrator/index_store.rs` | Add `get_all_embedding_ids_and_vectors(project_path) -> Vec<(usize, Vec<f32>)>` for HNSW rebuild. Keep existing `semantic_search` as fallback. |
| `orchestrator/hybrid_search.rs` | `search_semantic`: accept optional `Arc<HnswIndex>`. If present, use HNSW search → then fetch chunk_text from SQLite by ID. If absent, fall back to brute-force. |
| `services/tools/executor.rs` | Add `hnsw_index: Option<Arc<HnswIndex>>` field + setter. Wire into `execute_codebase_search` semantic path. |
| `commands/standalone.rs` | `semantic_search` command: prefer HnswIndex when available. |
| `commands/init.rs` | After IndexManager init, trigger HNSW load from disk. |

### Lifecycle Flow

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

Reindex (trigger_reindex)
    │
    ├─ index_store.delete_project_index()
    ├─ hnsw_index = HnswIndex::new()   // fresh, empty
    └─ start_indexing() → same as above
```

### Testing Strategy

1. **Unit tests** in `hnsw_index.rs`:
   - Create index → insert 100 random vectors → search → verify top-1 is exact match
   - Save to disk → load from disk → search → same results
   - Stale ID filtering works correctly
   - Empty index returns empty results

2. **Integration test** in `index_store.rs`:
   - `rebuild_from_store` produces same results as brute-force `semantic_search`

3. **Existing tests**: all `hybrid_search.rs` tests remain unchanged (HNSW is an optimization, not a behavior change).

---

## Phase 2: FTS5 Full-Text Search

### Goal

Replace `WHERE name LIKE '%query%'` with SQLite FTS5 ranked full-text search. Gain BM25 relevance scoring, boolean queries, prefix matching, and phrase search.

### Dependency

None. `rusqlite = { version = "0.32", features = ["bundled"] }` already includes FTS5.

### Schema Changes

Add to `database.rs` `init_schema()`, after the `file_symbols` and `file_index` table creation:

```sql
-- FTS5 virtual table for symbol full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS symbol_fts USING fts5(
    symbol_name,
    file_path,
    symbol_kind,
    doc_comment,
    signature,
    content='',           -- contentless: we manage content manually
    contentless_delete=1, -- allow DELETE on contentless table
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

Key tokenizer choices:
- `unicode61`: handles non-ASCII (CJK, accented chars)
- `tokenchars _`: treats underscore as part of tokens (so `user_controller` is one token, matching Rust/Python naming)
- `tokenchars _/.`: for file paths, also treats `/` and `.` as token chars

### FTS Sync Strategy

Use a **manual sync** approach (contentless FTS tables):

```rust
// On upsert_file_index: sync filepath FTS
conn.execute(
    "INSERT INTO filepath_fts(rowid, file_path, component, language)
     VALUES (?1, ?2, ?3, ?4)",
    params![file_index_id, item.path, item.component, item.language],
)?;

// On upsert symbols: sync symbol FTS (after DELETE + re-INSERT of file_symbols)
for symbol in &item.symbols {
    conn.execute(
        "INSERT INTO symbol_fts(rowid, symbol_name, file_path, symbol_kind, doc_comment, signature)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![symbol_row_id, symbol.name, item.path, kind_str, symbol.doc_comment, symbol.signature],
    )?;
}
```

On `delete_project_index`: also clear FTS entries.

### Files to Modify

| File | Change |
|------|--------|
| `storage/database.rs` | Add FTS5 virtual table creation in `init_schema()`. Add migration for existing databases. |
| `orchestrator/index_store.rs` | 1. `upsert_file_index`: after inserting symbols, INSERT INTO `symbol_fts` and `filepath_fts`. 2. `delete_project_index`: DELETE FROM both FTS tables. 3. Add new query methods: `fts_search_symbols(query, limit)` and `fts_search_files(query, project_path, limit)`. |
| `orchestrator/hybrid_search.rs` | Replace `search_symbols` and `search_file_paths` implementations to call the new FTS methods. |

### New Query Methods in `index_store.rs`

```rust
/// Full-text search for symbols using FTS5 with BM25 ranking.
///
/// Supports: simple terms, prefix (term*), phrases ("exact phrase"),
/// boolean (term1 AND term2), column filters (symbol_name:query).
pub fn fts_search_symbols(&self, query: &str, limit: usize) -> AppResult<Vec<SymbolMatch>> {
    let conn = self.get_connection()?;
    let mut stmt = conn.prepare(
        "SELECT s.rowid, sf.symbol_name, sf.file_path, sf.symbol_kind,
                sf.doc_comment, sf.signature,
                rank  -- FTS5 built-in BM25 rank
         FROM symbol_fts sf
         JOIN file_symbols fs ON fs.rowid = sf.rowid
         JOIN file_index fi ON fi.id = fs.file_index_id
         WHERE symbol_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2"
    )?;
    // ... map rows to SymbolMatch
}

/// Full-text search for file paths using FTS5 with BM25 ranking.
pub fn fts_search_files(&self, query: &str, project_path: &str, limit: usize) -> AppResult<Vec<FileIndexRow>> {
    let conn = self.get_connection()?;
    let mut stmt = conn.prepare(
        "SELECT ff.rowid, ff.file_path, ff.component, ff.language, rank
         FROM filepath_fts ff
         JOIN file_index fi ON fi.rowid = ff.rowid
         WHERE filepath_fts MATCH ?1
           AND fi.project_path = ?2
         ORDER BY rank
         LIMIT ?3"
    )?;
    // ... map rows to FileIndexRow
}
```

### Updated HybridSearchEngine Channels

```rust
// hybrid_search.rs — search_symbols (Phase 2)
fn search_symbols(&self, query: &str) -> AppResult<Vec<ChannelEntry>> {
    // Sanitize query for FTS5 (escape special chars, add implicit prefix)
    let fts_query = sanitize_fts_query(query);

    // Try FTS5 first
    match self.index_store.fts_search_symbols(&fts_query, self.config.channel_max_results) {
        Ok(results) if !results.is_empty() => {
            // Map to ChannelEntry...
            Ok(entries)
        }
        _ => {
            // Fallback to LIKE for very short queries or FTS syntax errors
            let pattern = format!("%{}%", query);
            let symbols = self.index_store.query_symbols(&pattern)?;
            // Map to ChannelEntry...
            Ok(entries)
        }
    }
}
```

### FTS Query Sanitization

FTS5 has special syntax (`AND`, `OR`, `NOT`, `*`, `"`, `NEAR`). User queries need sanitization:

```rust
/// Sanitize a user query for FTS5 MATCH.
///
/// - Wraps each token in double quotes to escape special chars
/// - Adds implicit wildcard for prefix matching
/// - Joins with implicit AND
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

### Testing Strategy

1. **Unit tests** in `index_store.rs`:
   - FTS insert + search roundtrip
   - BM25 ranking: more relevant results rank higher
   - Prefix matching: `"cont"` matches `"controller"`
   - Unicode: CJK characters, accented chars
   - Deletion: symbols removed after file delete

2. **Integration tests** in `hybrid_search.rs`:
   - Existing RRF tests still pass (FTS is a drop-in replacement for the channel)
   - New test: FTS results ranked by BM25, not alphabetical

3. **Migration test**: database with existing data (no FTS tables) → schema upgrade → FTS tables populated correctly.

---

## Phase 3: LSP Enhancement Layer

### Goal

Add optional LSP-based semantic enrichment on top of the Tree-sitter index. When language servers are detected on the user's system, enrich indexed symbols with type information, cross-references, and call graphs.

### Dependencies

```toml
# Cargo.toml — add under [dependencies]
lsp-types = "0.97"
```

`lsp-types` provides only type definitions (zero runtime cost). The LSP client transport is implemented manually using `tokio::process::Command` + stdin/stdout JSON-RPC, avoiding heavy framework dependencies.

### Architecture

```
IndexManager
    │
    └─ LspEnricher (new)
         │
         ├─ LspServerRegistry         — detects/manages language server processes
         │   ├─ RustAnalyzerAdapter
         │   ├─ PyrightAdapter
         │   ├─ GoplsAdapter
         │   ├─ VtslsAdapter
         │   └─ JdtlsAdapter
         │
         ├─ LspClient                  — JSON-RPC transport over stdin/stdout
         │   ├─ send_request()
         │   ├─ send_notification()
         │   └─ receive_response()
         │
         └─ EnrichmentPass             — orchestrates LSP queries after Tree-sitter index
              ├─ workspace/symbol      → validate/supplement symbol table
              ├─ textDocument/hover    → extract resolved types
              ├─ textDocument/references → build reference counts
              └─ textDocument/definition → build call graph
```

### Language Server Detection

Auto-detect from PATH on project open:

| Language | Binary | Fallback Locations |
|----------|--------|--------------------|
| Rust | `rust-analyzer` | `~/.cargo/bin/`, `~/.rustup/toolchains/*/bin/` |
| Python | `pyright-langserver`, `pylsp`, `basedpyright-langserver` | npm global, pip |
| Go | `gopls` | `~/go/bin/` |
| TypeScript/JS | `vtsls`, `typescript-language-server` | npm global |
| Java | `jdtls` | Homebrew prefix, manual config |

Detection is a `which`-style PATH check + known fallback directories. Results cached per session.

**Language servers are NOT bundled** — they are too large (40-200MB each), require their own runtimes (Node.js, JDK), and update frequently. Developers who work in a language almost always have the corresponding server installed.

### Schema Changes

Add to `database.rs`:

```sql
-- LSP-enriched symbol metadata
-- Populated asynchronously after Tree-sitter indexing completes.
ALTER TABLE file_symbols ADD COLUMN resolved_type TEXT;
ALTER TABLE file_symbols ADD COLUMN reference_count INTEGER DEFAULT 0;
ALTER TABLE file_symbols ADD COLUMN is_exported BOOLEAN DEFAULT 0;

-- Cross-reference table: caller → callee relationships
CREATE TABLE IF NOT EXISTS cross_references (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_path TEXT NOT NULL,
    -- Source (caller)
    source_file TEXT NOT NULL,
    source_line INTEGER NOT NULL,
    source_symbol TEXT,
    -- Target (definition)
    target_file TEXT NOT NULL,
    target_line INTEGER NOT NULL,
    target_symbol TEXT,
    -- Metadata
    reference_kind TEXT NOT NULL DEFAULT 'usage',  -- usage, call, import, type_ref
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(project_path, source_file, source_line, target_file, target_line)
);

CREATE INDEX IF NOT EXISTS idx_cross_refs_source
    ON cross_references(project_path, source_file);
CREATE INDEX IF NOT EXISTS idx_cross_refs_target
    ON cross_references(project_path, target_file, target_symbol);

-- LSP server detection cache
CREATE TABLE IF NOT EXISTS lsp_servers (
    language TEXT PRIMARY KEY,
    binary_path TEXT NOT NULL,
    server_name TEXT NOT NULL,
    version TEXT,
    detected_at TEXT DEFAULT CURRENT_TIMESTAMP
);
```

### New Modules

```
src-tauri/src/services/orchestrator/
    ├─ lsp_client.rs          — JSON-RPC transport + lifecycle
    ├─ lsp_registry.rs        — Server detection + adapter trait
    └─ lsp_enricher.rs        — Enrichment pass orchestration
```

#### Module: `lsp_client.rs`

Minimal JSON-RPC 2.0 client over stdio:

```rust
pub struct LspClient {
    child: tokio::process::Child,
    stdin: tokio::io::BufWriter<ChildStdin>,
    stdout: tokio::io::BufReader<ChildStdout>,
    next_id: AtomicI64,
    pending: DashMap<i64, oneshot::Sender<serde_json::Value>>,
    capabilities: Option<ServerCapabilities>,
}

impl LspClient {
    /// Spawn a language server process and complete the initialize handshake.
    pub async fn start(command: &str, args: &[&str], root_uri: &str) -> anyhow::Result<Self>;

    /// Send a request and wait for the response.
    pub async fn request<R: lsp_types::request::Request>(
        &self, params: R::Params
    ) -> anyhow::Result<R::Result>;

    /// Send a notification (no response expected).
    pub async fn notify<N: lsp_types::notification::Notification>(
        &self, params: N::Params
    ) -> anyhow::Result<()>;

    /// Graceful shutdown: shutdown request → exit notification → wait.
    pub async fn shutdown(self) -> anyhow::Result<()>;
}
```

#### Module: `lsp_registry.rs`

```rust
/// Trait for language-specific server adapters.
pub trait LspServerAdapter: Send + Sync {
    fn language(&self) -> &str;
    fn server_name(&self) -> &str;
    /// Detect if the server binary exists on the system.
    fn detect(&self) -> Option<PathBuf>;
    /// Command + args to spawn the server.
    fn command(&self) -> (&str, Vec<String>);
    /// Initialization options specific to this server.
    fn init_options(&self) -> Option<serde_json::Value>;
}

pub struct LspServerRegistry {
    adapters: Vec<Box<dyn LspServerAdapter>>,
    detected: RwLock<HashMap<String, PathBuf>>,  // language → binary_path
}

impl LspServerRegistry {
    pub fn new() -> Self;
    /// Run detection for all adapters. Returns detected language → server pairs.
    pub async fn detect_all(&self) -> HashMap<String, String>;
    /// Get adapter for a language (if server is detected).
    pub fn get_adapter(&self, language: &str) -> Option<&dyn LspServerAdapter>;
}
```

#### Module: `lsp_enricher.rs`

```rust
pub struct LspEnricher {
    registry: Arc<LspServerRegistry>,
    index_store: Arc<IndexStore>,
    /// Active LSP client connections, one per language.
    clients: RwLock<HashMap<String, Arc<LspClient>>>,
}

impl LspEnricher {
    pub fn new(registry: Arc<LspServerRegistry>, index_store: Arc<IndexStore>) -> Self;

    /// Run the enrichment pass for a project.
    ///
    /// 1. Detect available servers
    /// 2. Start LSP clients for detected languages
    /// 3. For each indexed file in those languages:
    ///    a. textDocument/didOpen
    ///    b. For each symbol: textDocument/hover → resolved_type
    ///    c. For each symbol: textDocument/references → reference_count
    ///    d. textDocument/didClose
    /// 4. Store enriched data in SQLite
    /// 5. Shutdown LSP clients
    pub async fn enrich_project(&self, project_path: &str) -> anyhow::Result<EnrichmentReport>;

    /// Shutdown all active LSP clients.
    pub async fn shutdown_all(&self);
}

pub struct EnrichmentReport {
    pub languages_enriched: Vec<String>,
    pub symbols_enriched: usize,
    pub references_found: usize,
    pub duration_ms: u64,
}
```

### Enrichment Flow (Detail)

```
LspEnricher::enrich_project(project_path)
    │
    ├─ 1. registry.detect_all()
    │      → { "rust": "/usr/bin/rust-analyzer", "go": "/usr/local/bin/gopls" }
    │
    ├─ 2. For each detected language:
    │      ├─ LspClient::start(command, args, root_uri)
    │      ├─ Send: initialize(rootUri, capabilities)
    │      ├─ Send: initialized()
    │      └─ Store in clients map
    │
    ├─ 3. Query symbols from index_store grouped by (language, file_path)
    │
    ├─ 4. For each file:
    │      ├─ textDocument/didOpen (full file content)
    │      │
    │      ├─ For each symbol in file:
    │      │   ├─ textDocument/hover(position) → extract type from MarkupContent
    │      │   │   → UPDATE file_symbols SET resolved_type = ? WHERE rowid = ?
    │      │   │
    │      │   ├─ textDocument/references(position, includeDeclaration=false)
    │      │   │   → count = len(locations)
    │      │   │   → UPDATE file_symbols SET reference_count = ? WHERE rowid = ?
    │      │   │   → INSERT INTO cross_references for each location
    │      │   │
    │      │   └─ textDocument/definition(position)
    │      │       → INSERT INTO cross_references (kind='definition')
    │      │
    │      └─ textDocument/didClose
    │
    ├─ 5. Rate limiting: max 10 hover/references requests per second per server
    │      to avoid overloading language servers on large projects.
    │
    └─ 6. shutdown_all()
```

### Integration with IndexManager

```rust
// index_manager.rs — add field
pub struct IndexManager {
    // ... existing fields ...
    lsp_enricher: Option<Arc<LspEnricher>>,
}

// In start_indexing, after BackgroundIndexer completes:
if let Some(ref enricher) = self.lsp_enricher {
    let pp = project_path.to_string();
    let enricher = Arc::clone(enricher);
    tokio::spawn(async move {
        match enricher.enrich_project(&pp).await {
            Ok(report) => info!(
                project = %pp,
                languages = ?report.languages_enriched,
                symbols = report.symbols_enriched,
                "LSP enrichment complete"
            ),
            Err(e) => warn!(
                project = %pp,
                error = %e,
                "LSP enrichment failed (Tree-sitter index still valid)"
            ),
        }
    });
}
```

### Frontend Changes

#### Settings UI: Code Intelligence Section

Add a new section in `SettingsDialog.tsx` after `EmbeddingSection`:

```
Settings > Code Intelligence
┌─────────────────────────────────────────────┐
│  Language Servers (auto-detected)            │
│                                              │
│  ● Rust     rust-analyzer  ✅ detected       │
│  ● Python   pyright        ❌ not found      │
│             ↳ Install: npm i -g pyright      │
│  ● Go       gopls          ✅ detected       │
│  ● TS/JS    vtsls          ✅ detected       │
│  ● Java     jdtls          ❌ not found      │
│                                              │
│  [Detect servers]  [Custom server path...]   │
│                                              │
│  ☑ Auto-enrich on index completion           │
│  Enrichment status: 2,451 symbols enriched   │
└─────────────────────────────────────────────┘
```

New Tauri commands:

| Command | Purpose |
|---------|---------|
| `detect_lsp_servers` | Run detection and return results |
| `get_lsp_status` | Get per-language server status |
| `trigger_lsp_enrichment` | Manually start enrichment pass |
| `get_enrichment_report` | Get results of last enrichment |

#### IndexStatus Enhancement

Update `IndexStatus.tsx` to show enrichment badge:

```
● Indexed  1,234 files  5,678 symbols  Semantic ✓  LSP ✓
```

### Files to Create

| File | Purpose |
|------|---------|
| `orchestrator/lsp_client.rs` | JSON-RPC transport over stdio |
| `orchestrator/lsp_registry.rs` | Server detection + adapters |
| `orchestrator/lsp_enricher.rs` | Enrichment pass orchestration |
| `commands/lsp.rs` | Tauri commands for LSP management |
| `src/components/Settings/LspSection.tsx` | Settings UI component |
| `src/store/lsp.ts` | Zustand store for LSP state |
| `src/types/lsp.ts` | TypeScript type definitions |
| `src/lib/lspApi.ts` | IPC wrappers |

### Files to Modify

| File | Change |
|------|--------|
| `storage/database.rs` | Add `cross_references` table, LSP columns migration, `lsp_servers` table |
| `orchestrator/mod.rs` | Add `pub mod lsp_client; pub mod lsp_registry; pub mod lsp_enricher;` |
| `orchestrator/index_manager.rs` | Add `lsp_enricher` field, trigger enrichment after indexing |
| `orchestrator/index_store.rs` | Add methods: `update_symbol_type()`, `update_reference_count()`, `insert_cross_reference()`, `get_symbols_for_enrichment()` |
| `commands/mod.rs` | Add `pub mod lsp;` |
| `main.rs` | Register LSP commands in `invoke_handler` |
| `src/components/Settings/SettingsDialog.tsx` | Add LspSection tab |
| `src/components/Settings/index.tsx` | Export LspSection |
| `src/components/shared/IndexStatus.tsx` | Show LSP enrichment badge |
| `src/i18n/locales/*/settings.json` | LSP-related i18n strings |
| `src/i18n/locales/*/common.json` | Enrichment status strings |

### Testing Strategy

1. **LspClient unit tests**:
   - JSON-RPC message framing (Content-Length header)
   - Request/response correlation by ID
   - Timeout on unresponsive server

2. **LspRegistry tests**:
   - Detection with mocked PATH
   - Fallback paths checked correctly
   - Caching works (second detect_all returns cached)

3. **LspEnricher integration tests** (require language servers installed — run in CI only):
   - Small Rust project → rust-analyzer enrichment → types and references populated
   - Small TS project → vtsls enrichment → types populated
   - Missing server → graceful skip, no error

4. **Database migration test**:
   - Existing database without LSP columns → upgrade → columns added with defaults

---

## Execution Order and Dependencies

```
Phase 1 (HNSW)  ────────────────┐
                                 ├──→ Can be done in parallel
Phase 2 (FTS5)  ────────────────┘
                                 │
                                 ▼
                           Phase 3 (LSP)
                     (depends on stable index layer)
```

### Recommended Story Breakdown

#### Phase 1: HNSW (5 stories)

| # | Story | Estimate |
|---|-------|----------|
| 1-1 | Add `hnsw_rs` dep + `hnsw_index.rs` module with create/insert/search/save/load | M |
| 1-2 | Wire HnswIndex into IndexManager lifecycle (create, load, rebuild) | M |
| 1-3 | Wire HnswIndex into BackgroundIndexer (insert during embedding pass, save at end) | S |
| 1-4 | Wire HnswIndex into hybrid_search.rs Semantic channel + executor.rs | M |
| 1-5 | Stale ID tracking + periodic rebuild + tests | S |

#### Phase 2: FTS5 (4 stories)

| # | Story | Estimate |
|---|-------|----------|
| 2-1 | Schema: create FTS5 virtual tables + migration for existing databases | S |
| 2-2 | Sync: update `upsert_file_index` and `delete_project_index` to maintain FTS tables | S |
| 2-3 | Query: `fts_search_symbols` + `fts_search_files` with BM25 ranking | S |
| 2-4 | Wire FTS into hybrid_search.rs channels + query sanitization + tests | M |

#### Phase 3: LSP (7 stories)

| # | Story | Estimate |
|---|-------|----------|
| 3-1 | `lsp_client.rs`: JSON-RPC transport + initialize/shutdown lifecycle | L |
| 3-2 | `lsp_registry.rs`: Server detection for 6 languages + adapter trait | M |
| 3-3 | Schema: cross_references table, LSP columns, lsp_servers cache | S |
| 3-4 | `lsp_enricher.rs`: Enrichment pass orchestration (hover + references) | L |
| 3-5 | Wire into IndexManager: auto-enrich after index, Tauri commands | M |
| 3-6 | Frontend: LspSection settings UI + Zustand store + IPC wrappers | M |
| 3-7 | Frontend: IndexStatus LSP badge + i18n + integration tests | S |

> Size estimates: S = small (1-2 files, <200 LOC), M = medium (2-4 files, 200-500 LOC), L = large (new subsystem, 500+ LOC)

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| hnsw_rs memory usage with large projects | mmap support via `ReloadOptions::set_mmap_threshold()` for projects >50K vectors |
| FTS5 tokenizer doesn't handle camelCase splitting | Add custom tokenizer or pre-process: split `getUserName` → `get user name` before indexing |
| LSP server crashes during enrichment | Catch panics per-file, skip failed files, report partial results |
| LSP enrichment too slow on large projects | Rate limit (10 req/s), enrichment is always async/background, show progress in UI |
| HNSW index file corruption | Always rebuildable from SQLite; validate on load, rebuild on error |
| hnsw_rs does not support deletion | Soft-delete with stale ID set + periodic rebuild (standard HNSW pattern) |

## Backward Compatibility

- **Phase 1**: SQLite schema unchanged. HNSW files are additive. Old databases work as-is; HNSW index is rebuilt on first use.
- **Phase 2**: FTS5 tables are additive. Existing `query_symbols` / `query_files_by_path` methods kept as fallback. Migration populates FTS from existing data.
- **Phase 3**: New columns have defaults (`resolved_type=NULL`, `reference_count=0`). New tables are additive. App functions normally without any LSP servers installed.
