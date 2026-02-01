---
description: "Complete the mega-plan by cleaning up planning files. All features should already be merged via mega-approve. Usage: /plan-cascade:mega-complete"
---

# Complete Mega Plan

Complete the mega-plan by cleaning up remaining planning files.

**Note**: In the new batch-by-batch execution model, code merging happens automatically when each batch completes (via `mega-approve`). This command only performs final cleanup.

## Path Storage Modes

This command works with both new and legacy path storage modes:

### New Mode (Default)
Cleanup removes files from user data directory:
- `~/.plan-cascade/<project-id>/mega-plan.json`
- `~/.plan-cascade/<project-id>/.state/.mega-status.json`
- `~/.plan-cascade/<project-id>/.worktree/` (remaining worktrees)
- `<project-root>/mega-findings.md` (user-visible file in project root)

### Legacy Mode
Cleanup removes files from project root:
- `<project-root>/mega-plan.json`
- `<project-root>/.mega-status.json`
- `<project-root>/.worktree/`
- `<project-root>/mega-findings.md`

The command auto-detects which mode is active.

## Step 1: Verify Mega Plan Exists

```bash
# Get mega-plan path from PathResolver
MEGA_PLAN_PATH=$(python3 -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_mega_plan_path())" 2>/dev/null || echo "mega-plan.json")

if [ ! -f "$MEGA_PLAN_PATH" ]; then
    echo "No mega-plan.json found at: $MEGA_PLAN_PATH"
    echo "Nothing to complete."
    exit 0
fi
```

## Step 2: Check Completion Status

Read `.mega-status.json` and `mega-plan.json` to verify:
1. All batches have been completed
2. All features have been merged

If any batches are still pending:

```
============================================================
CANNOT COMPLETE - BATCHES PENDING
============================================================

The following batches have not been completed:

  Batch 2:
    [ ] feature-003: Shopping Cart
    [ ] feature-004: Order Processing

Complete remaining batches first:
  /plan-cascade:mega-approve

Then run this command again.
============================================================
```

Exit without changes.

## Step 3: Verify Current Branch

```bash
# Check we're on the target branch
TARGET_BRANCH=$(read from mega-plan.json)
CURRENT_BRANCH=$(git branch --show-current)

if [ "$CURRENT_BRANCH" != "$TARGET_BRANCH" ]; then
    echo "Warning: Not on target branch ($TARGET_BRANCH)"
    echo "Currently on: $CURRENT_BRANCH"
fi
```

## Step 4: Show Completion Summary

```
============================================================
MEGA PLAN COMPLETION
============================================================

All features have been merged!

Target Branch: <target_branch>

Completed Features:
  Batch 1:
    [X] feature-001: User Authentication
    [X] feature-002: Product Catalog

  Batch 2:
    [X] feature-003: Shopping Cart
    [X] feature-004: Order Processing

Total: 4 features merged

Remaining cleanup:
  - Remove mega-plan.json
  - Remove mega-findings.md
  - Remove .mega-status.json
  - Prune any remaining worktrees

============================================================
```

## Step 5: Confirm Cleanup

Use AskUserQuestion:

**Proceed with cleanup?**

Options:
1. **Yes, cleanup** - Remove planning files
2. **Keep files** - Keep planning files for reference
3. **Cancel** - Do nothing

## Step 6: Cleanup Planning Files

If user selected "Yes, cleanup":

### 6.1: Remove Planning Files

```bash
# Get file paths from PathResolver (handles new vs legacy mode)
MEGA_PLAN_PATH=$(python3 -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_mega_plan_path())" 2>/dev/null || echo "mega-plan.json")
MEGA_STATUS_PATH=$(python3 -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_mega_status_path())" 2>/dev/null || echo ".mega-status.json")
MEGA_FINDINGS_PATH=$(python3 -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_mega_findings_path())" 2>/dev/null || echo "mega-findings.md")

# Remove mega-plan files
rm -f "$MEGA_PLAN_PATH"
rm -f "$MEGA_FINDINGS_PATH"
rm -f "$MEGA_STATUS_PATH"

echo "[OK] Removed mega-plan.json from: $MEGA_PLAN_PATH"
echo "[OK] Removed mega-findings.md from: $MEGA_FINDINGS_PATH"
echo "[OK] Removed .mega-status.json from: $MEGA_STATUS_PATH"
```

### 6.2: Cleanup Any Remaining Worktrees

```bash
# Get worktree base directory from PathResolver
WORKTREE_BASE=$(python3 -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_worktree_dir())" 2>/dev/null || echo ".worktree")

# Check for any remaining worktrees in the resolved location
if [ -d "$WORKTREE_BASE" ]; then
    # List and remove any remaining worktrees
    for dir in "$WORKTREE_BASE"/*/; do
        if [ -d "$dir" ]; then
            FEATURE_NAME=$(basename "$dir")
            git worktree remove "$dir" --force 2>/dev/null || rm -rf "$dir"
            echo "[OK] Removed worktree: $dir"
        fi
    done

    # Remove the worktree directory if empty
    rmdir "$WORKTREE_BASE" 2>/dev/null || true
fi

# Also check legacy location if different
if [ "$WORKTREE_BASE" != ".worktree" ] && [ -d ".worktree" ]; then
    for dir in .worktree/*/; do
        if [ -d "$dir" ]; then
            FEATURE_NAME=$(basename "$dir")
            git worktree remove "$dir" --force 2>/dev/null || rm -rf "$dir"
            echo "[OK] Removed legacy worktree: $dir"
        fi
    done
    rmdir .worktree 2>/dev/null || rm -rf .worktree
fi

# Prune git worktree list
git worktree prune
echo "[OK] Pruned git worktree list"

# Optionally clean up project data directory in new mode
PROJECT_DIR=$(python3 -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; r=PathResolver(Path.cwd()); print(r.get_project_dir()) if not r.is_legacy_mode() else print('')" 2>/dev/null || echo "")
if [ -n "$PROJECT_DIR" ] && [ -d "$PROJECT_DIR" ]; then
    # Check if directory is empty (only manifest.json might remain)
    FILE_COUNT=$(find "$PROJECT_DIR" -type f ! -name "manifest.json" | wc -l)
    if [ "$FILE_COUNT" -eq 0 ]; then
        echo "Project data directory is empty, cleaning up..."
        rm -rf "$PROJECT_DIR"
        echo "[OK] Removed project data directory: $PROJECT_DIR"
    fi
fi
```

### 6.3: Cleanup Remaining Feature Branches

```bash
# Delete any remaining mega-* branches
for branch in $(git branch --list "mega-*"); do
    git branch -d "$branch" 2>/dev/null || git branch -D "$branch"
    echo "[OK] Deleted branch: $branch"
done
```

## Step 7: Show Final Summary

```
============================================================
MEGA PLAN COMPLETED SUCCESSFULLY
============================================================

All features have been merged to <target_branch>!

Summary:
  Total features: 4
  Total batches: 2
  Target branch: <target_branch>

Cleanup completed:
  [X] Planning files removed
  [X] Worktrees cleaned up
  [X] Feature branches deleted

============================================================

Your code is now on the <target_branch> branch with all features.

Next steps:
  - Review merged code: git log --oneline -10
  - Run tests to verify integration
  - Push to remote: git push origin <target_branch>

============================================================
```

## Keep Files Option

If user selected "Keep files":

```
============================================================
MEGA PLAN COMPLETED (Files Kept)
============================================================

All features have been merged to <target_branch>!

Planning files kept for reference:
  - mega-plan.json
  - mega-findings.md
  - .mega-status.json

Note: These files are in .gitignore and won't be committed.

To cleanup later:
  rm mega-plan.json mega-findings.md .mega-status.json

============================================================
```

## Error Handling

### Worktree Removal Fails

```
Warning: Could not remove .worktree/<name>
Manual cleanup:
  rm -rf .worktree/<name>
  git worktree prune
```

### Branch Deletion Fails

```
Warning: Could not delete branch mega-<name>
This branch may have unmerged changes.
Force delete: git branch -D mega-<name>
```

### Not on Target Branch

```
Warning: You are not on the target branch (<target_branch>).
Current branch: <current>

The features were merged to <target_branch>.
Switch to it: git checkout <target_branch>
```

## Files That Should NOT Be Committed

The following files are in `.gitignore` and should never be committed:

```
.worktree/              # Git worktree directories
mega-plan.json          # Mega plan definition
mega-findings.md        # Shared findings
.mega-status.json       # Execution status
.planning-config.json   # Per-worktree config
prd.json                # PRD files
findings.md             # Per-feature findings
progress.txt            # Progress tracking
.agent-status.json      # Agent status
```

These are all planning/execution artifacts that are temporary and should not be part of the codebase.
