[中文版](PRD-Plan-Cascade-Standalone_zh.md)

# Plan Cascade Standalone - Product Requirements Document (PRD)

**Version**: 4.0.0
**Date**: 2026-01-29
**Author**: Plan Cascade Team
**Status**: Implementation In Progress

---

## Implementation Status Overview

> **Current Progress**: ~98% core functionality implemented
> **Last Updated**: 2026-01-29

### Feature Requirements Implementation Status

| Feature (Section) | Priority | Status | Notes |
|-------------------|----------|--------|-------|
| **4.1 Working Mode Selection** | P0 | ✅ Complete | |
| Standalone Orchestration Mode | P0 | ✅ Complete | ReAct engine + tool execution |
| Claude Max LLM Backend | P0 | ✅ Complete | `llm/providers/claude_max.py` |
| Claude API | P0 | ✅ Complete | `llm/providers/claude.py` |
| OpenAI | P0 | ✅ Complete | `llm/providers/openai.py` |
| DeepSeek | P1 | ✅ Complete | `llm/providers/deepseek.py` |
| Ollama | P2 | ✅ Complete | `llm/providers/ollama.py` |
| **4.2 Multi-Agent Collaboration** | P0 | ✅ Complete | |
| Phase-based Agent Assignment | P0 | ✅ Complete | `backends/phase_config.py` |
| Agent Executor | P0 | ✅ Complete | `backends/agent_executor.py` |
| **4.2 Simple Mode Features** | P0 | ✅ Complete | |
| One-click Workflow | P0 | ✅ Complete | `core/simple_workflow.py` |
| AI Auto Strategy Determination | P0 | ✅ Complete | `core/strategy_analyzer.py` |
| **4.3 Expert Mode Features** | P0 | ✅ Complete | |
| PRD Editor | P0 | ✅ Complete | `core/expert_workflow.py` |
| Execution Strategy Selection | P0 | ✅ Complete | direct/hybrid/mega |
| Agent Specification | P0 | ✅ Complete | Each Story can specify Agent |
| **4.4 Settings Page** | P0 | ✅ Complete | |
| Agent Configuration | P0 | ✅ Complete | `settings/models.py` |
| Quality Gate Configuration | P0 | ✅ Complete | `core/quality_gate.py` |
| API Key Secure Storage | P0 | ✅ Complete | Keyring integration |
| **4.5 CLI Features** | P1 | ✅ Complete | |
| `plan-cascade run` | P0 | ✅ Complete | Simple/expert mode |
| `plan-cascade config` | P0 | ✅ Complete | Configuration wizard |
| `plan-cascade status` | P1 | ✅ Complete | Status viewing |
| **4.6 Claude Code GUI Mode** | P0 | ⚠️ Partial | |
| Claude Code CLI Integration | P0 | ✅ Complete | `backends/claude_code.py` |
| GUI-specific Backend | P2 | ⏳ Planning | `backends/claude_code_gui.py` |
| Tool Call Visualization | P1 | ✅ Complete | Streaming event parsing |
| **4.7 Interactive REPL Mode** | P0 | ✅ Complete | |
| REPL Loop | P0 | ✅ Complete | `plan-cascade chat` |
| Special Commands | P0 | ✅ Complete | /exit, /clear, /status, /mode |
| Smart Intent Recognition | P0 | ✅ Complete | `core/intent_classifier.py` |

### Product Form Implementation Status

| Form | Status | Notes |
|------|--------|-------|
| CLI | ✅ Complete | `pip install plan-cascade` |
| Desktop (GUI) | ⏳ Planning | Tauri implementation, Phase 2 target |
| Claude Code Plugin | ✅ Complete | Existing Plugin maintains compatibility |

### Milestone Progress

| Phase | Status | Completion |
|-------|--------|------------|
| Phase 1: CLI + Dual-Mode | ✅ Complete | 100% |
| Phase 2: Desktop Application Alpha | ⏳ Planning | 0% |
| Phase 3: Feature Completion | ⏳ Pending | - |
| Phase 4: Advanced Features | ⏳ Pending | - |

---

## 1. Overview

### 1.1 Product Vision

Develop Plan Cascade into a **complete AI programming orchestration platform** with autonomous tool execution capabilities, making AI programming simple.

**Core Positioning**:
- As a **complete orchestration layer**: Execute tools itself, LLM only provides thinking (standalone mode)
- As a **graphical interface for Claude Code**: Compatible with all Claude Code features (GUI mode)
- Support **multiple LLM backends**: Claude Max, Claude API, OpenAI, DeepSeek, etc.

### 1.2 Core Value Propositions

| Value Point | Description |
|-------------|-------------|
| **Complete Orchestration Capability** | Autonomously executes tools (Read/Write/Edit/Bash/Glob/Grep), no external Agent dependency |
| **Zero Barrier Entry** | Claude Max members need no API Key, directly use Claude Code as LLM backend |
| **Dual-Mode Switching** | Simple mode for quick start, expert mode for fine-grained control |
| **Model Freedom** | Supports Claude Max, Claude API, OpenAI, DeepSeek, Ollama, etc. |
| **Philosophy Continuation** | Fully preserves Plan Cascade's core design philosophy |
| **Claude Code Compatible** | Can serve as complete GUI for Claude Code, compatible with all features |

### 1.3 Product Positioning

```
┌─────────────────────────────────────────────────────────────┐
│                    Plan Cascade                              │
│          Complete AI Programming Orchestration Platform      │
│                    + Claude Code GUI                         │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   ┌─ Working Mode Selection ─────────────────────────────────┐│
│   │                                                        ││
│   │  ● Standalone Orchestration Mode (Recommended)         ││
│   │    └─ Plan Cascade executes all tools                  ││
│   │    └─ LLM only provides thinking (Claude Max/API/      ││
│   │       OpenAI etc.)                                     ││
│   │                                                        ││
│   │  ○ Claude Code GUI Mode                                ││
│   │    └─ Serves as graphical interface for Claude Code    ││
│   │    └─ Claude Code executes tools, Plan Cascade         ││
│   │       provides visualization                           ││
│   │                                                        ││
│   └────────────────────────────────────────────────────────┘│
│                                                              │
│   ┌─ Standalone Mode: LLM Backend Selection ─────────────────┐│
│   │                                                        ││
│   │   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   ││
│   │   │ Claude Max  │  │ Claude API  │  │   OpenAI    │   ││
│   │   │(No API Key) │  │(API Key Req)│  │(API Key Req)│   ││
│   │   │ Get LLM via │  │  Direct     │  │  Direct     │   ││
│   │   │ Claude Code │  │  Calling    │  │  Calling    │   ││
│   │   └─────────────┘  └─────────────┘  └─────────────┘   ││
│   │                                                        ││
│   └────────────────────────────────────────────────────────┘│
│                                                              │
└─────────────────────────────────────────────────────────────┘

Standalone Orchestration Mode: Plan Cascade = Complete orchestration + Tool execution + LLM thinking
Claude Code GUI Mode: Plan Cascade = Visual interface for Claude Code
```

### 1.4 Target Users

| User Group | Scenario | Pain Point | Solution |
|------------|----------|------------|----------|
| **Claude Max Members** | Have Max subscription but no API Key | Claude Code Plugin is complex to use | Standalone orchestration mode + Claude Code LLM backend |
| **New Developers** | Want AI-assisted development | Claude Code CLI has high learning curve | Simple mode one-click completion |
| **Senior Developers** | Need fine-grained control | Existing tools lack control | Expert mode customization |
| **Small Teams** | Unified toolchain | Different members use different LLMs | Multi-backend support |
| **Enterprise Users** | Data security requirements | Need private deployment | Local model support |

---

## 2. Core Design Philosophy

### 2.1 Usability First

**Design Principle**: Users only need to describe what they want to do, the system automatically handles everything.

```
User Input
   │
   ▼
"Help me implement user login functionality, supporting OAuth and SMS verification"
   │
   ▼
┌─────────────────────────────────────────────────────────────┐
│                    Plan Cascade Auto-Processing              │
│                                                              │
│   ✓ Determine task scale → Auto-select execution strategy   │
│   ✓ Generate PRD and Stories                                 │
│   ✓ Analyze dependencies → Arrange execution batches         │
│   ✓ Select appropriate Agent                                 │
│   ✓ Execute tasks in parallel                                │
│   ✓ Auto quality check and retry                             │
│                                                              │
└─────────────────────────────────────────────────────────────┘
   │
   ▼
Complete
```

**Users don't need to understand**: Mega Plan, Hybrid Ralph, Worktree, batch dependencies, and other internal concepts.

### 2.2 Dual-Mode Design

#### Simple Mode (Default)

For: New users, quick tasks

```
┌─ Simple Mode ────────────────────────────────────────────────┐
│                                                              │
│   Describe the functionality you want to implement:          │
│   ┌────────────────────────────────────────────────────────┐│
│   │ Implement user login functionality, support OAuth and   ││
│   │ SMS verification                                        ││
│   └────────────────────────────────────────────────────────┘│
│                                                              │
│                              [Start] ← One-click, auto-      │
│                                        completes everything  │
│                                                              │
│  ────────────────────────────────────────────────────────── │
│                                                              │
│   Executing...                                               │
│   ┌────────────────────────────────────────────────────────┐│
│   │ ✓ Generate plan (5 tasks)                              ││
│   │ ✓ Batch 1: Database Schema, API routes (2/2)           ││
│   │ ⟳ Batch 2: OAuth login, SMS verification (1/2)         ││
│   │ ○ Batch 3: Integration tests                           ││
│   └────────────────────────────────────────────────────────┘│
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

#### Expert Mode

For: Senior users, needs fine-grained control

```
┌─ Expert Mode ────────────────────────────────────────────────┐
│                                                              │
│  ┌─ Step 1: Requirement Input ───────────────────────────────┐│
│  │ Implement user login functionality, support OAuth and SMS ││
│  └────────────────────────────────────────────────────────┘│
│                                            [Generate PRD]    │
│                                                              │
│  ┌─ Step 2: Review PRD ──────────────────────────────────────┐│
│  │                                                          ││
│  │  Execution Strategy:              AI Suggests: Hybrid Auto ││
│  │  ○ Direct Execute   ● Hybrid Auto   ○ Mega Plan          ││
│  │                                                          ││
│  │  □ Use Git Worktree for isolated development             ││
│  │                                                          ││
│  │  Stories:                                    [+ Add]     ││
│  │  ┌────────────────────────────────────────────────────┐ ││
│  │  │ □ Design database Schema                            │ ││
│  │  │   Priority: high  │  Agent: [claude-code ▼]         │ ││
│  │  │   Dependencies: none                    [Edit][Delete]│ ││
│  │  ├────────────────────────────────────────────────────┤ ││
│  │  │ □ Implement OAuth login                              │ ││
│  │  │   Priority: medium │  Agent: [aider ▼]               │ ││
│  │  │   Dependencies: [Schema ▼]              [Edit][Delete]│ ││
│  │  └────────────────────────────────────────────────────┘ ││
│  │                                                          ││
│  │  Quality Gates: [✓] TypeCheck  [✓] Test  [✓] Lint  [ ] Custom ││
│  │                                                          ││
│  └──────────────────────────────────────────────────────────┘│
│                                                              │
│                              [Save Draft]  [Start Execution] │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

#### Mode Comparison

| Feature | Simple Mode | Expert Mode |
|---------|-------------|-------------|
| Requirement Input | ✓ | ✓ |
| Auto-generate PRD | ✓ (direct execute) | ✓ (editable) |
| Review/Edit PRD | ✗ | ✓ |
| Select Execution Strategy | ✗ (AI auto) | ✓ |
| Specify Agent | ✗ (auto) | ✓ |
| Adjust Dependencies | ✗ | ✓ |
| Custom Quality Gates | ✗ (use default) | ✓ |
| Execution Monitoring | Simplified view | Detailed view |
| Log Viewing | Errors only | Full logs |

### 2.3 AI Automatic Strategy Determination

In simple mode, AI automatically selects the best execution strategy based on user requirements:

```
User Input                            AI Determination           Internal Execution
───────────────────────────────────────────────────────────────────────────
"Add an exit button"              →   Small task            →   Direct execution (no PRD)

"Implement user login             →   Medium feature        →   Hybrid Auto
 functionality"                                                  (auto-generate PRD)

"Develop a blog system            →   Large project         →   Mega Plan
 with users, articles, comments"                                 (multi-PRD cascade)

"Refactor payment module,         →   Requires isolation    →   Hybrid Worktree
 don't affect existing                                          (Git isolated development)
 functionality"
```

**Determination Dimensions**:
1. **Task Scale**: Single task / Multiple features / Complete project
2. **Complexity**: Whether decomposition into multiple Stories is needed
3. **Risk Level**: Whether isolated development is needed
4. **Dependencies**: Whether there are cross-module dependencies

### 2.4 Core Architecture Philosophy (Must Preserve)

#### Hierarchical Decomposition

```
Project (Mega Plan)
   │
   ├── Feature 1 (Hybrid Ralph / PRD)
   │      ├── Story 1.1
   │      ├── Story 1.2
   │      └── Story 1.3
   │
   └── Feature 2 (Hybrid Ralph / PRD)
          ├── Story 2.1
          └── Story 2.2
```

#### Parallel Execution

```
Batch 1: [Story A, Story B, Story C]  ← No dependencies, parallel execution
           ↓ All complete
Batch 2: [Story D, Story E]           ← Depends on Batch 1, parallel execution
           ↓ All complete
Batch 3: [Story F]                    ← Depends on Batch 2
```

#### Multi-Agent Collaboration

- Auto-select optimal Agent based on task type
- Support Agent fallback chain (auto-switch when primary Agent unavailable)
- Support phased Agent configuration

#### Quality Assurance

- **Quality Gates**: typecheck, test, lint, custom
- **Smart Retry**: Auto-retry on failure, inject failure context
- **Configurable**: Gate types, retry counts all configurable

#### State Tracking

- **File-based State Sharing**: prd.json, .agent-status.json
- **Finding Sharing**: findings.md records discoveries during development
- **Checkpoint Recovery**: Can resume from last state after interruption

---

## 3. Product Forms

### 3.1 Three Forms Unified Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Plan Cascade                               │
├───────────────────┬───────────────────┬─────────────────────┤
│   Desktop (GUI)   │      CLI          │  Claude Code Plugin │
├───────────────────┼───────────────────┼─────────────────────┤
│ • GUI version of  │ • Command line    │ • Depends on Claude │
│   CLI             │   operation       │   Code              │
│ • Simple/Expert   │ • Simple/Expert   │ • Runs as plugin    │
│   modes           │   modes           │ • Slash command     │
│ • Interactive     │ • Interactive     │   invocation        │
│   REPL            │   REPL            │                     │
│ • Optional        │ • pip install     │                     │
│   Claude Code     │   plan-cascade    │                     │
│   GUI mode        │                   │                     │
└───────────────────┴───────────────────┴─────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                   Plan Cascade Core                          │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │ Tool Execution│  │ ReAct Loop   │  │ PRD Generator│       │
│  │ Engine       │  │ Think→Act    │  │ Strategy     │       │
│  │ Read/Write   │  │ →Observe     │  │ Analysis     │       │
│  │ Edit/Bash    │  │             │  │ Batch        │       │
│  │ Glob/Grep    │  │             │  │ Orchestration│       │
│  └──────────────┘  └──────────────┘  └──────────────┘       │
│                           │                                  │
│                           ▼                                  │
│           ┌─────────────────────────────┐                   │
│           │      LLM Abstraction Layer  │                   │
│           │  Claude Max | API | OpenAI  │                   │
│           └─────────────────────────────┘                   │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 Desktop Positioning

**Desktop = GUI Version of CLI**

Desktop provides the same functionality as CLI, presented in a graphical interface:
- Visual requirement input and execution monitoring
- PRD editor (drag-and-drop, dependency visualization)
- Real-time tool call display
- File change preview

**Optional: Claude Code GUI Mode**

Desktop can serve as complete graphical interface for Claude Code:
- Chat view (interact with Claude Code)
- Tool call visualization
- Compatible with all Claude Code features

### 3.3 Release Artifacts

| Artifact | Description | Target Users |
|----------|-------------|--------------|
| **Desktop** | Windows/macOS/Linux installers | Users wanting graphical interface |
| **CLI** | `pip install plan-cascade` | Developers preferring command line |
| **Claude Code Plugin** | Existing Plugin (maintain compatibility) | Claude Code power users |
| **Python Package** | `plan-cascade-core` | Developers integrating into other tools |

---

## 4. Feature Requirements

### 4.1 Working Mode Selection (P0)

#### Standalone Orchestration Mode (Recommended)

Plan Cascade as complete orchestration layer, executing all tools itself:

```
┌─ Settings ───────────────────────────────────────────────────┐
│                                                              │
│  Working Mode:                                               │
│                                                              │
│  ● Standalone Orchestration Mode (Recommended)               │
│    └─ Plan Cascade executes all tools itself                │
│       (Read/Write/Edit/Bash)                                │
│    └─ LLM only provides thinking, Plan Cascade executes     │
│       actions                                               │
│    └─ Supports complete PRD-driven development flow         │
│                                                              │
│  ○ Claude Code GUI Mode                                      │
│    └─ Plan Cascade as graphical interface for Claude Code   │
│    └─ Claude Code executes tools, Plan Cascade provides     │
│       visualization                                         │
│    └─ Compatible with all Claude Code features              │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

#### Standalone Mode: LLM Backend Selection

```
┌─ LLM Backend Selection ───────────────────────────────────────┐
│                                                              │
│  ● Claude Max (No API Key Required)                          │
│    └─ Get LLM capability through local Claude Code           │
│    └─ Suitable for: Users with Claude Max but no API Key     │
│                                                              │
│  ○ Claude API                                                │
│    └─ Direct Anthropic API calls                             │
│    └─ Requires API Key configuration                         │
│                                                              │
│  ○ OpenAI                                                    │
│    └─ Use GPT-4o and other models                           │
│    └─ Requires API Key configuration                         │
│                                                              │
│  ○ DeepSeek                                                  │
│    └─ Recommended for users in China                         │
│    └─ Requires API Key configuration                         │
│                                                              │
│  ○ Ollama                                                    │
│    └─ Local models, completely offline                       │
│    └─ Requires Ollama address configuration                  │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

**Essential Difference Between Two Working Modes**:

| | Standalone Orchestration Mode | Claude Code GUI Mode |
|---|---|---|
| **Orchestration Layer** | Plan Cascade | Plan Cascade |
| **Tool Execution** | Plan Cascade executes itself | Claude Code executes |
| **LLM Source** | Multiple (Claude Max/API/OpenAI etc.) | Claude Code |
| **PRD-Driven** | ✅ Supported | ✅ Supported |
| **Batch Execution** | ✅ Supported | ✅ Supported |
| **Use Case** | Need other LLMs or offline use | Have Claude Max/Code subscription |

**Core Philosophy: Plan Cascade = Brain (Orchestration), Execution Layer = Hands (Tool Execution)**

Both modes are controlled by Plan Cascade for the orchestration workflow (PRD generation, dependency analysis, batch scheduling), the difference is only who executes tools:
- Standalone mode: Plan Cascade built-in tool engine executes
- GUI mode: Claude Code CLI executes

#### Supported LLM Backends

| Backend | Priority | Requires API Key | Notes |
|---------|----------|-----------------|-------|
| Claude Max | P0 | No | Get LLM via Claude Code, suitable for Max members |
| Claude API | P0 | Yes | Direct Anthropic API calls |
| OpenAI | P0 | Yes | GPT-4o and other models |
| DeepSeek | P1 | Yes | Users in China |
| Ollama | P2 | No | Local models |

### 4.2 Multi-Agent Collaboration (P0)

Plan Cascade supports multiple Agents working collaboratively, intelligently assigning different tasks to the most suitable Agent:

#### Supported Execution Agents

| Agent | Type | Description |
|-------|------|-------------|
| claude-code | Task Tool / CLI | Default Agent, built-in or via Claude Code CLI |
| codex | CLI | OpenAI Codex CLI |
| aider | CLI | AI pair programming assistant |
| amp-code | CLI | Amp Code CLI |
| cursor-cli | CLI | Cursor CLI |

#### Phase-Based Agent Assignment

```
┌─────────────────────────────────────────────────────────────┐
│                    Multi-Agent Collaboration                 │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   Execution Phase           Default Agent    Fallback Chain  │
│   ─────────────────────────────────────────────────────────  │
│   Planning                  codex           → claude-code    │
│   Implementation            claude-code     → codex → aider  │
│   Retry                     claude-code     → aider          │
│   Refactor                  aider           → claude-code    │
│   Review                    claude-code     → codex          │
│                                                              │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   Story Type                Default Agent                    │
│   ─────────────────────────────────────────────────────────  │
│   feature                   claude-code                      │
│   bugfix                    codex                            │
│   refactor                  aider                            │
│   test                      claude-code                      │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

#### Agent Resolution Priority

```
1. --agent command parameter (explicit override)
2. Phase-specific parameters (--impl-agent, --planning-agent)
3. Agent specified in Story
4. Story type override (bugfix → codex, refactor → aider)
5. Phase default Agent
6. Fallback chain (if Agent unavailable)
7. claude-code (ultimate fallback, always available)
```

### 4.3 Simple Mode Features (P0)

#### One-Click Workflow

```bash
# CLI
plan-cascade "Implement user login functionality, support OAuth"
# Auto-completes: analyze → generate plan → execute → quality check

# GUI
# Enter description → Click "Start" → Wait for completion
```

#### Simplified Status Display

```
Executing...

[████████████░░░░░░░░] 60%

✓ Generate plan (5 tasks)
✓ Database Schema
✓ API route structure
⟳ OAuth login (in progress...)
○ SMS verification login
○ Integration tests
```

### 4.4 Expert Mode Features (P0)

#### PRD Editor

- Visually edit Stories
- Drag-and-drop to adjust order
- Set dependencies
- Specify execution Agent

#### Execution Strategy Selection

```
Execution Strategy:                    AI Suggests: Hybrid Auto
○ Direct Execute (simple task, no PRD needed)
● Hybrid Auto (auto-generate PRD and execute)
○ Mega Plan (large project, multiple PRDs)

Isolation Options:
□ Use Git Worktree for isolated development
```

#### Agent Specification

Each Story can specify a different Agent:

```
┌─ Story: Implement OAuth login ───────────────────────────────┐
│                                                              │
│  Agent: [claude-code ▼]                                      │
│         ├─ claude-code (recommended)                         │
│         ├─ aider                                             │
│         ├─ codex                                             │
│         └─ builtin                                           │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### 4.5 Settings Page (P0)

#### Agent Configuration

```
┌─ Settings > Agent Configuration ─────────────────────────────┐
│                                                              │
│  Primary Backend (for orchestration)                         │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ ● Claude Code (recommended, no configuration needed)   │ │
│  │ ○ Claude API    [API Key: ••••••••••]                  │ │
│  │ ○ OpenAI        [API Key: ••••••••••] [Model: gpt-4o ▼]│ │
│  │ ○ DeepSeek      [API Key: ••••••••••]                  │ │
│  │ ○ Ollama        [URL: http://localhost:11434]          │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  ───────────────────────────────────────────────────────────│
│                                                              │
│  Execution Agents (for Story execution)           [+ Add]    │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ ✓ claude-code                              [Default]   │ │
│  │   └─ Path: claude                                      │ │
│  ├────────────────────────────────────────────────────────┤ │
│  │ ✓ aider                                    [Configure] │ │
│  │   └─ Command: aider --model gpt-4o                     │ │
│  ├────────────────────────────────────────────────────────┤ │
│  │ □ codex (not configured)                   [Configure] │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  Agent Auto-Selection Strategy:                              │
│  ○ Smart matching (auto-select based on task type)          │
│  ● Prefer using: [claude-code ▼]                            │
│  ○ Manual specification (select for each Story individually)│
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

#### Quality Gate Configuration

```
┌─ Settings > Quality Gates ───────────────────────────────────┐
│                                                              │
│  Default enabled checks:                                     │
│  [✓] TypeCheck (tsc / mypy / pyright)                       │
│  [✓] Test (pytest / jest / npm test)                        │
│  [✓] Lint (eslint / ruff)                                   │
│  [ ] Custom                                                  │
│                                                              │
│  Custom script:                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ npm run validate                                        │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  Retry settings:                                             │
│  Maximum retries: [3]                                        │
│  Retry interval: [Exponential backoff ▼]                    │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### 4.6 CLI Features (P1)

```bash
# Simple mode (default)
plan-cascade "Implement user login functionality"
# Auto-completes entire flow

# Expert mode
plan-cascade --expert "Implement user login functionality"

# Expert mode interaction
$ plan-cascade --expert "Implement user login"
✓ PRD generated (5 Stories)

? Select operation:
  > View/Edit PRD
    Modify Agent assignment
    Adjust dependencies
    Start execution
    Save draft and exit

# Step-by-step commands
plan-cascade generate "Implement user login"  # Only generate PRD
plan-cascade review                           # Interactive editing
plan-cascade run                              # Execute
plan-cascade status                           # View status
```

### 4.7 Interactive REPL Mode (P0)

CLI and Desktop both support interactive REPL for continuous dialogue:

```
┌─ Plan Cascade REPL ─────────────────────────────────────────┐
│                                                              │
│  plan-cascade> Analyze the project structure                 │
│                                                              │
│  [AI analyzes and responds...]                               │
│                                                              │
│  plan-cascade> Based on the above analysis, implement user   │
│                login functionality                           │
│                                                              │
│  [Intent recognition: TASK]                                  │
│  [Strategy analysis: hybrid_auto]                            │
│  [Generating PRD...]                                         │
│  [Executing...]                                              │
│                                                              │
│  plan-cascade> /status                                       │
│  Current session: abc123                                     │
│  Mode: simple                                                │
│  Project: /path/to/project                                   │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

**REPL Special Commands**:
- `/exit`, `/quit` - Exit
- `/clear` - Clear context
- `/status` - View session status
- `/mode [simple|expert]` - Switch mode
- `/history` - View conversation history
- `/config` - Configuration management

**Smart Intent Recognition**:
- Rule matching → LLM analysis → User confirmation
- Auto-distinguish TASK / QUERY / CHAT

---

## 5. Non-Functional Requirements

### 5.1 Performance Requirements

| Metric | Requirement |
|--------|-------------|
| Startup Time | < 3 seconds |
| Memory Usage | < 500MB (idle state) |
| Parallel Stories | Support at least 5 parallel |
| API Timeout | Configurable, default 5 minutes |

### 5.2 Compatibility Requirements

| Platform | Minimum Version |
|----------|-----------------|
| Windows | Windows 10 |
| macOS | macOS 11 (Big Sur) |
| Linux | Ubuntu 20.04 / equivalent |
| Python | 3.10+ (CLI/Core) |

### 5.3 Security Requirements

- API Key local encrypted storage
- Don't upload user code to third-party services (except LLM API)
- Shell command execution safety checks
- Sensitive file protection (.env, credentials, etc.)

---

## 6. Competitive Comparison

| Feature | Plan Cascade | Claude Code CLI | Cursor | Aider |
|---------|--------------|-----------------|--------|-------|
| **GUI** | ✅ | ❌ | ✅ | ❌ |
| **CLI Support** | ✅ | ✅ | ❌ | ✅ |
| **Multi-LLM Support** | ✅ | ❌ | ❌ | ✅ |
| **Task Decomposition** | ✅ Auto | ❌ Manual | ❌ | ❌ |
| **Parallel Execution** | ✅ | ❌ | ❌ | ❌ |
| **Quality Gates** | ✅ | ❌ | ❌ | ⚠️ |
| **Simple Mode** | ✅ | ❌ | ✅ | ❌ |
| **Expert Mode** | ✅ | ✅ | ❌ | ✅ |
| **Out of Box** | ✅ | ⚠️ | ✅ | ⚠️ |

---

## 7. Milestone Plan

### Phase 1: CLI + Dual-Mode

**Goal**: Independently runnable CLI, supporting simple/expert modes

**Scope**:
- [ ] Core package refactor
- [ ] LLM Provider abstraction layer
- [ ] Simple mode implementation
- [ ] Expert mode implementation
- [ ] AI auto strategy determination
- [ ] Basic CLI commands

### Phase 2: Desktop Application Alpha

**Goal**: Graphical interface

**Scope**:
- [ ] Tauri desktop framework
- [ ] Simple mode UI
- [ ] Expert mode UI
- [ ] Settings page
- [ ] Claude Code GUI mode

### Phase 3: Feature Completion

**Goal**: Production ready

**Scope**:
- [ ] Complete PRD editor
- [ ] Dependency visualization
- [ ] More LLM backends
- [ ] Auto-update

### Phase 4: Advanced Features

**Goal**: Differentiated advantages

**Scope**:
- [ ] Multi-Agent collaboration
- [ ] Git Worktree integration
- [ ] Team collaboration
- [ ] Plugin system

---

## 8. Success Metrics

| Metric | Target |
|--------|--------|
| User Onboarding Time | < 5 minutes (simple mode) |
| Task Completion Rate | > 80% (simple tasks) |
| User Retention | 30% (monthly active) |
| GitHub Stars | 1000+ (within 6 months) |

---

## 9. Appendix

### 9.1 Glossary (Can be hidden in user manual)

| Term | Definition | User Needs to Understand |
|------|------------|-------------------------|
| Mega Plan | Project-level planning | Expert mode optional |
| Hybrid Ralph | PRD-driven development mode | Expert mode optional |
| Story | Minimum executable task | Expert mode required |
| Batch | Set of tasks that can execute in parallel | Not needed |
| Quality Gate | Quality check | Configurable in settings |
| BuiltinAgent | Built-in Agent | Not needed |

### 9.2 Simple Mode vs Expert Mode Quick Reference

```
Simple Mode suitable for:
✓ New users
✓ Quick prototypes
✓ Single feature development
✓ Scenarios where configuration is unwanted

Expert Mode suitable for:
✓ Senior developers
✓ Complex projects
✓ Need fine-grained control
✓ Multi-person collaboration
```
