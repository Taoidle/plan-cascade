[中文版](README_zh.md)

# Plan Cascade

> **Three-Tier Cascading Parallel Development Framework** — Decompose from project to feature to story, execute in parallel at each level

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Claude Code Plugin](https://img.shields.io/badge/Claude%20Code-Plugin-blue)](https://claude.ai/code)
[![MCP Server](https://img.shields.io/badge/MCP-Server-purple)](https://modelcontextprotocol.io)
[![Version](https://img.shields.io/badge/version-4.1.0-brightgreen)](https://github.com/Taoidle/plan-cascade)
[![PyPI](https://img.shields.io/pypi/v/plan-cascade)](https://pypi.org/project/plan-cascade/)

---

## Overview

Plan Cascade is a **three-tier cascading AI parallel development framework** designed for large-scale software projects. It progressively decomposes complex projects and achieves efficient parallel development through multi-agent collaboration.

### Core Concepts

- **Progressive Decomposition**: Project → Feature → Story, refining task granularity at each level
- **Parallel Execution**: Independent tasks are processed in parallel within the same batch
- **Multi-Agent Collaboration**: Automatically selects the optimal agent based on task characteristics
- **Quality Assurance**: Automated quality gates + intelligent retry mechanism
- **State Tracking**: File-based state sharing with checkpoint recovery support

### Three-Tier Architecture

| Tier | Name | Responsibility | Artifact |
|------|------|----------------|----------|
| **Level 1** | Mega Plan | Project-level orchestration, manages multiple Features | `mega-plan.json` |
| **Level 2** | Hybrid Ralph | Feature-level development, auto-generates PRD | `prd.json` |
| **Level 3** | Stories | Story-level execution, parallel agent processing | Code changes |

---

## Usage Methods

| Method | Description | Use Case | Documentation |
|--------|-------------|----------|---------------|
| **Standalone CLI** | Independent command-line tool | Any terminal environment | [CLI Guide](docs/CLI-Guide.md) |
| **Claude Code Plugin** | Native integration, most complete features | Claude Code users | [Plugin Guide](docs/Plugin-Guide.md) |
| **Desktop App** | Graphical user interface | Users preferring GUI | [Desktop Guide](docs/Desktop-Guide.md) |
| **MCP Server** | Integration via MCP protocol | Cursor, Windsurf, etc. | [MCP Guide](docs/MCP-SERVER-GUIDE.md) |

---

## Quick Start

### Standalone CLI

```bash
# Install
pip install plan-cascade

# Configure
plan-cascade config --setup

# Simple mode - one-click execution
plan-cascade run "Implement user login feature"

# Expert mode - more control
plan-cascade run "Implement user login feature" --expert

# Interactive chat
plan-cascade chat
```

### Claude Code Plugin

```bash
# Install
claude plugins install Taoidle/plan-cascade

# Usage - Auto mode (recommended for new users)
/plan-cascade:auto "Your task description"

# Usage - Manual mode selection
/plan-cascade:hybrid-auto "Add search functionality"
/plan-cascade:approve --auto-run
```

### Desktop App

Download the installer for your platform from [GitHub Releases](https://github.com/Taoidle/plan-cascade/releases).

---

## Core Features

### Dual Mode Design

| Mode | Use Case | Characteristics |
|------|----------|-----------------|
| **Simple Mode** | New users, quick tasks | AI automatically determines strategy and executes |
| **Expert Mode** | Experienced users, fine-grained control | PRD editing, agent selection, quality gate configuration |

### AI Automatic Strategy Selection

In simple mode, AI automatically selects the execution strategy based on requirements:

| Input Type | Execution Strategy |
|------------|-------------------|
| Small task (e.g., "add a button") | Direct execution |
| Medium feature (e.g., "user login") | Hybrid Auto |
| Large project (e.g., "e-commerce platform") | Mega Plan |
| Requires isolation (e.g., "experimental refactoring") | Hybrid Worktree |

### Multi-LLM Backend

| Backend | Requires API Key | Description |
|---------|-----------------|-------------|
| Claude Code | No | Default, via Claude Code CLI |
| Claude Max | No | Obtain LLM via Claude Code |
| Claude API | Yes | Direct Anthropic API calls |
| OpenAI | Yes | GPT-4o, etc. |
| DeepSeek | Yes | DeepSeek Chat/Coder |
| Ollama | No | Local models |

### Multi-Agent Collaboration

Supports using different agents to execute stories:

| Agent | Type | Description |
|-------|------|-------------|
| claude-code | task-tool | Built-in, always available |
| codex | cli | OpenAI Codex |
| aider | cli | AI pair programming |
| amp-code | cli | Amp Code |
| cursor-cli | cli | Cursor CLI |

### Quality Gates

Automatic quality verification runs after each story completion:

| Gate | Tools |
|------|-------|
| TypeCheck | tsc, mypy, pyright |
| Test | pytest, jest |
| Lint | eslint, ruff |
| Custom | Custom scripts |

---

## Command Quick Reference

### CLI

```bash
plan-cascade run <description>          # Execute task
plan-cascade run <description> --expert # Expert mode
plan-cascade chat                       # Interactive chat
plan-cascade config --setup             # Configuration wizard
plan-cascade status                     # View status
```

### Claude Code Plugin

```bash
# Auto mode - AI automatically selects strategy
/plan-cascade:auto <description>        # Auto-select and execute best strategy

# Project level
/plan-cascade:mega-plan <description>   # Generate project plan
/plan-cascade:mega-approve              # Approve execution
/plan-cascade:mega-complete             # Complete and merge

# Feature level
/plan-cascade:hybrid-auto <description> # Generate PRD
/plan-cascade:approve --auto-run        # Approve and auto-execute
/plan-cascade:hybrid-complete           # Complete

# General
/plan-cascade:edit                      # Edit PRD
/plan-cascade:status                    # View status
```

---

## Project Structure

```
plan-cascade/
├── src/plan_cascade/       # Python core package
│   ├── core/               # Orchestration engine
│   ├── backends/           # Backend abstraction
│   ├── llm/                # LLM providers
│   ├── tools/              # Tool execution
│   ├── settings/           # Settings management
│   └── cli/                # CLI entry point
├── .claude-plugin/         # Plugin configuration
├── commands/               # Plugin commands
├── skills/                 # Plugin skills
├── mcp_server/             # MCP server
├── desktop/                # Desktop application
└── docs/                   # Documentation
    ├── CLI-Guide.md
    ├── Plugin-Guide.md
    ├── Desktop-Guide.md
    └── MCP-SERVER-GUIDE.md
```

---

## Documentation Index

| Document | Description |
|----------|-------------|
| [CLI Guide](docs/CLI-Guide.md) | Detailed CLI usage guide |
| [Plugin Guide](docs/Plugin-Guide.md) | Detailed Claude Code plugin guide |
| [Desktop Guide](docs/Desktop-Guide.md) | Desktop application guide |
| [MCP Server Guide](docs/MCP-SERVER-GUIDE.md) | MCP server configuration guide |
| [System Architecture](docs/System-Architecture.md) | System architecture and process design (with diagrams) |
| [Design Document](docs/Design-Plan-Cascade-Standalone.md) | Technical design document |
| [PRD Document](docs/PRD-Plan-Cascade-Standalone.md) | Product requirements document |

---

## Changelog

### v4.1.0

- **Auto Strategy Command** - New `/plan-cascade:auto` command
  - AI automatically analyzes task and selects optimal strategy
  - Supports 4 strategies: direct, hybrid-auto, hybrid-worktree, mega-plan
  - Keyword-based detection (not word count based)
  - No user confirmation required - direct execution

### v4.0.0

- **Standalone CLI Complete** - Independent command-line tool fully functional
  - Simple mode/Expert mode dual-mode support
  - Interactive REPL chat mode
  - AI automatic strategy selection
- **Multi-LLM Backend** - Support for 5 LLM providers
  - Claude Max (no API key required)
  - Claude API, OpenAI, DeepSeek, Ollama
- **Independent ReAct Engine** - Complete Think→Act→Observe loop
- **Documentation Restructure** - Split into separate usage guides

### v3.x

- **MCP Server** - Support for Cursor, Windsurf, etc.
- **Multi-Agent Collaboration** - Codex, Aider, etc.
- **Auto Iteration Loop** - Quality gates, intelligent retry
- **Mega Plan** - Project-level multi-feature orchestration

See [CHANGELOG.md](CHANGELOG.md) for the complete changelog.

---

## Project Origins

This project is forked from [OthmanAdi/planning-with-files](https://github.com/OthmanAdi/planning-with-files) (v2.7.1), significantly expanding upon its Manus-style file-based planning foundation.

---

## Acknowledgments

- **[OthmanAdi/planning-with-files](https://github.com/OthmanAdi/planning-with-files)** - Original project
- **[snarktank/ralph](https://github.com/snarktank/ralph)** - PRD format inspiration
- **Anthropic** - Claude Code, Plugin system, and MCP protocol

---

## License

MIT License

---

**Repository**: [Taoidle/plan-cascade](https://github.com/Taoidle/plan-cascade)

[![Star History Chart](https://api.star-history.com/svg?repos=Taoidle/plan-cascade&type=Date)](https://star-history.com/#Taoidle/plan-cascade&Date)
