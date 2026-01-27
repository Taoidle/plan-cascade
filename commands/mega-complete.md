---
description: "Complete the mega-plan by merging all features in dependency order and cleaning up worktrees. Usage: /planning-with-files:mega-complete [target-branch]"
---

# Complete Mega Plan

Complete the mega-plan by merging all features in dependency order and cleaning up.

## Arguments

- `target-branch` (optional): Target branch to merge into. Uses mega-plan's target_branch if not specified.

## Step 1: Verify Mega Plan Exists

```bash
if [ ! -f "mega-plan.json" ]; then
    echo "No mega-plan.json found."
    echo "Nothing to complete."
    exit 0
fi
```

## Step 2: Parse Arguments

```bash
TARGET_BRANCH="$ARGUMENTS"
if [ -z "$TARGET_BRANCH" ]; then
    # Read from mega-plan.json
    TARGET_BRANCH=$(python3 -c "import json; print(json.load(open('mega-plan.json'))['target_branch'])")
fi
```

## Step 3: Verify All Features Complete

Check each feature's status:

```bash
# Read mega-plan and verify all features are complete
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
  /planning-with-files:hybrid-status

Then run /planning-with-files:mega-complete again.
```

Exit without changes.

## Step 4: Show Merge Plan

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

Cleanup will include:
  - Remove .worktree/* directories
  - Delete mega-* branches
  - Remove mega-plan.json
  - Remove .mega-status.json
  - Remove mega-findings.md

============================================================
```

## Step 5: Confirm Action

Use AskUserQuestion:

**Proceed with merge and cleanup?**

Options:
1. **Yes, merge and cleanup** - Full completion
2. **Merge only** - Keep worktrees and files
3. **Cancel** - Don't do anything

## Step 6: Checkout Target Branch

```bash
git checkout "$TARGET_BRANCH"
git pull origin "$TARGET_BRANCH" 2>/dev/null || true
```

## Step 7: Merge Features in Order

For each feature in dependency order:

```bash
FEATURE_NAME="<name>"
BRANCH_NAME="mega-$FEATURE_NAME"

git merge "$BRANCH_NAME" --no-ff -m "Merge feature: <title>

Mega-plan feature: <feature-id>
Stories completed: <count>"
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

### Handle Merge Conflicts

If a conflict occurs:

```
============================================================
MERGE CONFLICT
============================================================

Conflict while merging feature-002: Product Catalog

Conflicting files:
  - src/api/products.ts
  - src/models/product.ts

To resolve:
  1. Edit conflicting files
  2. git add <resolved files>
  3. git commit
  4. Re-run /planning-with-files:mega-complete

Or abort: git merge --abort
```

Exit and let user resolve.

## Step 8: Cleanup Worktrees

If user selected "merge and cleanup":

```bash
# For each feature
git worktree remove ".worktree/$FEATURE_NAME" --force
```

Progress:

```
Cleaning up worktrees...

[OK] Removed .worktree/feature-auth
[OK] Removed .worktree/feature-products
[OK] Removed .worktree/feature-cart
[OK] Removed .worktree/feature-orders
[OK] Removed .worktree/ directory
```

## Step 9: Delete Feature Branches

```bash
# For each feature
git branch -d "mega-$FEATURE_NAME"
```

Progress:

```
Deleting feature branches...

[OK] Deleted mega-feature-auth
[OK] Deleted mega-feature-products
[OK] Deleted mega-feature-cart
[OK] Deleted mega-feature-orders
```

## Step 10: Cleanup Mega Files

```bash
rm -f mega-plan.json
rm -f .mega-status.json
rm -f mega-findings.md
```

## Step 11: Prune Git

```bash
git worktree prune
```

## Step 12: Show Completion Summary

```
============================================================
MEGA PLAN COMPLETED
============================================================

All features have been merged into <target_branch>!

Summary:
  Features merged: 4
  Target branch: <target_branch>

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
  - Review merged code: git log --oneline -10
  - Run tests
  - Push to remote: git push origin <target_branch>

============================================================
```

## Partial Completion (Merge Only)

If user selected "Merge only":

```
============================================================
MERGE COMPLETED (Cleanup Skipped)
============================================================

All features merged into <target_branch>.

Remaining cleanup (when ready):
  # Remove worktrees
  git worktree remove .worktree/<name> --force

  # Delete branches
  git branch -d mega-<name>

  # Remove files
  rm mega-plan.json .mega-status.json mega-findings.md

Or run /planning-with-files:mega-complete again for full cleanup.
============================================================
```

## Error Handling

### Feature Not Complete

Show which features are incomplete and their status.

### Worktree Removal Fails

```
Warning: Could not remove .worktree/<name>
Manual cleanup: rm -rf .worktree/<name> && git worktree prune
```

### Branch Deletion Fails

```
Warning: Could not delete branch mega-<name>
Force delete: git branch -D mega-<name>
```
