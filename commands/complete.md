---
description: "Complete a worktree task. Verifies all phases are complete, commits code changes (excluding planning files), merges to target branch, and removes worktree. Can be run from any directory. Usage: /plan-cascade:complete [target-branch]"
---

# Planning with Files - Complete Worktree Task

You are now completing a worktree task. This will:
1. Verify all phases are complete
2. **CRITICAL: Commit code changes (planning files excluded)**
3. Delete planning files from the worktree
4. Navigate to the root directory
5. Merge the task branch to target branch
6. Remove the worktree
7. Delete the task branch

## Path Storage Modes

This command works with both new and legacy path storage modes:

### New Mode (Default)
- Worktrees in: `~/.plan-cascade/<project-id>/.worktree/`
- State files in: `~/.plan-cascade/<project-id>/.state/`
- Cleanup removes files from user data directory

### Legacy Mode
- Worktrees in: `<project-root>/.worktree/`
- State files in project root

The command auto-detects which mode is active based on `.planning-config.json` contents.

## Step 1: Detect Current Location

Check if we're in a worktree or the root directory:

```bash
if [ -f ".planning-config.json" ]; then
    # We're in a worktree directory
    echo "Currently in worktree directory: $(pwd)"
    IN_WORKTREE=true
else
    # We're not in a worktree, check if there are any worktrees
    IN_WORKTREE=false

    # Check for worktrees
    WORKTREES=$(git worktree list 2>/dev/null | grep -v "\bare$" | wc -l)

    if [ "$WORKTREES" -eq 0 ]; then
        echo "ERROR: No worktrees found."
        echo "This command requires an existing worktree."
        echo ""
        echo "Create one first with:"
        echo "  /plan-cascade:worktree <task-name> <branch>"
        exit 1
    fi

    echo "Not in a worktree directory. Found $WORKTREES worktree(s):"
    echo ""
    git worktree list
    echo ""

    # Find all worktrees with .planning-config.json (check both new and legacy locations)
    echo "Scanning for planning worktrees..."
    WORKTREE_LIST=()

    # Get worktree base directory from PathResolver
    WORKTREE_BASE=$(uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_worktree_dir())" 2>/dev/null || echo ".worktree")

    while IFS= read -r line; do
        worktree_path=$(echo "$line" | awk '{print $1}')
        worktree_branch=$(echo "$line" | awk '{print $2}')

        # Check if this is a planning worktree
        if [ -f "$worktree_path/.planning-config.json" ]; then
            # Exclude hybrid mode worktrees (those use /plan-cascade:hybrid-complete)
            mode=$(jq -r '.mode // empty' "$worktree_path/.planning-config.json" 2>/dev/null)
            if [ "$mode" != "hybrid" ]; then
                task_name=$(jq -r '.task_name // empty' "$worktree_path/.planning-config.json" 2>/dev/null)
                WORKTREE_LIST+=("$worktree_path|$task_name|$worktree_branch")
            fi
        fi
    done < <(git worktree list 2>/dev/null | grep -v "\bare$")

    if [ ${#WORKTREE_LIST[@]} -eq 0 ]; then
        echo "ERROR: No planning worktrees found."
        echo "Found worktrees but none are in planning mode."
        echo ""
        echo "Note: Hybrid mode worktrees should use /plan-cascade:hybrid-complete"
        exit 1
    fi

    echo "Found ${#WORKTREE_LIST[@]} planning worktree(s):"
    echo ""

    # Display options
    for i in "${!WORKTREE_LIST[@]}"; do
        IFS='|' read -r path name branch <<< "${WORKTREE_LIST[$i]}"
        echo "  [$((i+1))] $name"
        echo "      Path: $path"
        echo "      Branch: $branch"
        echo ""
    done

    # Ask user to select
    echo "Which worktree would you like to complete?"
    read -p "Enter number (or 0 to cancel): " selection

    if [ "$selection" = "0" ]; then
        echo "Cancelled."
        exit 0
    fi

    if [ "$selection" -lt 1 ] || [ "$selection" -gt ${#WORKTREE_LIST[@]} ]; then
        echo "Invalid selection."
        exit 1
    fi

    # Get the selected worktree
    selected="${WORKTREE_LIST[$((selection-1))]}"
    IFS='|' read -r WORKTREE_PATH TASK_NAME TASK_BRANCH <<< "$selected"

    echo ""
    echo "Selected: $TASK_NAME"
    echo "Navigating to worktree: $WORKTREE_PATH"

    # Change to worktree directory
    cd "$WORKTREE_PATH" || {
        echo "ERROR: Failed to navigate to worktree: $WORKTREE_PATH"
        exit 1
    }

    echo "✓ Now in worktree: $(pwd)"
    IN_WORKTREE=true
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

## Step 5: CRITICAL - Check for Uncommitted Code Changes

**IMPORTANT**: Planning files are NOT included in the commit. We check only actual code changes.

```bash
# Define planning files to exclude from commit
PLANNING_FILES=(
    "task_plan.md"
    "findings.md"
    "progress.md"
    ".planning-config.json"
)

# Check if there are changes excluding planning files
echo "Checking for code changes (excluding planning files)..."

# Get all changes
ALL_CHANGES=$(git status --short --untracked-files=all --porcelain 2>/dev/null || true)

# Filter out planning files
CODE_CHANGES="$ALL_CHANGES"
for file in task_plan.md findings.md progress.md .planning-config.json; do
    CODE_CHANGES=$(echo "$CODE_CHANGES" | grep -v "$file" || true)
done

if [ -n "$CODE_CHANGES" ]; then
    echo "=========================================="
    echo "Uncommitted CODE changes detected!"
    echo "=========================================="
    echo ""
    echo "Planning files (task_plan.md, findings.md, etc.) are excluded."
    echo "These CODE changes will be LOST if not committed:"
    echo ""
    echo "$CODE_CHANGES"
    echo ""
    echo "You MUST commit these code changes before completing the task."
    echo ""
    echo "Options:"
    echo "  1) Auto-commit code changes with generated message"
    echo "  2) Stash all changes (including planning files)"
    echo "  3) Cancel and handle manually"
    echo ""
    read -p "Choose [1/2/3]: " choice

    case "$choice" in
        1)
            echo ""
            echo "Committing code changes (planning files excluded)..."
            COMMIT_MSG="Complete task: $TASK_NAME

Branch: $TASK_BRANCH
Target: $TARGET_FINAL

Co-Authored-By: Claude <noreply@anthropic.com>"

            # Add all files except planning files
            git add -A
            # Unstage planning files
            git reset HEAD task_plan.md findings.md progress.md .planning-config.json 2>/dev/null || true
            # Commit the rest
            git commit -m "$COMMIT_MSG" 2>/dev/null || echo "Note: Only planning files were changed"

            echo "✓ Code changes committed (planning files excluded)"
            ;;
        2)
            echo ""
            echo "Stashing all changes..."
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
            echo "Cancelled. Please commit your code changes manually:"
            echo "  git add <your files>"
            echo "  git commit -m 'Your message here'"
            echo ""
            echo "Planning files will be excluded automatically."
            echo "Then run this command again."
            exit 0
            ;;
        *)
            echo "Invalid choice. Aborted."
            exit 1
            ;;
    esac
else
    echo "✓ No uncommitted code changes"
fi
```

## Step 6: Delete Planning Files

Now that code changes are handled, delete planning files:

```bash
echo "Deleting planning files..."

# Core planning files
for file in task_plan.md findings.md progress.md; do
    if [ -f "$file" ]; then
        rm "$file"
        echo "  Deleted: $file"
    fi
done

# Config file
if [ -f ".planning-config.json" ]; then
    rm ".planning-config.json"
    echo "  Deleted: .planning-config.json"
fi

# State directory (if exists)
if [ -d ".state" ]; then
    rm -rf ".state"
    echo "  Deleted: .state/"
fi

# Locks directory (if exists)
if [ -d ".locks" ]; then
    rm -rf ".locks"
    echo "  Deleted: .locks/"
fi

# Clean up state files from user data directory (new mode)
USER_STATE_DIR=$(uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_state_dir())" 2>/dev/null || echo "")
if [ -n "$USER_STATE_DIR" ] && [ -d "$USER_STATE_DIR" ]; then
    echo "Cleaning up state files from user data directory..."
    rm -rf "$USER_STATE_DIR" 2>/dev/null || true
    echo "  Cleaned: user state directory"
fi

echo "✓ Planning files deleted"
```

## Step 7: Show What Will Happen

Before making any changes, show the user what will be done:

```
=== Worktree Completion Summary ===

Task: $TASK_NAME
Branch: $TASK_BRANCH
Target: $TARGET_FINAL

Latest commits:
{Show git log --oneline -3}

This will:
  1. Navigate to root directory
  2. Merge $TASK_BRANCH into $TARGET_FINAL
  3. Delete this worktree
  4. Delete the task branch

Proceed? [Y/n]:
```

Wait for user confirmation.

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

✓ Code changes committed (planning files excluded)
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

- **Planning files excluded**: The following are never committed and are cleaned up:
  - Planning files: `task_plan.md`, `findings.md`, `progress.md`
  - Config: `.planning-config.json`
  - Directories: `.state/`, `.locks/`
- **Only code changes**: Actual code changes are detected and committed
- **Auto-commit option**: Automatically commits code with generated message
- **Stash option**: Can stash changes if needed
- **Manual cancel**: Always allows manual intervention
- **Conflict handling**: Provides clear instructions for merge conflicts
- **Verification**: Shows exactly what will be merged before proceeding
