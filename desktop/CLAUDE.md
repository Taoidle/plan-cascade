# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Plan Cascade Desktop is a cross-platform AI programming orchestration app built with **Tauri 2** (React 18 frontend + pure Rust backend, no Python sidecar). It manages LLM-powered task execution with features like agent management, analytics, quality gates, git worktrees, timeline checkpoints, MCP server integration, knowledge base (RAG), plugins, guardrails, webhooks, remote control, and A2A protocol.

## Build & Development Commands

**Requires**: Node.js with pnpm, Rust toolchain

```bash
# Frontend
pnpm install                  # Install dependencies
pnpm dev                      # Vite dev server only (port 8173)
pnpm lint                     # ESLint (strict, zero warnings allowed)
pnpm typecheck                # TypeScript strict mode check
pnpm test                     # Vitest (jsdom environment)
pnpm test:watch               # Vitest watch mode
pnpm test:coverage            # Coverage report (60% threshold)

# Run a single frontend test file
pnpm test -- src/store/execution.test.ts

# Full app
pnpm tauri:dev                # Tauri dev with hot reload + devtools
pnpm tauri:build              # Production build for current platform
pnpm tauri:build:dev          # Debug build

# Backend (from src-tauri/)
cargo test                    # Rust unit + integration tests
cargo clippy                  # Rust linting
cargo check                   # Type checking
cargo build --features browser  # Build with optional browser automation (chromiumoxide)
```

Platform-specific builds: `pnpm tauri:build:windows`, `pnpm tauri:build:macos`, `pnpm tauri:build:linux`.

## Architecture

### Three-Layer Structure

```
Frontend (React/TypeScript)  ──Tauri IPC──>  Commands (Rust)  ──>  Services (Rust)  ──>  Storage (SQLite/Keyring/Config)
     src/                                  src-tauri/src/commands/  src-tauri/src/services/  src-tauri/src/storage/
```

**Frontend** (`src/`): React components organized by feature domain (`components/{Agents,Analytics,ClaudeCodeMode,ExpertMode,SimpleMode,Projects,Timeline,Settings,MCP,KnowledgeBase,Plugins,...}/`). State managed via Zustand stores (`store/`). IPC wrappers in `lib/tauri.ts`. Path alias: `@/*` maps to `src/*`.

**Backend** (`src-tauri/src/`): ~300 Tauri commands across 30+ domain modules in `commands/`. Business logic in `services/` (often with subdirectories for complex domains like `analytics/`, `claude_code/`, `orchestrator/`, `git/`, `streaming/`, `knowledge/`, `plugins/`, `guardrail/`, `webhook/`, `remote/`). Data structures in `models/`. Persistence via `storage/` (SQLite with r2d2 connection pooling, AES-256-GCM encrypted keyring for API keys, JSON config files).

### Cargo Workspace

Five crates with dependency chain: `core` <-- `llm` <-- `tools`, `core` <-- `quality-gates`.

| Crate | Path | Purpose |
|-------|------|---------|
| `plan-cascade-desktop` | `src-tauri/` | Main Tauri app, commands, services, storage |
| `plan-cascade-core` | `crates/core/` | Core traits, error types, context hierarchy, streaming, event actions |
| `plan-cascade-llm` | `crates/llm/` | LLM provider abstraction + streaming adapters |
| `plan-cascade-tools` | `crates/tools/` | Tool executor types, trait definitions, prompt fallback utilities |
| `plan-cascade-quality-gates` | `crates/quality-gates/` | Quality gate pipeline, validators, project type detection |

### App State Architecture

Eighteen Tauri-managed state objects initialized in `main.rs`:
- `AppState` — core services (database, keyring, config, memory store); lazy-initialized via `init_app` command
- Domain-specific: `ClaudeCodeState`, `AnalyticsState`, `QualityGatesState`, `WorktreeState`, `StandaloneState`, `SpecInterviewState`, `McpRuntimeState`, `LspState`, `PluginState`, `GuardrailState`, `WebhookState`, `RemoteState`, `TaskModeState`, `ExecutionRegistry`, `KnowledgeState`, `ArtifactState`, `GitState`

State is accessed in commands via `tauri::State<'_, T>`. `AppState` uses `Arc<RwLock<Option<T>>>` for lazy initialization — services start as `None` and are populated by `init_app`.

### UI Modes

The frontend renders mode-specific panels based on the active mode in the `mode` Zustand store:
- `simple` — Main chat mode with Git panel sidebar
- `expert` — PRD generation, dependency graph, strategy selection
- `claude-code` — Claude Code CLI GUI mode
- `projects` — Project/session browser
- `analytics` — Usage and cost dashboard
- `knowledge` — RAG knowledge base
- `artifacts` — Artifact version browser

### IPC Pattern

Commands return `Result<CommandResponse<T>, String>`. Frontend calls via `invoke<CommandResponse<T>>('command_name', { params })`. Real-time updates use Tauri event system (`listen`/`emit`).

### Key Backend Patterns

- **Command layer**: `#[tauri::command]` async functions that extract state, delegate to services, wrap results in `CommandResponse`
- **Service layer**: constructed with database pool, implements async business logic, returns `AppResult<T>`
- **Error handling**: `AppError` enum with typed variants (Database, Keyring, Config, etc.), `AppResult<T>` alias
- **Database access**: `state.with_database(|db| ...)` callback pattern for pool access

### Context Management Architecture

The orchestrator uses a three-layer context architecture to optimize token usage and prompt caching:

- **Layer 1 (Stable)**: System prompt + project index summary + tool definitions — maximum cache hit rate
- **Layer 2 (Semi-stable)**: Session memory with `[SESSION_MEMORY_V1]` marker — updated during compaction and before each LLM call
- **Layer 3 (Volatile)**: Conversation messages — grows and gets trimmed during compaction

Key components:

- **File Read Dedup Cache** (`executor.rs`): `Mutex<HashMap<(PathBuf, offset, limit), ReadCacheEntry>>` prevents redundant file reads. Second read of unchanged file returns short dedup message instead of full content.
- **Tool Result Truncation** (`service_helpers.rs`): Bounds tool output injected into LLM context (Read: 200 lines, Grep: 100, LS: 150, Bash: 150). Frontend ToolResult events retain full content.
- **SessionMemoryManager** (`service_helpers.rs`): Maintains session memory at fixed index 1 in messages vec. Accumulates file reads and findings, updates before each LLM call. Both compaction strategies preserve Layer 1+2.
- **Symbol Extraction** (`tree_sitter_parser.rs`, `analysis_index.rs`): Tree-sitter grammar-based extraction of functions, classes, structs, enums for Python, Rust, TypeScript, JavaScript, Go, Java. Max 30 symbols per file, skips files >500KB.
- **IndexStore** (`index_store.rs`): SQLite persistence for file index and symbols. Tables: `file_index` (with UNIQUE on project_path+file_path) and `file_symbols` (FK with CASCADE delete). Methods: `upsert_file_index()`, `query_symbols()`, `get_project_summary()`, `is_index_stale()`.
- **BackgroundIndexer** (`background_indexer.rs`): Tokio async task for non-blocking indexing. Full index on startup, incremental updates via mpsc channel from file watcher. SHA-256 content hashing for staleness detection.
- **CodebaseSearch Tool** (`executor.rs`, `definitions.rs`): Index-backed search with scopes: `symbols`, `files`, `all`. Optional component filter. Falls back to suggesting Grep when index unavailable.
- **Project Summary Injection** (`system_prompt.rs`): Deterministic (alphabetically sorted) project structure summary injected into system prompt. Critical for Ollama KV-cache stability.
- **Prefix-Stable Compaction** (`service_helpers.rs`): For non-Claude providers (Ollama/Qwen/DeepSeek/GLM), uses sliding-window deletion instead of LLM-summary rewrite. Preserves head (2 msgs) + tail (6 msgs), no LLM call needed.
- **Anthropic cache_control** (`anthropic.rs`): `anthropic-beta: prompt-caching-2024-07-31` header, system prompt as structured block with `cache_control: ephemeral`, last tool definition gets `cache_control`.

### LLM Provider Abstraction

Providers in `services/llm/` with different tool-calling reliability levels:

- **Reliable** (Anthropic, OpenAI): Native tool use, LLM-summary compaction
- **Unreliable** (Qwen, DeepSeek, GLM): Dual-channel tool calling — tools passed via native API AND prompt-based fallback instructions. Native `tool_calls` checked first, then text-based parsing with repair-hint retry.
- **None** (Ollama, MiniMax): Prompt-only tool calling, no native tool support

Compaction strategy follows provider reliability: reliable providers use LLM-summary rewrite, unreliable/none use prefix-stable sliding-window deletion (preserves head 2 + tail 6 messages).

### Streaming & Events

Real-time updates use Tauri's event system. Backend emits via `AppHandle::emit("event-name", &payload)` with `tokio::sync::mpsc` channels. Frontend listens via `listen()` from `@tauri-apps/api/event` in `useEffect` cleanup patterns.

### Adding a New Feature (End-to-End)

1. Create data model in `src-tauri/src/models/`
2. Create service in `src-tauri/src/services/` (constructed with database pool, returns `AppResult<T>`)
3. Create command module in `src-tauri/src/commands/` (async functions returning `Result<CommandResponse<T>, String>`)
4. Register commands in `main.rs` `invoke_handler![]`
5. Create TypeScript API wrapper in `src/lib/`
6. Create Zustand store in `src/store/`
7. Create React components in `src/components/`

## Code Conventions

- TypeScript strict mode with `noUnusedLocals`, `noUnusedParameters`
- ESLint zero-warning policy (`--max-warnings 0`)
- Unused variables prefixed with `_` (both TypeScript and Rust)
- Frontend tests in `src/**/*.{test,spec}.{ts,tsx}` using Vitest + jsdom + Testing Library
- Backend integration tests in `src-tauri/tests/integration/`
- Release builds use LTO, `opt-level = "s"`, and strip symbols
- Conventional commits: `type(scope): description` (e.g., `feat(analytics): add CSV export`, `fix(agents): resolve duplicate ID issue`)
- Optional `browser` feature flag enables chromiumoxide-based headless Chrome automation
