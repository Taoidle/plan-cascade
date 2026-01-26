#!/bin/bash
#
# Hybrid Ralph Worktree Completion Script
# Verifies PRD completion and cleans up worktree
#

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_msg() {
    local color=$1
    shift
    echo -e "${color}$*${NC}"
}

error_exit() {
    print_msg "$RED" "Error: $1"
    exit 1
}

# Check if in worktree mode
check_worktree_mode() {
    if [ ! -f ".planning-config.json" ]; then
        error_exit "Not in worktree mode. .planning-config.json not found."
    fi

    local mode=$(jq -r '.mode' .planning-config.json 2>/dev/null || echo "")
    if [ "$mode" != "hybrid" ]; then
        error_exit "Not in hybrid worktree mode. Use /planning-with-files:complete instead."
    fi
}

# Read planning config
read_config() {
    TASK_NAME=$(jq -r '.task_name' .planning-config.json 2>/dev/null || echo "")
    TASK_BRANCH=$(jq -r '.task_branch' .planning-config.json 2>/dev/null || echo "")
    ROOT_DIR=$(jq -r '.root_dir' .planning-config.json 2>/dev/null || echo "")
}

# Verify all stories complete
verify_stories_complete() {
    print_msg "$BLUE" "Verifying PRD completion..."

    if [ ! -f "prd.json" ]; then
        error_exit "prd.json not found. Cannot verify completion."
    fi

    # Get all story IDs
    local story_ids=$(jq -r '.stories[].id' prd.json 2>/dev/null || echo "")

    if [ -z "$story_ids" ]; then
        print_msg "$YELLOW" "⚠ No stories found in PRD"
        return 0
    fi

    local incomplete=""

    for story_id in $story_ids; do
        if ! grep -q "\[COMPLETE\] $story_id" progress.txt 2>/dev/null; then
            incomplete="$incomplete  - $story_id: $(jq -r ".stories[] | select(.id==\"$story_id\") | .title" prd.json)\n"
        fi
    done

    if [ -n "$incomplete" ]; then
        print_msg "$RED" "✗ Not all stories are complete:"
        echo -e "$incomplete"
        error_exit "Complete all stories before running /hybrid:complete"
    fi

    local total=$(echo "$story_ids" | wc -l)
    print_msg "$GREEN" "✓ All $total stories complete!"
}

# Show completion summary
show_completion_summary() {
    print_msg "$BLUE" "═══════════════════════════════════════════════════"
    print_msg "$BLUE" "  COMPLETION SUMMARY"
    print_msg "$BLUE" "═══════════════════════════════════════════════════"
    echo ""
    print_msg "$YELLOW" "Task: $TASK_NAME"
    print_msg "$YELLOW" "Branch: $TASK_BRANCH"
    print_msg "$YELLOW" "Target: ${TARGET_BRANCH:-auto-detect}"
    echo ""

    # Show stories
    if [ -f "prd.json" ]; then
        local story_count=$(jq '.stories | length' prd.json 2>/dev/null || echo "0")
        print_msg "$YELLOW" "Stories: $story_count total"

        jq -r '.stories[] | "  ✓ \(.id): \(.title)"' prd.json 2>/dev/null || true
    fi

    echo ""
    print_msg "$GREEN" "All stories complete!"
    echo ""

    # Show changed files
    if [ -n "$ROOT_DIR" ]; then
        cd "$ROOT_DIR" || error_exit "Cannot navigate to root directory"
    fi

    print_msg "$YELLOW" "Changes to merge:"
    git diff --name-status "$TASK_BRANCH" "${TARGET_BRANCH:-main}" 2>/dev/null || true
    echo ""
}

# Cleanup worktree files
cleanup_worktree_files() {
    print_msg "$BLUE" "Cleaning up worktree files..."

    local current_dir=$(pwd)

    # Remove planning files
    rm -f prd.json findings.md progress.txt .planning-config.json

    # Remove agent outputs
    rm -rf .agent-outputs

    print_msg "$GREEN" "✓ Worktree files cleaned up"

    # Navigate back to current dir (in worktree)
    cd "$current_dir" || true
}

# Navigate to root and merge
navigate_and_merge() {
    local target_branch=${1:-""}

    print_msg "$BLUE" "Navigating to root directory..."

    if [ -n "$ROOT_DIR" ]; then
        cd "$ROOT_DIR" || error_exit "Cannot navigate to root directory"
    else
        # Try to navigate up from .worktree/
        if [[ "$PWD" == */.worktree/* ]]; then
            cd "$PWD/../../" || error_exit "Cannot navigate to root"
        fi
    fi

    print_msg "$GREEN" "✓ Now in project root"

    # Detect target branch if not provided
    if [ -z "$target_branch" ]; then
        if git show-ref --verify --quiet refs/heads/main; then
            target_branch="main"
        elif git show-ref --verify --quiet refs/heads/master; then
            target_branch="master"
        else
            # Get current branch as fallback
            target_branch=$(git branch --show-current)
        fi
        print_msg "$YELLOW" "Auto-detected target branch: $target_branch"
    fi

    print_msg "$BLUE" "Merging $TASK_BRANCH to $target_branch..."

    # Checkout target branch
    git checkout "$target_branch" || error_exit "Cannot checkout $target_branch"

    # Merge task branch
    if git merge --no-ff "$TASK_BRANCH" -m "Merge $TASK_NAME (task branch)"; then
        print_msg "$GREEN" "✓ Merge successful!"
    else
        print_msg "$RED" "✗ Merge failed or has conflicts"
        print_msg "$YELLOW" "Please resolve conflicts and complete manually"
        print_msg "$YELLOW" "After resolving, run: git worktree remove .worktree/$TASK_NAME && git branch -D $TASK_BRANCH"
        exit 1
    fi

    # Remove worktree
    print_msg "$BLUE" "Removing worktree..."
    git worktree remove ".worktree/$TASK_NAME" || print_msg "$YELLOW" "⚠ Could not remove worktree (may need manual cleanup)"

    # Delete task branch
    print_msg "$BLUE" "Deleting task branch..."
    git branch -D "$TASK_BRANCH" || print_msg "$YELLOW" "⚠ Could not delete task branch"

    print_msg "$GREEN" "✓ Cleanup complete!"
}

# Main execution
main() {
    local target_branch=""

    if [ $# -gt 0 ]; then
        target_branch="$1"
    fi

    print_msg "$BLUE" "═══════════════════════════════════════════════════"
    print_msg "$BLUE" "  Hybrid Ralph + Worktree Completion"
    print_msg "$BLUE" "═══════════════════════════════════════════════════"
    echo ""

    # Phase 1: Verification
    check_worktree_mode
    read_config
    verify_stories_complete

    # Phase 2: Show summary
    show_completion_summary

    # Phase 3: Cleanup
    print_msg "$YELLOW" "Ready to complete and merge."
    print_msg "$YELLOW" "Press Enter to continue or Ctrl+C to cancel..."
    read -r

    cleanup_worktree_files

    # Phase 4: Navigate and merge
    navigate_and_merge "$target_branch"

    echo ""
    print_msg "$GREEN" "═══════════════════════════════════════════════════"
    print_msg "$GREEN" "  Task Complete!"
    print_msg "$GREEN" "═══════════════════════════════════════════════════"
    echo ""
    print_msg "$GREEN" "✓ All stories complete"
    print_msg "$GREEN" "✓ Changes merged to $target_branch"
    print_msg "$GREEN" "✓ Worktree removed"
    print_msg "$GREEN" "✓ Task branch deleted"
    echo ""
    print_msg "$YELLOW" "You can now:"
    echo "  - Start a new worktree task with /hybrid:worktree"
    echo "  - Continue working in the current directory"
    echo "  - Push changes with: git push origin $target_branch"
    echo ""
}

main "$@"
