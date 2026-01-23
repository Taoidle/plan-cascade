---
description: "Complete a worktree task. Verifies all phases are complete, deletes planning files, merges the task branch to target branch, and cleans up. Usage: /planning-with-files:complete [target-branch]"
---

# Planning with Files - Complete Task

You are now completing a worktree task. This will:
1. Verify all phases are complete
2. Delete the planning files
3. Merge the task branch to the target branch
4. Clean up the task branch

## Step 1: Read Configuration

First, read the planning configuration:

```bash
cat .planning-config.json 2>/dev/null || { echo "ERROR: No planning configuration found. Are you in a worktree session?"; exit 1; }
```

Parse the configuration to get:
- `mode`: Should be "worktree"
- `task_branch`: The name of the task branch
- `target_branch`: The target branch to merge into
- `planning_files`: List of planning files to delete

## Step 2: Parse Override Target (Optional)

If user provided a target branch argument, use that instead:

```bash
# If {{args}} is provided, use it as the override target
OVERRIDE_TARGET="{{args|first arg or empty}}"
```

If `OVERRIDE_TARGET` is provided and not empty, use it instead of `target_branch` from config.

## Step 3: Verify Task Completion

Check if all phases in task_plan.md are complete:

```bash
SCRIPT_DIR="${CLAUDE_PLUGIN_ROOT:-$HOME/.claude/plugins/planning-with-files}/scripts"
if command -v pwsh &> /dev/null && [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" || "$OS" == "Windows_NT" ]]; then
    pwsh -ExecutionPolicy Bypass -File "$SCRIPT_DIR/check-complete.ps1" 2>/dev/null || powershell -ExecutionPolicy Bypass -File "$SCRIPT_DIR/check-complete.ps1" 2>/dev/null || bash "$SCRIPT_DIR/check-complete.sh"
else
    bash "$SCRIPT_DIR/check-complete.sh"
fi
```

If the check fails (exit code 1), ask the user:

```
=== Task Completion Check Failed ===

Not all phases are marked as complete. Current status:
[Show the output from check-complete.sh]

Do you want to:
1. Continue anyway (incomplete phases)
2. Cancel and complete phases first

Please confirm or cancel.
```

Wait for user confirmation before proceeding.

## Step 4: Show What Will Happen

Before making any changes, show the user what will be done:

```
=== Worktree Completion Summary ===

Task Branch:    $TASK_BRANCH
Target Branch:  $TARGET_BRANCH (or override: $OVERRIDE_TARGET if provided)

Files to Delete:
  - task_plan.md
  - findings.md
  - progress.md
  - .planning-config.json

Actions:
  1. Delete planning files and config
  2. Stage and commit any uncommitted changes
  3. Switch to target branch
  4. Merge task branch into target
  5. Delete task branch
  6. Clean up worktree (if applicable)

Do you want to proceed? (yes/no)
```

Wait for user confirmation.

## Step 5: Check for Uncommitted Changes

```bash
# Check if there are uncommitted changes
if ! git diff-index --quiet HEAD --; then
    echo "There are uncommitted changes."
    echo "Please commit or stash them before completing the task."
    echo ""
    echo "Uncommitted files:"
    git status --short
    exit 1
fi
```

If there are uncommitted changes, ask the user if they want to:
1. Commit the changes now
2. Stash the changes
3. Cancel and handle manually

## Step 6: Delete Planning Files

```bash
echo "Deleting planning files..."

# Delete the three planning files
for file in task_plan.md findings.md progress.md; do
    if [ -f "$file" ]; then
        rm "$file"
        echo "  Deleted: $file"
    fi
done

# Delete the config file
if [ -f .planning-config.json ]; then
    rm .planning-config.json
    echo "  Deleted: .planning-config.json"
fi

echo "Planning files deleted."
```

## Step 7: Switch to Target Branch

```bash
TARGET_FINAL="${OVERRIDE_TARGET:-$TARGET_BRANCH}"

echo "Switching to target branch: $TARGET_FINAL"

# Fetch latest if remote exists
if git ls-remote --exit-code origin "$TARGET_FINAL" > /dev/null 2>&1; then
    git fetch origin "$TARGET_FINAL"
    git checkout "$TARGET_FINAL" || git checkout -b "$TARGET_FINAL" "origin/$TARGET_FINAL"
else
    git checkout "$TARGET_FINAL" || git checkout -b "$TARGET_FINAL"
fi

echo "Now on branch: $TARGET_FINAL"
```

## Step 8: Merge Task Branch

```bash
echo "Merging $TASK_BRANCH into $TARGET_FINAL..."

# Attempt merge
if git merge --no-ff -m "Merge task branch: $TASK_BRANCH" "$TASK_BRANCH"; then
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
    echo "  3. Run: git branch -D $TASK_BRANCH"
    echo ""
    echo "Or abort with: git merge --abort"
    exit 1
fi
```

## Step 9: Delete Task Branch

```bash
echo "Cleaning up task branch: $TASK_BRANCH"

# Delete the task branch
git branch -d "$TASK_BRANCH" || echo "Warning: Could not delete branch $TASK_BRANCH (may already be deleted or not merged)"

echo "Task branch deleted."
```

## Step 10: Cleanup Worktree (if applicable)

If a worktree was created (check if `.worktree` directory exists and has the task subdirectory):

```bash
WORKTREE_DIR=".worktree/$(basename $TASK_BRANCH)"

if [ -d "$WORKTREE_DIR" ]; then
    echo "Cleaning up worktree: $WORKTREE_DIR"

    # Remove worktree
    git worktree remove "$WORKTREE_DIR" 2>/dev/null || rm -rf "$WORKTREE_DIR"

    echo "Worktree cleaned up."
fi
```

## Step 11: Final Summary

```
=== Task Completed Successfully ===

Task branch $TASK_BRANCH has been merged into $TARGET_FINAL.

Planning files have been deleted.

You are now on branch: $(git branch --show-current)

Next:
  - Push the merge if needed: git push
  - Continue with your next task
```

## Step 12: Push Reminder (Optional)

Ask if the user wants to push the merge:

```
Would you like to push the merge to the remote repository? (yes/no)

If yes, run: git push origin $TARGET_FINAL
```

---

**Important:** This command completes the worktree workflow. After execution, the task branch is merged and cleaned up, and the user is back on their target branch.
