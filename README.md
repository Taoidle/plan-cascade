<div align="center">

# Plan Cascade

**AI-Powered Cascading Development Framework**

*Transform complex projects into parallel executable tasks with intelligent decomposition and multi-provider execution*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

| Component | Version | Status | Description |
|-----------|---------|--------|-------------|
| **Plugin** | [![4.4.0](https://img.shields.io/badge/version-4.4.0-brightgreen)](https://github.com/Taoidle/plan-cascade) | ![Stable](https://img.shields.io/badge/status-stable-brightgreen) | Claude Code integration |
| **Desktop** | [![0.1.0](https://img.shields.io/badge/version-0.1.0-blue)](./desktop) | ![Alpha](https://img.shields.io/badge/status-alpha-orange) | Local-first AI workstation |
| **CLI** | ![Dev](https://img.shields.io/badge/status-dev-yellow) | ![Dev](https://img.shields.io/badge/status-dev-yellow) | Command-line interface |
| **MCP Server** | ![Stable](https://img.shields.io/badge/status-stable-brightgreen) | ![Stable](https://img.shields.io/badge/status-stable-brightgreen) | Model Context Protocol |

[Why Plan Cascade?](#why-plan-cascade) • [Product Editions](#product-editions) • [Quick Start](#quick-start) • [Architecture](#architecture)

</div>

---

## Why Plan Cascade?

Traditional AI coding assistants hit a wall with large, complex projects:

| Challenge | Conventional AI | Plan Cascade |
|-----------|-----------------|--------------|
| **Complexity** | Gets lost in large codebases | Decomposes into manageable units |
| **Parallelism** | Sequential, one-at-a-time | Independent tasks run in parallel |
| **Context** | Lost during long sessions | Design docs + durable context survive compaction |
| **Quality** | Manual verification needed | Automated testing & linting at each step |
| **Control** | Black box execution | Transparent, inspectable workflow |

### The Solution: Cascading Decomposition

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Your Project Goal                            │
│            "Build a REST API with authentication"                   │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Level 1: Mega Plan                                                 │
│  ─────────────────────                                              │
│  Project-level orchestration → Manages multiple features in batches │
│  Output: mega-plan.json + design_doc.json                          │
└─────────────────────────────────────────────────────────────────────┘
                                  │
              ┌───────────────────┼───────────────────┐
              ▼                   ▼                   ▼
┌─────────────────────┐ ┌─────────────────────┐ ┌─────────────────────┐
│ Feature: Auth       │ │ Feature: API        │ │ Feature: Database   │
│ ───────────────     │ │ ───────────────     │ │ ───────────────     │
│ PRD + Design Doc    │ │ PRD + Design Doc    │ │ PRD + Design Doc    │
└─────────────────────┘ └─────────────────────┘ └─────────────────────┘
              │                   │                   │
              ▼                   ▼                   ▼
┌─────────────────────┐ ┌─────────────────────┐ ┌─────────────────────┐
│ Stories (Parallel)  │ │ Stories (Parallel)  │ │ Stories (Parallel)  │
│ ─────────────────   │ │ ─────────────────   │ │ ─────────────────   │
│ □ JWT Implementation│ │ □ CRUD Endpoints    │ │ □ Schema Design     │
│ □ Password Hashing  │ │ □ Rate Limiting     │ │ □ Migrations        │
│ □ Session Management│ │ □ Input Validation  │ │ □ Connection Pool   │
└─────────────────────┘ └─────────────────────┘ └─────────────────────┘
                                  │
                                  ▼
                        ┌─────────────────┐
                        │ Quality Gates   │
                        │ ─────────────   │
                        │ ✓ DoR / DoD     │
                        │ ✓ Test Coverage │
                        │ ✓ Lint / Format │
                        └─────────────────┘
```

---

## Product Editions

Plan Cascade is available in three editions to suit different workflows:

| Feature | Plugin | Desktop | CLI |
|---------|--------|---------|-----|
| **Target User** | Claude Code users | Multi-model teams | Automation/CI |
| **LLM Backend** | Claude Code only | 7+ providers (Claude, OpenAI, DeepSeek, Ollama...) | 7+ providers |
| **Offline Use** | ❌ | ✅ (Ollama) | ✅ (Ollama) |
| **Installation** | `claude plugins install` | Desktop app / `pip install` | `pip install` |
| **UI** | Slash commands | Full GUI with 4 workflow modes | Command-line |
| **Quality Gates** | ✅ Standard | ✅ Enterprise-grade with auto-retry | ✅ |
| **Security Model** | Basic | 5-layer (Guardrail → Gate → Policy → Sandbox → Audit) | Basic |
| **Worktree Integration** | ✅ | ✅ Visual diff viewer | ✅ |
| **Visual Workflow** | ❌ | ✅ Real-time timeline + checkpoints | ❌ |
| **MCP Stack** | Client only | Full stack (Manager + Client + Server) | Client only |
| **Knowledge System** | ❌ | ✅ Skills + Memory + RAG | ❌ |
| **Remote Control** | ❌ | ✅ A2A protocol + Telegram bot | ❌ |
| **Maturity** | Stable | Alpha | Development |

### Which Edition Should I Choose?

- **Choose Plugin** if you're a Claude Code power user who wants seamless integration
- **Choose Desktop** if you need multi-model support, visual workflows, or offline capability
- **Choose CLI** if you're building automation pipelines or CI/CD integration

---

## Core Capabilities

### Unified Workflow Kernel

All modes share a common foundation:

- **Unified lifecycle** — Consistent state management across modes
- **Event streaming** — Real-time progress updates via typed events
- **Mode handoff** — Seamless switching between Chat → Plan → Task
- **Checkpointing** — Recovery from interruptions

### Quality Gates Pipeline

Every Story passes through validation:

```
┌─────────┐   ┌─────────┐   ┌─────────────┐   ┌─────────────┐   ┌─────────┐
│   DoR   │ → │  Code   │ → │     DoD     │ → │ AI Verify   │ → │ Review  │
│ (Ready) │   │ (Write) │   │   (Done)    │   │ (No Stubs)  │   │ (Score) │
└─────────┘   └─────────┘   └─────────────┘   └─────────────┘   └─────────┘
     │             │               │                 │               │
     ▼             ▼               ▼                 ▼               ▼
  Validate      Implement      Verify all        Detect stub     Code quality
  requirements   solution       criteria          code & TODOs      scoring
```

### Design Document Hierarchy

Two-level architecture ensures consistency:

- **Project-level** — Global patterns, shared decisions (ADR-001, ADR-002...)
- **Feature-level** — Component-specific decisions (ADR-F001, ADR-F002...)

### External Framework Skills

Auto-injected best practices from Git submodules:

- React/Next.js — detected via `package.json`
- Vue/Nuxt — detected via `package.json`
- Rust — detected via `Cargo.toml`

---

## Architecture

```
┌────────────────────────────────────────────────────────────────────────────┐
│                            Plan Cascade Core                               │
├────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │
│  │  Strategy   │  │    PRD      │  │  Parallel   │  │   Quality   │      │
│  │  Selector   │  │  Generator  │  │  Executor   │  │    Gates    │      │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘      │
├────────────────────────────────────────────────────────────────────────────┤
│                         Agent Backend Layer                                 │
│  ┌────────────────────────────┐  ┌────────────────────────────┐           │
│  │   ClaudeCodeBackend        │  │     BuiltinBackend         │           │
│  │   (subprocess, no API)     │  │   (direct API, ReAct loop) │           │
│  └────────────────────────────┘  └────────────────────────────┘           │
├────────────────────────────────────────────────────────────────────────────┤
│                           LLM Provider Layer                                │
│    Anthropic │ OpenAI │ DeepSeek │ Ollama │ GLM │ Qwen │ MiniMax          │
└────────────────────────────────────────────────────────────────────────────┘
```

---

## Quick Start

### Plugin (Stable)

```bash
# Install in Claude Code
claude plugins install plan-cascade

# Use slash commands
/plan-cascade:auto "Implement user authentication"
```

### CLI (Development)

```bash
# Requires Python 3.10+ and uv
git clone https://github.com/Taoidle/plan-cascade.git
cd plan-cascade
uv run pytest tests/  # Run tests

# CLI entry point
uv run plan-cascade --help
```

### Desktop (Alpha)

See [desktop/README.md](./desktop/README.md) for the full-featured desktop application.

---

## Documentation

| Document | Description |
|----------|-------------|
| [Plugin Guide](./docs/Plugin-Guide.md) | Claude Code plugin usage |
| [CLI Guide](./docs/CLI-Guide.md) | Command-line interface |
| [Mega Plan Guide](./docs/Mega-Plan-Guide.md) | Multi-feature orchestration |
| [Desktop README](./desktop/README.md) | Desktop application |
| [PRD Template](./docs/prd-template.json) | PRD file format |

---

## Project Structure

```
plan-cascade/
├── src/plan_cascade/          # Core Python library
│   ├── core/                  # Orchestration engines
│   ├── backends/              # Agent abstraction layer
│   ├── state/                 # Thread-safe state management
│   ├── llm/                   # LLM provider abstraction
│   └── tools/                 # ReAct tool implementations
├── desktop/                   # Tauri desktop application
│   ├── src/                   # React frontend
│   └── src-tauri/             # Rust backend
├── skills/                    # Plugin skills
│   ├── hybrid-ralph/          # PRD-driven execution
│   ├── mega-plan/             # Multi-feature orchestration
│   └── planning-with-files/   # File-based planning
├── commands/                  # Slash command definitions
└── mcp_server/               # FastMCP server
```

---

## Roadmap

| Component | Current | Next Milestone |
|-----------|---------|----------------|
| **Plugin** | 4.4.0 Stable | 5.0.0 - Enhanced CLI integration |
| **Desktop** | 0.1.0 Alpha | 0.2.0 - Beta with full workflow |
| **CLI** | Development | 1.0.0 - Stable release |
| **MCP Server** | Stable | Enhanced tool support |

---

## Contributing

We welcome contributions! Please see our contributing guidelines for details.

---

## License

MIT License - see [LICENSE](../LICENSE) for details.
