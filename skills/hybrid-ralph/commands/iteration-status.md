---
name: iteration-status
description: Show iteration progress, batch status, retry counts, and quality gate results
arguments:
  - name: verbose
    description: "Show detailed information including quality gate results"
    required: false
  - name: json
    description: "Output as JSON"
    required: false
---

# /iteration-status

Show the current status of automatic iteration execution.

## Usage

```
/iteration-status
/iteration-status --verbose
/iteration-status --json
```

## What It Shows

### Summary View (Default)

```
ITERATION STATUS
================

Status: running
Progress: 75% (6/8 stories)
Current Batch: 2 of 3
Iteration: 4

Batches:
  Batch 1: ● Complete (3/3 stories)
  Batch 2: ◐ In Progress (2/3 stories, 1 running)
  Batch 3: ○ Pending (0/2 stories)

Retries: 2 total (1 exhausted)
Quality Gates: 5 passed, 1 failed
```

### Verbose View (--verbose)

Includes:
- Individual story status within each batch
- Quality gate results per story
- Retry history with failure reasons
- Agent used for each story
- Duration and timing information

```
ITERATION STATUS (Verbose)
==========================

Status: running
Started: 2026-01-28 10:30:00
Elapsed: 45m 23s

Batch 1: Complete
  ● story-001: Setup project structure [claude-code] (5m 12s)
    Quality: typecheck ✓, tests ✓, lint ✓
  ● story-002: Create database models [codex] (8m 45s)
    Quality: typecheck ✓, tests ✓, lint ✓
  ● story-003: Implement API routes [claude-code] (12m 30s)
    Quality: typecheck ✓, tests ✓, lint ✓

Batch 2: In Progress
  ● story-004: Add authentication [claude-code] (10m 15s)
    Quality: typecheck ✓, tests ✓, lint ✓
  ◐ story-005: Create frontend components [aider] (running for 8m)
  ✗ story-006: Integration tests [claude-code]
    Retry 1/3: quality_gate - 2 tests failing
    Quality: typecheck ✓, tests ✗, lint ✓

Batch 3: Pending
  ○ story-007: Documentation
  ○ story-008: Deployment setup
```

### JSON Output (--json)

Returns structured data for programmatic access:

```json
{
  "status": "running",
  "progress_percent": 75,
  "current_batch": 2,
  "total_batches": 3,
  "current_iteration": 4,
  "completed_stories": 6,
  "failed_stories": 1,
  "total_stories": 8,
  "batch_results": [
    {
      "batch_num": 1,
      "started_at": "2026-01-28T10:30:00Z",
      "completed_at": "2026-01-28T10:55:00Z",
      "stories_launched": 3,
      "stories_completed": 3,
      "stories_failed": 0,
      "quality_gate_failures": 0,
      "success": true
    }
  ]
}
```

## Status Symbols

| Symbol | Meaning |
|--------|---------|
| ● | Complete |
| ◐ | In Progress |
| ○ | Pending |
| ✗ | Failed |
| ↻ | Retrying |

## Quality Gate Status

Quality gates show pass/fail for each enabled gate:

| Symbol | Meaning |
|--------|---------|
| ✓ | Passed |
| ✗ | Failed |
| - | Skipped/Disabled |

## Retry Information

Shows retry attempts for failed stories:

```
✗ story-006: Integration tests
  Retry 1/3: quality_gate - 2 tests failing
  Retry 2/3: exit_code - Timeout after 600s
```

Retry states:
- `N/M` - Attempt N of M maximum
- `exhausted` - All retries used, story marked failed

## State Recovery

If iteration was interrupted, status shows recovery info:

```
Status: paused
Pause Reason: User interrupt (Ctrl+C)
Resume: Run /auto-run to continue
```

## Troubleshooting

### No iteration state
```
No active iteration found.
Use /auto-run to start automatic iteration.
```

### Stuck iteration
If a story appears stuck:
1. Check `/agent-status` for process status
2. Check `.agent-outputs/<story-id>.log` for details
3. Use `/auto-run --pause` then `/auto-run` to restart

## See Also

- `/auto-run` - Start automatic iteration
- `/status` - View basic story status
- `/agent-status` - View running agents
- `/agent-config` - View agent configuration
