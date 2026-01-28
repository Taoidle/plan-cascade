---
name: auto-run
description: Start automatic iteration through all batches
arguments:
  - name: mode
    description: "Iteration mode: until_complete (default), max_iterations, batch_complete"
    required: false
  - name: max-iterations
    description: "Maximum iterations for max_iterations mode (default: 50)"
    required: false
  - name: agent
    description: "Force a specific agent for all phases"
    required: false
  - name: impl-agent
    description: "Force a specific agent for implementation phase"
    required: false
  - name: retry-agent
    description: "Force a specific agent for retry attempts"
    required: false
  - name: no-fallback
    description: "Disable automatic agent fallback"
    required: false
  - name: no-quality-gates
    description: "Disable quality gate verification"
    required: false
  - name: dry-run
    description: "Show what would be done without executing"
    required: false
---

# /auto-run

Start automatic iteration through all PRD batches until completion.

## Usage

```
/auto-run
/auto-run --mode until_complete
/auto-run --mode max_iterations --max-iterations 10
/auto-run --mode batch_complete
/auto-run --agent codex
/auto-run --impl-agent aider --no-fallback
/auto-run --dry-run
```

## Iteration Modes

| Mode | Description |
|------|-------------|
| `until_complete` | Run until all stories are complete (default) |
| `max_iterations` | Run up to N iterations, then stop |
| `batch_complete` | Run only the current batch, then stop |

## What Happens

1. **Analyzes PRD** - Reads prd.json and generates execution batches
2. **Starts Iteration Loop** - Begins automatic batch execution
3. **Executes Batches** - Runs stories in parallel within each batch
4. **Runs Quality Gates** - Verifies each story after completion (if enabled)
5. **Handles Failures** - Automatically retries failed stories
6. **Progresses to Next Batch** - Moves to next batch when current is complete
7. **Reports Completion** - Shows final summary when done

## Agent Selection

Stories are assigned agents based on priority chain:

1. `--agent` command parameter (overrides all)
2. `--impl-agent` for implementation phase
3. `story.agent` in PRD
4. Story type override (bugfix → codex, refactor → aider)
5. Phase default agent
6. Fallback chain (if agent unavailable)
7. `claude-code` (always available)

Use `--no-fallback` to disable automatic fallback.

## Quality Gates

Quality gates run after each story completion:

| Gate | Description |
|------|-------------|
| `typecheck` | tsc, mypy, pyright (auto-detected) |
| `test` | pytest, jest, npm test (auto-detected) |
| `lint` | eslint, ruff (auto-detected, optional) |
| `custom` | User-defined scripts |

Configure in PRD:
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

Use `--no-quality-gates` to skip verification.

## Retry Management

Failed stories are automatically retried:

- **Max Retries**: 3 attempts (configurable in PRD)
- **Exponential Backoff**: Delay between retries increases
- **Failure Context Injection**: Retry prompts include previous error info
- **Agent Switching**: May try different agent on retry

Configure in PRD:
```json
{
  "retry_config": {
    "max_retries": 3,
    "inject_failure_context": true
  }
}
```

## State Files

Auto-run creates state files for recovery:

| File | Purpose |
|------|---------|
| `.iteration-state.json` | Current iteration progress |
| `.retry-state.json` | Retry history per story |
| `.agent-status.json` | Running agent status |

## Monitoring Progress

While auto-run is active:

```
/iteration-status     # Show iteration progress
/status               # Show story statuses
/agent-status         # Show running agents
```

## Pausing and Resuming

Auto-run can be paused and resumed:

- **Pause**: Use Ctrl+C or `/auto-run --pause`
- **Resume**: Run `/auto-run` again (will resume from last state)
- **Reset**: Delete `.iteration-state.json` to start fresh

## PRD Configuration

Add to prd.json for persistent settings:

```json
{
  "metadata": { ... },
  "stories": [ ... ],
  "iteration_config": {
    "mode": "until_complete",
    "max_iterations": 50,
    "quality_gates_enabled": true,
    "auto_retry_enabled": true
  }
}
```

## Examples

### Basic auto-run
```
/approve
/auto-run
```

### Limited iterations
```
/auto-run --mode max_iterations --max-iterations 5
```

### Single batch
```
/auto-run --mode batch_complete
```

### Force specific agent
```
/auto-run --agent aider --no-fallback
```

### Preview without executing
```
/auto-run --dry-run
```

## See Also

- `/approve` - Approve PRD (can include `--auto-run`)
- `/iteration-status` - View iteration progress
- `/agent-config` - Configure agents
- `/status` - View story status
