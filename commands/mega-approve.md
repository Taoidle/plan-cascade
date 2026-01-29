---
description: "Approve the mega-plan and start feature execution. Creates worktrees and generates PRDs for each feature. Usage: /planning-with-files:mega-approve [--auto-prd]"
---

# Approve Mega Plan and Start Execution

Approve the mega-plan and begin executing features in **batch-by-batch** order.

**IMPORTANT**: This command can be called multiple times:
1. First call: Starts Batch 1
2. When Batch 1 completes: Merges Batch 1, then starts Batch 2
3. And so on...

This ensures each batch builds upon the previous batch's code.

## Arguments

- `--auto-prd`: Automatically approve all generated PRDs (skip manual review)

## Step 1: Verify Mega Plan Exists

```bash
if [ ! -f "mega-plan.json" ]; then
    echo "No mega-plan.json found."
    echo "Use /planning-with-files:mega-plan <description> to create one first."
    exit 1
fi
```

## Step 2: Parse Arguments and State

Check if `--auto-prd` was specified:

```bash
AUTO_PRD=false
if [[ "$ARGUMENTS" == *"--auto-prd"* ]]; then
    AUTO_PRD=true
fi
```

Read current state from `.mega-status.json`:
- `current_batch`: Which batch is currently executing (0 = not started)
- `completed_batches`: List of completed batch numbers

## Step 3: Determine Current State

Calculate all batches from mega-plan.json based on dependencies:
- **Batch 1**: Features with no dependencies
- **Batch 2**: Features depending only on Batch 1 features
- **Batch N**: Features depending only on Batch 1..N-1 features

Check the current state:

### Case A: No batch started yet (current_batch = 0)
→ Start Batch 1

### Case B: Current batch is in progress
→ Check if all features in current batch are complete
→ If not complete: Show status and exit
→ If complete: Merge current batch, then start next batch

### Case C: All batches complete
→ Inform user to run `/planning-with-files:mega-complete`

## Step 4: Handle Batch Completion (if needed)

If current batch is complete, **merge it before starting the next batch**:

```
============================================================
BATCH <N> COMPLETED - MERGING TO TARGET BRANCH
============================================================

Merging completed features in dependency order...
```

### 4.1: Checkout Target Branch

```bash
TARGET_BRANCH=$(read from mega-plan.json)
git checkout "$TARGET_BRANCH"
git pull origin "$TARGET_BRANCH" 2>/dev/null || true
```

### 4.2: Merge Each Feature in Current Batch

For each feature in the completed batch (in dependency order):

```bash
FEATURE_NAME="<name>"
WORKTREE_PATH=".worktree/$FEATURE_NAME"
BRANCH_NAME="mega-$FEATURE_NAME"

# First, commit any uncommitted changes in the worktree (code only, not planning files)
cd "$WORKTREE_PATH"

# Stage only code files, explicitly exclude planning files
git add -A
git reset HEAD -- prd.json findings.md progress.txt .planning-config.json .agent-status.json mega-findings.md 2>/dev/null || true
git commit -m "feat: complete $FEATURE_NAME" || true

cd -

# Now merge from target branch
git merge "$BRANCH_NAME" --no-ff -m "Merge feature: <title>

Mega-plan feature: <feature-id>
Batch: <batch-number>"
```

Show progress:
```
[OK] Merged feature-001: <title>
[OK] Merged feature-002: <title>

Batch <N> merged successfully!
```

### 4.3: Cleanup Current Batch Worktrees

```bash
# Remove worktrees for completed batch
git worktree remove ".worktree/$FEATURE_NAME" --force
git branch -d "mega-$FEATURE_NAME"
```

### 4.4: Update Status

Update `.mega-status.json`:
- Add current batch to `completed_batches`
- Increment `current_batch` or set to next batch number

## Step 5: Start Next Batch

If there are more batches to execute:

### 5.1: Create Worktrees from UPDATED Target Branch

**CRITICAL**: Worktrees must be created from the **current** target branch HEAD, which now includes all previously merged batches.

```bash
# Make sure we're on the updated target branch
git checkout "$TARGET_BRANCH"

FEATURE_NAME="<feature-name>"
BRANCH_NAME="mega-$FEATURE_NAME"
WORKTREE_PATH=".worktree/$FEATURE_NAME"

# Create worktree from current HEAD (which includes previous batches)
git worktree add -b "$BRANCH_NAME" "$WORKTREE_PATH"
```

### 5.2: Initialize Each Worktree

Create planning files in each worktree:

1. `.planning-config.json`
2. `findings.md`
3. `progress.txt`
4. Copy `mega-findings.md` (for reference)

### 5.3: Generate PRDs

Launch Task agents to generate PRDs for each feature in the new batch.

### 5.4: Execute Stories

If `--auto-prd`, immediately start story execution.
Otherwise, prompt user to review PRDs.

## Step 6: Update Status

Update `.mega-status.json`:
```json
{
  "updated_at": "<timestamp>",
  "current_batch": <batch-number>,
  "completed_batches": [1, 2, ...],
  "features": {
    "<feature-id>": {
      "status": "in_progress",
      "batch": <batch-number>,
      "worktree": ".worktree/<name>"
    }
  }
}
```

## Step 7: Show Status

```
============================================================
BATCH <N> STARTED
============================================================

Previous batches merged: [1, 2, ...]
Current batch: <N>

Features in progress:
  [>] feature-003: <title>
      Worktree: .worktree/<name>/
      Branch base: <target_branch> (includes Batch 1-2 code)

  [>] feature-004: <title>
      Worktree: .worktree/<name>/
      Branch base: <target_branch> (includes Batch 1-2 code)

============================================================

Monitor progress: /planning-with-files:mega-status

When Batch <N> completes, run:
  /planning-with-files:mega-approve

To merge and start Batch <N+1>.

============================================================
```

## All Batches Complete

If all batches are done:

```
============================================================
ALL BATCHES COMPLETE
============================================================

All feature batches have been merged to <target_branch>!

Completed batches: [1, 2, 3]
Total features merged: <count>

Final cleanup:
  /planning-with-files:mega-complete

This will remove remaining planning files.

============================================================
```

## Error Handling

### Merge Conflict

```
============================================================
MERGE CONFLICT
============================================================

Conflict while merging feature-002: <title>

Conflicting files:
  - src/api/products.ts

To resolve:
  1. Resolve conflicts in the listed files
  2. git add <resolved-files>
  3. git commit
  4. Re-run /planning-with-files:mega-approve

Or abort: git merge --abort
```

### Feature Not Complete

```
============================================================
BATCH <N> NOT COMPLETE
============================================================

Cannot proceed - some features are still in progress:

  [>] feature-002: Product Catalog
      Status: in_progress
      Stories: 3/5 complete
      Location: .worktree/feature-products/

Wait for all features to complete, then run:
  /planning-with-files:mega-approve
```

## Execution Flow Summary

```
mega-approve (1st call)
    │
    ├─→ Create Batch 1 worktrees (from target_branch)
    ├─→ Generate PRDs
    └─→ Execute stories

    ... Batch 1 features execute ...

mega-approve (2nd call, after Batch 1 complete)
    │
    ├─→ Merge Batch 1 to target_branch
    ├─→ Cleanup Batch 1 worktrees
    ├─→ Create Batch 2 worktrees (from UPDATED target_branch)
    ├─→ Generate PRDs
    └─→ Execute stories

    ... Batch 2 features execute ...

mega-approve (3rd call, after Batch 2 complete)
    │
    ├─→ Merge Batch 2 to target_branch
    ├─→ Cleanup Batch 2 worktrees
    └─→ All batches complete! → mega-complete
```
