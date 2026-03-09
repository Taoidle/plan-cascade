[English](README.md)

<div align="center">

# Plan Cascade

**AI-Powered Cascading Development Framework**

*Decompose complex projects into parallel executable tasks with multi-provider execution*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-4.4.0-brightgreen)](https://github.com/Taoidle/plan-cascade)
[![Claude Code](https://img.shields.io/badge/Claude%20Code-Plugin-blue)](https://claude.ai/code)
[![MCP](https://img.shields.io/badge/MCP-Server-purple)](https://modelcontextprotocol.io)

| Component | Status |
|-----------|--------|
| Claude Code Plugin | ![Stable](https://img.shields.io/badge/status-stable-brightgreen) |
| MCP Server | ![Stable](https://img.shields.io/badge/status-stable-brightgreen) |
| Standalone CLI | ![In Development](https://img.shields.io/badge/status-in%20development-yellow) |
| Desktop App | ![Alpha](https://img.shields.io/badge/status-alpha-red) |

[Features](#features) • [Quick Start](#quick-start) • [Documentation](#documentation) • [Architecture](#architecture)

</div>

---

## Why Plan Cascade?

Traditional AI coding assistants struggle with large, complex projects. Plan Cascade solves this by:

- **Breaking down complexity** — Automatically decompose projects into manageable stories
- **Parallel execution** — Run independent tasks simultaneously with multiple agents
- **Maintaining context** — Design docs/PRDs plus execution context (with a durable tool journal) survive compaction/truncation
- **Quality assurance** — Automated testing and linting at each step

## Features

### Three-Tier Cascading Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Level 1: Mega Plan                                         │
│  ─────────────────                                          │
│  Project-level orchestration                                │
│  Manages multiple features in parallel batches              │
│  Output: mega-plan.json + design_doc.json                   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Level 2: Hybrid Ralph (Feature)                            │
│  ───────────────────────────────                            │
│  Feature-level development                                  │
│  Auto-generates PRD with user stories                       │
│  Output: prd.json + design_doc.json                         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Level 3: Story Execution                                   │
│  ────────────────────────                                   │
│  Parallel story execution with multi-provider support           │
│  Configurable LLM provider selection for each execution stage  │
│  Output: Code changes                                       │
└─────────────────────────────────────────────────────────────┘
```

### Core Modes

Plan Cascade provides three independent execution modes managed by a unified **Workflow Kernel**:

| Mode | Description | Best For |
|------|-------------|----------|
| **Chat** | Lightweight chat interface for simple Q&A | Quick questions, file lookups, trivial tasks |
| **Plan** | Domain-adaptive task decomposition with Steps | Content creation, research, multi-step workflows |
| **Task** | PRD-driven code development with Stories and Quality Gates | Feature development, complex implementations |

### Workflow Kernel

All three modes are managed by a unified **Workflow Kernel** that provides:

- **Unified session lifecycle** — Consistent state management across modes
- **Typed event streaming** — Real-time progress updates
- **Mode handoff** — Seamless switching between Chat/Plan/Task
- **Lightweight checkpointing** — Recovery from interruptions

### Plan Mode — Domain-Adaptive Task Decomposition

The **Plan Mode** provides a domain-independent task decomposition framework:

- **Domain Adapters** — Specialized handlers for different task types (General, Writing, Research, Marketing, DataAnalysis, etc.)
- **Step Decomposition** — Breaks tasks into executable Steps with dependencies
- **Batch Execution** — Parallel execution of independent Steps
- **Output Validation** — Validates Step outputs against acceptance criteria

### Task Mode — PRD-Driven Code Development

The **Task Mode** is the most powerful mode, implementing a complete software engineering methodology:

- **PRD Generation** — Auto-generate Product Requirements Document from task description
- **Story Decomposition** — Break down requirements into executable Stories with dependencies
- **Kahn Algorithm** — Topological sorting for optimal batch execution order
- **7-Level Agent Priority** — Intelligent agent selection (global > stage > story > inference > default > fallback > claude-code)
- **Complete Quality Gates** — Full validation pipeline after each Story:
  - DoR (Definition of Ready)
  - DoD (Definition of Done)
  - AI Verification (detects skeleton code)
  - Code Review (quality scoring)
  - TDD Compliance
- **Auto-Retry** — Exponential backoff on failures (5s → 10s → 20s)

### Multi-Provider Execution

Plan Cascade supports configuring different LLM providers for story execution. Providers are configured in Settings → Stage Agents tab.

| Provider | Type | Best For |
|----------|------|----------|
| `claude-sonnet` | LLM (default) | Balanced capability and speed |
| `claude-opus` | LLM | Complex tasks, best capability |
| `claude-haiku` | LLM | Simple tasks, fastest response |

Supported LLM providers: Anthropic, OpenAI, Ollama, DeepSeek.

**Note**: CLI agent support (codex, aider) for external tool execution is not yet implemented.

### Story Execution Modes

| Mode | Description |
|------|-------------|
| `LLM` | Use direct LLM API via OrchestratorService (default) |
| `CLI` | Use external CLI tools (not yet implemented) |

Story execution mode is automatically determined based on whether an LLM provider is configured.

### Auto-Generated Design Documents

Plan Cascade automatically generates technical design documents alongside PRDs:

- **Project-level**: Architecture, patterns, cross-feature decisions
- **Feature-level**: Component design, APIs, story mappings
- **Inheritance**: Feature docs inherit from project-level context

### Quality Gates

Automated verification after each story:
- TypeScript/Python type checking
- Unit and integration tests
- Linting (ESLint, Ruff)
- Custom validation scripts
- **AI Verification Gate** - Validates implementation against acceptance criteria and detects skeleton code
- **Code Review Gate** - AI-powered code review with quality scoring
- **TDD Compliance Gate** - Ensures test changes accompany code changes

### Desktop Application Features

The Desktop application provides a complete GUI with **50+ Tauri commands** covering:

| Category | Commands |
|----------|----------|
| **Task Mode** | Enter/exit task mode, generate/approve PRD, execution status, cancel/report |
| **Plan Mode** | Plan generation, session analysis, lifecycle reporting |
| **Git Operations** | Worktree management, branch operations, commit history |
| **Memory** | Semantic storage and retrieval with configurable embedding providers |
| **Knowledge** | Project knowledge management with hybrid search (HNSW + FTS5) |
| **Codebase Index** | LSP-based code intelligence, semantic search |
| **Skills** | External framework skills loading (React, Vue, Rust, etc.) |
| **MCP** | Model Context Protocol server integration |
| **Plugins** | Extensible plugin architecture |
| **Settings** | User preferences management |
| **Webhooks** | External callbacks |
| **Evaluation** | Code quality assessment |

### Memory System

The Desktop application includes a sophisticated **cross-session persistent memory system** that learns from user interactions and project context:

| Feature | Description |
|---------|-------------|
| **Storage** | SQLite-based persistent storage with TF-IDF embedding |
| **Scopes** | Project, Global (cross-project), and Session-level memory |
| **Retrieval** | 4-signal hybrid ranking (embedding + keyword + importance + recency) |
| **Categories** | Preference, Convention, Pattern, Correction, Fact |
| **Extraction** | LLM-driven automatic memory extraction from conversations |
| **Commands** | Explicit memory commands: `remember that...`, `forget that...`, `what do you remember about...` |
| **Maintenance** | Automatic decay, pruning, and compaction |

**Memory Categories:**

| Category | Description |
|----------|-------------|
| `Preference` | User preferences and habits |
| `Convention` | Project-specific conventions |
| `Pattern` | Discovered patterns in the codebase |
| `Correction` | Corrections made during development |
| `Fact` | Factual knowledge about the project |

**Explicit Memory Commands:**
- `remember that [content]` — Store a new memory
- `forget about [topic]` — Delete memories about a topic
- `what do you remember about [topic]` — Query memories

**Global Memory:**
Memory tagged with `__global__` is shared across all projects and sessions, enabling user preference persistence.

### External Framework Skills

Plan Cascade includes built-in framework-specific skills that are automatically detected and injected:

| Framework | Skills | Auto-Detection |
|-----------|--------|----------------|
| React/Next.js | `react-best-practices`, `web-design-guidelines` | `package.json` contains `react` or `next` |
| Vue/Nuxt | `vue-best-practices`, `vue-router-best-practices`, `vue-pinia-best-practices` | `package.json` contains `vue` or `nuxt` |
| Rust | `rust-coding-guidelines`, `rust-ownership`, `rust-error-handling`, `rust-concurrency` | `Cargo.toml` exists |

Skills are loaded from Git submodules and provide framework-specific guidance during story execution:

```bash
# Initialize external skills (first time)
git submodule update --init --recursive

# In a React project, skills are auto-detected:
/plan-cascade:auto "Add user profile component"
# → Automatically includes React best practices in context
```

## Quick Start

### Option 1: Claude Code Plugin (Recommended)

```bash
# Install the plugin
claude plugins install Taoidle/plan-cascade

# First-time setup (recommended, especially on Windows)
/plan-cascade:init

# Let AI choose the best strategy
/plan-cascade:auto "Build a REST API with user authentication and JWT tokens"
# → Defaults to FULL flow (spec auto + TDD on + confirmations). Override with --flow/--tdd/--no-confirm as needed.

# Or choose manually
/plan-cascade:hybrid-auto "Add password reset functionality"
/plan-cascade:approve --auto-run
```

### Option 2: Desktop Application (Alpha)

> **Note**: The Desktop application is currently in **Alpha** stage. Core capabilities are complete but some features may be unstable.

Based on **Tauri 2.0** with Rust backend and React frontend:

```bash
# Build from source
cd desktop
pnpm install
pnpm tauri dev

# Or run the built app (after build)
pnpm tauri build
```

### Option 3: Standalone CLI

> **Note**: The standalone CLI is currently in active development. Some features may be incomplete or unstable. For production use, we recommend the Claude Code Plugin.

```bash
# Install
pip install plan-cascade

# Configure
plan-cascade config --setup

# Run with auto-strategy
...
```

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture Design](desktop/docs/architecture-design.md) | Overall system architecture |
| [Kernel Design](desktop/docs/kernel-design.md) | Workflow kernel, Simple Plan/Task production specs |
| [Memory & Skills Design](desktop/docs/memory-skill-design.md) | Memory and skills architecture |
| [Codebase Index Design](desktop/docs/codebase-index-design.md) | HNSW/FTS5/LSP index design |
| [Developer Guide](desktop/docs/developer-guide-v2.md) | Development setup, project structure |
| [API Reference](desktop/docs/api-reference-v2.md) | Tauri command reference |

## Architecture

### Desktop Application Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Frontend (React + TypeScript)           │
├─────────────────────────────────────────────────────────────┤
│  110+ Zustand Stores  │  React Query  │  Radix UI        │
└─────────────────────────────────────────────────────────────┘
                              │
                    Tauri IPC (50+ commands)
                              │
┌─────────────────────────────────────────────────────────────┐
│                   Backend (Rust + Tauri 2.0)                │
├─────────────────────────────────────────────────────────────┤
│  Core Crate    │  LLM Crate   │  Quality Gates  │  Tools   │
│  Context/      │  OpenAI/     │  Detector/     │  Executor│
│  Events/       │  Anthropic/  │  Validator/   │  Traits  │
│  Streaming     │  DeepSeek/   │  Pipeline     │          │
│                │  Ollama/...   │               │          │
└─────────────────────────────────────────────────────────────┘
```

### LLM Providers Supported

| Provider | Models |
|----------|--------|
| OpenAI | GPT-4o, GPT-4o Mini |
| Anthropic | Claude Sonnet, Claude Haiku |
| DeepSeek | DeepSeek Chat |
| GLM | ZhipuAI (Dialogue + Embedding) |
| MiniMax | M2, M2.1, M2.5 (via Anthropic-compatible API) |
| Ollama | Local models |
| Qwen | Tongyi Qianwen (Dialogue + Embedding) |

### Search Capabilities

- **Semantic Search** — Configurable embedding providers (OpenAI, Qwen, GLM, Ollama, TF-IDF)
- **Hybrid Search** — HNSW + FTS5 combined
- **LSP-based** — Language Server Protocol for code intelligence

## License

MIT License - see [LICENSE](LICENSE) for details.
