---
description: "Complete Hybrid Ralph task in worktree, verify all stories complete, commit changes, merge to target branch, and cleanup worktree directory. Ensures all changes are committed before cleanup."
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

## Step 4: CRITICAL - Check for Uncommitted Changes

```bash
# Check if there are uncommitted changes
if ! git diff-index --quiet HEAD -- 2>/dev/null; then
    echo "=========================================="
    echo "CRITICAL: Uncommitted changes detected!"
    echo "=========================================="
    echo ""
    echo "These changes will be LOST if not committed:"
    git status --short
    echo ""
    echo "You MUST commit these changes before completing the task."
    echo ""
    echo "Options:"
    echo "  1) Auto-commit changes with generated message"
    echo "  2) Stash changes (for later)"
    echo "  3) Cancel and handle manually"
    echo ""
    read -p "Choose [1/2/3]: " choice

    case "$choice" in
        1)
            echo ""
            echo "Committing changes..."
            COMMIT_MSG="Complete hybrid task: $TASK_NAME

Stories completed: $COMPLETE_STORIES/$TOTAL_STORIES
Branch: $TASK_BRANCH
Target: $TARGET_FINAL

Co-Authored-By: Claude <noreply@anthropic.com>"

            git add -A
            git commit -m "$COMMIT_MSG"
            echo "✓ Changes committed"
            ;;
        2)
            echo ""
            echo "Stashing changes..."
            git stash push -m "WIP for $TASK_NAME"
            echo "✓ Changes stashed"
            echo ""
            echo "WARNING: Stashed changes are NOT included in the merge."
            echo "You can apply them later with: git stash pop"
            echo ""
            read -p "Continue anyway? [y/N]: " stash_confirm
            if [[ ! "$stash_confirm" =~ ^[Yy]$ ]]; then
                echo "Aborted."
                exit 1
            fi
            ;;
        3)
            echo ""
            echo "Cancelled. Please commit your changes manually:"
            echo "  git add -A"
            echo "  git commit -m 'Your message here'"
            echo ""
            echo "Then run this command again."
            exit 0
            ;;
        *)
            echo "Invalid choice. Aborted."
            exit 1
            ;;
    esac
else
    echo "✓ No uncommitted changes"
fi
```

## Step 5: Show Completion Summary

```
=== COMPLETION SUMMARY ===

Task: $TASK_NAME
Branch: $TASK_BRANCH
Target: $TARGET_FINAL

Stories: $TOTAL_STORIES total
  All complete ✓

Changes to merge:
{Show git diff --stat HEAD~1 HEAD or git log -1 --oneline}

Ready to merge to $TARGET_FINAL...
```

Wait for user confirmation.

## Step 6: Delete Planning Files

```bash
echo "Deleting planning files..."
rm -f prd.json findings.md progress.txt .planning-config.json
rm -rf .agent-outputs
echo "Planning files deleted"
```

## Step 7: Navigate to Root Directory

```bash
echo "Navigating to root directory..."
cd "$ROOT_DIR"
```

## Step 8: Switch to Target Branch

```bash
echo "Switching to target branch: $TARGET_FINAL"

# Fetch latest if remote exists
if git ls-remote --exit-code origin "$TARGET_FINAL" > /dev/null 2>&1; then
    git fetch origin "$TARGET_FINAL"
fi

git checkout "$TARGET_FINAL" || git checkout -b "$TARGET_FINAL"
```

## Step 9: Merge Task Branch

```bash
echo "Merging $TASK_BRANCH into $TARGET_FINAL..."

if git merge --no-ff -m "Merge hybrid task: $TASK_NAME

Completed $COMPLETE_STORIES stories
Branch: $TASK_BRANCH" "$TASK_BRANCH"; then
    echo "✓ Merge successful!"
else
    echo ""
    echo "=== MERGE CONFLICT DETECTED ==="
    echo ""
    echo "Please resolve conflicts manually, then:"
    echo "  1. git add ."
    echo "  2. git commit"
    echo "  3. git worktree remove $WORKTREE_DIR"
    echo "  4. git branch -d $TASK_BRANCH"
    echo ""
    echo "Or abort with: git merge --abort"
    exit 1
fi
```

## Step 10: Remove Worktree

```bash
echo "Removing worktree: $WORKTREE_DIR"
git worktree remove "$WORKTREE_DIR" 2>/dev/null || rm -rf "$WORKTREE_DIR"
echo "✓ Worktree removed"
```

## Step 11: Delete Task Branch

```bash
echo "Deleting task branch: $TASK_BRANCH"
git branch -d "$TASK_BRANCH" 2>/dev/null || echo "Warning: Could not delete branch"
echo "✓ Task branch deleted"
```

## Step 12: Show Final Summary

```
=== TASK COMPLETE ===

✓ Task: $TASK_NAME
✓ Stories: $COMPLETE_STORIES/$TOTAL_STORIES completed
✓ All changes committed
✓ Merged to: $TARGET_FINAL
✓ Worktree removed
✓ Task branch deleted

Current location: $ROOT_DIR
Current branch: $(git branch --show-current)

Next:
  - Review changes: git log --oneline -5
  - Push to remote: git push
  - Start a new task: /planning-with-files:hybrid-worktree
```

## Safety Features

- **Forces commit**: Won't proceed if there are uncommitted changes
- **Auto-commit option**: Can automatically commit with generated message
- **Stash option**: Can stash changes if needed
- **Manual cancel**: Always allows manual intervention
- **Conflict handling**: Provides clear instructions for merge conflicts
