#!/bin/bash
# Complete worktree mode for planning-with-files
# Merges the worktree branch and cleans up the worktree directory
# Usage: ./worktree-complete.sh [target-branch-override]
# Run this FROM INSIDE the worktree directory

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

OVERRIDE_TARGET="${1:-}"
CONFIG_FILE=".planning-config.json"

echo -e "${CYAN}=== Planning with Files - Complete Worktree Task ===${NC}"
echo ""

# Step 1: Verify we're in a worktree
if [ ! -f "$CONFIG_FILE" ]; then
    echo -e "${RED}ERROR: .planning-config.json not found${NC}"
    echo ""
    echo "Are you in a worktree directory?"
    echo "This command must be run from inside the worktree."
    exit 1
fi

# Step 2: Read configuration
config=$(cat "$CONFIG_FILE")
MODE=$(echo "$config" | grep '"mode"' | sed 's/.*"mode"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
TASK_NAME=$(echo "$config" | grep '"task_name"' | sed 's/.*"task_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
TASK_BRANCH=$(echo "$config" | grep '"task_branch"' | sed 's/.*"task_branch"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
TARGET_BRANCH=$(echo "$config" | grep '"target_branch"' | sed 's/.*"target_branch"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
WORKTREE_DIR=$(echo "$config" | grep '"worktree_dir"' | sed 's/.*"worktree_dir"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
ROOT_DIR=$(echo "$config" | grep '"root_dir"' | sed 's/.*"root_dir"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
ORIGINAL_BRANCH=$(echo "$config" | grep '"original_branch"' | sed 's/.*"original_branch"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')

if [ "$MODE" != "worktree" ]; then
    echo -e "${RED}ERROR: Not in worktree mode (current mode: $MODE)${NC}"
    exit 1
fi

# Use override target if provided
TARGET_FINAL="${OVERRIDE_TARGET:-$TARGET_BRANCH}"

echo -e "${BLUE}Current Worktree:${NC}"
echo "  Task Name:      $TASK_NAME"
echo "  Task Branch:    $TASK_BRANCH"
echo "  Target Branch:  $TARGET_BRANCH"
if [ -n "$OVERRIDE_TARGET" ]; then
    echo "  Override Target: $TARGET_FINAL"
fi
echo "  Worktree Dir:   $WORKTREE_DIR"
echo "  Root Directory: $ROOT_DIR"
echo "  Original Branch: $ORIGINAL_BRANCH"
echo ""

# Step 3: Verify task completion
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

# Step 4: Check for uncommitted changes
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

# Step 5: Show summary
echo ""
echo -e "${CYAN}=== Worktree Completion Summary ===${NC}"
echo ""
echo "This will:"
echo "  1. Delete planning files from worktree"
echo "  2. Navigate to root directory"
echo "  3. Merge $TASK_BRANCH into $TARGET_FINAL"
echo "  4. Delete this worktree"
echo "  5. Delete the task branch"
echo ""
read -p "Proceed? [Y/n]: " confirm
if [[ "$confirm" =~ ^[Nn]$ ]]; then
    echo "Cancelled."
    exit 0
fi

# Step 6: Delete planning files from worktree
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

# Step 7: Navigate to root directory
echo ""
echo -e "${BLUE}Navigating to root directory...${NC}"
cd "$ROOT_DIR"
echo "  Now in: $(pwd)"

# Step 8: Switch to target branch
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
    echo -e "${YELLOW}Warning: Could not checkout $TARGET_FINAL${NC}"
    echo "  Staying on: $(git branch --show-current)"
fi

# Step 9: Merge task branch
echo ""
echo -e "${BLUE}Merging $TASK_BRANCH into $TARGET_FINAL...${NC}"

if git merge --no-ff -m "Merge task branch: $TASK_NAME" "$TASK_BRANCH" 2>/dev/null; then
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
    echo "  3. Run: git worktree remove $WORKTREE_DIR"
    echo "  4. Run: git branch -d $TASK_BRANCH"
    echo ""
    echo "Or abort with: git merge --abort"
    exit 1
fi

# Step 10: Remove worktree
echo ""
echo -e "${BLUE}Removing worktree: $WORKTREE_DIR${NC}"
if git worktree remove "$WORKTREE_DIR" 2>/dev/null; then
    echo -e "${GREEN}Worktree removed${NC}"
else
    # Fallback to manual removal
    rm -rf "$WORKTREE_DIR"
    echo -e "${YELLOW}Worktree directory removed (manually)${NC}"
fi

# Step 11: Delete task branch
echo ""
echo -e "${BLUE}Deleting task branch: $TASK_BRANCH${NC}"
if git branch -d "$TASK_BRANCH" 2>/dev/null; then
    echo -e "${GREEN}Task branch deleted${NC}"
else
    echo -e "${YELLOW}Warning: Could not delete branch $TASK_BRANCH${NC}"
    echo "  You may need to delete it manually with: git branch -D $TASK_BRANCH"
fi

# Step 12: Summary
echo ""
echo -e "${GREEN}=== Task Completed Successfully ===${NC}"
echo ""
echo "Task: $TASK_NAME"
echo "Branch: $TASK_BRANCH merged into $TARGET_FINAL"
echo ""
echo "Planning files have been deleted."
echo "Worktree has been removed."
echo ""
echo "Current branch: $(git branch --show-current)"
echo "Current directory: $(pwd)"
echo ""
echo -e "${CYAN}=== Active Worktrees ===${NC}"
git worktree list
echo ""
echo -e "${YELLOW}Next:${NC}"
echo "  - Push the merge if needed: git push"
echo "  - Continue with your next task"
echo ""
