---
name: agent-config
description: View or edit agent configuration
arguments:
  - name: action
    description: "Action: show, detect, set-default, set-phase, refresh"
    required: false
  - name: agent
    description: "Agent name (for set-default)"
    required: false
  - name: phase
    description: "Phase name (for set-phase): planning, implementation, retry, refactor, review"
    required: false
---

# /agent-config

View and manage agent configuration for Plan Cascade.

## Usage

```
/agent-config
/agent-config --action show
/agent-config --action detect
/agent-config --action set-default --agent codex
/agent-config --action set-phase --phase implementation --agent aider
/agent-config --action refresh
```

## Actions

### show (default)

Display current agent configuration:

```
AGENT CONFIGURATION
===================

Default Agent: claude-code

Available Agents:
  ● claude-code    [task-tool] Claude Code Task tool (built-in)
  ● codex          [cli] OpenAI Codex CLI agent
  ○ amp-code       [cli] Amp Code CLI agent (NOT AVAILABLE)
  ● aider          [cli] Aider AI pair programming assistant
  ○ cursor-cli     [cli] Cursor CLI agent (NOT AVAILABLE)
  ○ claude-cli     [cli] Claude CLI standalone (NOT AVAILABLE)

Phase Defaults:
  planning:        codex → [claude-code]
  implementation:  claude-code → [codex, aider]
  retry:           claude-code → [aider]
  refactor:        aider → [claude-code]
  review:          claude-code → [codex]

Story Type Defaults:
  feature:         claude-code
  bugfix:          codex
  refactor:        aider
  test:            claude-code
```

### detect

Run cross-platform detection to find available agents:

```
AGENT DETECTION
===============

Platform: windows
Cache: .agent-detection.json (TTL: 1 hour)

Detection Results:
  codex:
    Status: Available
    Path: C:\Users\user\.local\bin\codex.exe
    Version: 0.8.2
    Method: path

  aider:
    Status: Available
    Path: C:\Users\user\AppData\Roaming\Python\Scripts\aider.exe
    Version: 0.52.1
    Method: common_location

  cursor-cli:
    Status: Not Found
    Checked: PATH, common locations, registry

  amp-code:
    Status: Not Found
    Checked: PATH, common locations

Detection updated. Cache refreshed.
```

### set-default

Set the default agent for all phases:

```
/agent-config --action set-default --agent codex
```

Output:
```
Default agent set to: codex
This will be used for stories without explicit agent assignment.
```

### set-phase

Set the default agent for a specific phase:

```
/agent-config --action set-phase --phase implementation --agent aider
```

Output:
```
Phase 'implementation' default agent set to: aider
Fallback chain: [claude-code, codex]
```

### refresh

Force refresh the agent detection cache:

```
/agent-config --action refresh
```

Clears `.agent-detection.json` and re-runs detection.

## Configuration Files

### agents.json

Main configuration file in project root:

```json
{
  "default_agent": "claude-code",
  "agents": {
    "claude-code": {
      "type": "task-tool",
      "description": "Claude Code Task tool (built-in)",
      "subagent_type": "general-purpose"
    },
    "codex": {
      "type": "cli",
      "command": "codex",
      "args": ["--prompt", "{prompt}"],
      "timeout": 600
    }
  },
  "phase_defaults": {
    "planning": {"default_agent": "codex", "fallback_chain": ["claude-code"]},
    "implementation": {"default_agent": "claude-code", "fallback_chain": ["codex", "aider"]}
  },
  "story_type_defaults": {
    "feature": "claude-code",
    "bugfix": "codex",
    "refactor": "aider"
  }
}
```

### .agent-detection.json

Cache file for agent detection:

```json
{
  "version": "1.0.0",
  "updated_at": "2026-01-28T10:30:00Z",
  "platform": "windows",
  "agents": {
    "codex": {
      "name": "codex",
      "available": true,
      "path": "C:\\Users\\user\\.local\\bin\\codex.exe",
      "version": "0.8.2",
      "detection_method": "path"
    }
  }
}
```

## Agent Types

### task-tool

Built-in Claude Code Task tool:
- Always available
- No external dependencies
- Uses `subagent_type` for specialization

### cli

External CLI tools:
- Requires installation
- Auto-detected on first use
- Falls back to claude-code if unavailable

## Detection Methods

The detector tries multiple methods:

1. **PATH** - `shutil.which()` / system PATH
2. **Common Locations** - Platform-specific install paths
3. **Windows Registry** - For installed applications
4. **Custom Paths** - User-specified locations

## Platform Support

| Platform | Detection Methods |
|----------|-------------------|
| Windows | PATH, common locations, registry |
| macOS | PATH, common locations (/opt/homebrew, /usr/local) |
| Linux | PATH, common locations (~/.local/bin, /usr/local) |

## Resolution Priority

When determining which agent to use:

1. Command-line override (`--agent`)
2. Phase-specific override (`--impl-agent`)
3. Story-level `agent` property
4. Story type override for phase
5. Phase default agent
6. Fallback chain
7. `claude-code` (ultimate fallback)

## Adding Custom Agents

Add to `agents.json`:

```json
{
  "agents": {
    "my-agent": {
      "type": "cli",
      "description": "My custom agent",
      "command": "my-agent",
      "args": ["--prompt", "{prompt}"],
      "timeout": 600,
      "env": {
        "MY_API_KEY": "${MY_API_KEY}"
      }
    }
  }
}
```

## Troubleshooting

### Agent not detected
1. Verify installation: `which <agent>`
2. Check PATH environment variable
3. Run `/agent-config --action refresh`
4. Add custom path to agents.json

### Fallback happening unexpectedly
1. Check `/agent-config --action detect` output
2. Verify agent is in PATH
3. Check for version/permission issues
4. Look for errors in `.agent-outputs/` logs

## See Also

- `/approve` - Approve PRD with agent override
- `/auto-run` - Run with specific agent
- `/agent-status` - View running agents
- `/iteration-status` - View iteration progress
