[中文版](README_zh.md)

<div align="center">

# Plan Cascade

**AI-Powered Cascading Development Framework**

*Decompose complex projects into parallel executable tasks with multi-agent collaboration*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-4.4.0-brightgreen)](https://github.com/Taoidle/plan-cascade)
[![Claude Code](https://img.shields.io/badge/Claude%20Code-Plugin-blue)](https://claude.ai/code)
[![MCP](https://img.shields.io/badge/MCP-Server-purple)](https://modelcontextprotocol.io)

| Component | Status |
|-----------|--------|
| Claude Code Plugin | ![Stable](https://img.shields.io/badge/status-stable-brightgreen) |
| MCP Server | ![Stable](https://img.shields.io/badge/status-stable-brightgreen) |
| Standalone CLI | ![In Development](https://img.shields.io/badge/status-in%20development-yellow) |
| Desktop App | ![Beta](https://img.shields.io/badge/status-beta-blue) |

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
│  Parallel story execution with multi-agent support          │
│  Automatic agent selection based on task type               │
│  Output: Code changes                                       │
└─────────────────────────────────────────────────────────────┘
```

### Multi-Agent Collaboration

| Agent | Type | Best For |
|-------|------|----------|
| `claude-code` | Built-in | General purpose (default) |
| `codex` | CLI | Bug fixes, quick implementations |
| `aider` | CLI | Refactoring, code improvements |
| `amp-code` | CLI | Alternative implementations |

Agents are automatically selected based on story type, or can be manually specified.

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

### Option 2: Standalone CLI

> **Note**: The standalone CLI is currently in active development. Some features may be incomplete or unstable. For production use, we recommend the Claude Code Plugin.

```bash
# Install
pip install plan-cascade

# Configure
plan-cascade config --setup

# Run with auto-strategy
plan-cascade run "Implement user authentication"

# Or use expert mode for more control
plan-cascade run "Implement user authentication" --expert
```

### Option 3: Desktop App

A cross-platform desktop application built with **Tauri 2.0** (Rust backend + React frontend). Provides a full GUI for all Plan Cascade capabilities plus standalone features like multi-provider LLM chat, knowledge base (RAG), analytics dashboard, and more.

```bash
cd desktop
pnpm install
pnpm tauri:dev        # Development mode with hot reload
pnpm tauri:build      # Production build for current platform
```

See the [Desktop README](desktop/README.md) for full details, or jump to the [Desktop App](#desktop-app) section below.

## Usage Examples

> **Note**: `/plan-cascade:auto` defaults to **FULL** flow (spec auto + TDD on + confirmations). Use `--flow standard|quick`, `--tdd auto|off`, or `--no-confirm` to opt out.

### Simple Task (Quick Direct Execution)
```bash
/plan-cascade:auto --flow quick "Fix the typo in the login button"
# → Executes directly without planning (quick flow)
```

### Medium Feature (Hybrid Auto)
```bash
/plan-cascade:auto "Implement OAuth2 login with Google and GitHub"
# → Generates PRD with 3-5 stories, executes in parallel
```

### Large Project (Mega Plan)
```bash
/plan-cascade:auto "Build an e-commerce platform with users, products, cart, and orders"
# → Creates mega-plan with 4 features, each with its own PRD
```

### With External Design Document
```bash
/plan-cascade:mega-plan "Build blog platform" ./architecture.md
# → Converts your design doc and uses it for guidance
```

### With Specific Agent
```bash
/plan-cascade:approve --impl-agent=aider --retry-agent=codex
# → Uses aider for implementation, codex for retries
```

## Desktop App

The desktop application is a standalone AI programming platform that operates independently of the Python core. It provides a rich GUI with its own pure-Rust backend.

### Execution Modes

| Mode | Description |
|------|-------------|
| **Claude Code** | Interactive GUI for Claude Code CLI with tool visualization |
| **Simple** | Direct LLM conversation with agentic tool use (file editing, shell, search) |
| **Expert** | Interview-driven PRD generation with dependency graph visualization |
| **Task** | PRD-driven autonomous multi-story execution with quality gates |
| **Plan** | Multi-feature mega-plan orchestration |

### LLM Providers

Connects to **7+ providers** with streaming support and intelligent tool-calling fallback:

| Provider | Tool Calling | Local |
|----------|:---:|:---:|
| Anthropic (Claude) | Native | |
| OpenAI (GPT) | Native | |
| DeepSeek | Dual-channel | |
| Qwen (Alibaba) | Dual-channel | |
| Zhipu GLM | Dual-channel | |
| Ollama | Prompt-only | Yes |
| MiniMax | Prompt-only | |

### Key Features

- **Agent Library** — Create reusable AI agents with custom prompts, tool constraints, and execution history
- **Quality Gates** — Automated test, lint, and type-check validation after each code generation step
- **Timeline & Checkpoints** — Session version control with branching, forking, and one-click rollback
- **Git Integration** — Full GUI for staging, committing, branching, merging, conflict resolution, and AI-assisted commit messages (46 git commands)
- **Knowledge Base (RAG)** — Semantic document search with HNSW vector indexing and multi-provider embeddings
- **Codebase Index** — Tree-sitter symbol extraction (6 languages) with background indexing and semantic search
- **MCP Integration** — Model Context Protocol server management and custom tool registration
- **Analytics Dashboard** — Token usage, cost tracking, and model performance comparison with CSV/JSON export
- **Agent Composer** — Visual canvas editor for multi-step agent pipelines (sequential, parallel, conditional)
- **Graph Workflow** — DAG-based workflow editor with draggable nodes and SVG edges
- **Plugins** — Framework skill injection (React, Vue, Rust) with marketplace support
- **Guardrails** — Rule-based constraints for sensitive data detection, code security, and custom regex patterns
- **Webhooks** — Event routing to Slack, Feishu, Discord, Telegram, or custom endpoints
- **Remote Control** — Telegram bot gateway and A2A (Agent-to-Agent) protocol
- **i18n** — English, Chinese (Simplified), Japanese

### Tech Stack

| Layer | Technology |
|-------|-----------|
| Frontend | React 18 + TypeScript + Zustand + Radix UI + Tailwind CSS + Monaco Editor |
| Backend | Rust + Tauri 2.0 + Tokio + SQLite + AES-256-GCM keyring |
| Code Analysis | Tree-sitter (Python, Rust, TypeScript, JavaScript, Go, Java) |
| Vector Search | HNSW (hnsw_rs) for embeddings, hybrid search with BM25 reranking |

### Build Targets

```bash
pnpm tauri:build:macos      # macOS Universal (Intel + Apple Silicon)
pnpm tauri:build:windows    # Windows x64 MSI
pnpm tauri:build:linux      # Linux x64 AppImage
```

For architecture details, development setup, and contribution guide, see the [Desktop README](desktop/README.md).

---

## Documentation

| Document | Description |
|----------|-------------|
| [Plugin Guide](docs/Plugin-Guide.md) | Claude Code plugin usage |
| [CLI Guide](docs/CLI-Guide.md) | Standalone CLI usage |
| [Desktop Guide](docs/Desktop-Guide.md) | Desktop application |
| [Desktop README](desktop/README.md) | Desktop development and architecture |
| [MCP Server Guide](docs/MCP-SERVER-GUIDE.md) | Integration with Cursor, Windsurf |
| [System Architecture](docs/System-Architecture.md) | Technical architecture |

## Architecture

### File Structure

```
plan-cascade/
├── src/plan_cascade/       # Python core
│   ├── core/               # Orchestration engine
│   ├── backends/           # Agent abstraction
│   ├── llm/                # LLM providers
│   └── cli/                # CLI entry
├── commands/               # Plugin commands
├── skills/                 # Plugin skills
├── mcp_server/             # MCP server
├── external-skills/        # Framework skills (React, Vue, Rust)
└── desktop/                # Desktop app (Tauri 2.0 + React 18)
    ├── src/                #   React frontend (283 components)
    │   ├── components/     #     UI by domain (23 feature areas)
    │   ├── store/          #     Zustand state (39 stores)
    │   └── lib/            #     Tauri IPC wrappers (30+ files)
    └── src-tauri/          #   Rust backend
        ├── src/commands/   #     359 IPC commands (43 modules)
        ├── src/services/   #     Business logic (150+ files)
        └── crates/         #     Workspace crates (core, llm, tools, quality-gates)
```

### Supported LLM Backends

| Backend | API Key Required | Notes |
|---------|-----------------|-------|
| Claude Code | No | Default, via Claude Code CLI |
| Claude API | Yes | Direct Anthropic API |
| OpenAI | Yes | GPT-4o, etc. |
| DeepSeek | Yes | DeepSeek Chat/Coder |
| Ollama | No | Local models |

## What's New in v4.4.0

- **`--agent` for hybrid-worktree** — PRD generation agent selection: `/plan-cascade:hybrid-worktree task branch "desc" --agent=codex`
- **Spec Interview (optional)** — Planning-time `spec.json/spec.md` workflow that compiles into PRD
- **Universal Resume** — `/plan-cascade:resume` auto-detects mode and routes to the right resume command
- **Dashboard + Gates** — Aggregated status view plus DoR/DoD/TDD quality gates
- **Compaction-Safe Session Journal** — Recent tool activity is persisted to `.state/claude-session/` and surfaced in `.hybrid-execution-context.md` / `.mega-execution-context.md`
- **Safer Auto Defaults** — `/plan-cascade:auto` defaults to FULL flow with `--spec auto`, `--tdd on`, and confirmations (override via `--flow`, `--tdd`, `--no-confirm`)

See [CHANGELOG.md](CHANGELOG.md) for full history.

## Contributing

Contributions are welcome! Please read our contributing guidelines before submitting PRs.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Acknowledgments

- [OthmanAdi/planning-with-files](https://github.com/OthmanAdi/planning-with-files) — Original inspiration
- [snarktank/ralph](https://github.com/snarktank/ralph) — PRD format
- [Anthropic](https://www.anthropic.com/) — Claude Code & MCP protocol
- [vercel-labs/agent-skills](https://github.com/vercel-labs/agent-skills) — React/Next.js best practices skills
- [vuejs-ai/skills](https://github.com/vuejs-ai/skills) — Vue.js best practices skills
- [actionbook/rust-skills](https://github.com/actionbook/rust-skills) — Rust meta-cognition framework skills

## License

[MIT License](LICENSE)

---

<div align="center">

**[GitHub](https://github.com/Taoidle/plan-cascade)** • **[Issues](https://github.com/Taoidle/plan-cascade/issues)** • **[Discussions](https://github.com/Taoidle/plan-cascade/discussions)**

[![Star History Chart](https://api.star-history.com/svg?repos=Taoidle/plan-cascade&type=Date)](https://star-history.com/#Taoidle/plan-cascade&Date)

</div>
