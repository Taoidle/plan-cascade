#!/bin/bash
# Initialize worktree mode for planning-with-files
# Usage: ./worktree-init.sh [branch-name] [target-branch]

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
TASK_BRANCH="${1:-task-$(date +%Y-%m-%d)}"
TARGET_BRANCH="${2:-}"
WORKTREE_DIR=".worktree/$(basename "$TASK_BRANCH")"
CONFIG_FILE=".planning-config.json"

echo -e "${GREEN}=== Planning with Files - Worktree Init ===${NC}"
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
echo "  Task Branch:    $TASK_BRANCH"
echo "  Target Branch:  $TARGET_BRANCH"
echo "  Worktree Dir:   $WORKTREE_DIR"
echo ""

# Step 3: Check for existing config
if [ -f "$CONFIG_FILE" ]; then
    echo -e "${YELLOW}WARNING: .planning-config.json already exists${NC}"
    echo ""
    echo "Existing configuration:"
    cat "$CONFIG_FILE"
    echo ""
    echo "Do you want to:"
    echo "  1) Continue with existing session"
    echo "  2) Start a new session (will overwrite config)"
    echo ""
    read -p "Choose [1/2]: " choice
    if [ "$choice" = "1" ]; then
        echo "Continuing with existing session..."
        exit 0
    fi
fi

# Step 4: Create task branch
echo -e "${GREEN}Creating task branch...${NC}"
if git show-ref --verify --quiet refs/heads/"$TASK_BRANCH"; then
    echo "Branch $TASK_BRANCH already exists. Checking it out..."
    git checkout "$TASK_BRANCH"
else
    git checkout "$TARGET_BRANCH" 2>/dev/null || {
        echo -e "${RED}ERROR: Cannot checkout target branch $TARGET_BRANCH${NC}"
        exit 1
    }
    git checkout -b "$TASK_BRANCH"
    echo -e "${GREEN}Created new branch: $TASK_BRANCH (from $TARGET_BRANCH)${NC}"
fi

# Step 5: Create planning config
cat > "$CONFIG_FILE" << EOF
{
  "mode": "worktree",
  "task_branch": "$TASK_BRANCH",
  "target_branch": "$TARGET_BRANCH",
  "worktree_dir": "$WORKTREE_DIR",
  "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "planning_files": [
    "task_plan.md",
    "findings.md",
    "progress.md"
  ]
}
EOF
echo -e "${GREEN}Created $CONFIG_FILE${NC}"

# Step 6: Create planning files
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "$SCRIPT_DIR/init-session.sh" ]; then
    bash "$SCRIPT_DIR/init-session.sh"
else
    # Fallback: create files manually
    cat > task_plan.md << PLANEOF
# Task Plan: [Task Description]

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
- [ ] Complete task with worktree-complete command
- **Status:** pending

## Decisions Made
| Decision | Rationale |
|----------|-----------|

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|

## Worktree Info
- **Branch:** $TASK_BRANCH
- **Target:** $TARGET_BRANCH
- **Complete with:** \`/planning-with-files:complete\`
PLANEOF

    cat > findings.md << FINDINGSEOF
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

    cat > progress.md << PROGRESSEOF
# Progress Log

## Session: $(date +%Y-%m-%d)

### Current Status
- **Phase:** 1 - Requirements & Discovery
- **Started:** $(date +%Y-%m-%d)
- **Branch:** $TASK_BRANCH

### Actions Taken
-

### Test Results
| Test | Expected | Actual | Status |
|------|----------|--------|--------|

### Errors
| Error | Resolution |
|-------|------------|
PROGRESSEOF

    echo -e "${GREEN}Created planning files${NC}"
fi

# Step 7: Summary
echo ""
echo -e "${GREEN}=== Worktree Session Created ===${NC}"
echo ""
echo "Branch:       $TASK_BRANCH"
echo "Target:       $TARGET_BRANCH"
echo "Config File:  $CONFIG_FILE"
echo ""
echo "Planning Files:"
echo "  - task_plan.md"
echo "  - findings.md"
echo "  - progress.md"
echo ""
echo -e "${YELLOW}Next Steps:${NC}"
echo "  1. Edit task_plan.md to define your task phases"
echo "  2. Work on your task in this isolated branch"
echo "  3. Use /planning-with-files:complete when done"
echo ""
