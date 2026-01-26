---
description: "Complete a worktree task. Verifies all phases are complete, ensures changes are committed, deletes planning files, merges to target branch, and removes worktree. Must be run from inside the worktree directory. Usage: /planning-with-files:complete [target-branch]"
---

# Planning with Files - Complete Worktree Task

You are now completing a worktree task. This will:
1. Verify all phases are complete
2. **CRITICAL: Ensure all changes are committed**
3. Delete planning files from the worktree
4. Navigate to the root directory
5. Merge the task branch to target branch
6. Remove the worktree
7. Delete the task branch

**IMPORTANT**: This command must be run from **inside the worktree directory**.

## Step 1: Verify You're in a Worktree

First, check if we're in a worktree directory:

```bash
if [ ! -f ".planning-config.json" ]; then
    echo "ERROR: .planning-config.json not found"
    echo "Are you in a worktree directory?"
    echo "This command must be run from inside the worktree."
    exit 1
fi
```

## Step 2: Read Configuration

Read the planning configuration from `.planning-config.json`:

```bash
config=$(cat .planning-config.json)
MODE=$(echo "$config" | jq -r '.mode // empty')
TASK_NAME=$(echo "$config" | jq -r '.task_name')
TASK_BRANCH=$(echo "$config" | jq -r '.task_branch')
TARGET_BRANCH=$(echo "$config" | jq -r '.target_branch')
WORKTREE_DIR=$(echo "$config" | jq -r '.worktree_dir')
ROOT_DIR=$(echo "$config" | jq -r '.root_dir')
ORIGINAL_BRANCH=$(echo "$config" | jq -r '.original_branch')
```

## Step 3: Parse Override Target (Optional)

If user provided a target branch argument, use that instead:

```bash
OVERRIDE_TARGET="{{args|first arg or empty}}"
TARGET_FINAL="${OVERRIDE_TARGET:-$TARGET_BRANCH}"
```

## Step 4: Verify Task Completion

Check if all phases in task_plan.md are complete:

```bash
SCRIPT_DIR="${CLAUDE_PLUGIN_ROOT:-$HOME/.claude/plugins/planning-with-files}/scripts"
if [ -f "$SCRIPT_DIR/check-complete.sh" ]; then
    bash "$SCRIPT_DIR/check-complete.sh"
fi
```

If the check fails (exit code 1), ask the user:

```
WARNING: Not all phases are marked complete

[Show the output from check-complete.sh]

Continue anyway? [y/N]:
```

Wait for user confirmation before proceeding.

## Step 5: CRITICAL - Check for Uncommitted Changes

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
            COMMIT_MSG="Complete task: $TASK_NAME

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

## Step 6: Show What Will Happen

Before making any changes, show the user what will be done:

```
=== Worktree Completion Summary ===

Task: $TASK_NAME
Branch: $TASK_BRANCH
Target: $TARGET_FINAL

Changes to merge:
{Show git log --oneline -3 or git diff --stat HEAD~1}

This will:
  1. Delete planning files from worktree
  2. Navigate to root directory
  3. Merge $TASK_BRANCH into $TARGET_FINAL
  4. Delete this worktree
  5. Delete the task branch

Proceed? [Y/n]:
```

Wait for user confirmation.

## Step 7: Delete Planning Files from Worktree

```bash
echo "Deleting planning files..."
for file in task_plan.md findings.md progress.md; do
    if [ -f "$file" ]; then
        rm "$file"
        echo "  Deleted: $file"
    fi
done
if [ -f ".planning-config.json" ]; then
    rm ".planning-config.json"
    echo "  Deleted: .planning-config.json"
fi
echo "✓ Planning files deleted"
```

## Step 8: Navigate to Root Directory

```bash
echo "Navigating to root directory..."
cd "$ROOT_DIR"
echo "  Now in: $(pwd)"
```

## Step 9: Switch to Target Branch

```bash
echo "Switching to target branch: $TARGET_FINAL"

# Fetch latest if remote exists
if git ls-remote --exit-code origin "$TARGET_FINAL" > /dev/null 2>&1; then
    git fetch origin "$TARGET_FINAL"
fi

git checkout "$TARGET_FINAL" || git checkout -b "$TARGET_FINAL" "origin/$TARGET_FINAL"
echo "  Checked out: $TARGET_FINAL"
```

## Step 10: Merge Task Branch

```bash
echo "Merging $TASK_BRANCH into $TARGET_FINAL..."

if git merge --no-ff -m "Merge task branch: $TASK_NAME" "$TASK_BRANCH"; then
    echo "✓ Merge successful!"
else
    echo ""
    echo "=== MERGE CONFLICT DETECTED ==="
    echo ""
    echo "Merge conflicts need to be resolved manually."
    echo ""
    echo "After resolving conflicts:"
    echo "  1. Run: git add ."
    echo "  2. Run: git commit"
    echo "  3. Run: git worktree remove $WORKTREE_DIR"
    echo "  4. Run: git branch -d $TASK_BRANCH"
    echo ""
    echo "Or abort with: git merge --abort"
    exit 1
fi
```

## Step 11: Remove Worktree

```bash
echo "Removing worktree: $WORKTREE_DIR"

if git worktree remove "$WORKTREE_DIR" 2>/dev/null; then
    echo "✓ Worktree removed"
else
    # Fallback to manual removal
    rm -rf "$WORKTREE_DIR"
    echo "✓ Worktree directory removed (manually)"
fi
```

## Step 12: Delete Task Branch

```bash
echo "Deleting task branch: $TASK_BRANCH"

if git branch -d "$TASK_BRANCH" 2>/dev/null; then
    echo "✓ Task branch deleted"
else
    echo "Warning: Could not delete branch $TASK_BRANCH"
    echo "  You may need to delete it manually with: git branch -D $TASK_BRANCH"
fi
```

## Step 13: Final Summary

```
=== Task Completed Successfully ===

Task: $TASK_NAME
Branch: $TASK_BRANCH merged into $TARGET_FINAL

✓ All changes committed
✓ Planning files deleted
✓ Worktree removed
✓ Task branch deleted

Current branch: $(git branch --show-current)
Current directory: $(pwd)

=== Active Worktrees ===
[Show git worktree list]

Next:
  - Review changes: git log --oneline -5
  - Push the merge: git push
  - Continue with your next task
```

## Safety Features

- **Forces commit**: Won't proceed if there are uncommitted changes
- **Auto-commit option**: Can automatically commit with generated message
- **Stash option**: Can stash changes if needed
- **Manual cancel**: Always allows manual intervention
- **Conflict handling**: Provides clear instructions for merge conflicts
- **Verification**: Shows exactly what will be merged before proceeding
