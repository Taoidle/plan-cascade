# Plan Cascade Desktop 内存与技能系统设计文档

**版本**: 1.0.0
**日期**: 2026-03-08
**范围**: Memory system architecture, skill system, V2 alignment

---

## 目录

1. [内存与技能架构 V2 概述](#1-内存与技能架构-v2-概述)
2. [ADR: 内存作用域模型 V2](#2-adr-内存作用域模型-v2)
3. [V2 对齐说明](#3-v2-对齐说明)
4. [关键术语定义](#4-关键术语定义)

---

## 1. 内存与技能架构 V2 概述

> 来源: `memory-skill-iteration-plan.md` 前半部分

### 1.1 V2 对齐说明

- 统一内存读取路径是 `query_memory_entries_v2` / `list_memory_entries_v2` / `memory_stats_v2`
- 规范存储是 `memory_entries_v2`，带有明确的 `scope/status/risk_tier/conflict_flag`
- 审查治理使用 `list_pending_memory_candidates_v2` 和 `review_memory_candidates_v2`
- 工作流阶段真相来源是 `Workflow Kernel modeSnapshots`; 前端阶段回填已移除
- ContextOps 添加内存特定 SLI 字段 (`memory_query_p95_ms`, `empty_hit_rate`, `candidate_count`, `review_backlog`, `approve_rate`, `reject_rate`)

### 1.2 架构愿景

#### 内存层次结构

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

### 1.3 数据流

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

### 1.4 关键术语

| 术语 | 定义 |
|------|------|
| **Semantic Memory** | 持久的事实关于用户和项目（偏好、约定、模式） |
| **Episodic Memory** | 特定过去交互的记录（发生了什么、什么成功/失败） |
| **Procedural Memory / Skill** | 可复用的任务执行模板（常见操作的逐步说明） |
| **Memory Extraction** | LLM 驱动的从会话交互中提炼结构化记忆条目的过程 |
| **Skill Injection** | 在 LLM 调用前将相关技能匹配并前置到用户提示 |
| **Memory Decay** | 对过时、未使用的记忆重要性自动降低 |

### 1.5 P0: 项目内存系统

#### 1.5.1 数据库架构

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

#### 1.5.2 搜索与排序算法

相关性评分公式:

```
final_score = w1 * embedding_similarity
            + w2 * keyword_overlap
            + w3 * importance
            + w4 * recency_score

其中:
  w1 = 0.40  (语义相关性)
  w2 = 0.25  (关键词匹配)
  w3 = 0.20  (重要性权重)
  w4 = 0.15  (近期性奖励)
```

### 1.6 P0: 技能系统

#### 1.6.1 支持的技能格式

| 格式 | 来源 | 特点 |
|------|------|------|
| Plan Cascade SKILL.md | builtin-skills/ | 完整功能 |
| adk-rust .skills/ | 外部框架 | 轻量级 |
| Convention Files | CLAUDE.md, AGENTS.md | 无 frontmatter |

#### 1.6.2 四源技能架构

```
┌─────────────────────────────────────────────────────────────────┐
│                    Skill Source Hierarchy                         │
│                                                                 │
│  Priority 1-50:   BUILTIN                                       │
│  │  Bundled with plan-cascade (python, go, java, typescript)   │
│  │                                                              │
│  Priority 51-100: EXTERNAL / SUBMODULE                          │
│  │  Community skills (Vercel React, Vue, Rust)                  │
│  │                                                              │
│  Priority 101-200: USER                                          │
│  │  User-defined skills from local paths or URLs                │
│  │                                                              │
│  Priority 201+:  PROJECT-LOCAL                                  │
│  │  Project .skills/ directory + convention files               │
│  │  Always highest priority (project-specific overrides all)    │
└─────────────────────────────────────────────────────────────────┘
```

#### 1.6.3 自动检测逻辑

技能根据项目文件分析自动激活:

| 技能 | 检测文件 | 检测模式 | 优先级 |
|------|----------|----------|--------|
| react-best-practices | package.json | "react", "next", "@react" | 100 |
| vue-best-practices | package.json | "vue", "nuxt", "@vue" | 100 |
| rust-coding-guidelines | Cargo.toml | "[package]", "[dependencies]" | 100 |
| typescript-best-practices | tsconfig.json, package.json | "typescript", "@types/" | 36 |

### 1.7 P1: Agentic 生命周期钩子

#### 1.7.1 钩子点

```
┌─────────────────────────────────────────────────────────────┐
│                      Agentic Loop                            │
│                                                             │
│  ①  on_session_start(ctx)                                   │
│      ↓                                                      │
│  ②  on_user_message(ctx, message) → Option<modified_msg>    │
│      ↓                                                      │
│  ┌─ Loop ──────────────────────────────────────────────┐   │
│  │  ③  on_before_llm(ctx, request) → Option<mod_req>   │   │
│  │      ↓                                               │   │
│  │      LLM Call                                        │   │
│  │      ↓                                               │   │
│  │  ④  on_after_llm(ctx, response) → Option<mod_resp>  │   │
│  │      ↓                                               │   │
│  │  ⑤  on_before_tool(ctx, name, args) → Option<skip>  │   │
│  │      ↓                                               │   │
│  │      Tool Execution                                  │   │
│  │      ↓                                               │   │
│  │  ⑥  on_after_tool(ctx, name, result)                │   │
│  │      ↓                                               │   │
│  │      Continue or break                               │   │
│  └──────────────────────────────────────────────────────┘   │
│      ↓                                                      │
│  ⑦  on_session_end(ctx, summary)                            │
│      ↓                                                      │
│  ⑧  on_compaction(ctx, compacted_messages)                  │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. ADR: 内存作用域模型 V2

> 来源: `ADR-memory-scope-model-v2.md`

### 2.1 上下文

V1 内存作用域语义通过 `project_path` 哨兵 (`__global__`, `__session__:*`) 间接编码，导致:
- Chat/Plan/Task 之间的查询行为模糊
- 多个调用站点的重复扇出逻辑
- 存储形状和检索逻辑之间的脆弱兼容性耦合

### 2.2 决策

采用 `memory_entries_v2` 作为规范内存存储，带有明确的作用域和治理字段:
- `scope` 在 `global|project|session` 中
- `project_path` 可为空
- `session_id` 可为空
- `status` 在 `active|pending_review|rejected|archived` 中
- `risk_tier` 在 `low|medium|high` 中
- `conflict_flag` 布尔型整数

使用语义唯一性强制:
- `UNIQUE(scope, IFNULL(project_path,''), IFNULL(session_id,''), content_hash)`

使用 `memory_fts_v2` 进行词汇候选检索，使用 `query_memory_entries_v2` 作为唯一编排的检索入口

### 2.3 后果

- 作用域语义是模式和 API 中的第一公民
- 通过统一后端编排，跨模式检索行为一致
- 治理流程 (`pending_review`, 审查审计) 是模型级，而非临时前端状态
- 遗留 `project_memories` 在 V2 期间通过单向同步触发器保持兼容性支持

---

## 3. V2 对齐说明

### 3.1 统一内存读取路径

V2 对齐的核心是统一所有内存操作的入口:

| 操作 | V2 命令 |
|------|---------|
| 查询 | `query_memory_entries_v2` |
| 列表 | `list_memory_entries_v2` |
| 统计 | `memory_stats_v2` |
| 待审查列表 | `list_pending_memory_candidates_v2` |
| 审查 | `review_memory_candidates_v2` |

### 3.2 规范存储

`memory_entries_v2` 表结构:

```sql
CREATE TABLE IF NOT EXISTS memory_entries_v2 (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL CHECK(scope IN ('global', 'project', 'session')),
    project_path TEXT,
    session_id TEXT,
    content TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('active', 'pending_review', 'rejected', 'archived')),
    risk_tier TEXT NOT NULL CHECK(risk_tier IN ('low', 'medium', 'high')),
    conflict_flag INTEGER NOT NULL DEFAULT 0,
    importance REAL NOT NULL DEFAULT 0.5,
    access_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_accessed_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(scope, IFNULL(project_path,''), IFNULL(session_id,''), content_hash)
);
```

### 3.3 治理流程

```
User Action
    │
    ▼
Memory Extraction (LLM-driven)
    │
    ▼
Entry Created with status='pending_review'
    │
    ▼
User Reviews via UI
    │
    ▼
Review Decision → status='active' | 'rejected'
    │
    ▼
Query reads status='active' only
```

### 3.4 ContextOps SLI

添加内存特定的服务水平指标:

| 指标 | 说明 |
|------|------|
| `memory_query_p95_ms` | 内存查询 P95 延迟 |
| `empty_hit_rate` | 空结果命中率 |
| `candidate_count` | 待审查候选项数量 |
| `review_backlog` | 审查积压数量 |
| `approve_rate` | 批准率 |
| `reject_rate` | 拒绝率 |

---

## 4. 关键术语定义

| 术语 | 定义 |
|------|------|
| **Memory Entry** | 内存条目，存储在 `memory_entries_v2` 中的基本数据单元 |
| **Scope** | 作用域，`global`（全局）、`project`（项目级）、`session`（会话级） |
| **Status** | 状态，`active`（活跃）、`pending_review`（待审查）、`rejected`（已拒绝）、`archived`（已归档） |
| **Risk Tier** | 风险等级，`low`（低）、`medium`（中）、`high`（高） |
| **Skill** | 技能，可复用的任务执行模板 |
| **Skill Injection** | 技能注入，将匹配的相关技能前置到用户内容 |
| **Memory Extraction** | 记忆提取，LLM 驱动的从会话中提取结构化记忆 |
| **Episodic Record** | 情景记录，特定过去交互的记录 |
| **Handoff Context** | 交接上下文，模式切换时传递的上下文数据 |
| **Workflow Kernel** | 工作流内核，管理 Chat/Plan/Task 生命周期的核心组件 |
| **modeSnapshots** | 模式快照，内核维护的各模式状态快照 |
| **RRF (Reciprocal Rank Fusion)** | 互惠排名融合，多通道搜索结果融合算法 |

---

## 相关文档

- [整体架构设计](./architecture-design.md)
- [内核系统设计](./kernel-design.md)
- [代码库索引设计](./codebase-index-design.md)
