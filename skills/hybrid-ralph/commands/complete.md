---
description: "Complete Hybrid Ralph task in worktree, verify all stories complete, commit code changes (excluding planning files), merge to target branch, and cleanup worktree directory. Can be run from any directory."
---

# Hybrid Ralph - Complete Worktree Task

You are completing a Hybrid Ralph task in a worktree and merging it to the target branch.

## Path Storage Modes

This command works with both new and legacy path storage modes:

### New Mode (Default)
- Worktrees located in: `~/.plan-cascade/<project-id>/.worktree/` (Unix) or `%APPDATA%/plan-cascade/<project-id>/.worktree/` (Windows)
- State files in: `~/.plan-cascade/<project-id>/.state/`
- Cleanup removes files from user data directory

### Legacy Mode
- Worktrees located in: `<project-root>/.worktree/`
- State files in project root
- Cleanup removes files from project root

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
        echo "This command requires an existing hybrid worktree."
        echo ""
        echo "Create one first with:"
        echo "  /hybrid:worktree <task-name> <branch> <description>"
        exit 1
    fi

    echo "Not in a worktree directory. Found $WORKTREES worktree(s):"
    echo ""
    git worktree list
    echo ""

    # Find all hybrid worktrees (check both new mode user directory and legacy project root)
    echo "Scanning for hybrid worktrees..."
    HYBRID_WORKTREES=()

    # Get worktree base directory from PathResolver (handles new vs legacy mode)
    WORKTREE_BASE=$(uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_worktree_dir())" 2>/dev/null || echo ".worktree")

    while IFS= read -r line; do
        worktree_path=$(echo "$line" | awk '{print $1}')
        worktree_branch=$(echo "$line" | awk '{print $2}')

        # Check if this is a hybrid worktree
        if [ -f "$worktree_path/.planning-config.json" ]; then
            mode=$(jq -r '.mode // empty' "$worktree_path/.planning-config.json" 2>/dev/null)
            if [ "$mode" = "hybrid" ]; then
                task_name=$(jq -r '.task_name // empty' "$worktree_path/.planning-config.json" 2>/dev/null)
                HYBRID_WORKTREES+=("$worktree_path|$task_name|$worktree_branch")
            fi
        fi
    done < <(git worktree list 2>/dev/null | grep -v "\bare$")

    if [ ${#HYBRID_WORKTREES[@]} -eq 0 ]; then
        echo "ERROR: No hybrid worktrees found."
        echo "Found worktrees but none are in hybrid mode."
        exit 1
    fi

    echo "Found ${#HYBRID_WORKTREES[@]} hybrid worktree(s):"
    echo ""

    # Display options
    for i in "${!HYBRID_WORKTREES[@]}"; do
        IFS='|' read -r path name branch <<< "$i"
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

    if [ "$selection" -lt 1 ] || [ "$selection" -gt ${#HYBRID_WORKTREES[@]} ]; then
        echo "Invalid selection."
        exit 1
    fi

    # Get the selected worktree
    selected="${HYBRID_WORKTREES[$((selection-1))]}"
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

## Step 2: Verify Worktree Mode (if in worktree)

```bash
MODE=$(jq -r '.mode // empty' .planning-config.json 2>/dev/null || echo "")
if [ "$MODE" != "hybrid" ]; then
    echo "ERROR: Not in hybrid worktree mode."
    exit 1
fi
```

## Step 3: Read Planning Config

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

## Step 4: Verify All Stories Complete

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

## Step 5: CRITICAL - Check for Uncommitted Code Changes

**IMPORTANT**: Planning files are NOT included in the commit. We check only actual code changes.

```bash
# Define planning files to exclude from commit
PLANNING_FILES=(
    "prd.json"
    "findings.md"
    "progress.txt"
    ".planning-config.json"
    ".agent-outputs/"
    "mega-findings.md"
    ".agent-status.json"
)

# Check if there are changes excluding planning files
echo "Checking for code changes (excluding planning files)..."

# Get all changes
ALL_CHANGES=$(git status --short --untracked-files=all --porcelain 2>/dev/null || true)

# Filter out planning files
CODE_CHANGES="$ALL_CHANGES"
for file in prd.json findings.md progress.txt .planning-config.json .agent-outputs mega-findings.md .agent-status.json; do
    CODE_CHANGES=$(echo "$CODE_CHANGES" | grep -v "$file" || true)
done

if [ -n "$CODE_CHANGES" ]; then
    echo "=========================================="
    echo "Uncommitted CODE changes detected!"
    echo "=========================================="
    echo ""
    echo "Planning files (prd.json, findings.md, etc.) are excluded."
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
            COMMIT_MSG="Complete hybrid task: $TASK_NAME

Stories completed: $COMPLETE_STORIES/$TOTAL_STORIES
Branch: $TASK_BRANCH
Target: $TARGET_FINAL

Co-Authored-By: Claude <noreply@anthropic.com>"

            # Add all files except planning files
            git add -A
            # Unstage planning files
            git reset HEAD prd.json findings.md progress.txt .planning-config.json mega-findings.md .agent-status.json 2>/dev/null || true
            git reset HEAD .agent-outputs/ 2>/dev/null || true
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

Now that code changes are handled, delete planning files from both worktree and user data directory:

```bash
echo "Deleting planning files..."

# Delete files from current worktree directory
# Planning documents
rm -f prd.json findings.md progress.txt .planning-config.json mega-findings.md
rm -f design_doc.json spec.json spec.md

# Status and state files
rm -f .agent-status.json .iteration-state.json .retry-state.json

# Context recovery files
rm -f .hybrid-execution-context.md .mega-execution-context.md

# Directories
rm -rf .agent-outputs
rm -rf .locks
rm -rf .state

echo "✓ Planning files deleted from worktree"

# Also clean up state files from user data directory (new mode)
USER_STATE_DIR=$(uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_state_dir())" 2>/dev/null || echo "")
if [ -n "$USER_STATE_DIR" ] && [ -d "$USER_STATE_DIR" ]; then
    echo "Cleaning up state files from user data directory..."
    rm -f "$USER_STATE_DIR/.iteration-state.json" 2>/dev/null || true
    rm -f "$USER_STATE_DIR/.agent-status.json" 2>/dev/null || true
    rm -f "$USER_STATE_DIR/.retry-state.json" 2>/dev/null || true
    rm -f "$USER_STATE_DIR/spec-interview.json" 2>/dev/null || true
    # Remove .state directory if empty
    rmdir "$USER_STATE_DIR" 2>/dev/null || true
    echo "✓ State files cleaned"
fi
```

## Step 7: Show Completion Summary

```
=== COMPLETION SUMMARY ===

Task: $TASK_NAME
Branch: $TASK_BRANCH
Target: $TARGET_FINAL

Stories: $TOTAL_STORIES total
  All complete ✓

Latest commits:
{Show git log --oneline -3}

Ready to merge to $TARGET_FINAL...
```

Wait for user confirmation.

## Step 8: Navigate to Root Directory

```bash
echo "Navigating to root directory: $ROOT_DIR"
cd "$ROOT_DIR"
echo "✓ Now in: $(pwd)"
```

## Step 9: Switch to Target Branch

```bash
echo "Switching to target branch: $TARGET_FINAL"

# Fetch latest if remote exists
if git ls-remote --exit-code origin "$TARGET_FINAL" > /dev/null 2>&1; then
    git fetch origin "$TARGET_FINAL"
fi

git checkout "$TARGET_FINAL" || git checkout -b "$TARGET_FINAL"
```

## Step 10: Merge Task Branch

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

## Step 11: Remove Worktree

```bash
echo "Removing worktree: $WORKTREE_DIR"
git worktree remove "$WORKTREE_DIR" 2>/dev/null || rm -rf "$WORKTREE_DIR"
echo "✓ Worktree removed"
```

## Step 12: Delete Task Branch

```bash
echo "Deleting task branch: $TASK_BRANCH"
git branch -d "$TASK_BRANCH" 2>/dev/null || echo "Warning: Could not delete branch"
echo "✓ Task branch deleted"
```

## Step 13: Show Final Summary

```
=== TASK COMPLETE ===

✓ Task: $TASK_NAME
✓ Stories: $COMPLETE_STORIES/$TOTAL_STORIES completed
✓ Code changes committed (planning files excluded)
✓ Planning files deleted
✓ Merged to: $TARGET_FINAL
✓ Worktree removed
✓ Task branch deleted

Current location: $ROOT_DIR
Current branch: $(git branch --show-current)

Next:
  - Review changes: git log --oneline -5
  - Push to remote: git push
  - Start a new task: /hybrid:worktree
```

## Usage Examples

```bash
# Option 1: Run from worktree directory (recommended)
cd .worktree/feature-auth
/hybrid:complete

# Option 2: Run from root directory (auto-detects worktree)
/hybrid:complete
# → Will show list of worktrees to select from

# Option 3: Specify target branch override
/hybrid:complete develop
```

## Safety Features

- **Smart directory detection**: Works from any directory
- **Auto worktree detection**: Lists all hybrid worktrees for selection
- **Planning files excluded**: The following files are NEVER committed and are cleaned up:
  - Planning documents: `prd.json`, `findings.md`, `progress.txt`, `mega-findings.md`
  - Design/spec files: `design_doc.json`, `spec.json`, `spec.md`
  - Config files: `.planning-config.json`
  - Status files: `.agent-status.json`, `.iteration-state.json`, `.retry-state.json`
  - Context files: `.hybrid-execution-context.md`, `.mega-execution-context.md`
  - Directories: `.agent-outputs/`, `.locks/`, `.state/`
- **Only code changes**: Actual code changes are detected and committed
- **Auto-commit option**: Automatically commits code with generated message
- **Stash option**: Can stash changes if needed
- **Manual cancel**: Always allows manual intervention
- **Conflict handling**: Provides clear instructions for merge conflicts
