---
description: "Show execution status of all stories in the PRD. Displays batch progress, individual story states, completion percentage, and recent activity logs."
---

# Hybrid Ralph - Show Execution Status

You are displaying the current execution status of all stories in the PRD.

## Step 1: Verify PRD Exists

```bash
if [ ! -f "prd.json" ]; then
    echo "ERROR: No PRD found."
    exit 1
fi
```

## Step 2: Read PRD and Progress Files

Read `prd.json` to get all stories.
Read `progress.txt` to get completion status.

## Step 3: Calculate Story Status

For each story in the PRD, determine its status:
- **Complete**: If `progress.txt` contains `[COMPLETE] {story_id}`
- **In Progress**: If `progress.txt` contains `[IN_PROGRESS] {story_id}`
- **Pending**: If no entry in `progress.txt`
- **Failed**: If `progress.txt` contains `[FAILED] {story_id}`

## Step 4: Calculate Execution Batches

Re-calculate batches based on story dependencies.

## Step 5: Display Status Report

```
============================================================
EXECUTION STATUS
============================================================

## Summary
  Total Stories: {count}
  Total Batches: {batch_count}

  Complete:     {complete_count} ●
  In Progress:  {in_progress_count} ◐
  Pending:      {pending_count} ○
  Failed:       {failed_count} ✗

## Progress
  [{progress_bar}] {percentage}%

## Current Batch: {current_batch_number}

  {stories in current batch with their statuses}

## All Batches

  Batch 1: {status} ({count} stories)
    {story statuses}

  Batch 2: {status} ({count} stories)
    {story statuses}

...

============================================================
```

Status indicators:
- `●` Complete
- `◐` In Progress
- `○` Pending
- `✗` Failed

## Step 6: Show Recent Activity

Display the last 10 entries from `progress.txt`:

```
## Recent Activity

{last 10 lines from progress.txt}
```

## Step 7: Show Failed Stories (if any)

If there are failed stories, show them:

```
## Failed Stories

  ✗ {story_id}: {title}
     {error details from progress.txt}
```

## Step 8: Show Completion Status

If all stories are complete:

```
✓ All stories complete!

Next:
  - /plan-cascade:hybrid-complete - Complete and merge (if in worktree)
  - /plan-cascade:show-dependencies - Review final state
```

If some stories are still pending/in-progress:

```
Next:
  - Continue monitoring with: /plan-cascade:hybrid-status
  - View progress details in: progress.txt
  - View agent logs in: .agent-outputs/
```
