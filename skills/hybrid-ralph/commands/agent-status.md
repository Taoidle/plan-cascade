---
name: hybrid:agent-status
description: View status of running agents and their processes
arguments:
  - name: story-id
    description: Show status for a specific story
    required: false
  - name: output
    description: Show agent output log (requires --story-id)
    required: false
---

# /hybrid:agent-status

View the status of all agents executing stories, including running, completed, and failed agents.

## Usage

```
/hybrid:agent-status                    # Show all agent status
/hybrid:agent-status --story-id story-001  # Show specific story
/hybrid:agent-status --story-id story-001 --output  # Show output log
```

## Your Task

Display the current agent execution status using the MCP tools.

### Step 1: Check Agent Status

Use the `check_agents` MCP tool to get updated status:

```
This will:
- Poll all running agent processes
- Detect completed/failed agents via result files
- Update .agent-status.json automatically
```

### Step 2: Display Results

If a specific story is requested, use `get_agent_result` to get details.
If output is requested, use `get_agent_output` to get the log.

### Step 3: Display Status

Display the status in this format:

```
============================================================
AGENT STATUS
============================================================

Available Agents:
  claude-code: [task-tool] Available (built-in)
  codex: [cli] Available
  amp-code: [cli] NOT AVAILABLE
  aider: [cli] Available
  cursor-cli: [cli] NOT AVAILABLE

Running (N):
  story-001: codex (PID: 12345)
    Started: 2026-01-28T10:30:00Z
    Output: .agent-outputs/story-001.log

  story-002: amp-code (PID: 12346)
    Started: 2026-01-28T10:30:05Z
    Output: .agent-outputs/story-002.log

Completed (N):
  story-003: claude-code
    Completed: 2026-01-28T10:25:00Z

Failed (N):
  story-004: codex
    Failed: 2026-01-28T10:20:00Z
    Error: Process exited with code 1

============================================================
Commands:
  Stop agent: /hybrid:agent-stop <story-id>
  View log: tail -f .agent-outputs/<story-id>.log
============================================================
```

### Step 4: Monitor Option

If running agents exist, offer to monitor:

```
Tip: To monitor agent output in real-time:
  tail -f .agent-outputs/story-001.log
```

## Agent Types

| Type | Description |
|------|-------------|
| `task-tool` | Claude Code's built-in Task tool (no PID) |
| `cli` | External CLI tool running as subprocess |

## MCP Tools

The following MCP tools are available for agent management:

| Tool | Description |
|------|-------------|
| `check_agents` | Poll all running agents, update status |
| `get_agent_status` | Get current agent status summary |
| `get_agent_result` | Get result of a completed agent |
| `get_agent_output` | Get output log of an agent |
| `wait_for_agent` | Wait for a specific agent to complete |
| `stop_agent` | Stop a running CLI agent |
| `execute_story_with_agent` | Execute a story with a specific agent |

## File Structure

```
.agent-outputs/
├── story-001.log           # Agent output log
├── story-001.prompt.txt    # Prompt sent to agent
├── story-001.result.json   # Execution result
├── story-002.log
└── ...

.agent-status.json          # Current agent status
progress.txt                # Progress log with agent info
```

## Troubleshooting

**Agent marked running but process is dead:**
Use the `check_agents` MCP tool - it will detect dead processes and update status based on result files.

**CLI agent not available:**
The specified CLI tool is not installed or not in PATH. Install it or use `claude-code` instead. The system will auto-fallback.

**No result file after agent exit:**
The agent may have crashed before the wrapper could write results. Check the output log for errors.

**Timeout issues:**
Adjust timeout in `agents.json` or use `--timeout` parameter when launching.

## See Also

- `/hybrid:status` - View overall PRD execution status
- `/hybrid:approve` - Start execution with agents
- `/cleanup-locks` - Clean up stale lock files
