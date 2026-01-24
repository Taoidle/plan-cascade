#!/bin/bash
# Initialize worktree mode for planning-with-files
# Creates an isolated Git worktree for parallel multi-task development
# Usage: ./worktree-init.sh [branch-name] [target-branch]

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Default values
TASK_NAME="${1:-task-$(date +%Y-%m-%d-%H%M)}"
TARGET_BRANCH="${2:-}"
WORKTREE_DIR=".worktree/$(basename "$TASK_NAME")"
TASK_BRANCH="$TASK_NAME"

# Save current branch
ORIGINAL_BRANCH=$(git branch --show-current)

echo -e "${CYAN}=== Planning with Files - Git Worktree Mode ===${NC}"
echo ""
echo -e "${BLUE}Multi-Task Parallel Development${NC}"
echo "Each task gets its own isolated worktree directory."
echo "You can run multiple tasks simultaneously without conflicts."
echo ""

# Step 1: Verify git repository
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    echo -e "${RED}ERROR: Not a git repository${NC}"
    exit 1
fi

# Step 2: Detect default branch
if [ -z "$TARGET_BRANCH" ]; then
    TARGET_BRANCH=$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@refs/remotes/origin/@@')
    if [ -z "$TARGET_BRANCH" ]; then
        if git show-ref --verify --quiet refs/heads/main; then
            TARGET_BRANCH="main"
        elif git show-ref --verify --quiet refs/heads/master; then
            TARGET_BRANCH="master"
        else
            TARGET_BRANCH="main"
        fi
    fi
fi

echo -e "${YELLOW}Configuration:${NC}"
echo "  Task Name:      $TASK_NAME"
echo "  Task Branch:    $TASK_BRANCH"
echo "  Target Branch:  $TARGET_BRANCH"
echo "  Worktree Path:  $WORKTREE_DIR"
echo "  Original Branch: $ORIGINAL_BRANCH"
echo ""

# Step 3: Check if worktree already exists
if [ -d "$WORKTREE_DIR" ]; then
    echo -e "${YELLOW}Worktree already exists: $WORKTREE_DIR${NC}"
    echo ""
    echo "This task is already in progress."
    echo ""
    read -p "Open existing worktree? [Y/n]: " choice
    if [[ ! "$choice" =~ ^[Nn]$ ]]; then
        echo ""
        echo -e "${GREEN}=== Opening Existing Worktree ===${NC}"
        echo ""
        echo "To work on this task, navigate to:"
        echo -e "${CYAN}  cd $WORKTREE_DIR${NC}"
        echo ""
        echo "Planning files are already in that directory."
        exit 0
    fi
    echo "Cancelled."
    exit 0
fi

# Step 4: Check if branch already exists (in other worktrees)
if git show-ref --verify --quiet refs/heads/"$TASK_BRANCH"; then
    echo -e "${YELLOW}Branch $TASK_BRANCH already exists${NC}"
    echo ""
    echo "This branch is checked out in another worktree."
    echo "Use that worktree or delete the branch first."
    exit 1
fi

# Step 5: Create Git Worktree
echo -e "${GREEN}Creating Git worktree...${NC}"
git worktree add -b "$TASK_BRANCH" "$WORKTREE_DIR" "$TARGET_BRANCH"
echo -e "${GREEN}Created worktree: $WORKTREE_DIR${NC}"

# Step 6: Create planning files in the worktree
echo ""
echo -e "${GREEN}Creating planning files in worktree...${NC}"

CONFIG_FILE="$WORKTREE_DIR/.planning-config.json"

# Create config in worktree
cat > "$CONFIG_FILE" << EOF
{
  "mode": "worktree",
  "task_name": "$TASK_NAME",
  "task_branch": "$TASK_BRANCH",
  "target_branch": "$TARGET_BRANCH",
  "worktree_dir": "$WORKTREE_DIR",
  "original_branch": "$ORIGINAL_BRANCH",
  "root_dir": "$(pwd)",
  "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "planning_files": [
    "task_plan.md",
    "findings.md",
    "progress.md"
  ]
}
EOF

# Create planning files in worktree
cat > "$WORKTREE_DIR/task_plan.md" << PLANEOF
# Task Plan: $TASK_NAME

## Goal
[One sentence describing the end state]

## Current Phase
Phase 1

## Phases

### Phase 1: Requirements & Discovery
- [ ] Understand user intent
- [ ] Identify constraints and requirements
- [ ] Document findings in findings.md
- **Status:** in_progress

### Phase 2: Planning & Structure
- [ ] Define technical approach
- [ ] Create project structure if needed
- [ ] Document decisions with rationale
- **Status:** pending

### Phase 3: Implementation
- [ ] Execute the plan step by step
- [ ] Write code to files before executing
- [ ] Test incrementally
- **Status:** pending

### Phase 4: Testing & Verification
- [ ] Verify all requirements met
- [ ] Document test results in progress.md
- [ ] Fix any issues found
- **Status:** pending

### Phase 5: Delivery
- [ ] Review all output files
- [ ] Ensure deliverables are complete
- [ ] Complete task with: \`/planning-with-files:complete\`
- **Status:** pending

## Decisions Made
| Decision | Rationale |
|----------|-----------|

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|

## Worktree Info
- **Task Name:** $TASK_NAME
- **Branch:** $TASK_BRANCH
- **Target:** $TARGET_BRANCH
- **Worktree:** $WORKTREE_DIR
- **Complete with:** \`/planning-with-files:complete\`
PLANEOF

cat > "$WORKTREE_DIR/findings.md" << FINDINGSEOF
# Findings & Decisions

## Requirements
-

## Research Findings
-

## Technical Decisions
| Decision | Rationale |
|----------|-----------|

## Issues Encountered
| Issue | Resolution |
|-------|------------|

## Resources
-
FINDINGSEOF

cat > "$WORKTREE_DIR/progress.md" << PROGRESSEOF
# Progress Log

## Session: $(date +%Y-%m-%d)

### Current Status
- **Phase:** 1 - Requirements & Discovery
- **Started:** $(date +%Y-%m-%d)
- **Branch:** $TASK_BRANCH
- **Task Name:** $TASK_NAME

### Actions Taken
-

### Test Results
| Test | Expected | Actual | Status |
|------|----------|--------|--------|

### Errors
| Error | Resolution |
|-------|------------|
PROGRESSEOF

echo -e "${GREEN}Planning files created${NC}"

# Step 7: List all active worktrees
echo ""
echo -e "${CYAN}=== Active Worktrees ===${NC}"
git worktree list

# Step 8: Final instructions
echo ""
echo -e "${GREEN}=== Worktree Session Created ===${NC}"
echo ""
echo -e "${YELLOW}IMPORTANT: Navigate to the worktree to work on this task${NC}"
echo ""
echo -e "${CYAN}cd $WORKTREE_DIR${NC}"
echo ""
echo "Once in the worktree directory:"
echo "  1. Edit task_plan.md to define your task phases"
echo "  2. Work on your task in this isolated environment"
echo "  3. Use /planning-with-files:complete when done"
echo ""
echo -e "${BLUE}Multi-Task Usage:${NC}"
echo "You can create multiple worktrees for parallel tasks:"
echo "  /planning-with-files:worktree task-auth-fix"
echo "  /planning-with-files:worktree task-refactor"
echo "  /planning-with-files:worktree task-docs"
echo ""
echo "Each task works in its own directory without conflicts."
echo ""
echo -e "${BLUE}To return to the main project:${NC}"
echo "  cd $(pwd)"
echo ""
