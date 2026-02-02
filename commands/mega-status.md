---
description: "Show detailed status of mega-plan execution including feature progress and story completion. Usage: /plan-cascade:mega-status"
---

# Mega Plan Status

Display comprehensive status of the mega-plan execution.

## Path Storage Modes

This command works with both new and legacy path storage modes:

### New Mode (Default)
- `mega-plan.json`: `~/.plan-cascade/<project-id>/mega-plan.json`
- `.mega-status.json`: `~/.plan-cascade/<project-id>/.state/.mega-status.json`
- Worktrees: `~/.plan-cascade/<project-id>/.worktree/<feature>/`

### Legacy Mode
- All files in project root

The command uses PathResolver to find files in the correct locations.

## Tool Usage Policy (CRITICAL)

**To avoid command confirmation prompts:**

1. **Use Read tool for file reading** - NEVER use `cat` via Bash
   - ✅ `Read("mega-plan.json")`, `Read(".mega-status.json")`
   - ❌ `Bash("cat mega-plan.json")`

2. **Use Glob tool for finding files** - NEVER use `ls` or `find` via Bash
   - ✅ `Glob(".worktree/*/progress.txt")`
   - ❌ `Bash("ls .worktree/")`

3. **Use Grep tool for content search** - NEVER use `grep` via Bash
   - ✅ `Grep("[FEATURE_COMPLETE]", path="...")`
   - ❌ `Bash("grep '[FEATURE_COMPLETE]' ...")`

4. **Only use Bash for actual system commands** (git, mkdir, etc.)

## Step 1: Verify Mega Plan Exists

```bash
# Get mega-plan path from PathResolver
MEGA_PLAN_PATH=$(uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_mega_plan_path())" 2>/dev/null || echo "mega-plan.json")

if [ ! -f "$MEGA_PLAN_PATH" ]; then
    echo "No mega-plan.json found at: $MEGA_PLAN_PATH"
    echo "Use /plan-cascade:mega-plan <description> to create one first."
    exit 1
fi
```

## Step 2: Read Current State

Read the mega-plan and sync status from worktrees:

```bash
# Get file paths from PathResolver
MEGA_PLAN_PATH=$(uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_mega_plan_path())" 2>/dev/null || echo "mega-plan.json")
MEGA_STATUS_PATH=$(uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_mega_status_path())" 2>/dev/null || echo ".mega-status.json")

cat "$MEGA_PLAN_PATH"
cat "$MEGA_STATUS_PATH" 2>/dev/null || echo "{}"
```

## Step 3: Sync Status from Worktrees

For each feature with a worktree, check:
- Does the worktree exist?
- Does prd.json exist?
- What's the story completion status?
- Check progress.txt for markers:
  - `[PRD_COMPLETE] {feature_id}` - PRD generation done
  - `[STORY_COMPLETE] {story_id}` - Story done
  - `[STORY_FAILED] {story_id}` - Story failed
  - `[FEATURE_COMPLETE] {feature_id}` - All stories done
  - `[FEATURE_FAILED] {feature_id}` - Feature failed

Update .mega-status.json with current statuses.

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
  /plan-cascade:mega-approve
  # This will merge current batch and start next batch

When all batches complete:
  /plan-cascade:mega-complete
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
  /plan-cascade:hybrid-status

To view feature logs:
  cat .worktree/<feature-name>/progress.txt
```

**If all features complete:**
```
All features are complete!
Run /plan-cascade:mega-complete to merge and clean up.
```

**If a feature failed:**
```
Feature <name> failed. To investigate:
  cd .worktree/<feature-name>
  cat progress.txt
  # Fix issues and re-run /plan-cascade:approve
```

**If execution was interrupted:**
```
To resume an interrupted mega-plan:
  /plan-cascade:mega-resume --auto-prd

This will:
  - Auto-detect current state from files
  - Skip already-completed work
  - Resume from where it left off
```

**If PRDs awaiting approval:**
```
PRDs waiting for approval:
  cd .worktree/<feature-name>
  cat prd.json
  /plan-cascade:approve
```

## Automated Execution Mode

When running `/plan-cascade:mega-approve --auto-prd`, the execution is fully automated:
- PRDs are generated automatically for each feature
- Stories are executed automatically
- Progress is monitored continuously
- Batches merge and transition automatically

To check progress during automated execution:
```
/plan-cascade:mega-status
```

Progress markers in worktree progress.txt files:
- `[PRD_COMPLETE] feature-xxx` - PRD generation finished
- `[STORY_COMPLETE] story-xxx` - Individual story completed
- `[FEATURE_COMPLETE] feature-xxx` - All stories done, ready for merge
