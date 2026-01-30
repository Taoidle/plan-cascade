# Plan Cascade Desktop User Guide

**Version**: 5.0.0
**Last Updated**: 2026-01-30

Welcome to Plan Cascade Desktop, a complete AI programming orchestration platform.

---

## Table of Contents

1. [Getting Started](#getting-started)
2. [Working Modes](#working-modes)
3. [Projects & Sessions](#projects--sessions)
4. [Agent Library](#agent-library)
5. [Analytics Dashboard](#analytics-dashboard)
6. [Quality Gates](#quality-gates)
7. [Git Worktree Support](#git-worktree-support)
8. [MCP Server Management](#mcp-server-management)
9. [Timeline & Checkpoints](#timeline--checkpoints)
10. [CLAUDE.md Editor](#claudemd-editor)
11. [Chat Features](#chat-features)
12. [Command Palette](#command-palette)
13. [Settings](#settings)
14. [Keyboard Shortcuts](#keyboard-shortcuts)

---

## Getting Started

### Installation

1. Download the installer for your platform:
   - **Windows**: `plan-cascade-desktop-5.0.0.msi`
   - **macOS**: `plan-cascade-desktop-5.0.0.dmg`
   - **Linux**: `plan-cascade-desktop-5.0.0.AppImage`

2. Run the installer and follow the prompts

3. Launch Plan Cascade Desktop

### First Launch

When you first open the app:

1. **Choose your working mode**:
   - **Claude Code GUI Mode** (recommended if you have Claude Code installed)
   - **Standalone Mode** (uses LLM APIs directly)

2. **Configure API keys** (for Standalone Mode):
   - Go to Settings > Providers
   - Enter your API key for Anthropic, OpenAI, DeepSeek, or configure Ollama

3. **Explore your projects**:
   - The app automatically scans `~/.claude/projects/`
   - Browse and resume past sessions

---

## Working Modes

Plan Cascade Desktop supports two working modes:

### Claude Code GUI Mode (Recommended)

This mode uses Claude Code CLI as the execution backend.

**Requirements**:
- Claude Code CLI installed (`claude` command available)
- Claude subscription (Max or Pro)

**Benefits**:
- Full Claude Code compatibility
- Access to all Claude Code features
- Automatic session tracking
- No API key required

**How to use**:
1. Select "Claude Code GUI Mode" in Settings
2. Open a project
3. Type your message and press Enter
4. Claude Code will execute tools and respond

### Standalone Mode

This mode calls LLM APIs directly without requiring Claude Code.

**Requirements**:
- API key for your chosen provider
- Or Ollama running locally for offline use

**Supported Providers**:
| Provider | Models | API Key Required |
|----------|--------|------------------|
| Anthropic | Claude 3.5 Sonnet, Claude 3 Opus | Yes |
| OpenAI | GPT-4 Turbo, o1, o3-mini | Yes |
| DeepSeek | DeepSeek Chat, DeepSeek R1 | Yes |
| Ollama | Llama 3.2, DeepSeek R1, QwQ | No (local) |

**How to use**:
1. Select "Standalone Mode" in Settings
2. Configure your API key
3. Choose a model
4. Type your message and press Enter

---

## Projects & Sessions

### Project Browser

Access your Claude Code projects from the sidebar.

**Features**:
- Browse all projects in `~/.claude/projects/`
- See project statistics (sessions, messages)
- Sort by name, last accessed, or creation date
- Search projects by name or path

**Project Card Information**:
- Project name and path
- Number of sessions
- Total messages
- Last activity timestamp

### Session Management

View and manage past coding sessions.

**Session List**:
- First message preview
- Message count
- Checkpoint count
- Timestamp

**Actions**:
- **Resume**: Continue from where you left off
- **View Details**: See session metadata
- **Export**: Save session as JSON
- **Delete**: Remove session (with confirmation)

### Searching

Use the search bar to find projects and sessions:

```
Search: "authentication"
```

Results include:
- Projects with matching names
- Sessions with matching messages
- CLAUDE.md files with matching content

---

## Agent Library

Create and manage custom AI agents for specific tasks.

### Creating an Agent

1. Click **+ Create Agent**
2. Fill in the details:
   - **Name**: Descriptive name (e.g., "Code Reviewer")
   - **Description**: What the agent does
   - **Model**: Select LLM model
   - **System Prompt**: Instructions for the agent
   - **Tools**: Select allowed tools (Read, Write, Edit, Bash, Glob, Grep)

3. Click **Save**

### Example Agents

**Code Reviewer**:
```
System Prompt:
You are an expert code reviewer. When reviewing code:
1. Check for security vulnerabilities
2. Identify performance issues
3. Suggest best practices
4. Point out potential bugs

Only use Read, Glob, and Grep tools. Do not modify files.
```

**Test Writer**:
```
System Prompt:
You are a test writing specialist. Generate comprehensive tests:
1. Unit tests for individual functions
2. Integration tests for APIs
3. Edge cases and error conditions
4. Maintain high code coverage

Use Jest/pytest conventions based on the project.
```

**Documentation Generator**:
```
System Prompt:
You generate documentation from source code:
1. Extract function signatures and comments
2. Generate API documentation
3. Create usage examples
4. Format in Markdown
```

### Running an Agent

1. Select an agent from the library
2. Click **Run**
3. Enter your input (e.g., file path, task description)
4. View results in the output panel

### Agent History

View past agent runs:
- Input provided
- Output generated
- Token usage
- Duration
- Success/failure status

---

## Analytics Dashboard

Track your AI usage and costs.

### Overview Panel

See at a glance:
- **Total Cost**: Spending for the selected period
- **Total Tokens**: Input + output tokens used
- **Requests**: Number of API calls
- **Average/Day**: Daily spending average

### Cost Over Time

Line chart showing spending trends:
- Daily, weekly, or monthly view
- Hover for exact values
- Compare to previous period

### Usage by Model

Breakdown of usage per model:
- Token counts
- Cost per model
- Percentage of total

### Usage by Project

See which projects use the most resources:
- Cost per project
- Token breakdown
- Identify expensive projects

### Exporting Data

Export your analytics data:
- **CSV**: For spreadsheets
- **JSON**: For programmatic use
- Filter by date range, provider, or project

---

## Quality Gates

Automatically validate your code after AI modifications.

### Project Type Detection

Quality gates automatically detect your project type:
- Node.js (package.json)
- Python (pyproject.toml, setup.py)
- Rust (Cargo.toml)
- Go (go.mod)

### Available Gates

| Gate | Node.js | Python | Rust | Go |
|------|---------|--------|------|-----|
| Type Check | tsc | mypy | cargo check | go vet |
| Tests | jest | pytest | cargo test | go test |
| Lint | eslint | ruff | clippy | golangci-lint |

### Running Quality Gates

**Automatic** (after AI execution):
1. Enable in Settings > Quality Gates
2. Gates run after each task completion
3. View results in the Quality panel

**Manual**:
1. Open Quality Gates panel
2. Select gates to run
3. Click **Run Selected**
4. View pass/fail status

### Custom Gates

Add your own quality checks:
1. Go to Settings > Quality Gates > Custom
2. Click **Add Custom Gate**
3. Configure:
   - Name
   - Command (e.g., `npm run lint:fix`)
   - Required (fail execution if gate fails)

---

## Git Worktree Support

Work on features in isolation without affecting your main codebase.

### What is a Worktree?

A Git worktree creates a separate working directory linked to the same repository. This allows:
- Parallel development on multiple features
- Clean isolation from main branch
- Easy cleanup when feature is complete

### Creating a Worktree

1. Open Command Palette (Ctrl+/)
2. Type "Create Worktree"
3. Enter:
   - **Task Name**: e.g., "feature-auth"
   - **Target Branch**: Branch to merge to (usually "main")
   - **PRD Path**: Optional PRD file for the task

4. Click **Create**

The app creates:
```
.worktrees/
  feature-auth/
    .planning-config.json
    (your project files)
```

### Working in a Worktree

- The worktree appears in your project list
- All changes are isolated to the worktree
- Main branch remains untouched

### Completing a Worktree

When your feature is done:
1. Open the worktree in the project browser
2. Click **Complete Task**
3. Enter a commit message
4. The app will:
   - Commit your changes (excluding planning files)
   - Merge to the target branch
   - Clean up the worktree

---

## MCP Server Management

Manage Model Context Protocol servers from a central UI.

### What is MCP?

MCP (Model Context Protocol) allows Claude to connect to external tools and data sources:
- File system access
- GitHub integration
- Database connections
- Custom tools

### Adding a Server

1. Go to MCP Servers in the sidebar
2. Click **+ Add Server**
3. Choose server type:
   - **Stdio**: Command-line based
   - **SSE**: HTTP-based

4. Configure:
   - **Name**: Display name
   - **Command/URL**: How to start/connect
   - **Arguments**: Command-line args
   - **Environment**: Environment variables

### Example: Filesystem Server

```
Name: filesystem
Type: stdio
Command: npx
Args: -y @anthropic/mcp-server-filesystem
Environment:
  ALLOWED_PATHS: /home/user/projects
```

### Importing from Claude Desktop

If you have MCP servers configured in Claude Desktop:
1. Click **Import from Claude Desktop**
2. Select servers to import
3. Review and confirm

### Testing Connectivity

1. Find the server in the list
2. Click **Test**
3. View connection status and latency

---

## Timeline & Checkpoints

Track your session history with checkpoints.

### Creating Checkpoints

Checkpoints save the state of your project at a point in time.

**Automatic Checkpoints**:
- Created after significant changes
- Configurable in Settings

**Manual Checkpoints**:
1. Click the **Checkpoint** button
2. Enter a label (e.g., "Before OAuth refactor")
3. Select files to track

### Timeline View

Visualize your session as a timeline:
```
Start
  |
  | "Help me implement auth"
  |
Checkpoint: "Basic auth setup"
  |
  | "Add OAuth support"
  |
 / \
|   Branch: "Try JWT"
|     |
|   (abandoned)
|
Checkpoint: "OAuth complete"
  |
Current
```

### Restoring Checkpoints

Go back to any previous state:
1. Select a checkpoint in the timeline
2. Click **Restore**
3. Optionally create a backup of current state
4. Files are restored to checkpoint state

### Branching

Create alternative approaches:
1. Select a checkpoint
2. Click **Fork Branch**
3. Enter branch name
4. Continue from that point independently

---

## CLAUDE.md Editor

Edit your project's CLAUDE.md configuration.

### What is CLAUDE.md?

CLAUDE.md provides context to AI about your project:
- Project overview
- Code style guidelines
- Architecture description
- Important files and patterns

### Using the Editor

1. Go to CLAUDE.md in the sidebar
2. Select a project or scan for files
3. Edit in the markdown editor
4. Preview rendered output
5. Click **Save**

### Template

```markdown
# Project Name

## Overview
Brief description of what this project does.

## Tech Stack
- Frontend: React + TypeScript
- Backend: Node.js + Express
- Database: PostgreSQL

## Code Style
- Use TypeScript strict mode
- Follow ESLint configuration
- Prefer functional components

## Architecture
```
src/
  components/  # React components
  hooks/       # Custom hooks
  utils/       # Helper functions
  api/         # API calls
```

## Important Patterns
- Use React Query for data fetching
- Prefer composition over inheritance
- Write tests for all utilities
```

### Scanning for Files

Find all CLAUDE.md files in your projects:
1. Click **Scan**
2. View list of found files
3. Click to open any file

---

## Chat Features

### Markdown Rendering

Messages support full GitHub Flavored Markdown:
- **Bold** and *italic* text
- Code blocks with syntax highlighting
- Tables and lists
- Links and images

### Code Blocks

Code blocks include:
- Syntax highlighting for 50+ languages
- Line numbers (toggleable)
- Copy button
- Language label

### File References

Reference files in your messages:
1. Type `@` to open file picker
2. Select file from suggestions
3. File content is included in context

Example:
```
Review @src/auth/handler.ts for security issues
```

### Drag and Drop

Attach files by dragging:
1. Drag files from your file explorer
2. Drop onto the chat input
3. Files appear as attachments
4. Send your message with files included

### Thinking Display

When using models with extended thinking:
- Thinking blocks show the AI's reasoning
- Collapsible for cleaner view
- Shows thinking duration

---

## Command Palette

Quick access to all features via keyboard.

### Opening

- Press **Ctrl+/** (Windows/Linux)
- Press **Cmd+/** (macOS)

### Using

1. Type to filter commands
2. Use arrow keys to navigate
3. Press Enter to execute
4. Press Esc to close

### Available Commands

| Command | Description |
|---------|-------------|
| New Session | Start a new chat session |
| Open Project | Switch to another project |
| Search Sessions | Search past conversations |
| Settings | Open settings panel |
| Toggle Theme | Switch light/dark mode |
| Usage Dashboard | View usage statistics |
| MCP Servers | Manage MCP connections |
| Agent Library | Browse custom agents |
| CLAUDE.md | Edit project config |
| Create Checkpoint | Save current state |
| Run Quality Gates | Validate code |
| Create Worktree | Start isolated task |

---

## Settings

Configure the application to your preferences.

### General

- **Theme**: Light, Dark, or System
- **Language**: English, Chinese, etc.
- **Telemetry**: Enable/disable usage analytics

### Working Mode

- **Claude Code GUI Mode**: Use Claude Code CLI
- **Standalone Mode**: Direct LLM API calls

### Providers

Configure API keys for each provider:
- Anthropic (Claude)
- OpenAI
- DeepSeek
- Ollama (local URL)

Keys are stored securely in your OS keychain.

### Quality Gates

- **Enable**: Run gates automatically
- **Default Gates**: Select which gates to run
- **Custom Gates**: Add your own checks

### Chat

- **Streaming**: Enable/disable streaming display
- **Thinking**: Show/hide thinking blocks
- **Auto-scroll**: Scroll to new messages

### Keyboard

Customize keyboard shortcuts for common actions.

---

## Keyboard Shortcuts

### Global

| Shortcut | Action |
|----------|--------|
| Ctrl+/ | Open Command Palette |
| Ctrl+, | Open Settings |
| Ctrl+K | Clear conversation |
| Ctrl+N | New session |
| Ctrl+Shift+P | Switch project |

### Chat

| Shortcut | Action |
|----------|--------|
| Enter | Send message |
| Shift+Enter | New line |
| Ctrl+C | Cancel execution |
| Up Arrow | Edit last message (in empty input) |
| Ctrl+Shift+C | Copy last response |

### Editor

| Shortcut | Action |
|----------|--------|
| Ctrl+S | Save file |
| Ctrl+Z | Undo |
| Ctrl+Shift+Z | Redo |
| Ctrl+F | Find |
| Ctrl+H | Find and replace |

---

## Tips & Tricks

### 1. Use System Prompts Effectively

Create project-specific system prompts:
```
You are working on a React + TypeScript e-commerce app.
Always use:
- Functional components with hooks
- React Query for data fetching
- Tailwind CSS for styling
- Zod for validation
```

### 2. Checkpoint Before Major Changes

Before refactoring or adding complex features:
1. Create a checkpoint
2. Name it descriptively
3. If things go wrong, restore easily

### 3. Use Agents for Repetitive Tasks

Create agents for common operations:
- Code review before commits
- Documentation generation
- Test writing
- Security scanning

### 4. Monitor Costs

Keep track of spending:
1. Check Analytics weekly
2. Set up budget alerts
3. Use cheaper models for simple tasks
4. Use Ollama for experiments

### 5. Organize with Worktrees

For complex features:
1. Create a worktree
2. Work in isolation
3. Run quality gates
4. Merge only when ready

### 6. Leverage MCP Servers

Extend capabilities with MCP:
- Connect to databases
- Access GitHub
- Query APIs
- Use custom tools

---

## Getting Help

- **Documentation**: Full docs at [docs.plan-cascade.dev](https://docs.plan-cascade.dev)
- **GitHub Issues**: Report bugs and request features
- **Community**: Join our Discord server
- **Email**: support@plan-cascade.dev
