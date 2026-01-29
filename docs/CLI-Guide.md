[中文版](CLI-Guide_zh.md)

# Plan Cascade - CLI Guide

**Version**: 4.0.0
**Last Updated**: 2026-01-29

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

CLI is suitable for:
- Need to use other LLMs (OpenAI, DeepSeek, etc.)
- Need offline use (Ollama)
- Prefer command line operations
- Automation script integration

Plugin is suitable for:
- Claude Code power users
- Need full Claude Code functionality
- Prefer /slash command interaction
