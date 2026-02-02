---
name: mega:complete
description: Complete the mega-plan - merge all features and clean up
arguments:
  - name: target-branch
    description: Target branch to merge into (optional, uses plan's target_branch if not specified)
    required: false
---

# /mega:complete

Complete the mega-plan by merging all features in dependency order and cleaning up.

## Your Task

### Step 1: Check for Mega Plan

**Use Read tool (NOT Bash) to check if mega-plan.json exists:**

```
Read("mega-plan.json")
```

If the file doesn't exist (Read returns error):
```
No mega-plan.json found.
Nothing to complete.
```

### Step 2: Verify All Features Complete

Check each feature's status:

```bash
uv run python "${CLAUDE_PLUGIN_ROOT}/skills/mega-plan/core/merge_coordinator.py" verify
```

If any features are not complete:

```
============================================================
CANNOT COMPLETE - FEATURES INCOMPLETE
============================================================

The following features are not yet complete:

  [>] feature-002: Product Catalog
      Status: in_progress
      Stories: 3/5 complete
      Location: .worktree/feature-products/

  [ ] feature-003: Shopping Cart
      Status: pending
      Blocked by: feature-002

============================================================

Complete the remaining features first:
  cd .worktree/feature-products
  /status

Then run /mega:complete again.
```

Exit without making changes.

### Step 3: Determine Target Branch

Check if target branch was specified in arguments:
- If `$ARGUMENTS` contains a branch name, use that
- Otherwise, use the target_branch from mega-plan.json

```bash
TARGET_BRANCH="${ARGUMENTS:-$(cat mega-plan.json | uv run python -c "import json,sys; print(json.load(sys.stdin)['target_branch'])")}"
```

### Step 4: Show Merge Plan

Display what will happen:

```
============================================================
MEGA PLAN COMPLETION
============================================================

All features are complete!

Target Branch: <target_branch>

Merge Order (dependency-based):
  1. feature-001: User Authentication
  2. feature-002: Product Catalog
  3. feature-003: Shopping Cart (after 1, 2)
  4. feature-004: Order Processing (after 3)

Cleanup:
  - Remove .worktree/feature-* directories
  - Delete mega-feature-* branches
  - Remove mega-plan.json
  - Remove .mega-status.json
  - Remove mega-findings.md

============================================================

Proceed with merge and cleanup? (This cannot be undone)
```

Use AskUserQuestion to confirm:

**Options:**
1. **Yes, merge and cleanup** - Proceed with full completion
2. **Merge only** - Merge but keep worktrees and files
3. **Cancel** - Don't do anything

### Step 5: Ensure on Target Branch

```bash
git checkout <target_branch>
git pull origin <target_branch>  # Optional: sync with remote
```

### Step 6: Merge Features in Order

For each feature in dependency order:

```bash
# Merge feature branch
git merge mega-<feature-name> --no-ff -m "Merge feature: <title>

<description summary>

Mega-plan feature: <feature-id>
Stories completed: N"
```

Show progress:

```
Merging features...

[OK] feature-001: User Authentication
     Merged mega-feature-auth into <target>

[OK] feature-002: Product Catalog
     Merged mega-feature-products into <target>

[OK] feature-003: Shopping Cart
     Merged mega-feature-cart into <target>

[OK] feature-004: Order Processing
     Merged mega-feature-orders into <target>

All features merged successfully!
```

### Step 7: Handle Merge Conflicts

If a merge conflict occurs:

```
============================================================
MERGE CONFLICT
============================================================

Conflict while merging feature-002: Product Catalog

Conflicting files:
  - src/api/products.ts
  - src/models/product.ts

Options:
1. Resolve conflicts manually, then run /mega:complete again
2. Abort this merge: git merge --abort

To resolve:
  git status
  # Edit conflicting files
  git add <resolved files>
  git commit
  /mega:complete
```

Exit and let user resolve.

### Step 8: Cleanup Worktrees

Remove each worktree:

```bash
# For each feature
git worktree remove .worktree/<feature-name> --force
```

Show progress:

```
Cleaning up worktrees...

[OK] Removed .worktree/feature-auth
[OK] Removed .worktree/feature-products
[OK] Removed .worktree/feature-cart
[OK] Removed .worktree/feature-orders
[OK] Removed .worktree/ directory
```

### Step 9: Delete Feature Branches

```bash
# For each feature
git branch -d mega-<feature-name>
```

Show progress:

```
Deleting feature branches...

[OK] Deleted mega-feature-auth
[OK] Deleted mega-feature-products
[OK] Deleted mega-feature-cart
[OK] Deleted mega-feature-orders
```

### Step 10: Cleanup Mega Files

Remove mega-plan related files:

```bash
rm -f mega-plan.json
rm -f .mega-status.json
rm -f mega-findings.md
```

### Step 11: Prune Git

Clean up any stale references:

```bash
git worktree prune
```

### Step 12: Show Completion Summary

```
============================================================
MEGA PLAN COMPLETED
============================================================

All features have been merged into <target_branch>!

Summary:
  Features merged: 4
  Target branch: main

Merged Features:
  1. feature-001: User Authentication
  2. feature-002: Product Catalog
  3. feature-003: Shopping Cart
  4. feature-004: Order Processing

Cleanup completed:
  [X] Worktrees removed
  [X] Feature branches deleted
  [X] Mega-plan files removed

============================================================

Your code is now on the <target_branch> branch.

Next steps:
  - Review the merged code: git log --oneline -10
  - Run tests: <your test command>
  - Push to remote: git push origin <target_branch>

============================================================
```

## Error Handling

### Feature Not Complete

```
Error: Cannot complete - feature-002 is not complete

To check status:
  /mega:status

To complete the feature:
  cd .worktree/feature-products
  /status
```

### Worktree Removal Fails

```
Warning: Could not remove .worktree/feature-auth
Reason: Directory not empty or in use

Manual cleanup:
  rm -rf .worktree/feature-auth
  git worktree prune
```

### Branch Deletion Fails

```
Warning: Could not delete branch mega-feature-auth
Reason: Not fully merged

Force delete (if needed):
  git branch -D mega-feature-auth
```

## Partial Completion (Merge Only)

If user selected "Merge only":

```
============================================================
MERGE COMPLETED (Cleanup Skipped)
============================================================

All features merged into <target_branch>.

Remaining cleanup (when ready):
  git worktree remove .worktree/<name> --force
  git branch -d mega-<name>
  rm mega-plan.json .mega-status.json mega-findings.md

Or run /mega:complete again to cleanup.
============================================================
```
