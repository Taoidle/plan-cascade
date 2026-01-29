[中文版](Plugin-Guide_zh.md)

# Plan Cascade - Claude Code Plugin Guide

**Version**: 4.1.0
**Last Updated**: 2026-01-29

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

## `/plan-cascade:auto` - AI Auto Strategy

The easiest entry point. AI analyzes your task description and automatically selects the best strategy.

### How It Works

1. You provide a task description
2. AI analyzes keywords and patterns
3. AI selects optimal strategy (direct, hybrid-auto, hybrid-worktree, or mega-plan)
4. Executes the strategy without confirmation

### Strategy Selection

| Strategy | Trigger Keywords | Example |
|----------|------------------|---------|
| **direct** | fix, typo, update, simple, single | "Fix the login button styling" |
| **hybrid-auto** | implement, create, feature, api | "Implement user authentication" |
| **hybrid-worktree** | experimental, refactor, isolated | "Experimental refactoring of payment module" |
| **mega-plan** | platform, system, 3+ modules | "Build e-commerce platform with users, products, orders" |

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
/plan-cascade:mega-approve [--auto-prd]      # Approve and execute
/plan-cascade:mega-status                    # View progress
/plan-cascade:mega-complete [branch]         # Merge and cleanup
```

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

```bash
# Use default agent (claude-code)
/plan-cascade:hybrid-auto "Implement user authentication"

# Specify using codex for execution
/plan-cascade:hybrid-auto "Implement user authentication" --agent codex

# Use different Agents for different phases
/plan-cascade:approve --impl-agent claude-code --retry-agent aider

# Disable auto-fallback
/plan-cascade:approve --agent codex --no-fallback
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
/plan-cascade:mega-approve [--auto-prd]      # Approve and execute
/plan-cascade:mega-status                    # View progress
/plan-cascade:mega-complete [branch]         # Merge and cleanup
```

### Feature-Level (Hybrid Ralph)

```bash
/plan-cascade:hybrid-worktree <name> <branch> <desc>  # Create development environment
/plan-cascade:hybrid-auto <desc> [--agent <name>]     # Generate PRD
/plan-cascade:approve [--agent <name>] [--auto-run]   # Execute
/plan-cascade:auto-run [--mode <mode>]                # Auto-iteration
/plan-cascade:iteration-status [--verbose]            # Iteration status
/plan-cascade:agent-config [--action <action>]        # Agent configuration
/plan-cascade:hybrid-status                           # Status
/plan-cascade:agent-status [--story-id <id>]          # Agent status
/plan-cascade:hybrid-complete [branch]                # Complete
/plan-cascade:edit                                    # Edit PRD
/plan-cascade:show-dependencies                       # Dependency graph
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
| `agents.json` | Configuration | Agent configuration |
| `findings.md` | Shared | Findings record |
| `progress.txt` | Shared | Progress log |
| `.agent-status.json` | Status | Agent status |
| `.iteration-state.json` | Status | Iteration state |
| `.retry-state.json` | Status | Retry record |
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
