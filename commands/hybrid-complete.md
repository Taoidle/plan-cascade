---
description: "Complete Hybrid Ralph task in worktree, verify all stories complete, merge to target branch, and cleanup worktree directory. Validates PRD completion, removes planning files, and performs git merge automatically."
---

# Hybrid Ralph - Complete Worktree Task

You are completing a Hybrid Ralph task in a worktree and merging it to the target branch.

## Step 1: Verify Worktree Mode

Check if in worktree mode:

```bash
if [ ! -f ".planning-config.json" ]; then
    echo "ERROR: Not in worktree mode. .planning-config.json not found."
    echo "This command must be run from inside a worktree directory."
    echo "Use /planning-with-files:complete for standard mode instead."
    exit 1
fi

MODE=$(jq -r '.mode' .planning-config.json 2>/dev/null || echo "")
if [ "$MODE" != "hybrid" ]; then
    echo "ERROR: Not in hybrid worktree mode."
    exit 1
fi
```

## Step 2: Read Planning Config

```bash
TASK_NAME=$(jq -r '.task_name' .planning-config.json)
TASK_BRANCH=$(jq -r '.task_branch' .planning-config.json)
TARGET_BRANCH=$(jq -r '.target_branch' .planning-config.json)
WORKTREE_DIR=$(jq -r '.worktree_dir' .planning-config.json)
ROOT_DIR=$(jq -r '.root_dir' .planning-config.json)

# Allow override from args
OVERRIDE_TARGET="{{args|first arg or empty}}"
TARGET_FINAL="${OVERRIDE_TARGET:-$TARGET_BRANCH}"
```

## Step 3: Verify All Stories Complete

Check if all stories in `prd.json` are marked complete in `progress.txt`:

```bash
# Count total stories in PRD
TOTAL_STORIES=$(jq '.stories | length' prd.json)

# Count completed stories in progress.txt
COMPLETE_STORIES=$(grep -c "\[COMPLETE\]" progress.txt 2>/dev/null || echo "0")

if [ "$COMPLETE_STORIES" -lt "$TOTAL_STORIES" ]; then
    echo "WARNING: Not all stories are complete"
    echo "Completed: $COMPLETE_STORIES / $TOTAL_STORIES"
    echo ""
    echo "Continue anyway? [y/N]"
    read -r response
    if [[ ! "$response" =~ ^[Yy]$ ]]; then
        echo "Aborted. Complete remaining stories first."
        exit 1
    fi
fi
```

## Step 4: Show Completion Summary

```
=== COMPLETION SUMMARY ===

Task: $TASK_NAME
Branch: $TASK_BRANCH
Target: $TARGET_FINAL

Stories: $TOTAL_STORIES total
  All complete ✓

Changes to merge:
{Show git diff --stat}

Ready to merge to $TARGET_FINAL...
```

Wait for user confirmation.

## Step 5: Delete Planning Files

```bash
echo "Deleting planning files..."
rm -f prd.json findings.md progress.txt .planning-config.json
rm -rf .agent-outputs
echo "Planning files deleted"
```

## Step 6: Navigate to Root Directory

```bash
echo "Navigating to root directory..."
cd "$ROOT_DIR"
```

## Step 7: Switch to Target Branch

```bash
echo "Switching to target branch: $TARGET_FINAL"
git checkout "$TARGET_FINAL" || git checkout -b "$TARGET_FINAL"
```

## Step 8: Merge Task Branch

```bash
echo "Merging $TASK_BRANCH into $TARGET_FINAL..."

if git merge --no-ff -m "Merge hybrid task: $TASK_NAME" "$TASK_BRANCH"; then
    echo "Merge successful!"
else
    echo ""
    echo "=== MERGE CONFLICT DETECTED ==="
    echo ""
    echo "Please resolve conflicts manually, then:"
    echo "  1. git add ."
    echo "  2. git commit"
    echo "  3. git worktree remove $WORKTREE_DIR"
    echo "  4. git branch -d $TASK_BRANCH"
    exit 1
fi
```

## Step 9: Remove Worktree

```bash
echo "Removing worktree: $WORKTREE_DIR"
git worktree remove "$WORKTREE_DIR" 2>/dev/null || rm -rf "$WORKTREE_DIR"
```

## Step 10: Delete Task Branch

```bash
echo "Deleting task branch: $TASK_BRANCH"
git branch -d "$TASK_BRANCH" 2>/dev/null || echo "Warning: Could not delete branch"
```

## Step 11: Show Final Summary

```
=== TASK COMPLETE ===

✓ Task: $TASK_NAME
✓ Merged to: $TARGET_FINAL
✓ Worktree removed
✓ Task branch deleted

Current location: $ROOT_DIR
Current branch: $(git branch --show-current)

Next:
  - Push changes: git push
  - Start a new task with: /planning-with-files:hybrid-worktree
```

## Notes

- This command MUST be run from inside the worktree directory
- All stories should be complete before running
- Merge conflicts must be resolved manually
- Main directory is now on the target branch with merged changes
