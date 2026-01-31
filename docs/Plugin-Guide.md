[中文版](Plugin-Guide_zh.md)

# Plan Cascade - Claude Code Plugin Guide

**Version**: 4.2.2
**Last Updated**: 2026-01-31

This document provides detailed instructions for using Plan Cascade as a Claude Code plugin.

---

## Installation

```bash
# Install from GitHub
claude plugins install Taoidle/plan-cascade

# Or clone and install locally
git clone https://github.com/Taoidle/plan-cascade.git
claude plugins install ./plan-cascade
```

---

## Command Overview

Plan Cascade provides four main entry commands, suitable for development scenarios of different scales:

| Entry Command | Use Case | Features |
|--------------|----------|----------|
| `/plan-cascade:auto` | Any task (AI auto-selects strategy) | Automatic strategy selection + Direct execution |
| `/plan-cascade:mega-plan` | Large projects (multiple related features) | Feature-level parallel + Story-level parallel |
| `/plan-cascade:hybrid-worktree` | Single complex feature | Worktree isolation + Story parallel |
| `/plan-cascade:hybrid-auto` | Simple features | Quick PRD generation + Story parallel |

---

## Design Document System

Plan Cascade automatically generates technical design documents (`design_doc.json`) to provide architectural context during story execution.

### Two-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Level 1: Project Design (from mega-plan.json)               │
│ ─────────────────────────────────────────────────────────── │
│ • Global architecture and system overview                   │
│ • Cross-feature components and patterns                     │
│ • Project-wide ADRs (architectural decisions)               │
│ • Feature mappings (which patterns/decisions apply where)   │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ inheritance
                          ▼
┌─────────────────────────────────────────────────────────────┐
│ Level 2: Feature Design (from prd.json)                     │
│ ─────────────────────────────────────────────────────────── │
│ • Feature-specific components                               │
│ • Feature-specific APIs and data models                     │
│ • Feature-specific ADRs (prefixed ADR-F###)                 │
│ • Story mappings (which components/decisions per story)     │
└─────────────────────────────────────────────────────────────┘
```

### Auto-Generation Flow

| Command | Design Doc Generated | Level |
|---------|---------------------|-------|
| `mega-plan` | Automatic after mega-plan.json | Project |
| `hybrid-worktree` | Automatic after prd.json | Feature (inherits from project) |
| `hybrid-auto` | Automatic after prd.json | Feature |

### External Design Documents

All three main commands support optional external design documents:

```bash
# mega-plan: 2nd argument
/plan-cascade:mega-plan "Build e-commerce platform" ./project-architecture.md

# hybrid-auto: 2nd argument
/plan-cascade:hybrid-auto "Implement user auth" ./auth-design.md

# hybrid-worktree: 4th argument
/plan-cascade:hybrid-worktree fix-auth main "Fix auth" ./architecture.md

# Supported formats: Markdown (.md), JSON (.json), HTML (.html)
```

External documents are automatically converted to the `design_doc.json` format.

### Design Document Commands

```bash
/plan-cascade:design-generate    # Manually generate design document
/plan-cascade:design-review      # Review design document
/plan-cascade:design-import      # Import external document
```

---

## External Framework Skills

Plan Cascade includes built-in framework-specific skills that are automatically detected and injected into story execution context.

### Supported Frameworks

| Framework | Skills | Auto-Detection |
|-----------|--------|----------------|
| React/Next.js | `react-best-practices`, `web-design-guidelines` | `package.json` contains `react` or `next` |
| Vue/Nuxt | `vue-best-practices`, `vue-router-best-practices`, `vue-pinia-best-practices` | `package.json` contains `vue` or `nuxt` |
| Rust | `rust-coding-guidelines`, `rust-ownership`, `rust-error-handling`, `rust-concurrency` | `Cargo.toml` exists |

### How It Works

1. **Detection**: When a story is executed, Plan Cascade scans the project for framework indicators (`package.json`, `Cargo.toml`)
2. **Loading**: Matching skills are loaded from Git submodules in `external-skills/`
3. **Injection**: Skill content is injected into the agent's context during implementation and retry phases

### Initialization

External skills are included as Git submodules. Initialize them after cloning:

```bash
git submodule update --init --recursive
```

### Skill Sources

| Source | Repository | Skills |
|--------|------------|--------|
| Vercel | [vercel-labs/agent-skills](https://github.com/vercel-labs/agent-skills) | React, Web Design |
| Vue.js | [vuejs-ai/skills](https://github.com/vuejs-ai/skills) | Vue, Pinia, Router |
| Rust | [actionbook/rust-skills](https://github.com/actionbook/rust-skills) | Coding Guidelines, Ownership, Error Handling, Concurrency |

---

## `/plan-cascade:auto` - AI Auto Strategy

The easiest entry point. AI analyzes your task description and automatically selects the best strategy.

### How It Works

1. You provide a task description
2. AI performs structured self-assessment analyzing:
   - **Scope**: How many functional areas are involved?
   - **Complexity**: Are there sub-task dependencies? Architecture decisions needed?
   - **Risk**: Could it break existing functionality? Needs isolation?
   - **Parallelization**: Can work be parallelized for efficiency?
3. AI outputs structured analysis with confidence score
4. AI selects optimal strategy and executes without confirmation

### Strategy Selection

| Analysis Result | Strategy | Example |
|----------------|----------|---------|
| 1 area, 1-2 steps, low risk | **direct** | "Fix the login button styling" |
| 2-3 areas, 3-7 steps, has dependencies | **hybrid-auto** | "Implement user authentication" |
| hybrid-auto + high risk or experimental | **hybrid-worktree** | "Experimental refactoring of payment module" |
| 4+ areas, multiple independent features | **mega-plan** | "Build e-commerce platform with users, products, orders" |

### Usage Example

```bash
# AI automatically determines strategy
/plan-cascade:auto "Fix the typo in README"
# → Uses direct strategy

/plan-cascade:auto "Implement user login with OAuth"
# → Uses hybrid-auto strategy

/plan-cascade:auto "Experimental refactoring of the API layer"
# → Uses hybrid-worktree strategy

/plan-cascade:auto "Build a blog platform with users, posts, comments, and RSS"
# → Uses mega-plan strategy
```

---

## `/plan-cascade:mega-plan` Workflow

Suitable for large project development containing multiple related feature modules.

### Use Cases

| Type | Scenario | Example |
|------|----------|---------|
| ✅ Suitable | Multi-module new project development | Build SaaS platform (user + subscription + billing + admin) |
| ✅ Suitable | Large-scale refactoring involving multiple subsystems | Monolith to microservices architecture |
| ✅ Suitable | Feature group development | E-commerce platform (users, products, cart, orders) |
| ❌ Not suitable | Single feature development | Only implement user authentication (use Hybrid Ralph) |
| ❌ Not suitable | Bug fixes | Fix login page form validation issue |

### Sequential Execution Between Batches

Mega-plan uses **sequential execution between batches** mode, ensuring each batch creates worktree from the updated target branch:

```
mega-approve (1st time) → Start Batch 1
    ↓ Batch 1 complete
mega-approve (2nd time) → Merge Batch 1 → Create Batch 2 from updated branch
    ↓ Batch 2 complete
mega-approve (3rd time) → Merge Batch 2 → ...
    ↓ All batches complete
mega-complete → Clean up planning files
```

Key points:
- `mega-approve` needs to be called multiple times (once per batch)
- Each batch creates worktree from **updated target branch**
- Planning files are not committed (added to .gitignore)

### Command Reference

```bash
/plan-cascade:mega-plan <description>        # Generate project plan
/plan-cascade:mega-edit                      # Edit plan
/plan-cascade:mega-approve --auto-prd        # Approve and auto-execute all batches
/plan-cascade:mega-resume --auto-prd         # Resume interrupted execution
/plan-cascade:mega-status                    # View progress
/plan-cascade:mega-complete [branch]         # Merge and cleanup
```

### Full Automation with --auto-prd

With `--auto-prd`, mega-approve runs the ENTIRE mega-plan automatically:
1. Creates worktrees for current batch
2. Generates PRDs for each feature (via Task agents)
3. Executes all stories (via Task agents)
4. Monitors until batch complete
5. Merges batch to target branch
6. Automatically continues to next batch
7. Only pauses on errors or merge conflicts

### Resume Interrupted Execution

If execution is interrupted:
```bash
/plan-cascade:mega-resume --auto-prd
```

This will:
- Auto-detect current state from files (mega-plan.json, .mega-status.json, worktrees)
- Skip already-completed features and stories
- Resume from where it left off

### Usage Example

```bash
# Scenario: Build e-commerce platform
/plan-cascade:mega-plan "Build e-commerce platform: user authentication, product management, shopping cart, order processing"

# View generated plan
/plan-cascade:mega-status

# Edit plan (optional)
/plan-cascade:mega-edit

# Approve first batch
/plan-cascade:mega-approve --auto-prd

# View execution progress
/plan-cascade:mega-status

# After batch completion, approve next batch
/plan-cascade:mega-approve

# Cleanup after all complete
/plan-cascade:mega-complete main
```

---

## `/plan-cascade:hybrid-worktree` Workflow

Suitable for single complex feature development requiring branch isolation.

### Use Cases

| Type | Scenario | Example |
|------|----------|---------|
| ✅ Suitable | Complete feature with multiple subtasks | User authentication (registration + login + password reset) |
| ✅ Suitable | Experimental feature requiring branch isolation | New payment channel integration test |
| ✅ Suitable | Medium-scale refactoring (5-20 files) | API layer unified error handling |
| ❌ Not suitable | Simple single-file modification | Modify a component's style |
| ❌ Not suitable | Quick prototype validation | Verify if a library is usable |

### Command Reference

```bash
/plan-cascade:hybrid-worktree <name> <branch> <desc>  # Create development environment
/plan-cascade:hybrid-auto <desc> [--agent <name>]     # Generate PRD
/plan-cascade:approve [--auto-run]                    # Execute PRD
/plan-cascade:hybrid-resume --auto                    # Resume interrupted execution
/plan-cascade:hybrid-status                           # View status
/plan-cascade:hybrid-complete [branch]                # Complete and merge
```

### Usage Example

```bash
# Create isolated development environment
/plan-cascade:hybrid-worktree feature-auth main "Implement user authentication: login, registration, password reset"

# Generate PRD
/plan-cascade:hybrid-auto "Implement user authentication feature"

# View and edit PRD
/plan-cascade:edit

# Approve and auto-execute
/plan-cascade:approve --auto-run

# View execution progress
/plan-cascade:hybrid-status

# Merge to main after completion
/plan-cascade:hybrid-complete main
```

---

## `/plan-cascade:hybrid-auto` Workflow

Suitable for quick development of simple features without Worktree isolation.

### Command Reference

```bash
/plan-cascade:hybrid-auto <desc> [--agent <name>]  # Generate PRD
/plan-cascade:approve [--auto-run]                 # Execute
/plan-cascade:edit                                 # Edit PRD
/plan-cascade:show-dependencies                    # View dependency graph
```

### Usage Example

```bash
# Quick PRD generation
/plan-cascade:hybrid-auto "Add password reset functionality"

# Approve and auto-execute
/plan-cascade:approve --auto-run
```

---

## Auto-Iteration and Quality Gates

### Start Auto-Iteration

```bash
# Start auto-iteration immediately after approval
/plan-cascade:approve --auto-run

# Or start separately
/plan-cascade:auto-run

# Limit maximum iterations
/plan-cascade:auto-run --mode max_iterations --max-iterations 10

# Execute current batch only
/plan-cascade:auto-run --mode batch_complete
```

### Iteration Modes

| Mode | Description |
|------|-------------|
| `until_complete` | Continue execution until all Stories complete (default) |
| `max_iterations` | Stop after executing at most N iterations |
| `batch_complete` | Stop after executing current batch only |

### Quality Gate Configuration

Configure in `prd.json`:

```json
{
  "quality_gates": {
    "enabled": true,
    "gates": [
      {"name": "typecheck", "type": "typecheck", "required": true},
      {"name": "tests", "type": "test", "required": true},
      {"name": "lint", "type": "lint", "required": false}
    ]
  }
}
```

### View Iteration Status

```bash
/plan-cascade:iteration-status [--verbose]
```

---

## Multi-Agent Collaboration

### Supported Agents

| Agent | Type | Description |
|-------|------|-------------|
| `claude-code` | task-tool | Claude Code Task tool (built-in, always available) |
| `codex` | cli | OpenAI Codex CLI |
| `amp-code` | cli | Amp Code CLI |
| `aider` | cli | Aider AI pair programming assistant |
| `cursor-cli` | cli | Cursor CLI |

### Specifying Agents

**For hybrid-auto (PRD generation):**
```bash
# Use default agent (claude-code)
/plan-cascade:hybrid-auto "Implement user authentication"

# Specify agent for PRD generation
/plan-cascade:hybrid-auto "Implement user authentication" --agent=codex
```

**For approve (story execution):**
```bash
# Global agent override (all stories)
/plan-cascade:approve --agent=codex

# Phase-specific agents
/plan-cascade:approve --impl-agent=claude-code --retry-agent=aider

# Disable auto-fallback
/plan-cascade:approve --agent=codex --no-fallback
```

**For mega-approve (feature execution):**
```bash
# Specify agents for different phases
/plan-cascade:mega-approve --auto-prd --prd-agent=codex --impl-agent=aider

# Global override
/plan-cascade:mega-approve --auto-prd --agent=claude-code
```

### Specifying Agent in PRD

```json
{
  "stories": [
    {
      "id": "story-001",
      "agent": "aider",
      "title": "Refactor data layer",
      ...
    }
  ]
}
```

### Agent Configuration File (agents.json)

```json
{
  "default_agent": "claude-code",
  "agents": {
    "claude-code": {"type": "task-tool"},
    "codex": {"type": "cli", "command": "codex"},
    "aider": {"type": "cli", "command": "aider"}
  },
  "phase_defaults": {
    "implementation": {
      "default_agent": "claude-code",
      "fallback_chain": ["codex", "aider"],
      "story_type_overrides": {
        "refactor": "aider",
        "bugfix": "codex"
      }
    }
  }
}
```

### Agent Priority

```
1. Command parameter --agent              # Highest priority
2. Phase override --impl-agent etc.
3. Story-level agent field
4. Story type override               # bugfix -> codex, refactor -> aider
5. Phase default Agent
6. Fallback chain
7. claude-code                       # Final fallback
```

---

## Complete Command Reference

### Auto Strategy

```bash
/plan-cascade:auto <description>             # AI auto-select and execute strategy
```

### Project-Level (Mega Plan)

```bash
/plan-cascade:mega-plan <description>        # Generate project plan
/plan-cascade:mega-edit                      # Edit plan
/plan-cascade:mega-approve --auto-prd        # Approve and auto-execute all batches
/plan-cascade:mega-resume --auto-prd         # Resume interrupted execution
/plan-cascade:mega-status                    # View progress
/plan-cascade:mega-complete [branch]         # Merge and cleanup
```

### Feature-Level (Hybrid Ralph)

```bash
/plan-cascade:hybrid-worktree <name> <branch> <desc>  # Create development environment
/plan-cascade:hybrid-auto <desc> [--agent <name>]     # Generate PRD
/plan-cascade:approve [--agent <name>] [--auto-run]   # Execute
/plan-cascade:hybrid-resume --auto                    # Resume interrupted execution
/plan-cascade:auto-run [--mode <mode>]                # Auto-iteration
/plan-cascade:iteration-status [--verbose]            # Iteration status
/plan-cascade:agent-config [--action <action>]        # Agent configuration
/plan-cascade:hybrid-status                           # Status
/plan-cascade:agent-status [--story-id <id>]          # Agent status
/plan-cascade:hybrid-complete [branch]                # Complete
/plan-cascade:edit                                    # Edit PRD
/plan-cascade:show-dependencies                       # Dependency graph
```

### Design Documents

```bash
/plan-cascade:design-generate            # Auto-generate design document
/plan-cascade:design-import <path>       # Import external design document
/plan-cascade:design-review              # Review design document
```

### Basic Planning

```bash
/plan-cascade:start                      # Start basic planning
/plan-cascade:worktree <name> <branch>   # Create Worktree
/plan-cascade:complete [branch]          # Complete
```

---

## Status File Reference

| File | Type | Description |
|------|------|-------------|
| `prd.json` | Planning | PRD document |
| `mega-plan.json` | Planning | Project plan |
| `design_doc.json` | Planning | Technical design document |
| `agents.json` | Configuration | Agent configuration |
| `findings.md` | Shared | Findings record |
| `mega-findings.md` | Shared | Project-level findings (mega-plan) |
| `progress.txt` | Shared | Progress log |
| `.mega-status.json` | Status | Mega-plan execution status |
| `.agent-status.json` | Status | Agent status |
| `.iteration-state.json` | Status | Iteration state |
| `.retry-state.json` | Status | Retry record |
| `.hybrid-execution-context.md` | Context | Hybrid task context for AI recovery |
| `.mega-execution-context.md` | Context | Mega-plan context for AI recovery |
| `.agent-outputs/` | Output | Agent logs |

---

## Troubleshooting

### Agent Unavailable

```
[AgentExecutor] Agent 'codex' unavailable (CLI 'codex' not found in PATH)
```

Solution: Install the corresponding Agent or use `--no-fallback` to disable fallback.

### Quality Gate Failure

```
[QualityGate] typecheck failed: error TS2304
```

Solution: Fix type errors and retry, or disable that gate in prd.json.

### Worktree Conflict

```
fatal: 'feature-xxx' is already checked out
```

Solution: Use `/plan-cascade:hybrid-complete` to clean up existing worktree.

### Interrupted Execution

If a mega-plan or hybrid task was interrupted (e.g., connection lost, Claude Code crashed):

```bash
# For mega-plan
/plan-cascade:mega-resume --auto-prd

# For hybrid task
/plan-cascade:hybrid-resume --auto
```

These commands will:
- Auto-detect current state from existing files
- Skip already-completed work
- Resume from where execution stopped
- Support both old and new progress marker formats

### Context Recovery After Long Sessions

Plan Cascade automatically generates context files that help recover execution state after:
- Context compression (AI summarizes old messages)
- Context truncation (old messages deleted)
- New conversation session
- Claude Code restart

**Context files generated:**
| File | Mode | Description |
|------|------|-------------|
| `.hybrid-execution-context.md` | Hybrid | Current batch, pending stories, progress summary |
| `.mega-execution-context.md` | Mega Plan | Active worktrees, parallel execution state |

These files are automatically updated via hooks during execution. If you notice the AI has lost context:

```bash
# Universal recovery command (auto-detects mode)
/plan-cascade:resume

# Or check the context file directly
cat .hybrid-execution-context.md
cat .mega-execution-context.md
```
