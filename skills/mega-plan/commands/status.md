---
name: mega:status
description: Show detailed status of mega-plan execution
---

# /mega:status

Display comprehensive status of the mega-plan execution.

## Your Task

### Step 1: Check for Mega Plan

First, verify mega-plan.json exists:

```bash
ls -la mega-plan.json 2>/dev/null
```

If it doesn't exist, inform the user:
```
No mega-plan.json found.
Use /mega:plan <description> to create one first.
```

### Step 2: Read Current State

Read the mega-plan and status files:

```bash
cat mega-plan.json
cat .mega-status.json 2>/dev/null || echo "{}"
```

### Step 3: Sync Status from Worktrees

Update status by checking each worktree:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/mega-plan/scripts/mega-sync.py"
```

### Step 4: Calculate Progress

For each feature, check:
- Does the worktree exist?
- Does prd.json exist in the worktree?
- What's the story completion status?

### Step 5: Display Comprehensive Status

Show a detailed status report:

```
============================================================
MEGA PLAN STATUS
============================================================

Project: <goal>
Mode: <execution_mode>
Target: <target_branch>

Overall Progress: ██████████░░░░░░░░░░ 50% (2/4 features)

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
      Current: story-003 - Implement search functionality

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
  # Continue story-003 execution

When Batch 1 completes:
  Batch 2 will start automatically (auto mode)

When all complete:
  /mega:complete

============================================================
```

### Status Symbols

Use these symbols for feature status:

| Symbol | Meaning |
|--------|---------|
| `[ ]` | Pending |
| `[~]` | PRD generated, awaiting approval |
| `[>]` | In progress |
| `[X]` | Complete |
| `[!]` | Failed |

### Progress Bar

Generate a visual progress bar:

```
Progress: ████████████░░░░░░░░ 60%
```

Use filled blocks (█) for complete percentage, empty blocks (░) for remaining.

### Step 6: Show Worktree Details

For each active worktree, show:

```
Worktree Details:
  .worktree/feature-products/
    Branch: mega-feature-products
    PRD: 5 stories
    Findings: 12 entries
    Last activity: 2 minutes ago
```

### Step 7: Check for Issues

Identify and highlight any issues:

```
============================================================
ISSUES
============================================================

[WARN] feature-002 has been in_progress for 2 hours
[WARN] Stale lock file detected: .locks/prd.json.lock
[ERROR] feature-003 failed - check .worktree/feature-cart/progress.txt

============================================================
```

### Step 8: Offer Actions

Based on status, suggest relevant actions:

**If features are in progress:**
```
To check a feature's progress:
  cd .worktree/<feature-name>
  /status

To view feature logs:
  cat .worktree/<feature-name>/progress.txt
```

**If all features complete:**
```
All features are complete!
Run /mega:complete to merge and clean up.
```

**If a feature failed:**
```
Feature <name> failed. To investigate:
  cd .worktree/<feature-name>
  cat progress.txt
  # Fix issues and re-run /approve
```
