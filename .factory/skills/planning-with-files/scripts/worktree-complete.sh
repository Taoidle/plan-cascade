#!/bin/bash
# Complete worktree mode for planning-with-files
# Usage: ./worktree-complete.sh [target-branch-override]

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

CONFIG_FILE=".planning-config.json"
OVERRIDE_TARGET="${1:-}"

echo -e "${GREEN}=== Planning with Files - Worktree Complete ===${NC}"
echo ""

# Step 1: Read configuration
if [ ! -f "$CONFIG_FILE" ]; then
    echo -e "${RED}ERROR: No planning configuration found${NC}"
    echo "Are you in a worktree session?"
    exit 1
fi

# Parse config (using grep for basic parsing)
MODE=$(grep '"mode"' "$CONFIG_FILE" | sed 's/.*"mode"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
TASK_BRANCH=$(grep '"task_branch"' "$CONFIG_FILE" | sed 's/.*"task_branch"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
TARGET_BRANCH=$(grep '"target_branch"' "$CONFIG_FILE" | sed 's/.*"target_branch"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
WORKTREE_DIR=$(grep '"worktree_dir"' "$CONFIG_FILE" | sed 's/.*"worktree_dir"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')

if [ "$MODE" != "worktree" ]; then
    echo -e "${RED}ERROR: Not in worktree mode (current mode: $MODE)${NC}"
    exit 1
fi

# Use override target if provided
TARGET_FINAL="${OVERRIDE_TARGET:-$TARGET_BRANCH}"

echo -e "${BLUE}Configuration:${NC}"
echo "  Task Branch:    $TASK_BRANCH"
echo "  Target Branch:  $TARGET_BRANCH"
if [ -n "$OVERRIDE_TARGET" ]; then
    echo "  Override:       $TARGET_FINAL"
fi
echo ""

# Step 2: Verify completion
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "$SCRIPT_DIR/check-complete.sh" ]; then
    echo -e "${BLUE}Checking task completion...${NC}"
    if ! bash "$SCRIPT_DIR/check-complete.sh" 2>&1; then
        echo ""
        echo -e "${YELLOW}WARNING: Not all phases are marked complete${NC}"
        echo ""
        read -p "Continue anyway? [y/N]: " confirm
        if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
            echo "Cancelled."
            exit 1
        fi
    fi
fi

# Step 3: Check for uncommitted changes
echo -e "${BLUE}Checking for uncommitted changes...${NC}"
if ! git diff-index --quiet HEAD -- 2>/dev/null; then
    echo -e "${YELLOW}There are uncommitted changes:${NC}"
    git status --short
    echo ""
    echo "Options:"
    echo "  1) Commit changes now"
    echo "  2) Stash changes"
    echo "  3) Cancel and handle manually"
    echo ""
    read -p "Choose [1/2/3]: " choice
    case "$choice" in
        1)
            read -p "Enter commit message: " msg
            git add -A
            git commit -m "${msg:-Complete task phase}"
            ;;
        2)
            git stash push -m "Worktree complete stash"
            ;;
        3)
            echo "Cancelled."
            exit 0
            ;;
        *)
            echo -e "${RED}Invalid choice${NC}"
            exit 1
            ;;
    esac
fi

# Step 4: Show summary
echo ""
echo -e "${BLUE}=== Worktree Completion Summary ===${NC}"
echo ""
echo "Files to Delete:"
echo "  - task_plan.md"
echo "  - findings.md"
echo "  - progress.md"
echo "  - .planning-config.json"
echo ""
echo "Actions:"
echo "  1. Delete planning files and config"
echo "  2. Switch to target branch ($TARGET_FINAL)"
echo "  3. Merge task branch ($TASK_BRANCH) into target"
echo "  4. Delete task branch"
echo "  5. Clean up worktree (if applicable)"
echo ""
read -p "Proceed? [Y/n]: " confirm
if [[ "$confirm" =~ ^[Nn]$ ]]; then
    echo "Cancelled."
    exit 0
fi

# Step 5: Delete planning files
echo ""
echo -e "${BLUE}Deleting planning files...${NC}"
for file in task_plan.md findings.md progress.md; do
    if [ -f "$file" ]; then
        rm "$file"
        echo "  Deleted: $file"
    fi
done
if [ -f "$CONFIG_FILE" ]; then
    rm "$CONFIG_FILE"
    echo "  Deleted: $CONFIG_FILE"
fi

# Step 6: Switch to target branch
echo ""
echo -e "${BLUE}Switching to target branch: $TARGET_FINAL${NC}"

# Fetch if remote exists
if git ls-remote --exit-code origin "$TARGET_FINAL" > /dev/null 2>&1; then
    git fetch origin "$TARGET_FINAL" 2>/dev/null || true
fi

if git checkout "$TARGET_FINAL" 2>/dev/null; then
    echo "  Checked out: $TARGET_FINAL"
elif git checkout -b "$TARGET_FINAL" "origin/$TARGET_FINAL" 2>/dev/null; then
    echo "  Created and checked out: $TARGET_FINAL from origin"
else
    echo -e "${YELLOW}Warning: Could not checkout $TARGET_FINAL, staying on $(git branch --show-current)${NC}"
fi

# Step 7: Merge task branch
echo ""
echo -e "${BLUE}Merging $TASK_BRANCH into $TARGET_FINAL...${NC}"

if git merge --no-ff -m "Merge task branch: $TASK_BRANCH" "$TASK_BRANCH" 2>/dev/null; then
    echo -e "${GREEN}Merge successful!${NC}"
else
    echo ""
    echo -e "${RED}=== MERGE CONFLICT DETECTED ===${NC}"
    echo ""
    echo "Merge conflicts need to be resolved manually."
    echo ""
    echo "After resolving conflicts:"
    echo "  1. Run: git add ."
    echo "  2. Run: git commit"
    echo "  3. Run: git branch -d $TASK_BRANCH"
    echo ""
    echo "Or abort with: git merge --abort"
    exit 1
fi

# Step 8: Delete task branch
echo ""
echo -e "${BLUE}Cleaning up task branch: $TASK_BRANCH${NC}"
if git branch -d "$TASK_BRANCH" 2>/dev/null; then
    echo "  Deleted branch: $TASK_BRANCH"
else
    echo -e "${YELLOW}Warning: Could not delete branch $TASK_BRANCH${NC}"
fi

# Step 9: Cleanup worktree if exists
WORKTREE_PATH=".worktree/$(basename "$TASK_BRANCH")"
if [ -d "$WORKTREE_PATH" ]; then
    echo ""
    echo -e "${BLUE}Cleaning up worktree: $WORKTREE_PATH${NC}"
    if git worktree remove "$WORKTREE_PATH" 2>/dev/null; then
        echo "  Removed worktree: $WORKTREE_PATH"
    else
        rm -rf "$WORKTREE_PATH" 2>/dev/null || echo -e "${YELLOW}Warning: Could not remove $WORKTREE_PATH${NC}"
    fi
fi

# Step 10: Summary
echo ""
echo -e "${GREEN}=== Task Completed Successfully ===${NC}"
echo ""
echo "Task branch $TASK_BRANCH has been merged into $TARGET_FINAL."
echo ""
echo "Planning files have been deleted."
echo ""
echo "Current branch: $(git branch --show-current)"
echo ""
echo -e "${YELLOW}Next:${NC}"
echo "  - Push the merge if needed: git push"
echo "  - Continue with your next task"
