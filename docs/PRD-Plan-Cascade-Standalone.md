[中文版](PRD-Plan-Cascade-Standalone_zh.md)

# Plan Cascade Desktop - Product Requirements Document (PRD)

**Version**: 5.0.0
**Date**: 2026-01-30
**Author**: Plan Cascade Team
**Status**: Complete

---

## Implementation Status Overview

> **Current Progress**: Desktop v5.0 Implementation Complete
> **Last Updated**: 2026-01-30

### Architecture Changes (v5.0)

| Change | Previous | New | Status |
|--------|----------|-----|--------|
| **Desktop Backend** | Python Sidecar (FastAPI) | Pure Rust Backend | ✅ Complete |
| **Dependency** | Requires Python 3.10+ | No Python required | ✅ Complete |
| **Distribution** | Complex (Python + Tauri) | Single executable | ✅ Complete |

### Feature Requirements Implementation Status

| Feature (Section) | Priority | Status | Notes |
|-------------------|----------|--------|-------|
| **4.1 Working Mode Selection** | P0 | ✅ Complete | |
| Standalone Orchestration Mode | P0 | ✅ Complete | `commands/standalone.rs` - 14 commands |
| Claude Code GUI Mode | P0 | ✅ Complete | `commands/claude_code.rs` - 7 commands |
| **4.2 Multi-Agent Collaboration** | P0 | ✅ Complete | |
| Phase-based Agent Assignment | P0 | ✅ Complete | CLI: `backends/phase_config.py` |
| Agent Executor | P0 | ✅ Complete | `services/agent_executor.rs` |
| **4.3 Simple Mode Features** | P0 | ✅ Complete | |
| One-click Workflow | P0 | ✅ Complete | CLI: `core/simple_workflow.py` |
| AI Auto Strategy Determination | P0 | ✅ Complete | CLI: `core/strategy_analyzer.py` |
| **4.4 Expert Mode Features** | P0 | ✅ Complete | |
| PRD Editor | P0 | ✅ Complete | CLI: `core/expert_workflow.py` |
| Execution Strategy Selection | P0 | ✅ Complete | direct/hybrid/mega |
| Agent Specification | P0 | ✅ Complete | Each Story can specify Agent |
| **4.5 Settings Page** | P0 | ✅ Complete | |
| Agent Configuration | P0 | ✅ Complete | `commands/settings.rs` |
| Quality Gate Configuration | P0 | ✅ Complete | `commands/quality_gates.rs` |
| API Key Secure Storage | P0 | ✅ Complete | Rust keyring integration |
| **4.6 CLI Features** | P1 | ✅ Complete | |
| `plan-cascade run` | P0 | ✅ Complete | Simple/expert mode |
| `plan-cascade config` | P0 | ✅ Complete | Configuration wizard |
| `plan-cascade status` | P1 | ✅ Complete | Status viewing |
| **4.7 Interactive REPL Mode** | P0 | ✅ Complete | |
| REPL Loop | P0 | ✅ Complete | `plan-cascade chat` |
| Special Commands | P0 | ✅ Complete | /exit, /clear, /status, /mode |
| **4.8 Project & Session Management** | P0 | ✅ Complete | |
| Visual Project Browser | P0 | ✅ Complete | `commands/projects.rs` - 3 commands |
| Session History | P0 | ✅ Complete | `commands/sessions.rs` - 4 commands |
| Smart Search | P1 | ✅ Complete | search_projects, search_sessions |
| **4.9 CC Agents** | P1 | ✅ Complete | |
| Custom AI Agents | P1 | ✅ Complete | `commands/agents.rs` - 14 commands |
| Agent Library | P1 | ✅ Complete | SQLite-backed registry |
| Background Execution | P1 | ✅ Complete | run_agent command |
| **4.10 Usage Analytics Dashboard** | P1 | ✅ Complete | |
| Cost Tracking | P1 | ✅ Complete | `commands/analytics.rs` - 22 commands |
| Token Analytics | P1 | ✅ Complete | aggregate_by_model, aggregate_by_project |
| Visual Charts | P2 | ✅ Complete | get_time_series, get_dashboard_summary |
| **4.11 MCP Server Management** | P1 | ✅ Complete | |
| Server Registry | P1 | ✅ Complete | `commands/mcp.rs` - 7 commands |
| Connection Testing | P1 | ✅ Complete | test_mcp_server |
| Claude Desktop Import | P2 | ✅ Complete | import_from_claude_desktop |
| **4.12 Timeline & Checkpoints** | P2 | ✅ Complete | |
| Session Versioning | P2 | ✅ Complete | `commands/timeline.rs` - 15 commands |
| Visual Timeline | P2 | ✅ Complete | get_timeline |
| Diff Viewer | P2 | ✅ Complete | get_checkpoint_diff, get_diff_from_current |
| **4.13 CLAUDE.md Management** | P1 | ✅ Complete | |
| Built-in Editor | P1 | ✅ Complete | `commands/markdown.rs` - 5 commands |
| Live Preview | P1 | ✅ Complete | read_claude_md |
| Project Scanner | P1 | ✅ Complete | scan_claude_md |
| **4.14 Real-time Streaming Chat** | P0 | ✅ Complete | |
| Streaming Response | P0 | ✅ Complete | Tauri events: standalone-event |
| Thinking Display | P0 | ✅ Complete | thinking_start/delta/end events |
| Typing Animation | P1 | ✅ Complete | text_delta streaming |
| **4.15 Tool Call Visualization** | P0 | ✅ Complete | |
| Tool State Display | P0 | ✅ Complete | tool_start, tool_result events |
| File Change Preview | P0 | ✅ Complete | Timeline diff commands |
| Tool History | P1 | ✅ Complete | Session tracking |
| **4.16 Chat Interaction Features** | P0 | ✅ Complete | |
| Markdown Rendering | P0 | ✅ Complete | Frontend components |
| Code Block Actions | P0 | ✅ Complete | Copy, line numbers |
| Drag & Drop Files | P1 | ✅ Complete | File attachment support |
| @ File Reference | P1 | ✅ Complete | File mention support |
| **4.17 Session Control** | P0 | ✅ Complete | |
| Interrupt/Cancel | P0 | ✅ Complete | cancel_standalone_execution |
| Regenerate Response | P0 | ✅ Complete | resume_standalone_execution |
| Edit & Resend | P1 | ✅ Complete | Session commands |
| Branch Conversation | P2 | ✅ Complete | fork_branch in timeline |
| **4.18 Command Palette** | P1 | ✅ Complete | |
| Quick Commands | P1 | ✅ Complete | 60+ global commands |
| Fuzzy Search | P1 | ✅ Complete | Frontend component |
| **4.19 Quality Gates Auto-Detection** | P0 | ✅ Complete | |
| Project Type Detection | P0 | ✅ Complete | `commands/quality_gates.rs` - 13 commands |
| Run Quality Gates | P0 | ✅ Complete | Node.js/Python/Rust/Go support |
| Custom Gates | P1 | ✅ Complete | run_custom_gates |
| **4.20 Git Worktree Support** | P0 | ✅ Complete | |
| Create Worktree | P0 | ✅ Complete | `commands/worktree.rs` - 6 commands |
| Complete Worktree | P0 | ✅ Complete | Commit, merge, cleanup |
| List Worktrees | P1 | ✅ Complete | list_worktrees |
| **4.21 Real-Time File Watching** | P1 | ✅ Complete | |
| notify crate Integration | P1 | ✅ Complete | `services/sync/` |
| File Change Events | P1 | ✅ Complete | Tauri events |
| **4.22 TypeScript API Wrappers** | P0 | ✅ Complete | |
| All Commands Wrapped | P0 | ✅ Complete | `src/lib/api/` - 115 commands |
| Type Safety | P0 | ✅ Complete | Full TypeScript types |
| Documentation | P0 | ✅ Complete | JSDoc comments |

### Product Form Implementation Status

| Form | Status | Notes |
|------|--------|-------|
| CLI | ✅ Complete | `pip install plan-cascade` |
| Desktop (GUI) | ✅ Complete | Pure Rust backend - 115 Tauri commands |
| Claude Code Plugin | ✅ Complete | Existing Plugin maintains compatibility |

### Milestone Progress

| Phase | Status | Completion |
|-------|--------|------------|
| Phase 1: CLI + Dual-Mode | ✅ Complete | 100% |
| Phase 2: Desktop Rust Backend | ✅ Complete | 100% |
| Phase 3: Core Desktop Features | ✅ Complete | 100% |
| Phase 4: Advanced Features | ✅ Complete | 100% |

### Implementation Summary

**Total Tauri Commands**: 115
- Initialization: 2
- Health: 1
- Settings: 2
- Projects: 3
- Sessions: 4
- Agents: 14
- Analytics: 22
- Quality Gates: 13
- Worktree: 6
- Standalone: 14
- Timeline: 15
- MCP: 7
- Markdown: 5
- Claude Code: 7

---

## 1. Overview

### 1.1 Product Vision

Develop Plan Cascade Desktop into a **complete AI programming orchestration platform** with:
- **Pure Rust backend** for optimal performance and easy distribution
- **Comprehensive project management** for Claude Code workflows
- **Advanced analytics and monitoring** capabilities

**Core Positioning**:
- As a **complete orchestration layer**: Execute tools itself, LLM only provides thinking (standalone mode)
- As a **graphical interface for Claude Code**: Compatible with all Claude Code features (GUI mode)
- As a **project management hub**: Manage projects, sessions, agents, and MCP servers
- Support **multiple LLM backends**: Claude Max, Claude API, OpenAI, DeepSeek, etc.

### 1.2 Core Value Propositions

| Value Point | Description |
|-------------|-------------|
| **Zero Dependencies** | Single executable, no Python or other runtime required |
| **Complete Orchestration** | Autonomously executes tools (Read/Write/Edit/Bash/Glob/Grep) |
| **Project Hub** | Central management for all Claude Code projects and sessions |
| **Agent Library** | Create and manage custom AI agents for different tasks |
| **Usage Insights** | Track costs, tokens, and usage patterns across projects |
| **MCP Integration** | Manage Model Context Protocol servers from a unified UI |
| **Session Timeline** | Version control for coding sessions with checkpoints |
| **Claude Code Compatible** | Serves as complete GUI for Claude Code |

### 1.3 Product Positioning

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      Plan Cascade Desktop v5.0                           │
│              Complete AI Programming Orchestration Platform              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─ Core Modules ──────────────────────────────────────────────────────┐│
│   │                                                                    ││
│   │  📁 Projects    🤖 Agents    📊 Analytics    🔌 MCP    ⏰ Timeline ││
│   │  └─ Browser     └─ Library   └─ Dashboard    └─ Servers └─ Checkpoints│
│   │  └─ Sessions    └─ Runner    └─ Cost Track   └─ Config  └─ Branches ││
│   │  └─ Search      └─ History   └─ Export       └─ Health  └─ Diff     ││
│   │                                                                    ││
│   └────────────────────────────────────────────────────────────────────┘│
│                                                                          │
│   ┌─ Working Mode Selection ────────────────────────────────────────────┐│
│   │                                                                    ││
│   │  ● Claude Code GUI Mode (Recommended)                              ││
│   │    └─ Plan Cascade as graphical interface for Claude Code          ││
│   │    └─ Claude Code CLI executes tools                               ││
│   │    └─ Full compatibility with Claude Code features                 ││
│   │                                                                    ││
│   │  ○ Standalone Orchestration Mode                                   ││
│   │    └─ Plan Cascade executes all tools itself                       ││
│   │    └─ Direct LLM API calls (Claude/OpenAI/DeepSeek/Ollama)        ││
│   │    └─ No Claude Code dependency                                    ││
│   │                                                                    ││
│   └────────────────────────────────────────────────────────────────────┘│
│                                                                          │
│   ┌─ Architecture ──────────────────────────────────────────────────────┐│
│   │                                                                    ││
│   │   React Frontend (TypeScript)                                      ││
│   │         │                                                          ││
│   │         ▼                                                          ││
│   │   Tauri IPC                                                        ││
│   │         │                                                          ││
│   │         ▼                                                          ││
│   │   Rust Backend ─────────────────────────────────────────────────── ││
│   │   │ • Project Manager    • Analytics Tracker   • Timeline Manager │││
│   │   │ • Agent Executor     • MCP Registry        • Markdown Editor  │││
│   │   │ • Claude Code CLI    • LLM Providers       • Tool Execution   │││
│   │   └─────────────────────────────────────────────────────────────── ││
│   │         │                                                          ││
│   │         ▼                                                          ││
│   │   Storage Layer                                                    ││
│   │   │ • SQLite (history, analytics)  • Keyring (secrets)            │││
│   │   │ • File System (projects)       • JSON Config                  │││
│   │   └─────────────────────────────────────────────────────────────── ││
│   │                                                                    ││
│   └────────────────────────────────────────────────────────────────────┘│
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 1.4 Target Users

| User Group | Scenario | Pain Point | Solution |
|------------|----------|------------|----------|
| **Claude Code Users** | Have Claude Code installed | CLI is powerful but lacks visual management | Desktop GUI with project browser |
| **Claude Max Members** | Have Max subscription | Want visual interface without API costs | Claude Code GUI mode |
| **Power Users** | Multiple projects, heavy usage | Need usage tracking and cost analysis | Analytics dashboard |
| **Team Leads** | Manage multiple agents/workflows | No central management for agents | Agent library |
| **MCP Users** | Use multiple MCP servers | Configuration scattered, hard to manage | MCP server registry |

---

## 2. Core Design Philosophy

### 2.1 Pure Rust Architecture

**Design Principle**: Single executable, zero runtime dependencies.

```
Previous Architecture (v4.x):
┌─────────────────┐     HTTP/WS      ┌─────────────────┐
│  Tauri + React  │ ◄──────────────► │  Python Sidecar │
│   (Frontend)    │                  │   (FastAPI)     │
└─────────────────┘                  └─────────────────┘
       │                                    │
       │                                    ▼
       │                             Python 3.10+ required
       ▼                             pip install dependencies
  Single binary                      Complex distribution

New Architecture (v5.0):
┌─────────────────────────────────────────────────────┐
│              Tauri Desktop Application               │
├─────────────────────────────────────────────────────┤
│  React Frontend  │      Rust Backend                │
│  (TypeScript)    │      (Native Code)               │
│                  │      • All business logic        │
│                  │      • SQLite embedded           │
│                  │      • HTTP client for LLM APIs  │
│                  │      • Process spawning for CLI  │
└─────────────────────────────────────────────────────┘
                    │
                    ▼
              Single executable
              No Python required
              Easy distribution
```

**Benefits**:
- Users download one file, run immediately
- No dependency conflicts
- Better performance (native code)
- Smaller distribution size
- Easier auto-update

### 2.2 Dual-Mode Design

Both modes share the same core features (Projects, Agents, Analytics, MCP, Timeline, CLAUDE.md).

#### Claude Code GUI Mode (Recommended)

For: Users with Claude Code installed

```
┌─ Claude Code GUI Mode ──────────────────────────────────────────────────┐
│                                                                          │
│   Plan Cascade Desktop                                                   │
│   ┌────────────────────────────────────────────────────────────────┐    │
│   │  📁 Projects │ 🤖 Agents │ 📊 Analytics │ 🔌 MCP │ ⏰ Timeline │    │
│   └────────────────────────────────────────────────────────────────┘    │
│                              │                                           │
│                              ▼                                           │
│   ┌────────────────────────────────────────────────────────────────┐    │
│   │                     Chat Interface                              │    │
│   │   ┌──────────────────────────────────────────────────────────┐ │    │
│   │   │ User: Help me implement user authentication               │ │    │
│   │   │                                                          │ │    │
│   │   │ Claude: I'll help you implement authentication...        │ │    │
│   │   │ [Tool Call: Read src/auth.ts]                            │ │    │
│   │   │ [Tool Call: Edit src/auth.ts]                            │ │    │
│   │   └──────────────────────────────────────────────────────────┘ │    │
│   │                                                                 │    │
│   │   [Type your message...]                          [Send]        │    │
│   └────────────────────────────────────────────────────────────────┘    │
│                              │                                           │
│                              ▼                                           │
│                    Claude Code CLI                                       │
│                    (claude --output-format stream-json)                  │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

#### Standalone Orchestration Mode

For: Users without Claude Code or wanting to use other LLMs

```
┌─ Standalone Mode ───────────────────────────────────────────────────────┐
│                                                                          │
│   LLM Backend:  [Claude API ▼]  Model: [claude-sonnet-4-20250514 ▼]      │
│                                                                          │
│   ┌────────────────────────────────────────────────────────────────┐    │
│   │                     Execution Interface                         │    │
│   │                                                                 │    │
│   │   Describe your task:                                           │    │
│   │   ┌──────────────────────────────────────────────────────────┐ │    │
│   │   │ Implement user authentication with OAuth support          │ │    │
│   │   └──────────────────────────────────────────────────────────┘ │    │
│   │                                                                 │    │
│   │   [████████████░░░░░░░░] 60%                                   │    │
│   │                                                                 │    │
│   │   ✓ Analyzed project structure                                  │    │
│   │   ✓ Generated PRD (5 stories)                                   │    │
│   │   ⟳ Executing: Implement OAuth provider                         │    │
│   │   ○ Pending: Add session management                             │    │
│   │                                                                 │    │
│   └────────────────────────────────────────────────────────────────┘    │
│                              │                                           │
│                              ▼                                           │
│                    Direct LLM API Calls                                  │
│                    Built-in Tool Execution                               │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Product Forms

### 3.1 Unified Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Plan Cascade                                     │
├───────────────────┬───────────────────┬─────────────────────────────────┤
│   Desktop (GUI)   │      CLI          │      Claude Code Plugin         │
├───────────────────┼───────────────────┼─────────────────────────────────┤
│ • Pure Rust       │ • Python package  │ • Depends on Claude Code        │
│   backend         │ • pip install     │ • Runs as plugin                │
│ • Single exe      │   plan-cascade    │ • Slash command invocation      │
│ • All features    │ • Simple/Expert   │                                 │
│   included        │   modes           │                                 │
│ • No dependencies │ • Interactive     │                                 │
│                   │   REPL            │                                 │
└───────────────────┴───────────────────┴─────────────────────────────────┘
```

### 3.2 Release Artifacts

| Artifact | Description | Target Users |
|----------|-------------|--------------|
| **Desktop (Windows)** | `.msi` / `.exe` installer | Windows users |
| **Desktop (macOS)** | `.dmg` / `.app` bundle | macOS users |
| **Desktop (Linux)** | `.AppImage` / `.deb` | Linux users |
| **CLI** | `pip install plan-cascade` | Developers preferring CLI |
| **Claude Code Plugin** | Existing Plugin | Claude Code power users |

---

## 4. Feature Requirements

### 4.1 Working Mode Selection (P0)

#### Claude Code GUI Mode (Recommended)

Plan Cascade as graphical interface for Claude Code:

```
┌─ Settings ───────────────────────────────────────────────────┐
│                                                              │
│  Working Mode:                                               │
│                                                              │
│  ● Claude Code GUI Mode (Recommended)                        │
│    └─ Requires Claude Code CLI installed                     │
│    └─ Claude Code executes all tools                         │
│    └─ Full compatibility with Claude Code features           │
│    └─ Automatic session tracking                             │
│                                                              │
│  ○ Standalone Orchestration Mode                             │
│    └─ No Claude Code required                                │
│    └─ Plan Cascade executes tools directly                   │
│    └─ Requires LLM API key configuration                     │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

#### Standalone Mode: LLM Backend Selection

```
┌─ LLM Backend Selection ───────────────────────────────────────┐
│                                                              │
│  ● Claude API                                                │
│    └─ Direct Anthropic API calls                             │
│    └─ Requires API Key                          [Configure]  │
│                                                              │
│  ○ OpenAI                                                    │
│    └─ GPT-4o and other models                               │
│    └─ Requires API Key                          [Configure]  │
│                                                              │
│  ○ DeepSeek                                                  │
│    └─ Cost-effective alternative                            │
│    └─ Requires API Key                          [Configure]  │
│                                                              │
│  ○ Ollama                                                    │
│    └─ Local models, completely offline                       │
│    └─ Requires Ollama running locally           [Configure]  │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### 4.2 Multi-Agent Collaboration (P0)

Plan Cascade supports multiple agents working together, intelligently assigning different tasks to the most suitable agent.

#### Supported Execution Agents

| Agent | Type | Description |
|-------|------|-------------|
| claude-code | Task Tool / CLI | Default agent, built-in or via Claude Code CLI |
| codex | CLI | OpenAI Codex CLI |
| aider | CLI | AI pair programming assistant |
| amp-code | CLI | Amp Code CLI |
| cursor-cli | CLI | Cursor CLI |

#### Phase-Based Agent Assignment

| Phase | Default Agent | Fallback Chain | Story Type Override |
|-------|--------------|----------------|---------------------|
| Planning | codex | claude-code | - |
| Implementation | claude-code | codex, aider | bugfix→codex, refactor→aider |
| Retry | claude-code | aider | - |
| Refactor | aider | claude-code | - |
| Review | claude-code | codex | - |

#### Agent Resolution Priority

```
1. --agent command parameter (explicit override)
2. Phase-specific parameters (--impl-agent, --planning-agent)
3. Story-level agent field in PRD
4. Story type override (bugfix → codex, refactor → aider)
5. Phase default agent
6. Fallback chain (if agent unavailable)
7. claude-code (ultimate fallback, always available)
```

### 4.3 Simple Mode Features (P0)

#### One-Click Workflow

```bash
# CLI
plan-cascade "Implement user login with OAuth"
# Auto: analyze → generate plan → execute → quality check

# GUI
# Enter description → Click "Start" → Wait for completion
```

#### Simplified Progress Display

```
Executing...

[████████████░░░░░░░░] 60%

✓ Generated plan (5 tasks)
✓ Database Schema
✓ API route structure
⟳ OAuth login (executing...)
○ SMS verification login
○ Integration tests
```

### 4.4 Expert Mode Features (P0)

#### PRD Editor

- Visual editing of Stories
- Drag-and-drop reordering
- Set dependency relationships
- Specify execution Agent per Story

#### Execution Strategy Selection

```
Execution Strategy:                    AI Suggestion: Hybrid Auto
○ Direct (simple task, no PRD)
● Hybrid Auto (auto-generate PRD and execute)
○ Mega Plan (large project, multiple PRDs)

Isolation Options:
□ Use Git Worktree for isolated development
```

#### Agent Specification

Each Story can specify a different Agent:

```
┌─ Story: Implement OAuth Login ───────────────────────────────────┐
│                                                                  │
│  Agent: [claude-code ▼]                                          │
│         ├─ claude-code (recommended)                             │
│         ├─ aider                                                 │
│         ├─ codex                                                 │
│         └─ builtin                                               │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### 4.5 Git Worktree Support (P0) - CORE FEATURE

Git Worktree provides isolated development environments for features, preventing interference with main codebase.

#### Hybrid Worktree Workflow

```
┌─ Hybrid Worktree Mode ───────────────────────────────────────────┐
│                                                                  │
│  /plan-cascade:hybrid-worktree feature-auth main "User auth"    │
│                                                                  │
│  Actions:                                                        │
│  1. Create Git branch: feature-auth                             │
│  2. Create Worktree: .worktrees/feature-auth/                   │
│  3. Initialize: .planning-config.json                           │
│  4. Generate/Load PRD                                           │
│  5. Execute Stories (parallel agents)                           │
│  6. On completion:                                               │
│     - Commit code (exclude planning files)                      │
│     - Merge to target branch (main)                             │
│     - Remove Worktree                                           │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

#### Worktree Configuration File

```json
// .planning-config.json
{
  "mode": "hybrid",
  "task_name": "feature-auth",
  "task_branch": "feature-auth",
  "target_branch": "main",
  "created_at": "2026-01-30T10:00:00Z"
}
```

#### Use Cases

| Type | Scenario | Example |
|------|----------|---------|
| ✅ Suitable | Feature with multiple subtasks | User auth (register + login + reset) |
| ✅ Suitable | Experimental feature requiring isolation | New payment integration test |
| ✅ Suitable | Medium-scale refactoring (5-20 files) | API layer unified error handling |
| ❌ Not suitable | Simple single-file modification | Modify a component's style |
| ❌ Not suitable | Quick prototype validation | Verify if a library works |

### 4.6 Mega Plan Execution (P0) - CORE FEATURE

Mega Plan orchestrates large projects with multiple related feature modules.

#### Sequential Batch Execution

```
mega-approve (1st) → Start Batch 1
    ├─ Create Worktrees from current branch
    ├─ Generate PRDs for each feature (Task agents)
    ├─ Execute all stories (Task agents)
    ↓ Batch 1 complete
mega-approve (2nd) → Merge Batch 1 → Create Batch 2 from updated branch
    ↓ Batch 2 complete
mega-approve (3rd) → Merge Batch 2 → ...
    ↓ All batches complete
mega-complete → Clean up planning files
```

#### Mega Plan Structure

```json
// mega-plan.json
{
  "metadata": {
    "version": "1.0",
    "created_at": "2026-01-30T10:00:00Z"
  },
  "goal": "Build e-commerce platform",
  "description": "Complete platform with users, products, cart, orders",
  "target_branch": "main",
  "execution_mode": "auto",
  "features": [
    {
      "id": "feature-users",
      "name": "User System",
      "description": "User registration, login, profile management",
      "priority": 1,
      "dependencies": [],
      "status": "pending"
    },
    {
      "id": "feature-products",
      "name": "Product System",
      "description": "Product CRUD, categories, search",
      "priority": 1,
      "dependencies": [],
      "status": "pending"
    },
    {
      "id": "feature-orders",
      "name": "Order System",
      "description": "Shopping cart, checkout, order management",
      "priority": 2,
      "dependencies": ["feature-users", "feature-products"],
      "status": "pending"
    }
  ]
}
```

#### Full Automation with --auto-prd

With `--auto-prd`, mega-approve runs the ENTIRE mega-plan automatically:
1. Creates worktrees for current batch
2. Generates PRDs for each feature (via Task agents)
3. Executes all stories (via Task agents)
4. Monitors until batch complete
5. Merges batch to target branch
6. Automatically continues to next batch
7. Only pauses on errors or merge conflicts

### 4.7 Dependency Analysis & Visualization (P0)

#### Automatic Batch Generation

Stories are automatically grouped into batches based on dependencies:

```
Batch 1: [Story A, Story B, Story C]  ← No dependencies, parallel execution
           ↓ All complete
Batch 2: [Story D, Story E]           ← Depend on Batch 1, parallel execution
           ↓ All complete
Batch 3: [Story F]                    ← Depends on Batch 2
```

#### Dependency Graph Visualization

```
/plan-cascade:show-dependencies

┌─ Dependency Graph ─────────────────────────────────────────────────┐
│                                                                    │
│   story-001 (Database Schema)                                      │
│       │                                                            │
│       ├──→ story-002 (API Routes)                                  │
│       │        │                                                   │
│       │        └──→ story-004 (Frontend Forms)                     │
│       │                                                            │
│       └──→ story-003 (Email Service)                               │
│                │                                                   │
│                └──→ story-005 (Integration Tests)                  │
│                                                                    │
│   Execution Batches:                                               │
│   Batch 1: story-001                                               │
│   Batch 2: story-002, story-003                                    │
│   Batch 3: story-004, story-005                                    │
│                                                                    │
│   ⚠️ Issues Detected:                                              │
│   • None                                                           │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

#### Circular Dependency Detection

The system automatically detects and reports circular dependencies:

```
⚠️ Circular Dependency Detected!

story-002 → story-004 → story-005 → story-002

Please edit the PRD to resolve this issue.
```

### 4.8 Auto-Iteration System (P0)

#### Iteration Modes

| Mode | Description |
|------|-------------|
| `until_complete` | Continue until all Stories complete (default) |
| `max_iterations` | Stop after at most N iterations |
| `batch_complete` | Stop after completing current batch only |

#### Iteration Configuration

```json
// In prd.json
{
  "iteration_config": {
    "mode": "until_complete",
    "max_iterations": 50,
    "poll_interval_seconds": 10,
    "batch_timeout_seconds": 3600,
    "quality_gates_enabled": true,
    "auto_retry_enabled": true
  }
}
```

#### Iteration Flow

```
Start Auto-Iteration
    │
    ├─→ Initialize iteration state
    │
    ├─→ Main Loop:
    │       │
    │       ├─ Get current batch stories
    │       ├─ Start agents in parallel
    │       ├─ Poll for completion (10s intervals)
    │       ├─ Run quality gates (if enabled)
    │       ├─ Handle failures + retries
    │       ├─ Check completion condition
    │       └─ Advance to next batch
    │
    ├─→ Save final state
    │
    └─→ Generate execution report
```

### 4.9 Quality Gates with Auto-Detection (P0)

#### Automatic Project Type Detection

Quality gates automatically detect project type and select appropriate tools:

| Project Type | Detection | TypeCheck | Test | Lint |
|--------------|-----------|-----------|------|------|
| Node.js | package.json | tsc | jest, npm test | eslint |
| Python | pyproject.toml, setup.py | mypy, pyright | pytest | ruff, flake8 |
| Rust | Cargo.toml | cargo check | cargo test | clippy |
| Go | go.mod | go vet | go test | golangci-lint |

#### Quality Gate Configuration

```json
// In prd.json
{
  "quality_gates": {
    "enabled": true,
    "gates": [
      {
        "name": "typecheck",
        "type": "typecheck",
        "enabled": true,
        "required": true,
        "timeout_seconds": 300
      },
      {
        "name": "tests",
        "type": "test",
        "enabled": true,
        "required": true,
        "command_override": "npm test -- --coverage"
      },
      {
        "name": "lint",
        "type": "lint",
        "enabled": true,
        "required": false
      },
      {
        "name": "custom",
        "type": "custom",
        "enabled": false,
        "script": "./scripts/validate.sh"
      }
    ]
  }
}
```

#### Retry Management

```json
// In prd.json
{
  "retry_config": {
    "max_retries": 3,
    "exponential_backoff": true,
    "base_delay_seconds": 5,
    "inject_failure_context": true,
    "switch_agent_on_retry": false
  }
}
```

### 4.10 State File System (P1)

#### State Files Overview

| File | Type | Description |
|------|------|-------------|
| `prd.json` | Planning | PRD document with goals, stories, dependencies |
| `mega-plan.json` | Planning | Project-level plan managing multiple features |
| `agents.json` | Config | Agent configuration with phase defaults |
| `findings.md` | Shared | Agent findings record, supports tag filtering |
| `progress.txt` | Shared | Progress timeline with agent execution info |
| `.agent-status.json` | State | Agent running/completed/failed status |
| `.iteration-state.json` | State | Auto-iteration progress and batch results |
| `.retry-state.json` | State | Retry history and failure records |
| `.mega-status.json` | State | Mega-plan execution state |
| `.planning-config.json` | Config | Worktree task configuration |

#### Progress Markers

```
# progress.txt markers
[COMPLETE] story-001          # Story completed (Hybrid mode)
[STORY_COMPLETE] story-001    # Story completed (Mega mode)
[FEATURE_COMPLETE] feature-1  # Feature completed
[PRD_COMPLETE] feature-1      # PRD generation completed
[FAILED] story-001            # Story failed
```

#### Mega-Status Structure

```json
// .mega-status.json
{
  "current_batch": 2,
  "completed_batches": [1],
  "features": {
    "feature-users": {
      "status": "completed",
      "worktree": ".worktrees/feature-users",
      "prd_generated": true,
      "stories_completed": 5,
      "stories_total": 5
    },
    "feature-orders": {
      "status": "in_progress",
      "worktree": ".worktrees/feature-orders",
      "prd_generated": true,
      "stories_completed": 2,
      "stories_total": 4
    }
  }
}
```

### 4.11 Settings Page (P0)

#### Agent Configuration

```
┌─ Settings > Agent Configuration ─────────────────────────────────┐
│                                                                  │
│  Main Backend (for orchestration)                                │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ ● Claude Code (recommended, no config needed)              │ │
│  │ ○ Claude API    [API Key: ••••••••••]                      │ │
│  │ ○ OpenAI        [API Key: ••••••••••] [Model: gpt-4o ▼]    │ │
│  │ ○ DeepSeek      [API Key: ••••••••••]                      │ │
│  │ ○ Ollama        [URL: http://localhost:11434]              │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ─────────────────────────────────────────────────────────────── │
│                                                                  │
│  Execution Agents (for Story execution)             [+ Add]      │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ ✓ claude-code                              [Default]       │ │
│  │   └─ Path: claude                                          │ │
│  ├────────────────────────────────────────────────────────────┤ │
│  │ ✓ aider                                    [Configure]     │ │
│  │   └─ Command: aider --model gpt-4o                         │ │
│  ├────────────────────────────────────────────────────────────┤ │
│  │ □ codex (not configured)                   [Configure]     │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Agent Selection Strategy:                                       │
│  ○ Smart matching (auto-select based on task type)              │
│  ● Prefer: [claude-code ▼]                                      │
│  ○ Manual (select for each Story)                               │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### 4.12 CLI Features (P1)

```bash
# Simple mode (default)
plan-cascade "Implement user login"
# Auto-completes entire flow

# Expert mode
plan-cascade --expert "Implement user login"

# Expert mode interaction
$ plan-cascade --expert "Implement user login"
✓ Generated PRD (5 Stories)

? Select operation:
  > view    - View PRD
    edit    - Edit PRD
    agent   - Specify Agent
    run     - Start execution
    save    - Save draft
    quit    - Exit

# Step-by-step commands
plan-cascade generate "Implement user login"  # Generate PRD only
plan-cascade review                           # Interactive edit
plan-cascade run                              # Execute
plan-cascade status                           # View status

# Resume commands
plan-cascade resume                           # Auto-detect and resume
```

### 4.13 Interactive REPL Mode (P0)

CLI and Desktop both support interactive REPL for continuous conversation:

```
┌─ Plan Cascade REPL ──────────────────────────────────────────────┐
│                                                                  │
│  plan-cascade> Analyze the project structure                     │
│                                                                  │
│  [AI analyzes and responds...]                                   │
│                                                                  │
│  plan-cascade> Based on above analysis, implement user login     │
│                                                                  │
│  [Intent recognition: TASK]                                      │
│  [Strategy analysis: hybrid_auto]                                │
│  [Generating PRD...]                                             │
│  [Executing...]                                                  │
│                                                                  │
│  plan-cascade> /status                                           │
│  Session: abc123                                                 │
│  Mode: simple                                                    │
│  Project: /path/to/project                                       │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

**REPL Special Commands**:
- `/exit`, `/quit` - Exit
- `/clear` - Clear context
- `/status` - View session status
- `/mode [simple|expert]` - Switch mode
- `/history` - View conversation history
- `/config` - Configuration management

### 4.8 Project & Session Management (P0) - NEW

Visual management for Claude Code projects and sessions.

```
┌─ Projects ───────────────────────────────────────────────────────────────┐
│                                                                          │
│  🔍 Search projects...                              [⚙️] [➕ New Project] │
│                                                                          │
│  ┌─ Recent Projects ────────────────────────────────────────────────────┐│
│  │                                                                      ││
│  │  📁 my-web-app                                    Last: 2h ago       ││
│  │     /Users/dev/projects/my-web-app                                   ││
│  │     12 sessions • 1,234 messages                                     ││
│  │                                                      [Open] [⋮]      ││
│  │  ────────────────────────────────────────────────────────────────── ││
│  │  📁 api-service                                   Last: Yesterday    ││
│  │     /Users/dev/projects/api-service                                  ││
│  │     8 sessions • 567 messages                                        ││
│  │                                                      [Open] [⋮]      ││
│  │  ────────────────────────────────────────────────────────────────── ││
│  │  📁 mobile-app                                    Last: 3 days ago   ││
│  │     /Users/dev/projects/mobile-app                                   ││
│  │     5 sessions • 234 messages                                        ││
│  │                                                      [Open] [⋮]      ││
│  │                                                                      ││
│  └──────────────────────────────────────────────────────────────────────┘│
│                                                                          │
│  ┌─ Session History (my-web-app) ───────────────────────────────────────┐│
│  │                                                                      ││
│  │  💬 "Help me implement user authentication"       Jan 30, 14:23     ││
│  │     45 messages • 3 checkpoints                    [Resume] [⋮]     ││
│  │                                                                      ││
│  │  💬 "Fix the login bug on mobile"                 Jan 29, 10:15     ││
│  │     23 messages • 1 checkpoint                     [Resume] [⋮]     ││
│  │                                                                      ││
│  │  💬 "Add dark mode support"                       Jan 28, 16:45     ││
│  │     67 messages • 5 checkpoints                    [Resume] [⋮]     ││
│  │                                                                      ││
│  └──────────────────────────────────────────────────────────────────────┘│
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

**Features**:
- Browse all projects in `~/.claude/projects/`
- View session history with first message preview
- Resume past sessions with full context
- Search across projects and sessions
- Session metadata (timestamps, message counts)

### 4.9 CC Agents (P1) - NEW

Create and manage custom AI agents with specialized behaviors.

```
┌─ Agent Library ──────────────────────────────────────────────────────────┐
│                                                                          │
│  [➕ Create Agent]                        🔍 Search agents...            │
│                                                                          │
│  ┌─ My Agents ──────────────────────────────────────────────────────────┐│
│  │                                                                      ││
│  │  🤖 Code Reviewer                                                    ││
│  │     Reviews code for bugs, security issues, and best practices       ││
│  │     Model: claude-sonnet-4-20250514 • Runs: 45                                   ││
│  │                                              [Run] [Edit] [⋮]        ││
│  │  ────────────────────────────────────────────────────────────────── ││
│  │  🤖 Test Writer                                                      ││
│  │     Generates comprehensive unit tests for your code                 ││
│  │     Model: claude-sonnet-4-20250514 • Runs: 23                                   ││
│  │                                              [Run] [Edit] [⋮]        ││
│  │  ────────────────────────────────────────────────────────────────── ││
│  │  🤖 Documentation Generator                                          ││
│  │     Creates documentation from code and comments                     ││
│  │     Model: claude-haiku • Runs: 78                                   ││
│  │                                              [Run] [Edit] [⋮]        ││
│  │                                                                      ││
│  └──────────────────────────────────────────────────────────────────────┘│
│                                                                          │
│  ┌─ Agent Editor ───────────────────────────────────────────────────────┐│
│  │                                                                      ││
│  │  Name: [Code Reviewer                                    ]           ││
│  │  Description: [Reviews code for bugs and security issues ]           ││
│  │                                                                      ││
│  │  Model: [claude-sonnet-4-20250514 ▼]                                             ││
│  │                                                                      ││
│  │  System Prompt:                                                      ││
│  │  ┌──────────────────────────────────────────────────────────────┐   ││
│  │  │ You are an expert code reviewer. Focus on:                   │   ││
│  │  │ 1. Security vulnerabilities                                  │   ││
│  │  │ 2. Performance issues                                        │   ││
│  │  │ 3. Code style and best practices                             │   ││
│  │  │ 4. Potential bugs and edge cases                             │   ││
│  │  └──────────────────────────────────────────────────────────────┘   ││
│  │                                                                      ││
│  │  Tools: [✓] Read [✓] Glob [✓] Grep [ ] Write [ ] Edit [ ] Bash      ││
│  │                                                                      ││
│  │                                        [Cancel] [Save]               ││
│  │                                                                      ││
│  └──────────────────────────────────────────────────────────────────────┘│
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

**Features**:
- Create agents with custom system prompts
- Select model and allowed tools per agent
- Run agents in background (non-blocking)
- View execution history with logs
- Import/export agent configurations

### 4.10 Usage Analytics Dashboard (P1) - NEW

Track API usage, costs, and patterns.

```
┌─ Analytics Dashboard ────────────────────────────────────────────────────┐
│                                                                          │
│  Period: [Last 30 days ▼]                              [Export CSV]      │
│                                                                          │
│  ┌─ Overview ───────────────────────────────────────────────────────────┐│
│  │                                                                      ││
│  │   Total Cost        Total Tokens       Requests       Avg/Day       ││
│  │   $127.45           2.4M               1,234          $4.25         ││
│  │   ↑ 12% vs prev     ↑ 8% vs prev       ↓ 3%           ↑ 15%         ││
│  │                                                                      ││
│  └──────────────────────────────────────────────────────────────────────┘│
│                                                                          │
│  ┌─ Cost Over Time ─────────────────────────────────────────────────────┐│
│  │                                                                      ││
│  │  $8 │                              ╭─╮                               ││
│  │     │                           ╭──╯ ╰──╮    ╭──╮                   ││
│  │  $6 │        ╭──╮    ╭──╮    ╭──╯       ╰────╯  ╰──╮               ││
│  │     │     ╭──╯  ╰────╯  ╰────╯                      ╰──╮           ││
│  │  $4 │  ╭──╯                                            ╰──╮        ││
│  │     │──╯                                                  ╰──      ││
│  │  $2 │                                                              ││
│  │     └────────────────────────────────────────────────────────────  ││
│  │      Jan 1    Jan 8    Jan 15    Jan 22    Jan 29                  ││
│  │                                                                      ││
│  └──────────────────────────────────────────────────────────────────────┘│
│                                                                          │
│  ┌─ By Model ────────────────┐  ┌─ By Project ──────────────────────────┐│
│  │                           │  │                                       ││
│  │  claude-sonnet-4-20250514        │  │  my-web-app          $45.20 (35%)     ││
│  │  ████████████████ $89.20  │  │  ████████████████                     ││
│  │                           │  │                                       ││
│  │  claude-haiku             │  │  api-service          $32.10 (25%)    ││
│  │  ████████ $32.15          │  │  ████████████                         ││
│  │                           │  │                                       ││
│  │  gpt-4o                   │  │  mobile-app           $28.50 (22%)    ││
│  │  ██ $6.10                 │  │  ██████████                           ││
│  │                           │  │                                       ││
│  └───────────────────────────┘  └───────────────────────────────────────┘│
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

**Features**:
- Real-time cost tracking
- Token breakdown by model, project, time period
- Visual charts for usage trends
- Export data for accounting
- Budget alerts (optional)

### 4.11 MCP Server Management (P1) - NEW

Manage Model Context Protocol servers.

```
┌─ MCP Servers ────────────────────────────────────────────────────────────┐
│                                                                          │
│  [➕ Add Server]  [📥 Import from Claude Desktop]                        │
│                                                                          │
│  ┌─ Configured Servers ─────────────────────────────────────────────────┐│
│  │                                                                      ││
│  │  🟢 filesystem                                          [Enabled]   ││
│  │     stdio • @anthropic/mcp-server-filesystem                         ││
│  │     Paths: /Users/dev/projects                                       ││
│  │                                    [Test] [Configure] [Disable]      ││
│  │  ────────────────────────────────────────────────────────────────── ││
│  │  🟢 github                                              [Enabled]   ││
│  │     stdio • @anthropic/mcp-server-github                            ││
│  │     Token: ghp_****...                                               ││
│  │                                    [Test] [Configure] [Disable]      ││
│  │  ────────────────────────────────────────────────────────────────── ││
│  │  🔴 postgres                                           [Disabled]   ││
│  │     stdio • @anthropic/mcp-server-postgres                          ││
│  │     Connection: postgresql://...                                     ││
│  │                                    [Test] [Configure] [Enable]       ││
│  │                                                                      ││
│  └──────────────────────────────────────────────────────────────────────┘│
│                                                                          │
│  ┌─ Add Server ─────────────────────────────────────────────────────────┐│
│  │                                                                      ││
│  │  Server Type: [stdio ▼]                                              ││
│  │                                                                      ││
│  │  Name:    [my-server                                    ]            ││
│  │  Command: [npx                                          ]            ││
│  │  Args:    [-y @anthropic/mcp-server-filesystem          ]            ││
│  │                                                                      ││
│  │  Environment Variables:                                              ││
│  │  ┌──────────────────────────────────────────────────────────────┐   ││
│  │  │ ALLOWED_PATHS=/Users/dev/projects                            │   ││
│  │  └──────────────────────────────────────────────────────────────┘   ││
│  │                                                                      ││
│  │                              [Test Connection] [Cancel] [Save]       ││
│  │                                                                      ││
│  └──────────────────────────────────────────────────────────────────────┘│
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

**Features**:
- Central registry for MCP servers
- Add servers via UI (stdio, SSE types)
- Import from Claude Desktop config
- Test server connectivity
- Enable/disable servers easily

### 4.12 Timeline & Checkpoints (P2) - NEW

Session versioning with visual timeline.

```
┌─ Timeline ───────────────────────────────────────────────────────────────┐
│                                                                          │
│  Session: "Implement user authentication"                                │
│                                                                          │
│  ┌─ Timeline View ──────────────────────────────────────────────────────┐│
│  │                                                                      ││
│  │   ○ Start                                              Jan 30, 14:23 ││
│  │   │                                                                  ││
│  │   │  "Help me implement user authentication"                         ││
│  │   │                                                                  ││
│  │   ◆ Checkpoint: "Basic auth setup"                     Jan 30, 14:45 ││
│  │   │  └─ 12 messages • 3 files changed                                ││
│  │   │                                                                  ││
│  │   │  "Add OAuth provider support"                                    ││
│  │   │                                                                  ││
│  │   ├──◇ Branch: "Try JWT approach"                      Jan 30, 15:10 ││
│  │   │  │  └─ 8 messages • 2 files changed                              ││
│  │   │  │                                                               ││
│  │   │  ○ (abandoned)                                                   ││
│  │   │                                                                  ││
│  │   ◆ Checkpoint: "OAuth complete"                       Jan 30, 15:30 ││
│  │   │  └─ 25 messages • 7 files changed                                ││
│  │   │                                                                  ││
│  │   ● Current                                            Jan 30, 16:00 ││
│  │      └─ 45 messages • 12 files changed                               ││
│  │                                                                      ││
│  │                                    [Create Checkpoint] [Fork Branch] ││
│  │                                                                      ││
│  └──────────────────────────────────────────────────────────────────────┘│
│                                                                          │
│  ┌─ Diff Viewer (Basic auth setup → OAuth complete) ────────────────────┐│
│  │                                                                      ││
│  │  src/auth/provider.ts                                    [+45 -12]  ││
│  │  ┌──────────────────────────────────────────────────────────────┐   ││
│  │  │  15  │ - const auth = basicAuth();                           │   ││
│  │  │  15  │ + const auth = oauthProvider({                        │   ││
│  │  │  16  │ +   clientId: process.env.OAUTH_CLIENT_ID,            │   ││
│  │  │  17  │ +   clientSecret: process.env.OAUTH_SECRET,           │   ││
│  │  │  18  │ + });                                                 │   ││
│  │  └──────────────────────────────────────────────────────────────┘   ││
│  │                                                                      ││
│  │  src/auth/middleware.ts                                  [+23 -5]   ││
│  │  src/routes/login.ts                                     [+67 -0]   ││
│  │                                                                      ││
│  └──────────────────────────────────────────────────────────────────────┘│
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

**Features**:
- Create checkpoints at any point
- Visual branching timeline
- Restore to any checkpoint instantly
- Fork sessions from checkpoints
- Diff viewer between checkpoints

### 4.13 CLAUDE.md Management (P1) - NEW

Edit and manage CLAUDE.md files.

```
┌─ CLAUDE.md Editor ───────────────────────────────────────────────────────┐
│                                                                          │
│  ┌─ Files ──────────┐  ┌─ Editor ────────────────────────────────────────┐│
│  │                  │  │                                                ││
│  │  📁 Projects     │  │  /Users/dev/my-web-app/CLAUDE.md               ││
│  │  ├─ my-web-app   │  │                                                ││
│  │  │  └─ CLAUDE.md │  │  ┌──────────────────────────────────────────┐  ││
│  │  ├─ api-service  │  │  │ # My Web App                              │  ││
│  │  │  └─ CLAUDE.md │  │  │                                          │  ││
│  │  └─ mobile-app   │  │  │ ## Project Overview                       │  ││
│  │     ├─ CLAUDE.md │  │  │ This is a Next.js web application...     │  ││
│  │     └─ src/      │  │  │                                          │  ││
│  │        └─ ...    │  │  │ ## Code Style                             │  ││
│  │                  │  │  │ - Use TypeScript for all files           │  ││
│  │  ─────────────── │  │  │ - Follow ESLint configuration            │  ││
│  │                  │  │  │ - Use Prettier for formatting            │  ││
│  │  🔍 Scan for     │  │  │                                          │  ││
│  │  CLAUDE.md files │  │  │ ## Architecture                          │  ││
│  │                  │  │  │ ```                                      │  ││
│  │                  │  │  │ src/                                     │  ││
│  │                  │  │  │ ├── components/                          │  ││
│  │                  │  │  │ ├── pages/                               │  ││
│  │                  │  │  │ └── utils/                               │  ││
│  │                  │  │  │ ```                                      │  ││
│  │                  │  │  └──────────────────────────────────────────┘  ││
│  │                  │  │                                                ││
│  └──────────────────┘  │                                    [Save]      ││
│                        │                                                ││
│  ┌─ Preview ───────────────────────────────────────────────────────────┐││
│  │                                                                     │││
│  │  # My Web App                                                       │││
│  │                                                                     │││
│  │  ## Project Overview                                                │││
│  │  This is a Next.js web application...                               │││
│  │                                                                     │││
│  │  ## Code Style                                                      │││
│  │  • Use TypeScript for all files                                     │││
│  │  • Follow ESLint configuration                                      │││
│  │  • Use Prettier for formatting                                      │││
│  │                                                                     │││
│  └─────────────────────────────────────────────────────────────────────┘││
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

**Features**:
- Browse all CLAUDE.md files
- Built-in markdown editor
- Live preview with syntax highlighting
- Quick scan to find all CLAUDE.md files
- Templates for common configurations

### 4.14 Real-time Streaming Chat (P0) - NEW

Desktop provides real-time streaming conversation experience with AI responses and thinking display.

#### Streaming Response Display

```
┌─ Chat View ───────────────────────────────────────────────────────────────┐
│                                                                           │
│  ┌─ User ─────────────────────────────────────────────────────────────┐   │
│  │ Implement a user authentication system with OAuth support          │   │
│  └────────────────────────────────────────────────────────────────────┘   │
│                                                                           │
│  ┌─ Assistant ────────────────────────────────────────────────────────┐   │
│  │                                                                    │   │
│  │  ┌─ 💭 Thinking ──────────────────────────────────────── [▼] ──┐  │   │
│  │  │ I need to analyze the project structure first to understand │  │   │
│  │  │ the existing authentication patterns. Let me check the      │  │   │
│  │  │ current codebase for any auth-related files...              │  │   │
│  │  │                                                              │  │   │
│  │  │ Key considerations:                                          │  │   │
│  │  │ 1. Check for existing auth middleware                        │  │   │
│  │  │ 2. Identify the database schema for users                    │  │   │
│  │  │ 3. Look for OAuth provider configurations                    │  │   │
│  │  └──────────────────────────────────────────────────────────────┘  │   │
│  │                                                                    │   │
│  │  I'll implement the authentication system. Let me start by        │   │
│  │  examining the existing project structure.█                       │   │
│  │                                        ↑ cursor (streaming)       │   │
│  └────────────────────────────────────────────────────────────────────┘   │
│                                                                           │
│  ┌─ Tool Calls ───────────────────────────────────────────────────────┐   │
│  │  ✓ Glob  **/*.{ts,js}                    42 files     0.3s        │   │
│  │  ✓ Read  src/middleware/auth.ts          128 lines    0.1s        │   │
│  │  ⟳ Read  src/config/oauth.ts             reading...               │   │
│  └────────────────────────────────────────────────────────────────────┘   │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

#### Thinking Display Features

| Feature | Description |
|---------|-------------|
| **Collapsible** | Thinking blocks can be collapsed/expanded |
| **Real-time Streaming** | Thinking content streams in real-time |
| **Visual Distinction** | Different styling from regular response |
| **Time Indicator** | Shows thinking duration |
| **Auto-collapse** | Option to auto-collapse when response starts |

#### Streaming Configuration

```
┌─ Settings > Chat ─────────────────────────────────────────────────────────┐
│                                                                           │
│  Streaming Display                                                        │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │ [✓] Enable streaming display (show text as it arrives)            │  │
│  │ [✓] Show typing animation                                          │  │
│  │ [ ] Auto-scroll to bottom                                          │  │
│  │                                                                     │  │
│  │ Streaming speed: [Normal ▼]                                        │  │
│  │                   ├─ Instant (no animation)                        │  │
│  │                   ├─ Fast                                          │  │
│  │                   └─ Normal                                        │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                           │
│  Thinking Display                                                         │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │ [✓] Show thinking blocks (extended thinking)                      │  │
│  │ [✓] Stream thinking content                                        │  │
│  │ [ ] Auto-collapse thinking when response starts                    │  │
│  │ [ ] Hide thinking blocks by default                                │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

#### Unified Streaming Abstraction Layer (P0)

Both working modes (Claude Code GUI + Standalone Multi-LLM) must support streaming through a unified abstraction layer:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                  Unified Stream Event Interface                              │
│                     (Frontend consumes this)                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│  TextDelta { content }           - Incremental text                         │
│  ThinkingStart { id }            - Thinking block start (Claude only)       │
│  ThinkingDelta { id, content }   - Thinking incremental (Claude only)       │
│  ThinkingEnd { id, duration }    - Thinking block end (Claude only)         │
│  ToolStart { id, name, args }    - Tool execution start                     │
│  ToolResult { id, success, output } - Tool execution result                 │
│  Usage { tokens_in, tokens_out, cost } - Token usage stats                  │
│  Error { message }               - Error occurred                           │
│  Complete { session_id, stats }  - Stream complete                          │
└─────────────────────────────────────────────────────────────────────────────┘
                                    ▲
                                    │ Adapts to unified format
        ┌───────────────┬───────────┴───────────┬───────────────┐
        │               │                       │               │
┌───────┴───────┐ ┌─────┴─────┐ ┌───────────────┴───────────────┐
│  Claude Code  │ │  Claude   │ │     OpenAI-Compatible         │
│  CLI Adapter  │ │API Adapter│ │  (OpenAI/DeepSeek/Ollama)     │
├───────────────┤ ├───────────┤ ├───────────────────────────────┤
│ stream-json   │ │ SSE with  │ │ SSE with tool_calls           │
│ format        │ │ thinking  │ │ (no thinking support)         │
│ + thinking    │ │ blocks    │ │                               │
│ + tool_use    │ │           │ │                               │
└───────────────┘ └───────────┘ └───────────────────────────────┘
```

**Provider Feature Matrix**:

| Provider | Streaming | Thinking | Tool Calls | Format | Thinking Format |
|----------|-----------|----------|------------|--------|-----------------|
| Claude Code CLI | ✅ | ✅ | ✅ | `stream-json` | `thinking` block |
| Claude API | ✅ | ✅ | ✅ | SSE `content_block_delta` | `thinking` block |
| OpenAI | ✅ | ⚠️ Conditional | ✅ | SSE `chat.completion.chunk` | `reasoning_content` (o1/o3 only) |
| DeepSeek | ✅ | ⚠️ Conditional | ✅ | SSE (OpenAI-compatible) | `<think>...</think>` tags (R1 only) |
| Ollama | ✅ | ⚠️ Model-dependent | ⚠️ Limited | JSON stream | Follows hosted model format |

**Thinking Support Details**:

| Provider | Model Requirements | API Parameter | Output Format |
|----------|-------------------|---------------|---------------|
| Claude | All models with extended thinking | `anthropic-beta: interleaved-thinking` | Dedicated `thinking` content block |
| OpenAI | o1, o1-mini, o1-pro, o3-mini | `reasoning_effort: "medium"` | `reasoning_content` field in response |
| DeepSeek | DeepSeek-R1, DeepSeek-R1-Distill | Default enabled | `<think>...</think>` XML tags in content |
| Ollama | DeepSeek-R1, QwQ, etc. | Depends on model | Follows original model format |

**Notes**:
- Thinking display adapts based on provider and model capabilities
- Frontend checks `supports_thinking()` and conditionally shows/hides Thinking UI
- When model doesn't support thinking, the section is gracefully hidden
- Tool call format varies by provider but unified by adapter layer

### 4.15 Tool Call Visualization (P0) - NEW

Real-time visualization of tool execution with detailed feedback.

#### Tool Call States

```
┌─ Tool Execution Panel ────────────────────────────────────────────────────┐
│                                                                           │
│  ┌─ Current Execution ────────────────────────────────────────────────┐  │
│  │                                                                    │  │
│  │  ⟳ Edit  src/auth/handler.ts                                      │  │
│  │    ├─ old_string: "function login(..."  (42 chars)                │  │
│  │    ├─ new_string: "async function login(..."  (48 chars)          │  │
│  │    └─ Status: Writing...                                          │  │
│  │                                                                    │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                           │
│  ┌─ History ──────────────────────────────────────────────────────────┐  │
│  │                                                                    │  │
│  │  ✓ Glob   **/*.ts                    0.3s   42 matches            │  │
│  │    └─ [View Results]                                              │  │
│  │                                                                    │  │
│  │  ✓ Read   src/auth/handler.ts        0.1s   128 lines             │  │
│  │    └─ [View Content]                                              │  │
│  │                                                                    │  │
│  │  ✓ Read   src/config/database.ts     0.1s   64 lines              │  │
│  │    └─ [View Content]                                              │  │
│  │                                                                    │  │
│  │  ✗ Bash   npm test                   2.3s   Exit code: 1          │  │
│  │    └─ [View Error] [Retry]                                        │  │
│  │                                                                    │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                           │
│  Statistics: 4 calls │ 3 success │ 1 failed │ Total: 2.8s               │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

#### File Change Preview

```
┌─ File Changes ────────────────────────────────────────────────────────────┐
│                                                                           │
│  src/auth/handler.ts                                    [Revert] [Accept] │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │  @@ -15,7 +15,7 @@                                                 │  │
│  │   import { validateToken } from './utils';                         │  │
│  │                                                                    │  │
│  │ - function login(username: string, password: string) {            │  │
│  │ + async function login(username: string, password: string) {      │  │
│  │     const user = await findUser(username);                         │  │
│  │     if (!user) {                                                   │  │
│  │       throw new AuthError('User not found');                       │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                           │
│  src/config/oauth.ts                                    [Revert] [Accept] │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │  + // OAuth provider configuration                                 │  │
│  │  + export const oauthConfig = {                                    │  │
│  │  +   google: {                                                     │  │
│  │  +     clientId: process.env.GOOGLE_CLIENT_ID,                     │  │
│  │  +     ...                                                         │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                           │
│  Changes: 2 files │ +45 lines │ -3 lines          [Revert All] [Accept All]│
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

### 4.16 Chat Interaction Features (P0) - NEW

#### Message Features

| Feature | Description |
|---------|-------------|
| **Markdown Rendering** | Full GFM support with syntax highlighting |
| **Code Blocks** | Syntax highlighting, copy button, line numbers |
| **Image Display** | Inline image preview (screenshots, diagrams) |
| **Message Actions** | Copy, regenerate, edit & resend |
| **Branch Conversations** | Create conversation branches from any message |

#### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Enter` | Send message |
| `Shift+Enter` | New line |
| `Ctrl+C` | Cancel current operation |
| `Ctrl+/` | Open command palette |
| `Ctrl+K` | Clear conversation |
| `Ctrl+Shift+C` | Copy last response |
| `↑` (in empty input) | Edit last message |

#### Drag & Drop Support

```
┌─ Chat Input ──────────────────────────────────────────────────────────────┐
│                                                                           │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │                                                                    │  │
│  │  ┌─────────────┐  ┌─────────────┐                                 │  │
│  │  │ 📄 app.ts   │  │ 🖼️ error.png │  Drop files here or @mention   │  │
│  │  │  (attached) │  │  (attached) │                                 │  │
│  │  └─────────────┘  └─────────────┘                                 │  │
│  │                                                                    │  │
│  │  Fix the error shown in the screenshot. The relevant code is in   │  │
│  │  @src/components/Button.tsx                                        │  │
│  │                                                                    │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                           │
│  [📎 Attach] [@] [/]                                          [Send ➤]   │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

#### @ File Reference

```
┌─ File Reference ──────────────────────────────────────────────────────────┐
│                                                                           │
│  Type @ to reference files:                                              │
│                                                                           │
│  @src/                                                                    │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │  📁 src/components/                                                │  │
│  │  📁 src/utils/                                                     │  │
│  │  📄 src/app.ts                          Modified 2 hours ago      │  │
│  │  📄 src/config.ts                       Modified yesterday        │  │
│  │  📄 src/index.ts                        Modified 3 days ago       │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                           │
│  Recent files:                                                            │
│  📄 src/auth/handler.ts  │  📄 src/api/routes.ts  │  📄 package.json    │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

### 4.17 Session Control (P0) - NEW

#### Interrupt & Cancel

```
┌─ Execution Control ───────────────────────────────────────────────────────┐
│                                                                           │
│  ┌─ Running Task ─────────────────────────────────────────────────────┐  │
│  │                                                                    │  │
│  │  ⟳ Implementing user authentication...                            │  │
│  │                                                                    │  │
│  │  Current: Reading src/middleware/auth.ts                          │  │
│  │  Progress: 3/7 tool calls                                         │  │
│  │  Duration: 00:01:23                                               │  │
│  │                                                                    │  │
│  │                    [⏸️ Pause]  [⏹️ Stop]  [🔄 Restart]             │  │
│  │                                                                    │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                           │
│  ⚠️ Stopping will cancel the current operation. Changes already made    │
│     will not be automatically reverted.                                  │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

#### Regenerate & Edit

```
┌─ Message Actions ─────────────────────────────────────────────────────────┐
│                                                                           │
│  ┌─ Assistant ────────────────────────────────────────────────────────┐  │
│  │                                                                    │  │
│  │  I've implemented the login function with basic validation...     │  │
│  │                                                                    │  │
│  │  ┌───────────────────────────────────────────────────────────────┐│  │
│  │  │ // Code block...                                              ││  │
│  │  └───────────────────────────────────────────────────────────────┘│  │
│  │                                                                    │  │
│  │  ───────────────────────────────────────────────────────────────  │  │
│  │  [📋 Copy] [🔄 Regenerate] [✏️ Edit & Resend] [🌿 Branch Here]    │  │
│  │                                                                    │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

### 4.18 Command Palette (P1) - NEW

Quick access to all features via keyboard.

```
┌─ Command Palette (Ctrl+/) ────────────────────────────────────────────────┐
│                                                                           │
│  > new session                                                            │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │                                                                    │  │
│  │  📝 New Session                          Start a new chat session │  │
│  │  📁 Open Project...                      Switch to another project │  │
│  │  🔍 Search Sessions...                   Search past conversations │  │
│  │  ───────────────────────────────────────────────────────────────  │  │
│  │  ⚙️ Settings                             Open settings panel       │  │
│  │  🎨 Toggle Theme                         Switch light/dark mode    │  │
│  │  📊 Usage Dashboard                      View usage statistics     │  │
│  │  ───────────────────────────────────────────────────────────────  │  │
│  │  🔌 MCP Servers                          Manage MCP connections   │  │
│  │  🤖 Agent Library                        Browse custom agents      │  │
│  │  📋 CLAUDE.md                            Edit project config       │  │
│  │                                                                    │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                           │
│  Type to filter • ↑↓ to navigate • Enter to select • Esc to close       │
│                                                                           │
└───────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Non-Functional Requirements

### 5.1 Performance Requirements

| Metric | Requirement |
|--------|-------------|
| Startup Time | < 2 seconds |
| Memory Usage | < 200MB (idle state) |
| Binary Size | < 50MB (compressed) |
| Project Scan | < 1 second for 100 projects |
| Search Response | < 100ms |

### 5.2 Compatibility Requirements

| Platform | Minimum Version |
|----------|-----------------|
| Windows | Windows 10 |
| macOS | macOS 11 (Big Sur) |
| Linux | Ubuntu 20.04 / equivalent |
| Claude Code | v1.0+ (for GUI mode) |

### 5.3 Security Requirements

- API Keys stored in OS keychain (not config files)
- No telemetry without explicit consent
- All network requests use HTTPS
- Local SQLite database encrypted at rest
- Sensitive data never logged

---

## 6. Milestone Plan

### Phase 1: Rust Backend Foundation (2 weeks)

**Goal**: Replace Python sidecar with pure Rust backend

**Scope**:
- [ ] Rust backend architecture setup
- [ ] Claude Code CLI integration
- [ ] Basic Tauri commands
- [ ] Settings management (Rust)
- [ ] SQLite database setup

### Phase 2: Core Desktop Features (3 weeks)

**Goal**: Essential management features

**Scope**:
- [ ] Project & Session Browser
- [ ] CLAUDE.md Editor
- [ ] MCP Server Management
- [ ] Basic Analytics

### Phase 3: Advanced Features (3 weeks)

**Goal**: Complete feature set

**Scope**:
- [ ] CC Agents
- [ ] Timeline & Checkpoints
- [ ] Advanced Analytics
- [ ] Standalone Mode (LLM direct)

### Phase 4: Polish & Release (2 weeks)

**Goal**: Production ready

**Scope**:
- [ ] UI/UX polish
- [ ] Performance optimization
- [ ] Documentation
- [ ] Auto-update system
- [ ] Release builds for all platforms

---

## 7. Success Metrics

| Metric | Target |
|--------|--------|
| App Startup Time | < 2 seconds |
| Binary Size | < 50MB |
| User Onboarding | < 2 minutes |
| Feature Adoption | > 50% use Projects browser |
| User Satisfaction | > 4.0/5.0 rating |

---

## 8. Appendix

### 8.1 Glossary

| Term | Definition | User Needs to Understand |
|------|------------|--------------------------|
| Claude Code GUI Mode | Desktop serves as visual interface for Claude Code | Yes |
| Standalone Mode | Desktop operates independently with direct LLM API | Yes |
| CC Agent | Custom AI agent with specific system prompt | Yes |
| Checkpoint | Saved snapshot of a session state | Yes |
| MCP Server | Model Context Protocol server for extended capabilities | Advanced users |

### 8.2 File Locations

| Data | Location |
|------|----------|
| Projects | `~/.claude/projects/` |
| Sessions | `~/.claude/projects/{project-id}/sessions/` |
| Desktop Config | `~/.plan-cascade/config.json` |
| Desktop Database | `~/.plan-cascade/data.db` |
| Agent Library | `~/.plan-cascade/agents/` |
| MCP Config | `~/.plan-cascade/mcp-servers.json` |

