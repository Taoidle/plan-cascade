[中文版](CLI-Guide_zh.md)

# Plan Cascade - CLI Guide

**Version**: 4.3.3
**Last Updated**: 2026-02-02

This document provides detailed instructions for using the Plan Cascade standalone CLI tool.

---

## Installation

```bash
# Install from PyPI
pip install plan-cascade

# Install with LLM support
pip install plan-cascade[llm]

# Install all dependencies
pip install plan-cascade[all]
```

---

## Quick Start

```bash
# Configuration wizard (first-time use)
plan-cascade config --setup

# Simple mode - one-click execution
plan-cascade run "Implement user login functionality"

# Expert mode - more control
plan-cascade run "Implement user login functionality" --expert

# Interactive chat mode
plan-cascade chat

# Auto-run with parallel execution (NEW)
plan-cascade auto-run --parallel

# Mega-plan for large projects (NEW)
plan-cascade mega plan "Build e-commerce platform"
```

---

## Dual-Mode Design

### Simple Mode (Default)

Designed for new users and quick tasks, AI automatically determines strategy and executes.

```bash
plan-cascade run "Add an exit button"
# -> AI determines: Small task -> Direct execution

plan-cascade run "Implement user login functionality"
# -> AI determines: Medium feature -> Generate PRD -> Auto-execute

plan-cascade run "Build e-commerce platform: users, products, orders"
# -> AI determines: Large project -> Mega Plan -> Multi-PRD cascade
```

### Expert Mode

Designed for experienced users, provides fine-grained control.

```bash
plan-cascade run "Implement user login" --expert
```

Expert mode supports:
- View and edit PRD
- Select execution strategy
- Specify Agent for each Story
- Adjust dependencies
- Configure quality gates

---

## Global Options

The following options are available for all commands:

```bash
--legacy-mode            Use legacy path mode (store files in project root instead of user directory)
--project <path>         Project path (default: current directory)
--verbose                Enable verbose output
```

**Legacy Mode**: By default, Plan Cascade stores planning files in a platform-specific user directory (`~/.plan-cascade/<project-id>/` on Unix or `%APPDATA%/plan-cascade/<project-id>/` on Windows). Use `--legacy-mode` to store files in the project root directory instead (compatible with older versions).

---

## Command Reference

### run - Execute Tasks

```bash
plan-cascade run <description> [options]

Options:
  -e, --expert           Expert mode
  -b, --backend <name>   Backend selection (claude-code|claude-api|openai|deepseek|ollama)
  --model <name>         Specify model
  --project <path>       Project path
```

Examples:

```bash
# Simple mode
plan-cascade run "Add search functionality"

# Expert mode
plan-cascade run "Refactor user module" --expert

# Using OpenAI
plan-cascade run "Implement comment feature" --backend openai --model gpt-4o
```

### config - Configuration Management

```bash
plan-cascade config [options]

Options:
  --show     Display current configuration
  --setup    Run configuration wizard
```

Examples:

```bash
# View configuration
plan-cascade config --show

# Configuration wizard
plan-cascade config --setup
```

### chat - Interactive REPL

```bash
plan-cascade chat [options]

Options:
  -p, --project <path>   Project path
  -b, --backend <name>   Backend selection
```

REPL Special Commands:

| Command | Description |
|---------|-------------|
| `/exit`, `/quit` | Exit |
| `/clear` | Clear context |
| `/status` | View session status |
| `/mode [simple\|expert]` | Switch mode |
| `/history` | View conversation history |
| `/config` | Configuration management |
| `/help` | Help |

Examples:

```bash
plan-cascade chat

> Analyze the project structure
(AI analyzes and responds)

> Based on the above analysis, implement user login functionality
(Intent recognition: TASK)
(Strategy analysis)
(Execute task)

> /status
Session: abc123
Mode: simple
Project: /path/to/project

> /mode expert
Mode changed to: expert

> /exit
```

### status - View Status

```bash
plan-cascade status

# Example output:
Task: Implement user login
Progress: 3/5
  ✓ Design database Schema
  ✓ Implement API routes
  ✓ OAuth login
  ⟳ SMS verification login (in progress)
  ○ Integration tests
```

### version - Version Information

```bash
plan-cascade version
```

### auto-run - Automatic Batch Execution (NEW)

Automatically iterate through PRD batches until completion with quality gates and retry management.

```bash
plan-cascade auto-run [options]

Options:
  -m, --mode <mode>        Iteration mode (until_complete|max_iterations|batch_complete)
  --max-iterations <n>     Maximum iterations for max_iterations mode (default: 10)
  -a, --agent <name>       Default agent for story execution
  --impl-agent <name>      Agent for implementation stories
  --retry-agent <name>     Agent to use for retry attempts
  --dry-run                Show execution plan without running
  --no-quality-gates       Disable quality gates (typecheck, test, lint)
  --verify                 Enable AI verification gate (default: from config)
  --no-verify              Disable AI verification gate
  --verify-agent <name>    Agent for AI verification (default: from config)
  --no-review              Disable AI code review gate
  --review-agent <name>    Agent for AI code review (default: from config)
  --no-fallback            Disable agent fallback on failure
  --parallel               Execute stories within batches in parallel
  --max-concurrency <n>    Maximum parallel stories (default: CPU count)
  -p, --project <path>     Project path
```

Examples:

```bash
# Run until all stories complete
plan-cascade auto-run

# Run with parallel execution
plan-cascade auto-run --parallel --max-concurrency 4

# Limit to 5 iterations
plan-cascade auto-run --mode max_iterations --max-iterations 5

# Dry run to see execution plan
plan-cascade auto-run --dry-run

# Use specific agents
plan-cascade auto-run --agent aider --retry-agent claude-code
```

### mega - Mega-Plan Workflow (NEW)

Commands for managing multi-feature project plans.

```bash
plan-cascade mega <subcommand> [options]

Subcommands:
  plan <description>    Generate multi-feature plan
  approve               Start execution of approved plan
  status                View execution progress
  complete              Finalize and merge all features
  edit                  Interactively edit features
  resume                Resume interrupted execution
```

Examples:

```bash
# Generate mega-plan
plan-cascade mega plan "Build e-commerce platform with users, products, orders"

# Approve and start execution
plan-cascade mega approve --auto-prd

# Check status
plan-cascade mega status --verbose

# Complete when done
plan-cascade mega complete
```

### worktree - Git Worktree Integration (NEW)

Commands for managing isolated development environments using Git worktrees.

```bash
plan-cascade worktree <subcommand> [options]

Subcommands:
  create <name> <branch> [desc]   Create isolated worktree
  complete [name] [options]        Merge and cleanup worktree
  list                             List active worktrees

Complete Options:
  --force                  Force completion even with uncommitted changes (changes will be lost)
  --no-merge               Skip merge to target branch
```

Examples:

```bash
# Create worktree for a feature
plan-cascade worktree create feature-auth main "Implement authentication"

# List all worktrees
plan-cascade worktree list

# Complete and merge
plan-cascade worktree complete feature-auth
```

### design - Design Document System (NEW)

Commands for managing architectural design documents.

```bash
plan-cascade design <subcommand> [options]

Subcommands:
  generate              Generate design_doc.json (auto-detects level)
  show                  Display current design document
  review                Interactive editing of design document
  import <file>         Convert external document (MD, JSON, HTML)
  validate              Validate design document structure
```

Examples:

```bash
# Generate design document
plan-cascade design generate

# Show design document
plan-cascade design show --verbose

# Import from Markdown
plan-cascade design import ./architecture.md

# Interactive review
plan-cascade design review
```

### skills - External Skill Management (NEW)

Commands for managing framework-specific skills.

```bash
plan-cascade skills <subcommand> [options]

Subcommands:
  list                  List all configured skills
  detect                Detect applicable skills for project
  show <name>           Display skill content
  summary               Show skills that will be loaded
  validate              Validate skill configuration
```

Examples:

```bash
# List all skills
plan-cascade skills list --verbose

# Detect applicable skills
plan-cascade skills detect --phase implementation

# Show specific skill
plan-cascade skills show react-best-practices
```

### deps - Dependency Graph Visualization (NEW)

Display visual dependency graph for stories/features.

```bash
plan-cascade deps [options]

Options:
  -f, --format <type>    Output format (tree|flat|table|json)
  --critical-path        Show critical path analysis
  --check                Check for dependency issues
  -p, --project <path>   Project path
```

Examples:

```bash
# Show dependency tree
plan-cascade deps

# Show as table
plan-cascade deps --format table

# Check for issues
plan-cascade deps --check

# Output as JSON
plan-cascade deps --format json
```

### migrate - Path Migration (NEW)

Migrate planning files between legacy mode (project root) and new mode (user directory).

```bash
plan-cascade migrate <subcommand> [options]

Subcommands:
  detect                Scan for legacy files in project
  run [--dry-run]       Migrate to new path mode
  rollback              Revert to legacy mode
```

Examples:

```bash
# Detect legacy files
plan-cascade migrate detect

# Preview migration without making changes
plan-cascade migrate run --dry-run

# Perform actual migration
plan-cascade migrate run

# Rollback if needed
plan-cascade migrate rollback
```

### resume - Context Recovery (NEW)

Auto-detect and resume interrupted tasks.

```bash
plan-cascade resume [options]

Options:
  -a, --auto             Non-interactive resume
  -v, --verbose          Show detailed state information
  -j, --json             Output as JSON
  -p, --project <path>   Project path
```

Examples:

```bash
# Show recovery plan
plan-cascade resume

# Auto-resume without prompts
plan-cascade resume --auto

# Verbose output
plan-cascade resume --verbose
```

---

## LLM Backend Configuration

### Supported Backends

| Backend | Requires API Key | Description |
|---------|-----------------|-------------|
| `claude-code` | No | Via Claude Code CLI (default) |
| `claude-max` | No | Get LLM via Claude Code |
| `claude-api` | Yes | Direct Anthropic API calls |
| `openai` | Yes | OpenAI GPT-4o, etc. |
| `deepseek` | Yes | DeepSeek Chat/Coder |
| `ollama` | No | Local models |

### Configuration Examples

```bash
# Use configuration wizard
plan-cascade config --setup

# Select backend:
#   1. Claude Code (recommended, no API Key required)
#   2. Claude API
#   3. OpenAI
#   4. DeepSeek
#   5. Ollama (local)
```

### Environment Variables

```bash
# Claude API
export ANTHROPIC_API_KEY=sk-ant-...

# OpenAI
export OPENAI_API_KEY=sk-...

# DeepSeek
export DEEPSEEK_API_KEY=sk-...

# Ollama
export OLLAMA_BASE_URL=http://localhost:11434
```

---

## AI Automatic Strategy Determination

In simple mode, AI automatically selects the best execution strategy based on requirements:

| Input | AI Determination | Execution Strategy |
|-------|-----------------|-------------------|
| "Add an exit button" | Small task | Direct execution (no PRD) |
| "Implement user login functionality" | Medium feature | Hybrid Auto (auto-generate PRD) |
| "Develop a blog system with users, articles, comments" | Large project | Mega Plan (multi-PRD cascade) |
| "Refactor payment module without affecting existing functionality" | Requires isolation | Hybrid Worktree |

Determination Dimensions:
1. **Task Scale**: Single task / Multiple features / Complete project
2. **Complexity**: Whether decomposition into multiple Stories is needed
3. **Risk Level**: Whether isolated development is needed
4. **Dependencies**: Whether there are cross-module dependencies

---

## Expert Mode Details

### Workflow

```
1. Enter requirement description
       ↓
2. Generate PRD
       ↓
3. Interactive menu
   ├── view    - View PRD
   ├── edit    - Edit PRD
   ├── agent   - Specify Agent
   ├── run     - Execute
   ├── save    - Save draft
   └── quit    - Exit
       ↓
4. Execute and monitor
```

### Interactive Example

```bash
$ plan-cascade run "Implement user login" --expert

✓ PRD generated (5 Stories)

? Select operation:
  > view   - View PRD
    edit   - Edit PRD
    agent  - Specify Agent
    run    - Start execution
    save   - Save draft
    quit   - Exit
```

### PRD Editing

```bash
? Select content to edit:
  > Modify Story
    Add Story
    Delete Story
    Adjust dependencies
    Modify priority
    Return
```

### Agent Assignment

```bash
? Assign Agent for Story:
  Story 1: Design database Schema
  > claude-code (recommended)
    aider
    codex

  Story 2: Implement OAuth login
  > aider
    claude-code
    codex
```

---

## Configuration File

Configuration file located at `~/.plan-cascade/config.yaml`:

```yaml
# Backend configuration
backend: claude-code  # claude-code | claude-api | openai | deepseek | ollama
provider: claude      # claude | openai | deepseek | ollama
model: ""            # Leave empty for default

# Execution Agents
agents:
  - name: claude-code
    enabled: true
    command: claude
    is_default: true
  - name: aider
    enabled: true
    command: aider --model gpt-4o
  - name: codex
    enabled: false
    command: codex

# Agent selection strategy
agent_selection: prefer_default  # smart | prefer_default | manual
default_agent: claude-code

# Quality gates
quality_gates:
  typecheck: true
  test: true
  lint: true
  custom: false
  custom_script: ""
  max_retries: 3

# AI Verification Gate
verification_gate:
  enabled: true              # Enable AI verification (default: true)
  confidence_threshold: 0.7  # Minimum confidence for pass
  timeout: 120               # Verification timeout in seconds
  skeleton_detection:
    patterns: ["pass", "...", "NotImplementedError", "TODO", "FIXME", "stub"]
    strict: true             # Fail on any skeleton code detected

# Execution configuration
max_parallel_stories: 3
max_iterations: 50
timeout_seconds: 300

# UI configuration
default_mode: simple  # simple | expert
theme: system        # light | dark | system
```

---

## Troubleshooting

### API Key Not Configured

```
Error: Claude API key is required
```

Solution:

```bash
plan-cascade config --setup
# Or set environment variable
export ANTHROPIC_API_KEY=sk-ant-...
```

### Backend Unavailable

```
Error: Backend 'ollama' not available
```

Solution: Ensure Ollama is started and running on the correct port.

### Model Not Supported

```
Error: Model 'gpt-5' not found
```

Solution: Check if the model name is correct, use `--model` to specify a valid model.

---

## Differences from Claude Code Plugin

| Feature | CLI | Plugin |
|---------|-----|--------|
| Installation | pip install | claude plugins install |
| Usage | Command line | /slash commands |
| Backend support | Multiple LLMs | Claude Code |
| Tool execution | Built-in ReAct | Claude Code |
| Offline use | Supported (Ollama) | Not supported |
| Mega-plan workflow | Supported | Supported |
| Worktree integration | Supported | Supported |
| Design documents | Supported | Supported |
| External skills | Supported | Supported |
| Parallel execution | Supported | Supported |
| Context recovery | Supported | Supported |
| Dependency visualization | Supported | Supported |

CLI is suitable for:
- Need to use other LLMs (OpenAI, DeepSeek, etc.)
- Need offline use (Ollama)
- Prefer command line operations
- Automation script integration
- CI/CD pipeline integration

Plugin is suitable for:
- Claude Code power users
- Need full Claude Code functionality
- Prefer /slash command interaction
- Interactive development workflow
