# Plan Cascade MCP Server Guide

Plan Cascade provides an MCP (Model Context Protocol) server that enables integration with Cursor, Amp Code, and other MCP-compatible AI coding tools.

## Quick Start

### Installation

```bash
# Install dependencies
pip install 'mcp[cli]'

# Or install the full package
pip install -e .
```

### Running the Server

```bash
# Run with stdio transport (default, for IDE integration)
python -m mcp_server.server

# Run with SSE transport
python -m mcp_server.server --transport sse --port 8080

# Enable debug logging
python -m mcp_server.server --debug
```

### Testing with MCP Inspector

```bash
npx @anthropic/mcp-inspector python -m mcp_server.server
```

## IDE Configuration

> **Tip:** Pre-configured examples are available in the `mcp-configs/` directory.
> See [mcp-configs/README.md](../mcp-configs/README.md) for detailed setup instructions.

### Cursor

Copy `mcp-configs/cursor-mcp.json` to `.cursor/mcp.json`:

```bash
cp mcp-configs/cursor-mcp.json .cursor/mcp.json
# Edit the file and replace {{PLAN_CASCADE_PATH}} with actual path
```

Or create manually:

```json
{
  "mcpServers": {
    "plan-cascade": {
      "command": "python",
      "args": ["-m", "mcp_server.server"],
      "cwd": "/path/to/plan-cascade",
      "env": {
        "PYTHONPATH": "/path/to/plan-cascade"
      }
    }
  }
}
```

### Claude Code

```bash
# Add server (run from plan-cascade directory)
claude mcp add plan-cascade -- python -m mcp_server.server

# Verify
claude mcp list
```

### Windsurf

Copy config to Windsurf's MCP location:

```bash
# macOS/Linux
cp mcp-configs/windsurf-mcp.json ~/.codeium/windsurf/mcp_config.json

# Windows
copy mcp-configs\windsurf-mcp.json %USERPROFILE%\.codeium\windsurf\mcp_config.json
```

### Amp Code

```json
{
  "mcp": {
    "servers": {
      "plan-cascade": {
        "command": "python",
        "args": ["-m", "mcp_server.server"],
        "cwd": "/path/to/plan-cascade"
      }
    }
  }
}
```

### Cline (VS Code)

Add to VS Code `settings.json`:

```json
{
  "cline.mcpServers": {
    "plan-cascade": {
      "command": "python",
      "args": ["-m", "mcp_server.server"],
      "cwd": "/path/to/plan-cascade"
    }
  }
}
```

### Continue

Add to `~/.continue/config.json`:

```json
{
  "experimental": {
    "modelContextProtocolServers": [
      {
        "transport": {
          "type": "stdio",
          "command": "python",
          "args": ["-m", "mcp_server.server"],
          "cwd": "/path/to/plan-cascade"
        }
      }
    ]
  }
}
```

### Zed

Add to `~/.config/zed/settings.json`:

```json
{
  "context_servers": {
    "plan-cascade": {
      "command": {
        "path": "python",
        "args": ["-m", "mcp_server.server"],
        "env": {
          "PYTHONPATH": "/path/to/plan-cascade"
        }
      }
    }
  }
}
```

## Available Tools

### PRD Tools (Feature Level)

| Tool | Description |
|------|-------------|
| `prd_generate` | Generate a PRD from task description |
| `prd_add_story` | Add a user story to the PRD |
| `prd_validate` | Validate PRD structure and dependencies |
| `prd_get_batches` | Get parallel execution batches |
| `prd_update_story_status` | Update story status (pending/in_progress/complete) |
| `prd_detect_dependencies` | Auto-detect dependencies between stories |

### Mega Plan Tools (Project Level)

| Tool | Description |
|------|-------------|
| `mega_generate` | Generate a mega-plan from project description |
| `mega_add_feature` | Add a feature to the mega-plan |
| `mega_validate` | Validate mega-plan structure |
| `mega_get_batches` | Get parallel feature execution batches |
| `mega_update_feature_status` | Update feature status |
| `mega_get_merge_plan` | Get ordered merge plan when complete |

### Execution Tools (Task Level)

| Tool | Description |
|------|-------------|
| `get_story_context` | Get full context for a story (dependencies, findings) |
| `get_execution_status` | Get overall PRD execution status |
| `append_findings` | Record findings during development |
| `mark_story_complete` | Mark a story as complete |
| `get_progress` | Get progress timeline summary |
| `cleanup_locks` | Clean up stale lock files |

## Available Resources

| Resource URI | Description |
|--------------|-------------|
| `plan-cascade://prd` | Current PRD (prd.json) |
| `plan-cascade://mega-plan` | Current mega-plan (mega-plan.json) |
| `plan-cascade://findings` | Development findings (findings.md) |
| `plan-cascade://progress` | Progress timeline (progress.txt) |
| `plan-cascade://mega-status` | Mega-plan execution status |
| `plan-cascade://mega-findings` | Project-level findings |
| `plan-cascade://story/{story_id}` | Specific story details |
| `plan-cascade://feature/{feature_id}` | Specific feature details |

## Workflow Examples

### Single Feature Development

```
1. prd_generate("Implement user authentication with JWT...")
2. prd_add_story("Design user schema", "Create database schema for users...", priority="high")
3. prd_add_story("Implement registration", "Create registration endpoint...", dependencies=["story-001"])
4. prd_validate()
5. prd_get_batches()

# For each story:
6. get_story_context("story-001")
7. [do development work]
8. append_findings("Decided to use bcrypt for password hashing...", story_id="story-001")
9. mark_story_complete("story-001")
```

### Large Project with Mega-Plan

```
1. mega_generate("Build e-commerce platform...", target_branch="main")
2. mega_add_feature("feature-auth", "User Authentication", "Implement JWT auth...")
3. mega_add_feature("feature-products", "Product Catalog", "Implement product CRUD...")
4. mega_add_feature("feature-cart", "Shopping Cart", "...", dependencies=["feature-001", "feature-002"])
5. mega_validate()
6. mega_get_batches()

# For each feature:
7. [create worktree]
8. prd_generate("...feature description...")
9. [develop feature using PRD workflow]
10. mega_update_feature_status("feature-001", "complete")

# When all features complete:
11. mega_get_merge_plan()
```

## Three-Layer Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Layer 1: Mega Plan                       │
│                    (Project Level)                          │
│   mega_generate, mega_add_feature, mega_validate            │
│   mega_get_batches, mega_update_feature_status              │
│   mega_get_merge_plan                                       │
├─────────────────────────────────────────────────────────────┤
│                    Layer 2: PRD                             │
│                    (Feature Level)                          │
│   prd_generate, prd_add_story, prd_validate                 │
│   prd_get_batches, prd_update_story_status                  │
│   prd_detect_dependencies                                   │
├─────────────────────────────────────────────────────────────┤
│                    Layer 3: Execution                       │
│                    (Task Level)                             │
│   get_story_context, get_execution_status                   │
│   append_findings, mark_story_complete                      │
│   get_progress, cleanup_locks                               │
└─────────────────────────────────────────────────────────────┘
```

## File Formats

### prd.json

```json
{
  "metadata": {
    "created_at": "2026-01-28T10:00:00",
    "version": "1.0.0",
    "description": "Feature description"
  },
  "goal": "Main goal statement",
  "objectives": ["Objective 1", "Objective 2"],
  "stories": [
    {
      "id": "story-001",
      "title": "Story title",
      "description": "Detailed description",
      "priority": "high",
      "dependencies": [],
      "status": "pending",
      "acceptance_criteria": ["Criterion 1"],
      "context_estimate": "medium",
      "tags": ["api"]
    }
  ]
}
```

### mega-plan.json

```json
{
  "metadata": {
    "created_at": "2026-01-28T10:00:00",
    "version": "1.0.0"
  },
  "goal": "Project goal",
  "description": "Full project description",
  "execution_mode": "auto",
  "target_branch": "main",
  "features": [
    {
      "id": "feature-001",
      "name": "feature-auth",
      "title": "User Authentication",
      "description": "Description for PRD generation",
      "priority": "high",
      "dependencies": [],
      "status": "pending"
    }
  ]
}
```

## Compatibility

The MCP server is fully compatible with the Claude Code plugin:

- Files generated by MCP tools can be used with Claude Code `/hybrid-*` commands
- Files generated by Claude Code can be read by MCP resources
- Both systems share the same state files (prd.json, findings.md, etc.)

This allows you to:
- Start development in Cursor with MCP
- Continue in Claude Code with slash commands
- Switch between tools seamlessly

## Troubleshooting

### Server Won't Start

```bash
# Check if mcp is installed
pip show mcp

# Install if missing
pip install 'mcp[cli]'
```

### Import Errors

Ensure the PYTHONPATH includes the plan-cascade directory:

```bash
export PYTHONPATH="/path/to/plan-cascade:$PYTHONPATH"
python -m mcp_server.server
```

### Lock File Errors

If you encounter lock errors due to interrupted operations:

```python
# Use the cleanup_locks tool
cleanup_locks()
```

Or manually remove lock files:

```bash
rm -rf .locks/*.lock
```

### Connection Issues in Cursor

1. Verify the MCP server configuration in `.cursor/mcp.json`
2. Check that Python path is correct
3. Ensure no firewall blocking (for SSE transport)
4. Try running the server manually to see error messages
