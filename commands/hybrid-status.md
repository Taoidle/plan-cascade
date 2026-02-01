---
description: "Show execution status of all stories in the PRD. Displays batch progress, individual story states, completion percentage, and recent activity logs."
---

# Hybrid Ralph - Show Execution Status

You are displaying the current execution status of all stories in the PRD.

## Path Storage Modes

This command works with both new and legacy path storage modes:

### New Mode (Default)
- PRD file: In worktree or `~/.plan-cascade/<project-id>/prd.json`
- Progress file: Always in working directory `progress.txt`
- Agent outputs: `.agent-outputs/` in working directory

### Legacy Mode
- PRD file: In worktree or project root `prd.json`
- Progress file: `progress.txt` in working directory

User-visible files always remain in the working directory for easy access.

## Tool Usage Policy (CRITICAL)

**To avoid command confirmation prompts:**

1. **Use Read tool for file reading** - NEVER use `cat` via Bash
   - ✅ `Read("prd.json")`, `Read("progress.txt")`
   - ❌ `Bash("cat prd.json")`

2. **Use Grep tool for content search** - NEVER use `grep` via Bash
   - ✅ `Grep("[COMPLETE]", path="progress.txt")`
   - ❌ `Bash("grep '[COMPLETE]' progress.txt")`

3. **Parse file contents in your response** - After reading with Read tool, count markers yourself

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
- **Complete**: If `progress.txt` contains `[COMPLETE] {story_id}` or `[STORY_COMPLETE] {story_id}`
- **In Progress**: If `progress.txt` contains `[IN_PROGRESS] {story_id}`
- **Pending**: If no entry in `progress.txt`
- **Failed**: If `progress.txt` contains `[FAILED] {story_id}` or `[STORY_FAILED] {story_id}` or `[ERROR] {story_id}`

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

If execution was interrupted:

```
To resume an interrupted task:
  /plan-cascade:hybrid-resume --auto

This will:
  - Auto-detect current state from files
  - Skip already-completed stories
  - Resume execution from where it left off
```
