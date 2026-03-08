# Plan Cascade Desktop 代码库索引设计文档

**版本**: 1.0.0
**日期**: 2026-03-08
**范围**: Codebase indexing, search pipeline, hybrid search architecture

---

## 目录

1. [代码库索引当前状态](#1-代码库索引当前状态)
2. [架构图](#2-架构图)
3. [Phase 1: HNSW 向量索引设计](#3-phase-1-hnsw-向量索引设计)
4. [Phase 2: FTS5 全文搜索设计](#4-phase-2-fts5-全文搜索设计)
5. [Phase 3: LSP 增强层设计](#5-phase-3-lsp-增强层设计)
6. [搜索管道说明](#6-搜索管道说明)

---

## 1. 代码库索引当前状态

> 来源: `codebase-index-iteration-plan.md`

### 1.1 已实现功能

代码库索引功能目前包括:

1. **HNSW ANN 语义搜索** - 使用 SQLite 回退
2. **FTS5 BM25 搜索** - 符号和文件路径通道，支持 LIKE 回退
3. **LSP  enrichment 元数据** - 解析类型 + 引用计数用于搜索输出

剩余差距是产品化和 UX 一致性（多根、上下文交接和操作级可观测性），而非核心搜索原语

### 1.2 核心组件

| 组件 | 位置 | 角色 |
|------|------|------|
| `EmbeddingService` | `services/orchestrator/embedding_service.rs` | TF-IDF 向量化，词汇搜索 |
| `IndexStore` | `services/orchestrator/index_store.rs` | SQLite 表管理 |
| `HybridSearchEngine` | `services/orchestrator/hybrid_search.rs` | 多通道搜索融合 |
| `BackgroundIndexer` | `services/orchestrator/background_indexer.rs` | 后台索引构建 |

---

## 2. 架构图

### 2.1 三阶段后整体架构

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

## 3. Phase 1: HNSW 向量索引设计

### 3.1 目标

将 O(n) 暴力余弦相似度扫描替换为 O(log n) 近似最近邻搜索，使用 `hnsw_rs`。目标: **<1ms** 每查询在 100K 向量（当前约 50ms）

### 3.2 存储模型: SQLite + HNSW Sidecar

```
~/.local/share/plan-cascade/
  └─ app.db                           ← SQLite (file_embeddings table unchanged)
  └─ hnsw_indexes/
       └─ <project-hash>/
            ├─ embeddings.hnsw.graph  ← HNSW topology
            └─ embeddings.hnsw.data   ← vector data
```

- **SQLite 仍是所有嵌入数据的真相来源** (CRUD, backup, migration)
- **HNSW 文件是派生缓存** — 可随时删除并从 SQLite 重建
- Project hash = SHA-256 of project_path, truncated to 16 hex chars

### 3.3 HNSW 参数

| 参数 | 值 | 理由 |
|------|-----|------|
| `max_nb_connection` | 24 | <100K 向量时良好的召回/速度平衡 |
| `max_layer` | 16 | 标准默认值 |
| `ef_construction` | 200 | 更高 = 更好索引质量，一次性成本 |
| `ef_search` | 64 | 查询时可调; 64 给出 >95% 召回率 |
| Distance | `DistCosine` | 匹配当前 `cosine_similarity` 函数 |

### 3.4 生命周期流程

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

### 3.5 删除注意事项

`hnsw_rs` **不支持**点删除。对于增量文件更新:

1. **软删除**: 维护一个 `HashSet<usize>` 标记过时 IDs; 从搜索结果中过滤
2. **定期重建**: 当过时 IDs 超过总数 10% 时，从 SQLite 重建
3. **重新索引**: 始终全新构建

---

## 4. Phase 2: FTS5 全文搜索设计

### 4.1 目标

用 SQLite FTS5 排名全文搜索替换 `WHERE name LIKE '%query%'`。获得 BM25 相关性评分、布尔查询、前缀匹配和短语搜索

### 4.2 Schema 变更

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

### 4.3 关键分词器选择

- `unicode61`: 处理非 ASCII (CJK, 重音字符)
- `tokenchars _`: 将下划线视为 token 一部分（匹配 Rust/Python 命名）
- `tokenchars _/.`: 对于文件路径，也将 `/` 和 `.` 视为 token 字符

### 4.4 FTS 查询清理

FTS5 有特殊语法 (`AND`, `OR`, `NOT`, `*`, `"`, `NEAR`)。用户查询需要清理:

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

## 5. Phase 3: LSP 增强层设计

### 5.1 目标

在 Tree-sitter 索引之上添加可选的基于 LSP 的语义 enrichment。当在用户系统上检测到语言服务器时，用类型信息、交叉引用和调用图丰富索引符号

### 5.2 语言服务器检测

从 PATH 自动检测项目打开:

| 语言 | 二进制 | 回退位置 |
|------|--------|----------|
| Rust | `rust-analyzer` | `~/.cargo/bin/`, `~/.rustup/toolchains/*/bin/` |
| Python | `pyright-langserver`, `pylsp`, `basedpyright-langserver` | npm 全局, pip |
| Go | `gopls` | `~/go/bin/` |
| TypeScript/JS | `vtsls`, `typescript-language-server` | npm 全局 |
| Java | `jdtls` | Homebrew 前缀, 手动配置 |

### 5.3 架构

```
IndexManager
    │
    └─ LspEnricher (new)
         │
         ├─ LspServerRegistry         — 检测/管理语言服务器进程
         │   ├─ RustAnalyzerAdapter
         │   ├─ PyrightAdapter
         │   ├─ GoplsAdapter
         │   ├─ VtslsAdapter
         │   └─ JdtlsAdapter
         │
         ├─ LspClient                  — stdin/stdout 上的 JSON-RPC 传输
         │   ├─ send_request()
         │   ├─ send_notification()
         │   └─ receive_response()
         │
         └─ EnrichmentPass             — 在 Tree-sitter 索引后编排 LSP 查询
              ├─ workspace/symbol      → 验证/补充符号表
              ├─ textDocument/hover    → 提取解析类型
              ├─ textDocument/references → 构建引用计数
              └─ textDocument/definition → 构建调用图
```

### 5.4 Schema 变更

```sql
-- LSP-enriched symbol metadata
ALTER TABLE file_symbols ADD COLUMN resolved_type TEXT;
ALTER TABLE file_symbols ADD COLUMN reference_count INTEGER DEFAULT 0;
ALTER TABLE file_symbols ADD COLUMN is_exported BOOLEAN DEFAULT 0;

-- Cross-reference table
CREATE TABLE IF NOT EXISTS cross_references (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_path TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_line INTEGER NOT NULL,
    source_symbol TEXT,
    target_file TEXT NOT NULL,
    target_line INTEGER NOT NULL,
    target_symbol TEXT,
    reference_kind TEXT NOT NULL DEFAULT 'usage',
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(project_path, source_file, source_line, target_file, target_line)
);

-- LSP server detection cache
CREATE TABLE IF NOT EXISTS lsp_servers (
    language TEXT PRIMARY KEY,
    binary_path TEXT NOT NULL,
    server_name TEXT NOT NULL,
    version TEXT,
    detected_at TEXT DEFAULT CURRENT_TIMESTAMP
);
```

### 5.5 Enrichment 流程

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
    │      │   ├─ textDocument/hover(position) → extract type
    │      │   ├─ textDocument/references(position) → reference count
    │      │   └─ textDocument/definition(position) → call graph
    │      │
    │      └─ textDocument/didClose
    │
    ├─ 5. Rate limiting: max 10 req/s per server
    │
    └─ 6. shutdown_all()
```

---

## 6. 搜索管道说明

### 6.1 混合搜索流程

```
User Query
    │
    ├─ Channel 1: FTS5 Symbol Search
    │   ├─ Query: symbol_fts with BM25
    │   ├─ Fallback: LIKE if FTS fails
    │   └─ Output: Vec<SymbolMatch>
    │
    ├─ Channel 2: FTS5 FilePath Search
    │   ├─ Query: filepath_fts with BM25
    │   ├─ Fallback: LIKE if FTS fails
    │   └─ Output: Vec<FileMatch>
    │
    └─ Channel 3: HNSW Semantic Search
        ├─ Query: HNSW index with ef_search=64
        ├─ Fallback: Brute-force if no index
        └─ Output: Vec<SemanticMatch>
            │
            └─ RRF Fusion (Reciprocal Rank Fusion)
                 │
                 └─ Final Ranked Results
```

### 6.2 RRF 融合算法

```rust
/// Reciprocal Rank Fusion
/// 公式: score(d) = Σ 1 / (k + rank_i(d))
/// 其中 k = 60 (常量), rank_i(d) = 通道 i 中文档 d 的排名
fn rrf_fusion(results: Vec<Vec<(doc_id, score)>>) -> Vec<(doc_id, fused_score)> {
    const K: f32 = 60.0;
    let mut fused: HashMap<doc_id, f32> = HashMap::new();

    for channel_results in results {
        for (rank, (doc_id, _)) in channel_results.iter().enumerate() {
            let entry = fused.entry(*doc_id).or_insert(0.0);
            *entry += 1.0 / (K + (rank + 1) as f32);
        }
    }

    let mut sorted: Vec<_> = fused.into_iter().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    sorted
}
```

### 6.3 搜索类型

| 搜索类型 | 通道 | 评分方法 |
|----------|------|----------|
| 符号搜索 | FTS5 | BM25 |
| 路径搜索 | FTS5 | BM25 |
| 语义搜索 | HNSW | 余弦相似度 |
| 混合搜索 | All | RRF 融合 |

---

## 相关文档

- [整体架构设计](./architecture-design.md)
- [内核系统设计](./kernel-design.md)
- [内存与技能设计](./memory-skill-design.md)
