---
description: "Start a new task in an isolated Git worktree for parallel multi-task development. Creates a task branch, worktree directory with planning files, and leaves the main directory untouched. Usage: /planning-with-files:worktree [task-name] [target-branch]. Example: /planning-with-files:worktree feature-login main"
---

# Planning with Files - Git Worktree Mode

You are now starting a task in **Git Worktree Mode**. This creates an isolated environment for your task with its own branch and directory, enabling **parallel multi-task development**.

## What is Git Worktree Mode?

Git worktree allows you to have multiple working trees attached to the same repository, each on a different branch. This means:

- **Multiple tasks, no conflicts**: Each task works in its own directory
- **No branch switching**: Stay on your main branch while working on feature branches
- **Isolated environments**: Each task has its own files and planning documents
- **Easy cleanup**: When done, merge and remove the worktree

## Step 1: Determine Configuration

First, check for existing worktrees:

```bash
git worktree list
```

If there are existing worktrees, show them to the user and ask if they want to create another one.

## Step 2: Parse Parameters

Parse the user's command arguments:
- **Task name**: `{{args}}` - First argument (or use `task-YYYY-MM-DD-HHMM` format for uniqueness)
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

## Step 5: Determine Task Names

Set these variables:
```bash
TASK_NAME="{{args|first arg or 'task-' + date + '-' + time}}"
TASK_BRANCH="$TASK_NAME"
TARGET_BRANCH="{{args|second arg or $DEFAULT_BRANCH}}"
WORKTREE_DIR=".worktree/$(basename $TASK_NAME)"
ORIGINAL_BRANCH=$(git branch --show-current)
ROOT_DIR=$(pwd)
```

Example with command `/planning-with-files:worktree feature-login main`:
- `TASK_NAME = "feature-login"`
- `TASK_BRANCH = "feature-login"`
- `TARGET_BRANCH = "main"`
- `WORKTREE_DIR = ".worktree/feature-login"`

Example with no args `/planning-with-files:worktree`:
- `TASK_NAME = "task-2026-01-23-1430"` (includes time for uniqueness)
- `TASK_BRANCH = "task-2026-01-23-1430"`
- `TARGET_BRANCH = "main"` (detected)
- `WORKTREE_DIR = ".worktree/task-2026-01-23-1430"`

## Step 6: Check for Existing Worktree

```bash
if [ -d "$WORKTREE_DIR" ]; then
    echo "Worktree already exists: $WORKTREE_DIR"
    echo "This task is already in progress."
    echo "Navigate to: cd $WORKTREE_DIR"
    exit 0
fi
```

## Step 7: Create Git Worktree

Create the actual Git worktree:

```bash
# Check if branch already exists in another worktree
if git show-ref --verify --quiet refs/heads/"$TASK_BRANCH"; then
    echo "ERROR: Branch $TASK_BRANCH already exists in another worktree"
    exit 1
fi

# Create the worktree
git worktree add -b "$TASK_BRANCH" "$WORKTREE_DIR" "$TARGET_BRANCH"
echo "Created worktree: $WORKTREE_DIR"
```

**Important**: This uses `git worktree add` which creates a real separate working directory. The main directory remains unchanged and on its original branch.

## Step 8: Create Planning Configuration in Worktree

Save the worktree configuration **inside the worktree directory**:

```bash
cat > "$WORKTREE_DIR/.planning-config.json" << EOF
{
  "mode": "worktree",
  "task_name": "$TASK_NAME",
  "task_branch": "$TASK_BRANCH",
  "target_branch": "$TARGET_BRANCH",
  "worktree_dir": "$WORKTREE_DIR",
  "original_branch": "$ORIGINAL_BRANCH",
  "root_dir": "$ROOT_DIR",
  "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "planning_files": [
    "task_plan.md",
    "findings.md",
    "progress.md"
  ]
}
EOF
```

## Step 9: Create Planning Files in Worktree

Create the three planning files **inside the worktree directory**:

```bash
# Create task_plan.md in worktree
cat > "$WORKTREE_DIR/task_plan.md" << 'PLANEOF'
# Task Plan: [TASK_NAME]

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
- [ ] Complete task with: `/planning-with-files:complete`
- **Status:** pending

## Decisions Made
| Decision | Rationale |
|----------|-----------|

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|

## Worktree Info
- **Task Name:** [TASK_NAME]
- **Branch:** [TASK_BRANCH]
- **Target:** [TARGET_BRANCH]
- **Worktree:** [WORKTREE_DIR]
- **Complete with:** `/planning-with-files:complete`
PLANEOF

# Create findings.md and progress.md similarly in the worktree
```

## Step 10: List Active Worktrees

Show the user all active worktrees:

```bash
echo "=== Active Worktrees ==="
git worktree list
```

## Step 11: Display Summary and Instructions

```
=== Worktree Session Created ===

IMPORTANT: Navigate to the worktree to work on this task

cd [WORKTREE_DIR]

Once in the worktree directory:
  1. Edit task_plan.md to define your task phases
  2. Work on your task in this isolated environment
  3. Use /planning-with-files:complete when done

Multi-Task Usage:
You can create multiple worktrees for parallel tasks:
  /planning-with-files:worktree task-auth-fix
  /planning-with-files:worktree task-refactor
  /planning-with-files:worktree task-docs

Each task works in its own directory without conflicts.

To return to the main project:
  cd [ROOT_DIR]
```

## Directory Structure

After creating a worktree, your project structure looks like:

```
project/
├── .git/
├── .worktree/
│   ├── task-auth-fix/          ← Worktree 1
│   │   ├── .planning-config.json
│   │   ├── task_plan.md
│   │   ├── findings.md
│   │   ├── progress.md
│   │   └── (complete project copy)
│   └── task-api-refactor/       ← Worktree 2 (can exist simultaneously)
│       ├── .planning-config.json
│       ├── task_plan.md
│       ├── findings.md
│       ├── progress.md
│       └── (complete project copy)
├── src/
├── main.go
└── ...                          ← Main directory (unchanged)
```

## Key Differences from Branch Mode

| Feature | Branch Mode | Worktree Mode |
|---------|-------------|---------------|
| Directory | Same directory, switch branches | Separate directory per task |
| Parallel tasks | No (must switch branches) | Yes (multiple directories) |
| Isolation | Partial (same working files) | Complete (separate files) |
| Main branch | Changes when switching | Stays on original branch |
| Cleanup | Delete branch | Remove worktree + delete branch |

## Usage Example

```bash
# Start task 1
/planning-with-files:worktask fix-auth-bug
cd .worktree/fix-auth-bug
# ... work on auth bug ...

# In another terminal, start task 2 (parallel!)
/planning-with-files:worktree refactor-api
cd .worktree/refactor-api
# ... work on api refactor ...

# Complete task 1 (from its directory)
cd .worktree/fix-auth-bug
/planning-with-files:complete

# Complete task 2 (from its directory)
cd .worktree/refactor-api
/planning-with-files:complete
```

---

**Important Reminders**:
- Tell the user to `cd` into the worktree directory to work on the task
- The main directory remains on its original branch
- Multiple worktrees can exist simultaneously for parallel tasks
- When done, use `/planning-with-files:complete` from **inside** the worktree directory
