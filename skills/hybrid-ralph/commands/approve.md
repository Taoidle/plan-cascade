---
name: approve
description: Approve the current PRD and begin execution
arguments:
  - name: agent
    description: Override default agent for all stories (e.g., codex, amp-code)
    required: false
  - name: impl-agent
    description: Override agent for implementation phase only
    required: false
  - name: planning-agent
    description: Override agent for planning phase only
    required: false
  - name: retry-agent
    description: Override agent for retry attempts
    required: false
  - name: no-fallback
    description: Disable automatic agent fallback (fail if agent unavailable)
    required: false
  - name: auto-run
    description: Start automatic iteration after approval
    required: false
  - name: auto-run-mode
    description: "Mode for auto-run: until_complete, max_iterations, batch_complete"
    required: false
  - name: max-iterations
    description: Maximum iterations for auto-run (default 50)
    required: false
---

# /approve

Approve the current PRD and begin parallel execution of stories.

## Usage

```
/approve
/approve --agent codex
/approve --impl-agent aider
/approve --auto-run
/approve --auto-run --auto-run-mode until_complete
/approve --agent codex --no-fallback
```

## What Happens After Approval

1. **Creates execution plan** - Analyzes dependencies and creates batches
2. **Shows execution summary** - Displays batches and execution strategy
3. **Starts Batch 1** - Launches parallel agents for first batch of stories
4. **Monitors progress** - Tracks story completion in progress.txt

## Agent Selection

Stories can be executed using different agents:

| Agent | Type | Description |
|-------|------|-------------|
| `claude-code` | task-tool | Built-in Task tool (default, always available) |
| `codex` | cli | OpenAI Codex CLI |
| `amp-code` | cli | Amp Code CLI |
| `aider` | cli | Aider AI pair programming |
| `cursor-cli` | cli | Cursor CLI |
| `claude-cli` | cli | Claude CLI (standalone) |

Agent priority (highest to lowest):
1. `--agent` command argument (global override)
2. `--impl-agent` / `--planning-agent` (phase-specific override)
3. `story.agent` in PRD
4. Story type override for phase (e.g., bugfix → codex)
5. Phase default agent (from agents.json phase_defaults)
6. Fallback chain (if agent unavailable)
7. `claude-code` (always available fallback)

Use `--no-fallback` to disable automatic fallback and fail if specified agent is unavailable.

## Execution Flow

```
┌─────────────────────────────────────────────────────────┐
│  PRD Approved                                            │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Resolve Agents                                          │
│  - Check PRD metadata for default agent                 │
│  - Check each story for agent override                  │
│  - Verify CLI availability (fallback to claude-code)    │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Generate Execution Batches                              │
│  - Batch 1: Stories with no dependencies (parallel)      │
│  - Batch 2: Stories whose Batch 1 deps are complete      │
│  - Batch N: Continue until all stories complete          │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Execute Batch 1                                         │
│  - Launch agents (Task tool or CLI) for each story      │
│  - Each agent gets filtered context                     │
│  - Agents write findings with story tags                │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Wait for Batch Completion                               │
│  - Monitor progress.txt for completion markers          │
│  - Check /status for real-time updates                  │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
                    ┌─────────┐
                    │ More    │──── Yes ────▶ Start Next Batch
                    │ Batches?│
                    └─────────┘
                          │ No
                          ▼
┌─────────────────────────────────────────────────────────┐
│  All Stories Complete                                    │
│  - Merge worktree if in worktree mode                   │
│  - Show completion summary                              │
└─────────────────────────────────────────────────────────┘
```

## Context Filtering

Each agent receives only relevant context:
- Their story description and acceptance criteria
- Summaries of completed dependencies
- Findings tagged with their story ID

This keeps context focused and efficient.

## Progress Tracking

Monitor execution with:
- `/status` - Show current batch and story statuses
- `/agent-status` - Show running agents and their processes
- `progress.txt` - Detailed progress log with agent info
- `.agent-status.json` - Structured agent status data
- `.agent-outputs/` - Individual agent logs

Progress log format now includes agent information:
```
[2026-01-28 10:30:00] story-001: [START] via codex (pid:12345)
[2026-01-28 10:35:00] story-001: [COMPLETE] via codex
[2026-01-28 10:30:05] story-002: [START] via amp-code (pid:12346)
[2026-01-28 10:36:00] story-002: [FAILED] via amp-code: exit code 1
```

## Completion

When all stories complete:
- `[COMPLETE]` markers appear in progress.txt
- Agent status updated in `.agent-status.json`
- Worktree is merged to target branch (if applicable)
- Summary shows successful and failed stories with agent info

## Auto-Run Mode

Use `--auto-run` to start automatic iteration after approval:

```
/approve --auto-run
/approve --auto-run --auto-run-mode max_iterations --max-iterations 10
```

This is equivalent to running `/approve` followed by `/auto-run`.

### Auto-Run Modes

| Mode | Description |
|------|-------------|
| `until_complete` | Run until all stories complete (default) |
| `max_iterations` | Run up to N iterations |
| `batch_complete` | Run only first batch |

## See Also

- `/status` - Check execution status
- `/agent-status` - Check agent status
- `/edit` - Modify PRD before approval
- `/show-dependencies` - View dependency graph
- `/auto-run` - Start automatic iteration
- `/iteration-status` - View iteration progress
- `/agent-config` - View/edit agent configuration
