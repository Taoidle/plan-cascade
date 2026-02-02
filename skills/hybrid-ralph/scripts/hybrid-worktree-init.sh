#!/bin/bash
#
# Hybrid Ralph Worktree Initialization Script
# Combines Git worktree creation with Hybrid Ralph PRD generation
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Print colored message
print_msg() {
    local color=$1
    shift
    echo -e "${color}$*${NC}"
}

# Print error and exit
error_exit() {
    print_msg "$RED" "Error: $1"
    exit 1
}

# Check if we're in a git repository
check_git_repo() {
    if ! git rev-parse --git-dir > /dev/null 2>&1; then
        error_exit "Not a git repository. Please run: git init"
    fi
}

# Detect default branch
detect_default_branch() {
    local branch
    branch=$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@^refs/remotes/origin/@@')
    if [ -z "$branch" ]; then
        # Fallback to checking main or master
        if git show-ref --verify --quiet refs/heads/main; then
            branch="main"
        elif git show-ref --verify --quiet refs/heads/master; then
            branch="master"
        else
            branch="main"  # Default to main
        fi
    fi
    echo "$branch"
}

# Create worktree
create_worktree() {
    local task_name=$1
    local target_branch=$2

    # Generate timestamp for branch name
    local timestamp=$(date +%Y%m%d-%H%M)
    local task_branch="task-${timestamp}"

    # Worktree path
    local worktree_path=".worktree/${task_name}"

    # Check if worktree already exists
    if [ -d "$worktree_path" ]; then
        error_exit "Worktree already exists at ${worktree_path}"
    fi

    print_msg "$BLUE" "Creating Git worktree..."
    print_msg "$YELLOW" "  Task name: ${task_name}"
    print_msg "$YELLOW" "  Task branch: ${task_branch}"
    print_msg "$YELLOW" "  Target branch: ${target_branch}"
    print_msg "$YELLOW" "  Worktree path: ${worktree_path}"

    # Create worktree
    git worktree add -b "$task_branch" "$worktree_path" "$target_branch"

    if [ $? -eq 0 ]; then
        print_msg "$GREEN" "✓ Worktree created successfully!"
    else
        error_exit "Failed to create worktree"
    fi

    echo "$worktree_path"
}

# Initialize hybrid ralph in worktree
init_hybrid_ralph() {
    local worktree_path=$1
    local task_description=$2

    print_msg "$BLUE" "Initializing Hybrid Ralph in worktree..."

    # Create planning config
    cat > "${worktree_path}/.planning-config.json" << EOF
{
  "mode": "hybrid",
  "task_name": "$(basename "$worktree_path")",
  "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "task_branch": "$(cd "${worktree_path}" && git branch --show-current)",
  "root_dir": "$(cd "${worktree_path}/../.." && pwd)"
}
EOF

    print_msg "$GREEN" "✓ Planning config created"

    # Create initial PRD template
    if [ -n "$task_description" ]; then
        print_msg "$BLUE" "Generating PRD from description..."
        uv run python "${SCRIPT_DIR}/prd-generate.py" "$task_description" > "${worktree_path}/prd.json" 2>/dev/null || true
        print_msg "$GREEN" "✓ PRD template created"
    fi

    # Create empty files
    touch "${worktree_path}/findings.md"
    touch "${worktree_path}/progress.txt"

    print_msg "$GREEN" "✓ Hybrid Ralph initialized!"
}

# Main execution
main() {
    # Get script directory
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

    # Parse arguments
    TASK_NAME=""
    TARGET_BRANCH=""
    TASK_DESCRIPTION=""

    if [ $# -lt 1 ]; then
        echo "Usage: $0 <task-name> [target-branch] [task-description]"
        echo ""
        echo "Examples:"
        echo "  $0 feature-auth main 'Implement user auth'"
        echo "  $0 refactor-api main"
        exit 1
    fi

    TASK_NAME="$1"
    shift

    if [ $# -gt 0 ]; then
        TARGET_BRANCH="$1"
        shift
    fi

    # Remaining arguments form the task description
    TASK_DESCRIPTION="$*"

    # Validate task name
    if [ -z "$TASK_NAME" ]; then
        error_exit "Task name is required"
    fi

    # Check git repository
    check_git_repo

    # Detect target branch if not provided
    if [ -z "$TARGET_BRANCH" ]; then
        TARGET_BRANCH=$(detect_default_branch)
        print_msg "$YELLOW" "Auto-detected target branch: ${TARGET_BRANCH}"
    fi

    # Verify target branch exists
    if ! git show-ref --verify --quiet "refs/heads/${TARGET_BRANCH}" && \
       ! git show-ref --verify --quiet "refs/remotes/origin/${TARGET_BRANCH}"; then
        error_exit "Target branch '${TARGET_BRANCH}' not found"
    fi

    print_msg "$BLUE" "═══════════════════════════════════════════════════"
    print_msg "$BLUE" "  Hybrid Ralph + Worktree Setup"
    print_msg "$BLUE" "═══════════════════════════════════════════════════"
    echo ""

    # Create worktree
    WORKTREE_PATH=$(create_worktree "$TASK_NAME" "$TARGET_BRANCH")

    echo ""

    # Initialize hybrid ralph
    init_hybrid_ralph "$WORKTREE_PATH" "$TASK_DESCRIPTION"

    echo ""
    print_msg "$GREEN" "═══════════════════════════════════════════════════"
    print_msg "$GREEN" "  Setup Complete!"
    print_msg "$GREEN" "═══════════════════════════════════════════════════"
    echo ""
    print_msg "$YELLOW" "Next steps:"
    echo "  1. cd $WORKTREE_PATH"
    echo "  2. Review/edit prd.json if needed"
    echo "  3. Run: /approve"
    echo ""
    print_msg "$YELLOW" "Or run hybrid commands from the worktree:"
    echo "  cd $WORKTREE_PATH"
    echo "  /hybrid:auto '$TASK_DESCRIPTION'"
    echo "  /approve"
    echo ""
    print_msg "$YELLOW" "When complete:"
    echo "  /hybrid:complete $TARGET_BRANCH"
    echo ""
}

# Run main
main "$@"
