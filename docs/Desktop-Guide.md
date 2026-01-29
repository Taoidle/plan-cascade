[中文版](Desktop-Guide_zh.md)

# Plan Cascade - Desktop Application Guide

**Version**: 4.0.0
**Last Updated**: 2026-01-29
**Status**: Planning (Phase 2)

This document introduces how to use the Plan Cascade Desktop application.

---

## Overview

Plan Cascade Desktop is a graphical version of the CLI, providing the same functionality as the CLI with a more intuitive interface.

### Core Features

- **Dual-mode switching** - One-click switch between simple mode/expert mode
- **Visual PRD editing** - Drag and drop, dependency visualization
- **Real-time progress monitoring** - Graphical execution progress
- **Tool call preview** - Real-time display of Agent operations
- **File change diff** - Real-time code change preview
- **Claude Code GUI** - Can serve as a graphical frontend for Claude Code

---

## Installation

### Windows

Download `.msi` or `.exe` installer:

```
plan-cascade-4.0.0-x64.msi
plan-cascade-4.0.0-setup.exe
```

### macOS

Download `.dmg` installer:

```
plan-cascade-4.0.0-x64.dmg      # Intel
plan-cascade-4.0.0-arm64.dmg    # Apple Silicon
```

### Linux

Download `.AppImage` or `.deb`:

```
plan-cascade-4.0.0-x86_64.AppImage
plan-cascade-4.0.0-amd64.deb
```

Download the latest version from [GitHub Releases](https://github.com/Taoidle/plan-cascade/releases).

---

## Interface Overview

```
┌─────────────────────────────────────────────────────────────────┐
│  Plan Cascade                        [Simple ▼] [⚙ Settings]    │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─ Task Input ─────────────────────────────────────────────────┐│
│  │                                                              ││
│  │  Implement user login functionality with OAuth and SMS       ││
│  │                                                              ││
│  │                                        [Start Execution]     ││
│  └──────────────────────────────────────────────────────────────┘│
│                                                                  │
│  ┌─ Execution Progress ─────────────────────────────────────────┐│
│  │                                                              ││
│  │  [████████████░░░░░░░░] 60%                                  ││
│  │                                                              ││
│  │  ✓ Generate plan (5 tasks)                                   ││
│  │  ✓ Database Schema                                           ││
│  │  ✓ API route structure                                       ││
│  │  ⟳ OAuth login (in progress...)                              ││
│  │  ○ SMS verification login                                    ││
│  │  ○ Integration tests                                         ││
│  │                                                              ││
│  └──────────────────────────────────────────────────────────────┘│
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Simple Mode

### Usage Flow

1. **Enter requirements** - Describe the functionality you want to implement in the text box
2. **Click execute** - AI automatically analyzes, plans, and executes
3. **Wait for completion** - View real-time progress

### Interface Elements

- **Input box** - Supports multi-line input
- **Progress bar** - Overall completion percentage
- **Task list** - Stories execution status
- **Log panel** - Error and warning information

---

## Expert Mode

### Usage Flow

1. **Enter requirements** - Describe functionality
2. **Generate PRD** - View AI-generated plan
3. **Edit and adjust** - Modify Stories, dependencies, Agent
4. **Execute and monitor** - Detailed logs and progress

### PRD Editor

```
┌─ PRD Editor ─────────────────────────────────────────────────────┐
│                                                                   │
│  Execution Strategy:                      AI Suggests: Hybrid Auto │
│  ○ Direct Execute   ● Hybrid Auto   ○ Mega Plan                   │
│                                                                   │
│  □ Use Git Worktree for isolated development                      │
│                                                                   │
│  Stories:                                             [+ Add]     │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ ☐ Design database Schema                                     │ │
│  │   Priority: high  │  Agent: [claude-code ▼]                  │ │
│  │   Dependencies: none                         [Edit] [Delete] │ │
│  ├─────────────────────────────────────────────────────────────┤ │
│  │ ☐ Implement OAuth login                                      │ │
│  │   Priority: medium │  Agent: [aider ▼]                       │ │
│  │   Dependencies: [Schema ▼]                   [Edit] [Delete] │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│  Quality Gates: [✓] TypeCheck  [✓] Test  [✓] Lint  [ ] Custom    │
│                                                                   │
│                               [Save Draft]  [Start Execution]     │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

### Dependency Visualization

```
┌─ Dependency Graph ───────────────────────────────────────────────┐
│                                                                   │
│         ┌─────────────┐                                          │
│         │   Schema    │                                          │
│         └──────┬──────┘                                          │
│                │                                                  │
│       ┌────────┴────────┐                                        │
│       ▼                 ▼                                        │
│  ┌─────────┐       ┌─────────┐                                   │
│  │  OAuth  │       │   SMS   │                                   │
│  └────┬────┘       └────┬────┘                                   │
│       │                 │                                        │
│       └────────┬────────┘                                        │
│                ▼                                                  │
│         ┌─────────────┐                                          │
│         │    Test     │                                          │
│         └─────────────┘                                          │
│                                                                   │
│  Batch 1: [Schema]                                               │
│  Batch 2: [OAuth, SMS]                                           │
│  Batch 3: [Test]                                                 │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

---

## Settings Page

### Backend Configuration

```
┌─ Settings > Backend Configuration ───────────────────────────────┐
│                                                                   │
│  Primary Backend:                                                 │
│  ● Claude Code (recommended, no API Key required)                │
│  ○ Claude API    [API Key: ••••••••••]                           │
│  ○ OpenAI        [API Key: ••••••••••] [Model: gpt-4o ▼]         │
│  ○ DeepSeek      [API Key: ••••••••••]                           │
│  ○ Ollama        [URL: http://localhost:11434]                   │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

### Agent Configuration

```
┌─ Settings > Agent Configuration ─────────────────────────────────┐
│                                                                   │
│  Execution Agents:                                    [+ Add]     │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ ✓ claude-code                                    [Default]   │ │
│  │   └─ Path: claude                                            │ │
│  ├─────────────────────────────────────────────────────────────┤ │
│  │ ✓ aider                                          [Configure] │ │
│  │   └─ Command: aider --model gpt-4o                           │ │
│  ├─────────────────────────────────────────────────────────────┤ │
│  │ □ codex (not configured)                         [Configure] │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│  Agent Auto-Selection Strategy:                                   │
│  ○ Smart matching (auto-select based on task type)               │
│  ● Prefer using: [claude-code ▼]                                 │
│  ○ Manual specification (select for each Story individually)     │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

### Quality Gate Configuration

```
┌─ Settings > Quality Gates ───────────────────────────────────────┐
│                                                                   │
│  Default enabled checks:                                          │
│  [✓] TypeCheck (tsc / mypy / pyright)                            │
│  [✓] Test (pytest / jest / npm test)                             │
│  [✓] Lint (eslint / ruff)                                        │
│  [ ] Custom                                                       │
│                                                                   │
│  Custom script:                                                   │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ npm run validate                                             │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│  Retry settings:                                                  │
│  Maximum retries: [3]                                             │
│  Retry interval: [Exponential backoff ▼]                         │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

---

## Claude Code GUI Mode

Desktop can serve as a complete graphical interface for Claude Code:

```
┌─ Claude Code GUI Mode ───────────────────────────────────────────┐
│                                                                   │
│  ┌─ Chat View ───────────────────────────────────────────────┐   │
│  │ User: Implement user login functionality                   │   │
│  │                                                            │   │
│  │ Claude: I'll help you implement user login. First let me   │   │
│  │         analyze the project...                             │   │
│  │                                                            │   │
│  │ [Reading src/auth/...]                                     │   │
│  │ [Writing src/auth/login.py...]                             │   │
│  │                                                            │   │
│  └────────────────────────────────────────────────────────────┘   │
│                                                                   │
│  ┌─ Tool Calls ──────────────────────────────────────────────┐   │
│  │ ✓ Read src/auth/base.py                                   │   │
│  │ ✓ Read src/auth/oauth.py                                  │   │
│  │ ⟳ Edit src/auth/login.py                                  │   │
│  └────────────────────────────────────────────────────────────┘   │
│                                                                   │
│  ┌─ File Change Preview ─────────────────────────────────────┐   │
│  │ - old_code()                                               │   │
│  │ + new_code()                                               │   │
│  └────────────────────────────────────────────────────────────┘   │
│                                                                   │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │ Enter message...                                    [Send] │  │
│  └────────────────────────────────────────────────────────────┘  │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

---

## Tech Stack

| Component | Technology | Description |
|-----------|------------|-------------|
| Framework | Tauri | Lightweight cross-platform (~10MB) |
| Frontend | React + TypeScript | Mature ecosystem |
| State Management | Zustand | Lightweight and easy to use |
| UI Components | Radix UI + Tailwind | Good accessibility |
| Backend | Python Sidecar (FastAPI) | Reuse core code |

---

## Development Progress

Desktop application is currently in **Phase 2** planning stage:

| Feature | Status |
|---------|--------|
| Tauri framework setup | ⏳ Planning |
| Simple mode UI | ⏳ Planning |
| Expert mode UI | ⏳ Planning |
| PRD editor | ⏳ Planning |
| Dependency graph visualization | ⏳ Planning |
| Claude Code GUI | ⏳ Planning |
| Settings page | ⏳ Planning |
| Multi-platform builds | ⏳ Planning |

Expected release: After Phase 2 completion

---

## Differences from CLI

| Feature | Desktop | CLI |
|---------|---------|-----|
| Interface | Graphical | Command line |
| PRD editing | Visual drag-and-drop | Text editing |
| Progress monitoring | Real-time graphics | Text output |
| Dependencies | Visual graph | Text list |
| File changes | Diff preview | Log output |
| Use case | Prefer GUI | Prefer CLI |

Both share the same core code and have identical functionality.
