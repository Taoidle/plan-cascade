<div align="center">

# Plan Cascade Desktop

**AI-Powered Programming Orchestration Platform**

A cross-platform desktop application that decomposes complex development tasks into parallel executable workflows with multi-agent collaboration, powered by a Rust backend and React frontend.

[![Version](https://img.shields.io/badge/version-0.1.0-blue)](package.json)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-FFC131?logo=tauri&logoColor=white)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-18.3-61dafb?logo=react&logoColor=white)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-2021_Edition-dea584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green)](../LICENSE)

[English](./README.md) | [简体中文](./README_zh-CN.md)

</div>

---

## Overview

Plan Cascade Desktop is a comprehensive AI programming assistant built on **Tauri 2.0**. It connects to 7+ LLM providers, provides intelligent code generation with agentic tool use, and orchestrates multi-step development workflows — from simple Q&A to fully autonomous PRD-driven feature implementation.

### Why Plan Cascade Desktop?

- **Pure Rust backend** — minimal memory footprint, no Python/Node runtime needed at runtime
- **Security first** — API keys encrypted with AES-256-GCM, stored locally, never transmitted
- **Works with your models** — supports Anthropic, OpenAI, DeepSeek, Ollama (local), Qwen, Zhipu GLM, MiniMax
- **Multiple execution modes** — choose the right level of autonomy for every task
- **Full-stack type safety** — TypeScript strict mode + Rust compile-time checks
- **Cross-platform** — Windows, macOS (Universal binary), and Linux

---

## Features

### Multi-Mode Execution

| Mode | Description | Use Case |
|------|-------------|----------|
| **Claude Code** | Interactive chat with Claude Code CLI integration | Real-time pair programming |
| **Simple** | Direct LLM conversation with agentic tool use | Quick tasks and Q&A |
| **Expert** | PRD generation with dependency graph visualization | Feature planning and decomposition |
| **Task** | PRD-driven autonomous multi-story execution | Complex feature implementation |
| **Plan** | Multi-feature mega-plan orchestration | Project-level coordination |

### Agent Library

Create and manage specialized AI agents with custom system prompts, tool constraints, model selection, and execution history. Agents are reusable across projects and sessions.

### Quality Gates

Automated validation pipeline that runs after each code generation step:
- Test execution (unit, integration, e2e)
- Linting and formatting checks
- Type checking
- Custom validation rules per project

### Timeline & Checkpoints

Session-level version control for AI-generated changes:
- Automatic state snapshots at key milestones
- Branch and fork workflows for exploring alternatives
- One-click rollback to any checkpoint

### Git Worktree Integration

Isolated development environments for parallel task execution:
- Automatic branch creation and worktree setup
- Safe merge workflows with conflict detection
- Multi-task parallel development

### Knowledge Base (RAG)

Semantic document search powered by embeddings:
- Index project docs, design specs, and references
- Multi-provider embedding support
- Automatic change detection and re-indexing

### Codebase Index

AI-powered code search and understanding:
- Tree-sitter based symbol extraction (functions, classes, structs, enums)
- Background indexing with file watching
- HNSW vector search for semantic code queries

### MCP Integration

Full [Model Context Protocol](https://modelcontextprotocol.io/) support:
- Server registry management
- Custom tool and resource provider configuration
- SSE and stdio transport support

### Analytics Dashboard

Track usage, costs, and performance across all LLM providers:
- Token consumption and cost breakdown by model/provider
- Historical trend visualization
- Session-level usage attribution

### Additional Capabilities

- **Guardrails** — rule-based constraints on tool execution for safety
- **Webhooks** — event routing to Slack, Feishu, Discord, or custom endpoints
- **Remote Control** — Telegram bot and A2A protocol agent discovery
- **Plugin System** — framework-specific skill injection (React, Vue, Rust)
- **PDF / Image Export** — export conversations and artifacts
- **Internationalization** — English, Chinese (Simplified), Japanese

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Plan Cascade Desktop                        │
├───────────────────────────┬─────────────────────────────────────┤
│   React Frontend          │   Rust Backend (Tauri 2.0)          │
│   ───────────────────     │   ─────────────────────────         │
│   Radix UI Components     │   300+ IPC Commands                 │
│   Zustand State (50)      │   42+ Service Modules               │
│   Monaco Editor           │   SQLite + r2d2 Pool                │
│   i18next (3 languages)   │   AES-256-GCM Keyring               │
│   Tailwind CSS            │   Tree-sitter Code Parsing          │
│   Fuse.js Fuzzy Search    │   HNSW Vector Search                │
├───────────────────────────┴─────────────────────────────────────┤
│                        Tauri IPC Bridge                         │
├───────────┬──────────────┬──────────────┬───────────────────────┤
│ Claude    │ LLM          │ Git          │ MCP                   │
│ Code CLI  │ Providers    │ Worktrees    │ Servers               │
│           │ (7+)         │              │                       │
└───────────┴──────────────┴──────────────┴───────────────────────┘
```

### Cargo Workspace

The Rust backend is organized into 5 crates:

| Crate | Purpose |
|-------|---------|
| `plan-cascade-desktop` | Main Tauri app — commands, services, storage |
| `plan-cascade-core` | Core traits, error types, context hierarchy, streaming |
| `plan-cascade-llm` | LLM provider abstraction with streaming adapters |
| `plan-cascade-tools` | Tool executor framework and definitions |
| `plan-cascade-quality-gates` | Quality gate pipeline and project type detection |

### Project Structure

```
desktop/
├── src/                          # React frontend
│   ├── components/               #   UI components by domain
│   │   ├── Agents/               #     Agent library
│   │   ├── Analytics/            #     Usage & cost dashboard
│   │   ├── ClaudeCodeMode/       #     Claude Code CLI integration
│   │   ├── ExpertMode/           #     PRD & strategy planning
│   │   ├── SimpleMode/           #     Direct LLM chat
│   │   ├── TaskMode/             #     Autonomous task execution
│   │   ├── KnowledgeBase/        #     RAG document search
│   │   ├── Timeline/             #     Checkpoint browser
│   │   ├── MCP/                  #     MCP server management
│   │   ├── Settings/             #     Configuration UI
│   │   └── shared/               #     Reusable components
│   ├── store/                    #   Zustand state management
│   ├── lib/                      #   IPC API wrappers
│   ├── i18n/                     #   Translations (en, zh, ja)
│   └── types/                    #   TypeScript type definitions
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── commands/             #   Tauri IPC command handlers
│   │   ├── services/             #   Business logic layer
│   │   ├── models/               #   Data structures
│   │   └── storage/              #   SQLite, keyring, config
│   └── crates/                   #   Workspace crates
│       ├── core/                 #     Core traits & types
│       ├── llm/                  #     LLM provider adapters
│       ├── tools/                #     Tool execution framework
│       └── quality-gates/        #     Validation pipeline
└── docs/                         # Documentation
```

---

## Quick Start

### Prerequisites

| Dependency | Version | Notes |
|------------|---------|-------|
| [Node.js](https://nodejs.org/) | 18+ | Frontend build tooling |
| [pnpm](https://pnpm.io/) | 8+ | Package manager |
| [Rust](https://rustup.rs/) | 1.70+ | Backend compilation |
| System libs | — | See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) |

### Install & Run

```bash
# Clone the repository
git clone https://github.com/plan-cascade/plan-cascade
cd plan-cascade/desktop

# Install frontend dependencies
pnpm install

# Start development (frontend + backend with hot reload)
pnpm tauri:dev
```

On first run, the Rust backend compiles from source — this takes a few minutes. Subsequent starts are fast.

### Production Build

```bash
# Build for current platform
pnpm tauri:build

# Platform-specific builds
pnpm tauri:build:macos      # macOS Universal (Intel + Apple Silicon)
pnpm tauri:build:windows    # Windows x64 MSI
pnpm tauri:build:linux      # Linux x64 AppImage
```

---

## Development

### Commands

```bash
# Frontend
pnpm dev                    # Vite dev server only (port 8173)
pnpm build                  # TypeScript compile + Vite build
pnpm lint                   # ESLint (zero-warning policy)
pnpm typecheck              # TypeScript strict mode check
pnpm test                   # Run tests (Vitest)
pnpm test:watch             # Watch mode
pnpm test:coverage          # Coverage report (60% threshold)

# Backend (from src-tauri/)
cargo test                  # Unit + integration tests
cargo clippy                # Rust linting
cargo check                 # Type check
cargo build --features browser  # Build with headless Chrome support

# Full app
pnpm tauri:dev              # Dev mode with hot reload + devtools
pnpm tauri:build:dev        # Debug build
```

### Code Quality

- **TypeScript**: strict mode with `noUnusedLocals` and `noUnusedParameters`
- **ESLint**: zero-warning policy (`--max-warnings 0`)
- **Prettier**: enforced via pre-commit hooks (Husky + lint-staged)
- **Rust**: clippy for linting, release builds with LTO and symbol stripping
- **Commits**: conventional format — `type(scope): description`

---

## Supported LLM Providers

| Provider | Tool Calling | Local | Notes |
|----------|:---:|:---:|-------|
| [Anthropic](https://www.anthropic.com/) (Claude) | Native | | Prompt caching support |
| [OpenAI](https://openai.com/) (GPT) | Native | | |
| [DeepSeek](https://www.deepseek.com/) | Dual-channel | | Native + prompt fallback |
| [Qwen](https://www.alibabacloud.com/en/solutions/generative-ai/qwen) (Alibaba) | Dual-channel | | |
| [Zhipu GLM](https://www.zhipuai.cn/) | Dual-channel | | |
| [Ollama](https://ollama.com/) | Prompt-only | Yes | Any local model |
| [MiniMax](https://www.minimaxi.com/) | Prompt-only | | |

**Dual-channel**: tools passed via native API and prompt-based fallback for reliability.

---

## Documentation

| Document | Description |
|----------|-------------|
| [User Guide](./docs/user-guide.md) | Feature walkthrough for end users |
| [Developer Guide](./docs/developer-guide.md) | Architecture deep-dive and contribution guide |
| [API Reference](./docs/api-reference.md) | Complete IPC command documentation |
| [Migration Guide](./docs/migration-v5.md) | Upgrade from v4.x to v5.0 |
| [Codebase Index Plan](./docs/codebase-index-iteration-plan.md) | Semantic search iteration roadmap |
| [Memory Skill Plan](./docs/memory-skill-iteration-plan.md) | Agent memory system design |

---

## Contributing

Contributions are welcome! Please see the [Developer Guide](./docs/developer-guide.md) for architecture details and conventions.

```bash
# 1. Fork and clone
git clone https://github.com/<your-username>/plan-cascade
cd plan-cascade/desktop

# 2. Create a feature branch
git checkout -b feat/your-feature

# 3. Make changes, ensure quality checks pass
pnpm lint && pnpm typecheck && pnpm test

# 4. Commit with conventional message
git commit -m "feat(scope): add your feature"

# 5. Push and open a Pull Request
git push origin feat/your-feature
```

### Guidelines

- All tests must pass before merging
- ESLint zero-warning policy — no suppressions without justification
- Update relevant documentation for user-facing changes
- Follow existing patterns for new commands, services, and components

---

## Troubleshooting

**Build fails with "linker 'cc' not found"**

```bash
# macOS
xcode-select --install

# Ubuntu / Debian
sudo apt install build-essential libwebkit2gtk-4.1-dev libappindicator3-dev

# Fedora
sudo dnf install gcc webkit2gtk4.1-devel libappindicator-gtk3-devel
```

**Tauri dev server won't start**

```bash
cargo clean                           # Clear Rust build cache
rm -rf node_modules && pnpm install   # Reinstall frontend deps
```

**API keys not saving**

The app uses a local encrypted file store (AES-256-GCM) rather than the OS keychain. Check that the app has write permissions to its data directory.

---

## Tech Stack

### Frontend

| Category | Library | Version |
|----------|---------|---------|
| Framework | React | 18.3 |
| State Management | Zustand | 5.0 |
| UI Primitives | Radix UI | latest |
| Code Editor | Monaco Editor | 4.7 |
| Styling | Tailwind CSS | 3.4 |
| i18n | i18next | 25.8 |
| Markdown | react-markdown + rehype + remark | 10.1 |
| Math Rendering | KaTeX | 0.16 |
| Drag & Drop | @dnd-kit | latest |
| Syntax Highlighting | Prism React Renderer | 2.4 |
| Fuzzy Search | Fuse.js | 7.1 |

### Backend

| Category | Library | Version |
|----------|---------|---------|
| Desktop Framework | Tauri | 2.0 |
| Async Runtime | Tokio | 1.x |
| Database | rusqlite (bundled SQLite) | 0.32 |
| Connection Pool | r2d2 | latest |
| HTTP Client | Reqwest | 0.12 |
| Encryption | aes-gcm | 0.10 |
| Code Parsing | tree-sitter | 0.24 |
| Vector Search | hnsw_rs | 0.3 |
| File Watching | notify | 6.x |
| LLM SDKs | ollama-rs, async-dashscope, anthropic-async, zai-rs | various |

---

## License

[MIT](../LICENSE)

---

## Acknowledgments

- [Tauri](https://tauri.app/) — cross-platform desktop framework
- [Anthropic](https://www.anthropic.com/) — Claude API and Claude Code
- [Radix UI](https://www.radix-ui.com/) — accessible, headless UI primitives
- [Monaco Editor](https://microsoft.github.io/monaco-editor/) — code editor component
- [Tree-sitter](https://tree-sitter.github.io/) — incremental code parsing
