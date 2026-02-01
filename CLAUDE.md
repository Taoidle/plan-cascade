# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Plan Cascade is an AI-powered cascading development framework that decomposes complex projects into parallel executable tasks with multi-agent collaboration. It operates as a Claude Code Plugin (stable), MCP Server (stable), Standalone CLI (in development), and Desktop App (in development).

## Build & Development Commands

```bash
# Install dependencies
pip install -e ".[all]"

# Run tests
pytest tests/                          # All tests
pytest tests/test_prd_generator.py     # Single test file

# Linting
ruff check src/

# Type checking
mypy src/

# Initialize external framework skills
git submodule update --init --recursive
```

**Entry points**: `plan-cascade` (CLI) and `plan-cascade-mcp` (MCP server)

## Architecture

### Three-Tier Execution Model

```
Mega Plan (project-level) → mega-plan.json + design_doc.json
    ↓
Hybrid Ralph (feature-level) → prd.json + design_doc.json
    ↓
Story Execution (task-level) → Code changes
```

### Key Directories

- `src/plan_cascade/core/` - Orchestration engines (PRD generation, parallel execution, quality gates, retry management)
- `src/plan_cascade/backends/` - Agent abstraction layer with two implementations:
  - `claude_code.py` - ClaudeCodeBackend: subprocess-based, no API key needed
  - `builtin.py` - BuiltinBackend: direct LLM API with ReAct loop
- `src/plan_cascade/state/` - Thread-safe state management with platform-specific file locking
- `src/plan_cascade/llm/` - LLM provider abstraction (Anthropic, OpenAI, DeepSeek, Ollama)
- `commands/` - 23 Plugin command definitions (markdown files)
- `skills/` - Plugin skills providing execution context
- `mcp_server/` - FastMCP server implementation
- `external-skills/` - Git submodules with framework-specific best practices (React, Vue, Rust)

### Core Components

**Strategy Selection** (`core/strategy.py`, `core/intent_classifier.py`): Analyzes tasks and selects execution mode (DIRECT, HYBRID_AUTO, HYBRID_WORKTREE, MEGA_PLAN).

**PRD/Mega Generation** (`core/prd_generator.py`, `core/mega_generator.py`): Decomposes tasks into stories with dependencies.

**Parallel Execution** (`core/parallel_executor.py`, `core/orchestrator.py`): Executes stories in dependency-resolved batches.

**Quality Gates** (`core/quality_gate.py`): TypeCheck, Test, Lint validation after each story.

**Agent Selection** (`backends/agent_executor.py`): Matches agents to story types with fallback chains.

**State Persistence** (`state/state_manager.py`): Thread-safe file operations with Unix/Windows locking.

**Context Recovery** (`state/context_recovery.py`): Auto-generates `.hybrid-execution-context.md` and `.mega-execution-context.md` for session recovery.

### Design Document Hierarchy

Two-level architecture where feature docs inherit from project docs:
- **Project-level**: Global architecture, cross-feature components, project-wide ADRs
- **Feature-level**: Feature-specific components, APIs, story-component mappings, feature ADRs (prefixed ADR-F###)

### External Framework Skills

Auto-detected from project files and injected into story execution:
- React/Next.js: detected via `package.json`
- Vue/Nuxt: detected via `package.json`
- Rust: detected via `Cargo.toml`

Skills loaded from Git submodules in `external-skills/`.

## Key Patterns

**Dual Backend Architecture**: ClaudeCodeBackend (GUI mode, subprocess) and BuiltinBackend (autonomous mode, direct API) share the same `AgentBackend` interface.

**File-Based AI State**: All state persisted to JSON/markdown files (`prd.json`, `mega-plan.json`, `.iteration-state.json`, `.agent-status.json`) enabling recovery and multi-agent coordination.

**Command as Documentation**: Each `commands/*.md` file contains step-by-step AI instructions with CRITICAL markers for mandatory tool usage.

**Quality-Gated Execution**: RetryManager with exponential backoff (1s→16s, max 5 retries) and different retry agents for failures.

## Plugin Commands

Main commands (see `commands/` for full list):
- `/plan-cascade:auto` - AI auto-strategy selection
- `/plan-cascade:mega-plan` - Multi-feature project planning
- `/plan-cascade:hybrid-auto` - Feature-level with auto-PRD generation
- `/plan-cascade:hybrid-worktree` - Isolated Git worktree execution
- `/plan-cascade:approve` - Approval and execution flow
- `/plan-cascade:resume` - Recovery from interruption
