---
name: planning-with-files:hybrid-status
description: Show execution status of all stories in the PRD. Displays batch progress, individual story states, completion percentage, and recent activity logs.
disable-model-invocation: true
---

# /planning-with-files:hybrid-status

Show the current execution status of all stories in the PRD, including batch progress and individual story states.

## Usage

```
/planning-with-files:hybrid-status
```

## What It Shows

### Summary
- Total stories and batches
- Stories by status (complete, in progress, pending, failed)
- Overall progress percentage

### Current Batch
- Which batch is currently executing
- Stories in the current batch with their statuses

### All Batches
- All batches with their stories
- Status of each story

### Recent Activity
- Last 10 entries from progress.txt

### Failed Stories
- List of any failed stories (if applicable)

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

## Summary
  Total Stories: 4
  Total Batches: 2

  Complete:     2 ✓
  In Progress:  1 ◐
  Pending:      1 ○
  Failed:       0 ✗

## Progress
  [███░░░░░░░░░░░░░░░░░░░░░░░░░░] 50.0%

## Current Batch: 2

  ◐ story-003: Implement user login [in_progress]
  ○ story-004: Implement password reset [pending]

## All Batches

  Batch 1: ✓ (2 stories)
    ● story-001: Design database schema
    ● story-002: Design API endpoints

  Batch 2: ○ (2 stories)
    ◐ story-003: Implement user login
    ○ story-004: Implement password reset

============================================================
```

## Completion

When all stories are complete, you'll see:

```
✓ All stories complete!
```

## See Also

- `/planning-with-files:hybrid-auto` - Start a new task
- `/planning-with-files:hybrid-manual` - Load existing PRD
- `/planning-with-files:approve` - Approve PRD and begin execution
