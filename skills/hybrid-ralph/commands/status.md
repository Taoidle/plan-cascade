---
name: hybrid:status
description: Show execution status of all stories in the PRD
---

# /status

Show the current execution status of all stories in the PRD, including batch progress and individual story states.

## Usage

```
/status
```

## What It Shows

### Agent Summary
- Running agents count
- Completed agents count
- Failed agents count

### Summary
- Total stories and batches
- Stories by status (complete, in progress, pending, failed)
- Overall progress percentage

### Current Batch
- Which batch is currently executing
- Stories in the current batch with their statuses
- Agent used for each story

### All Batches
- All batches with their stories
- Status of each story
- Agent info (e.g., `[via codex]`)

### Recent Activity
- Last 10 entries from progress.txt (includes agent info)

### Failed Stories
- List of any failed stories (if applicable)
- Agent and error information

## Status Indicators

| Symbol | Meaning |
|--------|---------|
| ● | Complete |
| ◐ | In Progress |
| ○ | Pending |
| ✗ | Failed |

## Example Output

```
============================================================
EXECUTION STATUS
============================================================

Agents: 2 running, 2 completed, 0 failed

## Summary
  Total Stories: 4
  Total Batches: 2

  Complete:     2 ✓
  In Progress:  2 ◐
  Pending:      0 ○
  Failed:       0 ✗

## Progress
  [██████████████░░░░░░░░░░░░░░░] 50.0%

## Current Batch: 2

  ◐ story-003: Implement user login [in_progress] [via codex]
  ◐ story-004: Implement password reset [in_progress] [via amp-code]

## All Batches

  Batch 1: ✓ (2 stories)
    ● story-001: Design database schema [via claude-code]
    ● story-002: Design API endpoints [via claude-code]

  Batch 2: ○ (2 stories)
    ◐ story-003: Implement user login [via codex]
    ◐ story-004: Implement password reset [via amp-code]

============================================================
```

## Completion

When all stories are complete, you'll see:

```
✓ All stories complete!
```

## See Also

- `/hybrid:auto` - Start a new task
- `/hybrid:manual` - Load existing PRD
- `/approve` - Approve PRD and begin execution
- `/agent-status` - View detailed agent status
