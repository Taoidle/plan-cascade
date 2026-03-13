<div align="center">

# Plan Cascade Desktop

**Local-First AI Development Workstation**

*A complete AI-powered development environment — Chat, Plan, Task, Debug — all in one desktop app*

[![Version](https://img.shields.io/badge/version-0.1.0-blue)](./package.json)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-FFC131?logo=tauri&logoColor=white)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-18.3-61dafb?logo=react&logoColor=white)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-2021-dea584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-green)](../LICENSE)

[English](./README.md) | [简体中文](./README_zh-CN.md)

</div>

---

## Why Desktop?

AI coding assistants are transforming how we build software. But using them effectively often means juggling multiple tools:

- Chat in one app, code in another
- No visibility into what the AI is actually doing
- Switching contexts loses important details
- Enterprise concerns: security, audit trails, data control

**Plan Cascade Desktop** solves these problems with a unified, local-first AI workstation.

### Key Differentiators

| Capability | Plan Cascade Desktop | Cursor / Copilot | Claude Code |
|------------|---------------------|------------------|-------------|
| **Multi-Model Support** | 7+ providers | Limited | Claude only |
| **Offline Mode** | ✅ Ollama | ❌ | ❌ |
| **4 Workflow Modes** | Chat/Plan/Task/Debug | Single mode | Single mode |
| **Unified State Kernel** | SSOT architecture | Per-session | Per-session |
| **Cross-Mode Context** | Zero information loss | ❌ | ❌ |
| **Quality Gates** | Full pipeline + auto-fix | Basic lint | ❌ |
| **5-Layer Security** | Guardrail → Policy → Sandbox → Audit | Basic confirmations | Basic |
| **Skill System** | 4 sources + priority management | ❌ | ❌ |
| **Memory System** | TF-IDF + 4-signal ranking | Conversation history | Conversation history |
| **Remote Control** | A2A + Telegram | ❌ | ❌ |
| **MCP Full Stack** | Server + Client + Import | ❌ | Partial |

---

## Four Workflow Modes

All modes are driven by a **Unified Workflow Kernel (SSOT)**, ensuring state consistency and seamless context handoff.

### Chat Mode — Conversational Interface

```
ready → submitting → streaming → paused/failed/cancelled
```

**Features:**
- Streaming responses with pause/resume
- Message queuing and turn management
- File drag & drop, @-references
- Slash commands integration

### Plan Mode — Structured Execution

```
idle → analyzing → clarifying → planning → executing → completed
```

**Features:**
- Step orchestration with parallel execution (maxParallel=4)
- Automatic retry with backoff (800ms)
- Batch gating — blocks subsequent batches on failure
- Output quality gates — rejects empty output, TODO narratives

### Task Mode — Full Development Workflow

```
idle → interviewing → exploring → generating_prd → executing → completed
```

**Features:**
- Requirements analysis and clarification
- Automatic PRD and design document generation
- Strategy recommendation (flow level, TDD mode, quality gates)
- Full quality gates pipeline

### Debug Mode — Professional Debugging

```
intaking → clarifying → reproducing → hypothesizing → testing → patching → verifying
```

**Features:**
- **Hypothesis Management** — Track multiple hypotheses with supporting/refuting evidence
- **Capability Tiers** — `dev_full` / `staging_limited` / `prod_observe_only`
- **Approval Mechanism** — Patch preview, dangerous tools require manual confirmation
- **Verification Reports** — Structured checklists and residual risk assessment

### Cross-Mode Context Handoff

```
┌─────────────────────────────────────────────────────────────────┐
│                    Workflow Kernel (SSOT)                       │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │              modeSnapshots & modeRuntimeMeta                ││
│  │  ┌───────┐  ┌───────┐  ┌───────┐  ┌───────┐               ││
│  │  │ Chat  │  │ Plan  │  │ Task  │  │ Debug │               ││
│  │  └───────┘  └───────┘  └───────┘  └───────┘               ││
│  └─────────────────────────────────────────────────────────────┘│
│           ↕ HandoffContext (cross-mode context transfer)        │
└─────────────────────────────────────────────────────────────────┘
```

**Benefits:**
- Chat → Plan/Task: Automatic conversation context import
- Plan/Task → Chat: Structured summary handoff
- Zero information loss on mode switching

---

## Core Features

### 🏆 Enterprise-Grade Security

5-Layer Security Model:

| Layer | Component | Function |
|-------|-----------|----------|
| 1 | **Guardrail** | Sensitive data detection and redaction |
| 2 | **Permission Gate** | Tool-level authorization |
| 3 | **Policy Engine v2** | Configurable security policies |
| 4 | **Sandbox** | Isolated execution environment |
| 5 | **Audit Log** | Complete operation trail |

### 🎯 Quality Gates Pipeline

Automated multi-dimensional validation:

```
┌─────────┐   ┌─────────┐   ┌─────────────┐   ┌─────────────┐   ┌─────────┐
│   DoR   │ → │  Code   │ → │     DoD     │ → │ AI Verify   │ → │ Review  │
│ (Ready) │   │ (Write) │   │   (Done)    │   │ (No Stubs)  │   │ (Score) │
└─────────┘   └─────────┘   └─────────────┘   └─────────────┘   └─────────┘
     │             │               │                 │               │
     ▼             ▼               ▼                 ▼               ▼
  Validate      Implement       All criteria     Detect stub     Code quality
  requirements   solution       met             & TODO          scoring
```

**Supported Checks:**
- Format (Prettier, Black, rustfmt...)
- Lint (ESLint, Ruff, Clippy...)
- Type Check (TypeScript, mypy...)
- Security (injection detection, credential scanning)
- Complexity (cyclomatic complexity thresholds)

### 🧠 Growable Knowledge System

**Skill System (4 Sources + 2-Stage Selection):**

| Source | Priority | Examples |
|--------|----------|----------|
| Built-in Skills | Highest | hybrid-ralph, mega-plan, planning-with-files |
| External Skills (Git Submodules) | High | React, Vue, Rust best practices |
| User Skills (Project) | Medium | Project-specific workflows |
| Dynamic Skills (Generated) | Low | On-the-fly instructions |

**Memory System (TF-IDF + 4-Signal Ranking):**

```
Score = w₁×Recency + w₂×Frequency + w₃×Semantic + w₄×Importance
```

- Automatic decay and pruning
- Persistent across sessions
- Context-aware retrieval

### 🌐 Remote Control

**A2A Protocol (Agent-to-Agent):**

- JSON-RPC 2.0 + SSE transport
- 5-layer security protection
- Multi-platform adapters (Telegram, Discord, Slack)

**Example — Telegram Remote Control:**

```
User: /task implement user login
Bot: Task started. [View Progress → http://localhost:3000]
Bot: ⚠️ Quality gate failed: test coverage below 80%
User: /retry with --skip-coverage
Bot: Task completed. ✓ 5/5 stories passed
```

### 🔌 MCP Full Stack

| Capability | Description |
|------------|-------------|
| **MCP Client** | Connect to any MCP server |
| **MCP Server** | Expose Desktop tools as MCP resources |
| **Claude Desktop Import** | One-click config migration |
| **Registry** | Discover and manage MCP servers |
| **Health Check** | Monitor server availability |

---

## Multi-Model Support

| Provider | API Key | Offline | Best For |
|----------|---------|---------|----------|
| **Anthropic** | Required | ❌ | Complex reasoning |
| **OpenAI** | Required | ❌ | General purpose |
| **DeepSeek** | Required | ❌ | Cost-effective coding |
| **Ollama** | Optional | ✅ | Privacy, air-gapped |
| **GLM** | Required | ❌ | Chinese language |
| **Qwen** | Required | ❌ | Multilingual |
| **MiniMax** | Required | ❌ | Long context |

---

## Simple Workspace Layout

```
┌────────────────────────────────────────────────────────────────────────┐
│  Plan Cascade Desktop                              ─ □ ×             │
├────────────────┬───────────────────────────────────────────────────────┤
│                │  Messages                                           │
│  Files         │  ┌─────────────────────────────────────────────────┐ │
│  ┌──────────┐  │  │ 🤖 I'll help you implement the login feature.  │ │
│  │ src/     │  │  │ Let me analyze the codebase first...           │ │
│  │ ├─ api/  │  │  │                                                 │ │
│  │ ├─ auth/ │  │  │ 📋 Created PRD with 5 stories:                  │ │
│  │ └─ ...   │  │  │   ✓ story-001: JWT implementation               │ │
│  └──────────┘  │  │   ✓ story-002: Password hashing                 │ │
│                │  │   → story-003: Session management (in progress)  │ │
│  Worktrees     │  │   ○ story-004: Token refresh                     │ │
│  ┌──────────┐  │  │   ○ story-005: Logout                           │ │
│  │ feature/ │  │  └─────────────────────────────────────────────────┘ │
│  │ fix-bug/ │  │                                                       │
│  └──────────┘  │  Input: [Implement user authentication...]  [Send]   │
├────────────────┴───────────────────────────────────────────────────────┤
│  [Chat] [Plan] [Task] [Debug]                    Quality Gates: ✓ 3/3  │
└────────────────────────────────────────────────────────────────────────┘
```

---

## Tech Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| **Frontend** | React 18.3 + TypeScript | UI components |
| **State** | Zustand + Immer | Client state management |
| **Styling** | Tailwind CSS + Radix UI | Design system |
| **Backend** | Rust (Tauri 2.0) | Native performance |
| **IPC** | Tauri Commands | Frontend-Rust bridge |
| **Database** | SQLite + Tauri SQL | Persistent storage |
| **Indexing** | HNSW + FTS5 | Code & knowledge search |

### Rust Crates

| Crate | Purpose |
|-------|---------|
| `core` | Builders, LLM clients, quality gates |
| `llm` | Multi-provider LLM abstraction |
| `tools` | ReAct tool implementations |
| `quality-gates` | Format, lint, type check, security |

---

## Getting Started

### Prerequisites

- Node.js 20+
- pnpm
- Rust stable toolchain
- Tauri CLI: `pnpm add -g @tauri-apps/cli`

### Development

```bash
cd desktop
pnpm install
pnpm tauri:dev
```

### Production Build

```bash
cd desktop
pnpm tauri:build
```

---

## Commands Reference

```bash
# Frontend development
pnpm dev              # Start Vite dev server
pnpm build            # Build frontend
pnpm lint             # Run ESLint
pnpm typecheck        # TypeScript check
pnpm test             # Run tests

# Desktop application
pnpm tauri:dev        # Development mode
pnpm tauri:build      # Production build
pnpm tauri:build:dev  # Development build

# Rust backend
cd src-tauri
cargo test            # Run Rust tests
cargo check           # Fast compile check
```

---

## Project Structure

```
desktop/
├── src/                    # React frontend
│   ├── components/         # Reusable UI components
│   ├── pages/              # Workspace pages
│   ├── stores/             # Zustand state stores
│   ├── hooks/              # Custom React hooks
│   └── services/           # Frontend services
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── commands/       # Tauri IPC commands
│   │   ├── models/         # Data models
│   │   └── services/       # Core services
│   └── crates/             # Rust crates
│       ├── core/           # Core business logic
│       ├── llm/            # LLM integrations
│       ├── tools/          # Tool implementations
│       └── quality-gates/  # Quality validation
└── docs/                   # Desktop-specific docs
```

---

## Roadmap

| Version | Milestone | Status |
|---------|-----------|--------|
| 0.1.0 | Core 4 modes + basic quality gates | ✅ Current |
| 0.2.0 | Full A2A remote control | 🚧 In Progress |
| 0.3.0 | Analytics dashboard + cost tracking | 📋 Planned |
| 0.4.0 | Team collaboration features | 📋 Planned |
| 1.0.0 | Stable release | 📋 Planned |

---

## License

MIT License - see [LICENSE](../LICENSE) for details.

---

## Contributing

Contributions are welcome! Please read the contributing guidelines before submitting PRs.
