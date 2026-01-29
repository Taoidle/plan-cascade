---
description: "Show detailed status of mega-plan execution including feature progress and story completion. Usage: /planning-with-files:mega-status"
---

# Mega Plan Status

Display comprehensive status of the mega-plan execution.

## Step 1: Verify Mega Plan Exists

```bash
if [ ! -f "mega-plan.json" ]; then
    echo "No mega-plan.json found."
    echo "Use /planning-with-files:mega-plan <description> to create one first."
    exit 1
fi
```

## Step 2: Read Current State

Read the mega-plan and sync status from worktrees:

```bash
cat mega-plan.json
cat .mega-status.json 2>/dev/null || echo "{}"
```

## Step 3: Sync Status from Worktrees

For each feature with a worktree, check:
- Does the worktree exist?
- Does prd.json exist?
- What's the story completion status?
- Any errors in progress.txt?

Update mega-plan.json with current statuses.

## Step 4: Calculate Progress

- Total features
- Completed features
- In-progress features
- Pending features
- Failed features (if any)

Calculate percentage: `(completed / total) * 100`

## Step 5: Generate Progress Bar

```
Progress: ████████████░░░░░░░░ 60%
```

Use filled blocks for complete percentage, empty for remaining.

## Step 6: Display Comprehensive Status

```
============================================================
MEGA PLAN STATUS
============================================================

Project: <goal>
Mode: <auto|manual>
Target: <target_branch>

Overall Progress: ████████████░░░░░░░░ 50% (2/4 features)

============================================================
FEATURE STATUS
============================================================

Batch 1 (Parallel):
  [X] feature-001: User Authentication
      Worktree: .worktree/feature-auth/
      Stories: 4/4 complete (100%)
      Status: complete

  [>] feature-002: Product Catalog
      Worktree: .worktree/feature-products/
      Stories: 2/5 complete (40%)
      Status: in_progress
      Current: story-003 - Implement search

Batch 2 (Waiting for Batch 1):
  [ ] feature-003: Shopping Cart
      Worktree: not created
      Dependencies: feature-001, feature-002
      Status: pending

  [ ] feature-004: Order Processing
      Worktree: not created
      Dependencies: feature-003
      Status: pending

============================================================
BATCH SUMMARY
============================================================

Batch 1: 1/2 complete (50%)
  - feature-001: complete
  - feature-002: in_progress (2/5 stories)

Batch 2: 0/2 complete (waiting)
  - Blocked by: feature-002

============================================================
NEXT ACTIONS
============================================================

Current work:
  cd .worktree/feature-products
  # Continue story execution

When current batch completes:
  /planning-with-files:mega-approve
  # This will merge current batch and start next batch

When all batches complete:
  /planning-with-files:mega-complete
  # This will cleanup planning files

============================================================
```

## Batch-by-Batch Execution Model

```
Batch 1: feature-001, feature-002 (parallel, from target_branch)
    │
    └─→ When ALL complete: mega-approve merges to target_branch
                                │
Batch 2: feature-003, feature-004 (parallel, from UPDATED target_branch)
    │                           ↑ includes Batch 1 code
    └─→ When ALL complete: mega-approve merges to target_branch
                                │
Final: mega-complete (cleanup only)
```

This ensures each batch's features have access to code from previous batches.

## Status Symbols

| Symbol | Meaning |
|--------|---------|
| `[ ]` | Pending |
| `[~]` | PRD generated, awaiting approval |
| `[>]` | In progress |
| `[X]` | Complete |
| `[!]` | Failed |

## Step 7: Check for Issues

Identify and highlight any issues:

```
============================================================
ISSUES DETECTED
============================================================

[WARN] feature-002 has been in_progress for 2 hours
[WARN] Stale lock file: .locks/prd.json.lock
[ERROR] feature-003 failed - check .worktree/feature-cart/progress.txt

============================================================
```

## Step 8: Suggest Actions

Based on current status:

**If features are in progress:**
```
To check a feature's detailed progress:
  cd .worktree/<feature-name>
  /planning-with-files:hybrid-status

To view feature logs:
  cat .worktree/<feature-name>/progress.txt
```

**If all features complete:**
```
All features are complete!
Run /planning-with-files:mega-complete to merge and clean up.
```

**If a feature failed:**
```
Feature <name> failed. To investigate:
  cd .worktree/<feature-name>
  cat progress.txt
  # Fix issues and re-run /planning-with-files:approve
```

**If PRDs awaiting approval:**
```
PRDs waiting for approval:
  cd .worktree/<feature-name>
  cat prd.json
  /planning-with-files:approve
```
