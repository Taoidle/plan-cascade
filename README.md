[中文版](README_zh.md)

<div align="center">

# Plan Cascade

**AI-Powered Cascading Development Framework**

*Decompose complex projects into parallel executable tasks with multi-agent collaboration*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-4.3.2-brightgreen)](https://github.com/Taoidle/plan-cascade)
[![Claude Code](https://img.shields.io/badge/Claude%20Code-Plugin-blue)](https://claude.ai/code)
[![MCP](https://img.shields.io/badge/MCP-Server-purple)](https://modelcontextprotocol.io)

| Component | Status |
|-----------|--------|
| Claude Code Plugin | ![Stable](https://img.shields.io/badge/status-stable-brightgreen) |
| MCP Server | ![Stable](https://img.shields.io/badge/status-stable-brightgreen) |
| Standalone CLI | ![In Development](https://img.shields.io/badge/status-in%20development-yellow) |
| Desktop App | ![In Development](https://img.shields.io/badge/status-in%20development-yellow) |

[Features](#features) • [Quick Start](#quick-start) • [Documentation](#documentation) • [Architecture](#architecture)

</div>

---

## Why Plan Cascade?

Traditional AI coding assistants struggle with large, complex projects. Plan Cascade solves this by:

- **Breaking down complexity** — Automatically decompose projects into manageable stories
- **Parallel execution** — Run independent tasks simultaneously with multiple agents
- **Maintaining context** — Design documents and PRDs keep AI focused on architecture
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

> **Note**: The desktop application is currently in active development. Stay tuned for updates.

Download from [GitHub Releases](https://github.com/Taoidle/plan-cascade/releases) (coming soon).

## Usage Examples

### Simple Task (Direct Execution)
```bash
/plan-cascade:auto "Fix the typo in the login button"
# → Executes directly without planning
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

## Documentation

| Document | Description |
|----------|-------------|
| [Plugin Guide](docs/Plugin-Guide.md) | Claude Code plugin usage |
| [CLI Guide](docs/CLI-Guide.md) | Standalone CLI usage |
| [Desktop Guide](docs/Desktop-Guide.md) | Desktop application |
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
└── desktop/                # Desktop app (Tauri + React)
```

### Supported LLM Backends

| Backend | API Key Required | Notes |
|---------|-----------------|-------|
| Claude Code | No | Default, via Claude Code CLI |
| Claude API | Yes | Direct Anthropic API |
| OpenAI | Yes | GPT-4o, etc. |
| DeepSeek | Yes | DeepSeek Chat/Coder |
| Ollama | No | Local models |

## What's New in v4.3.2

- **FORMAT Gate** — Auto-format code after story completion using ruff/prettier/cargo fmt/gofmt (PRE_VALIDATION phase)
- **AI Code Review Gate** — 5-dimension code review: Code Quality (25pts), Naming & Clarity (20pts), Complexity (20pts), Pattern Adherence (20pts), Security (15pts)
- **Three-Phase Gate Execution** — Gates now execute in PRE_VALIDATION → VALIDATION → POST_VALIDATION order
- **--no-review Flag** — Disable AI code review (enabled by default)
- **Gate Cache Invalidation** — Cache automatically invalidated after FORMAT gate modifies files

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
