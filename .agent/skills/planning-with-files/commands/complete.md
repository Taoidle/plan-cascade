---
description: "Complete a worktree task. Verifies all phases are complete, deletes planning files from the worktree, navigates to root directory, merges the task branch to target branch, and removes the worktree. Must be run from inside the worktree directory. Usage: /planning-with-files:complete [target-branch]"
---

# Planning with Files - Complete Worktree Task

You are now completing a worktree task. This will:
1. Verify all phases are complete
2. Delete planning files from the worktree
3. Navigate to the root directory
4. Merge the task branch to the target branch
5. Remove the worktree
6. Delete the task branch

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
MODE=$(echo "$config" | grep '"mode"' | sed 's/.*"mode"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
TASK_NAME=$(echo "$config" | grep '"task_name"' | sed 's/.*"task_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
TASK_BRANCH=$(echo "$config" | grep '"task_branch"' | sed 's/.*"task_branch"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
TARGET_BRANCH=$(echo "$config" | grep '"target_branch"' | sed 's/.*"target_branch"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
WORKTREE_DIR=$(echo "$config" | grep '"worktree_dir"' | sed 's/.*"worktree_dir"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
ROOT_DIR=$(echo "$config" | grep '"root_dir"' | sed 's/.*"root_dir"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
ORIGINAL_BRANCH=$(echo "$config" | grep '"original_branch"' | sed 's/.*"original_branch"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
```

Parse the configuration to get all necessary information.

## Step 3: Parse Override Target (Optional)

If user provided a target branch argument, use that instead:

```bash
# If {{args}} is provided, use it as the override target
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

## Step 5: Check for Uncommitted Changes

```bash
if ! git diff-index --quiet HEAD -- 2>/dev/null; then
    echo "There are uncommitted changes:"
    git status --short
    echo ""
    echo "Options:"
    echo "  1) Commit changes now"
    echo "  2) Stash changes"
    echo "  3) Cancel and handle manually"
    echo ""
    read -p "Choose [1/2/3]: " choice
    # Handle the choice...
fi
```

## Step 6: Show What Will Happen

Before making any changes, show the user what will be done:

```
=== Worktree Completion Summary ===

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
```

## Step 8: Navigate to Root Directory

```bash
echo "Navigating to root directory..."
cd "$ROOT_DIR"
echo "  Now in: $(pwd)"
```

Important: We switch to the root directory to perform the merge operation.

## Step 9: Switch to Target Branch

```bash
TARGET_FINAL="${OVERRIDE_TARGET:-$TARGET_BRANCH}"

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
    echo "Merge successful!"
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
    echo "Worktree removed"
else
    # Fallback to manual removal
    rm -rf "$WORKTREE_DIR"
    echo "Worktree directory removed (manually)"
fi
```

## Step 12: Delete Task Branch

```bash
echo "Deleting task branch: $TASK_BRANCH"

if git branch -d "$TASK_BRANCH" 2>/dev/null; then
    echo "Task branch deleted"
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

Planning files have been deleted.
Worktree has been removed.

Current branch: $(git branch --show-current)
Current directory: $(pwd)

=== Active Worktrees ===
[Show git worktree list]

Next:
  - Push the merge if needed: git push
  - Continue with your next task
```

## Example Workflow

```bash
# Start a task
/planning-with-files:worktree fix-auth-bug

# Navigate to worktree (as instructed)
cd .worktree/fix-auth-bug

# ... work on the task ...
# ... update task_plan.md as you progress ...

# When done, complete the task (from inside worktree)
/planning-with-files:complete

# You'll be returned to the root directory automatically
# The worktree is cleaned up, branch is merged
```

## Important Notes

1. **Run from worktree**: This command must be run from inside the worktree directory (where `.planning-config.json` exists)

2. **Automatic navigation**: The command automatically navigates back to the root directory for the merge

3. **Cleanup**: Both the worktree directory and the task branch are cleaned up

4. **Merge conflicts**: If there are merge conflicts, you'll need to resolve them manually and then clean up

5. **Active worktrees**: After completion, the command shows all remaining active worktrees so you can see what other tasks are in progress
