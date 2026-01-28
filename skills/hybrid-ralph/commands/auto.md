---
name: hybrid:auto
description: Generate PRD from task description and enter review mode
arguments:
  - name: description
    description: Task description to generate PRD from
    required: true
  - name: agent
    description: Agent to use for PRD generation and story execution (e.g., codex, amp-code, claude-code)
    required: false
---

# /hybrid:auto

Automatically generate a Product Requirements Document (PRD) from your task description and enter review mode.

## Usage

```
/hybrid:auto <task description>
/hybrid:auto <task description> --agent <agent-name>
```

## What It Does

1. **Parses your task description** - Takes your natural language task description
2. **Launches Planning Agent** - Uses Claude Code's Task tool to analyze and plan
3. **Generates PRD draft** - Creates prd.json with:
   - Goal and objectives
   - User stories with priorities
   - Dependency analysis
   - Context size estimates
4. **Enters review mode** - Shows the PRD for your approval

## Example

```
/hybrid:auto Implement a user authentication system with login, registration, and password reset
```

With a specific agent:
```
/hybrid:auto Implement a user authentication system --agent codex
```

## Agent Support

You can specify which agent to use for PRD generation and story execution:

| Agent | Description |
|-------|-------------|
| `claude-code` | Built-in Task tool (default, always available) |
| `codex` | OpenAI Codex CLI |
| `amp-code` | Amp Code CLI |
| `aider` | Aider AI pair programming |
| `cursor-cli` | Cursor CLI |
| `claude-cli` | Claude CLI (standalone) |

If the specified agent is not available, it automatically falls back to `claude-code`.

You can also set a default agent in `prd.json` metadata:
```json
{
  "metadata": {
    "default_agent": "codex"
  }
}
```

Or specify per-story agents:
```json
{
  "stories": [
    {
      "id": "story-001",
      "agent": "amp-code",
      ...
    }
  ]
}
```

## After PRD Generation

You'll see the PRD review with options to:
- `/approve` - Accept the PRD and start execution
- `/edit` - Open prd.json in your editor for manual changes
- `/hybrid:replan` - Regenerate the PRD with different parameters

## Notes

- The Planning Agent will analyze your codebase to understand existing patterns
- Stories are automatically prioritized (high/medium/low)
- Dependencies are detected between stories
- Context estimates help agents work efficiently
- Use `/agent-status` to check running agents

## See Also

- `/hybrid:manual` - Load an existing PRD file
- `/approve` - Approve the current PRD
- `/edit` - Edit the current PRD
- `/agent-status` - View running agent status
