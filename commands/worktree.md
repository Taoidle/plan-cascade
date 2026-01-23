---
description: "Start a new task in an isolated Git worktree. Creates a task branch, worktree directory, and planning files. Usage: /planning-with-files:worktree [branch-name] [target-branch]. Example: /planning-with-files:worktree fix-auth-bug main"
---

# Planning with Files - Worktree Mode

You are now starting a task in **Git Worktree Mode**. This creates an isolated environment for your task with its own branch and planning files.

## Step 1: Determine Configuration

First, check if a planning configuration already exists:

```bash
cat .planning-config.json 2>/dev/null || echo "No existing config found"
```

If a config exists, ask the user if they want to:
- **Continue** the existing worktree session
- **Start a new** worktree session (will clean up existing)

If no config exists, proceed with the settings below.

## Step 2: Parse Parameters

Parse the user's command arguments:
- **Branch name**: `{{args}}` - First argument (or use `task-YYYY-MM-DD` format)
- **Target branch**: Second argument (or auto-detect `main`/`master`)

## Step 3: Verify Git Repository

Check this is a valid git repository:

```bash
git rev-parse --git-dir > /dev/null 2>&1 || { echo "ERROR: Not a git repository"; exit 1; }
```

## Step 4: Detect Default Branch

```bash
DEFAULT_BRANCH=$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@refs/remotes/origin/@@')
if [ -z "$DEFAULT_BRANCH" ]; then
    # Fallback detection
    if git show-ref --verify --quiet refs/heads/main; then
        DEFAULT_BRANCH="main"
    elif git show-ref --verify --quiet refs/heads/master; then
        DEFAULT_BRANCH="master"
    else
        DEFAULT_BRANCH="main"
    fi
fi
echo "Default branch detected: $DEFAULT_BRANCH"
```

## Step 5: Determine Branch Names

Set these variables:
```bash
TASK_BRANCH="{{args|first arg or 'task-' + date}}"
TARGET_BRANCH="{{args|second arg or $DEFAULT_BRANCH}}"
WORKTREE_DIR=".worktree/$(basename $TASK_BRANCH)"
```

Example with command `/planning-with-files:worktree feature-login main`:
- `TASK_BRANCH = "feature-login"`
- `TARGET_BRANCH = "main"`
- `WORKTREE_DIR = ".worktree/feature-login"`

Example with no args `/planning-with-files:worktree`:
- `TASK_BRANCH = "task-2026-01-23"`
- `TARGET_BRANCH = "main"` (detected)
- `WORKTREE_DIR = ".worktree/task-2026-01-23"`

## Step 6: Create Git Branch

Create and switch to the new task branch:

```bash
# Check if branch already exists
if git show-ref --verify --quiet refs/heads/"$TASK_BRANCH"; then
    echo "Branch $TASK_BRANCH already exists. Checking it out..."
    git checkout "$TASK_BRANCH"
else
    # Create new branch from target branch
    git checkout "$TARGET_BRANCH" || { echo "ERROR: Cannot checkout target branch $TARGET_BRANCH"; exit 1; }
    git checkout -b "$TASK_BRANCH"
    echo "Created new branch: $TASK_BRANCH (from $TARGET_BRANCH)"
fi
```

## Step 7: Create Planning Configuration

Save the worktree configuration:

```bash
cat > .planning-config.json << EOF
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
echo "Created .planning-config.json"
```

## Step 8: Create Planning Files

Create the three planning files in the current directory:

```bash
# Use the init-session script or create files directly
SCRIPT_DIR="${CLAUDE_PLUGIN_ROOT:-$HOME/.claude/plugins/planning-with-files}/scripts"

if [ -f "$SCRIPT_DIR/init-session.sh" ]; then
    bash "$SCRIPT_DIR/init-session.sh"
else
    echo "Creating planning files manually..."
    # Create task_plan.md
    cat > task_plan.md << 'PLANEOF'
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
- [ ] Use /planning-with-files:complete to finish
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
- **Complete with:** `/planning-with-files:complete`
PLANEOF

    # Create findings.md
    cat > findings.md << 'FINDINGSEOF'
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

    # Create progress.md
    cat > progress.md << 'PROGRESSEOF'
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
fi

echo "Created planning files: task_plan.md, findings.md, progress.md"
```

## Step 9: Display Summary

Show the user what was created:

```
=== Worktree Session Created ===

Branch:       $TASK_BRANCH
Target:       $TARGET_BRANCH
Config File:  .planning-config.json

Planning Files:
  - task_plan.md
  - findings.md
  - progress.md

Next Steps:
  1. Edit task_plan.md to define your task phases
  2. Work on your task in this isolated branch
  3. Use /planning-with-files:complete when done

The planning files will be deleted and the branch merged when you complete the task.
```

## Step 10: Remind User

Remind the user:
- Read the planning-with-files skill for the full workflow
- Update task_plan.md as you progress through phases
- Use `/planning-with-files:complete` to finish and merge

---

**Important:** After this command completes, the user is now in worktree mode with their task branch active. Continue with normal task execution following the planning-with-files workflow.
