---
description: "Start a new task in an isolated Git worktree with Hybrid Ralph PRD mode. Creates worktree, branch, and initializes PRD for parallel story execution. Usage: /planning-with-files:hybrid-worktree <task-name> <target-branch> <task-description>"
---

# Hybrid Ralph + Worktree Mode

You are starting a task in **Git Worktree + Hybrid Ralph mode**. This combines isolated parallel development with PRD-based story execution.

## Step 1: Parse Parameters

Parse user arguments:
- **Task name**: First arg (or `task-YYYY-MM-DD-HHMM`)
- **Target branch**: Second arg (or auto-detect `main`/`master`)
- **Task description**: Remaining args or ask user

```bash
TASK_NAME="{{args|arg 1 or 'task-' + date + '-' + time}}"
TARGET_BRANCH="{{args|arg 2 or auto-detect}}"
TASK_DESC="{{args|args 3+ or ask user}}"
```

## Step 2: Verify Git Repository

```bash
git rev-parse --git-dir > /dev/null 2>&1 || { echo "ERROR: Not a git repository"; exit 1; }
```

## Step 3: Detect Default Branch

```bash
DEFAULT_BRANCH=$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@refs/remotes/origin/@@')
if [ -z "$DEFAULT_BRANCH" ]; then
    if git show-ref --verify --quiet refs/heads/main; then
        DEFAULT_BRANCH="main"
    elif git show-ref --verify --quiet refs/heads/master; then
        DEFAULT_BRANCH="master"
    else
        DEFAULT_BRANCH="main"
    fi
fi
TARGET_BRANCH="${TARGET_BRANCH:-$DEFAULT_BRANCH}"
```

## Step 4: Set Variables

```bash
TASK_BRANCH="$TASK_NAME"
WORKTREE_DIR=".worktree/$(basename $TASK_NAME)"
ORIGINAL_BRANCH=$(git branch --show-current)
ROOT_DIR=$(pwd)
```

## Step 5: Check for Existing Worktree

```bash
if [ -d "$WORKTREE_DIR" ]; then
    echo "Worktree already exists: $WORKTREE_DIR"
    echo "Navigate to: cd $WORKTREE_DIR"
    exit 0
fi
```

## Step 6: Create Git Worktree

```bash
if git show-ref --verify --quiet refs/heads/"$TASK_BRANCH"; then
    echo "ERROR: Branch $TASK_BRANCH already exists"
    exit 1
fi

git worktree add -b "$TASK_BRANCH" "$WORKTREE_DIR" "$TARGET_BRANCH"
echo "Created worktree: $WORKTREE_DIR"
```

## Step 7: Create Planning Configuration

```bash
cat > "$WORKTREE_DIR/.planning-config.json" << EOF
{
  "mode": "hybrid",
  "task_name": "$TASK_NAME",
  "task_branch": "$TASK_BRANCH",
  "target_branch": "$TARGET_BRANCH",
  "worktree_dir": "$WORKTREE_DIR",
  "original_branch": "$ORIGINAL_BRANCH",
  "root_dir": "$ROOT_DIR",
  "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
```

## Step 8: Create Initial PRD Structure in Worktree

```bash
cat > "$WORKTREE_DIR/prd.json" << 'PRDEOF'
{
  "metadata": {
    "created_at": null,
    "version": "1.0.0",
    "description": null
  },
  "goal": null,
  "objectives": [],
  "stories": []
}
PRDEOF

cat > "$WORKTREE_DIR/findings.md" << 'FINdingEOF'
# Findings

Research and discovery notes will be accumulated here.
FINdingEOF

cat > "$WORKTREE_DIR/progress.txt" << 'PROGEEOF'
# Progress Log

Story execution progress will be tracked here.
PROGEOF
```

## Step 9: Display Summary

```
=== Hybrid Ralph Worktree Created ===

Worktree: $WORKTREE_DIR
Branch: $TASK_BRANCH
Target: $TARGET_BRANCH

IMPORTANT: Navigate to the worktree:

  cd $WORKTREE_DIR

Then generate your PRD:

  /planning-with-files:hybrid-auto $TASK_DESC

Or load existing PRD:

  /planning-with-files:hybrid-manual path/to/prd.json

Multi-Task Usage:
  Multiple worktrees can run in parallel:
  - /planning-with-files:hybrid-worktree task-auth-fix main "fix auth bug"
  - /planning-with-files:hybrid-worktree task-refactor main "refactor API"

Each works in isolated directories with separate PRDs.
```

## Step 10: Show Active Worktrees

```bash
echo "=== Active Worktrees ==="
git worktree list
```

---

**Next Steps for User:**
1. `cd $WORKTREE_DIR`
2. Run `/planning-with-files:hybrid-auto <description>` to generate PRD
3. Or run `/planning-with-files:hybrid-manual <path>` to load existing PRD
4. Use `/planning-with-files:approve` to start parallel story execution
5. Use `/planning-with-files:hybrid-complete` when done
