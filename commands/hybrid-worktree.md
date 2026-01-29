---
description: "Start a new task in an isolated Git worktree with Hybrid Ralph PRD mode. Creates worktree, branch, loads existing PRD or auto-generates from description. Usage: /plan-cascade:hybrid-worktree <task-name> <target-branch> <prd-path-or-description>"
---

# Hybrid Ralph + Worktree Mode (Fully Automated)

You are starting a task in **Git Worktree + Hybrid Ralph mode**. This will create the worktree and handle the PRD automatically.

## Step 1: Parse Parameters

Parse user arguments:
- **Task name**: First arg (or `task-YYYY-MM-DD-HHMM`)
- **Target branch**: Second arg (or auto-detect `main`/`master`)
- **PRD path OR description**: Third arg
  - If it's an existing file path → Load that PRD
  - Otherwise → Use as task description to auto-generate PRD

```bash
TASK_NAME="{{args|arg 1 or 'task-' + date + '-' + time}}"
TARGET_BRANCH="{{args|arg 2 or auto-detect}}"
PRD_ARG="{{args|arg 3 or ask user 'Provide PRD file path or task description'}}"
```

## Step 2: Detect Operating System and Shell

Detect the current operating system to use appropriate commands:

```bash
# Detect OS
OS_TYPE="$(uname -s 2>/dev/null || echo Windows)"
case "$OS_TYPE" in
    Linux*|Darwin*|MINGW*|MSYS*)
        SHELL_TYPE="bash"
        echo "✓ Detected Unix-like environment (bash)"
        ;;
    *)
        # Check if PowerShell is available on Windows
        if command -v pwsh >/dev/null 2>&1 || command -v powershell >/dev/null 2>&1; then
            SHELL_TYPE="powershell"
            echo "✓ Detected Windows environment (PowerShell)"
        else
            SHELL_TYPE="bash"
            echo "✓ Using bash (default)"
        fi
        ;;
esac
```

**Important**: Throughout this command, use:
- **Bash syntax** when `SHELL_TYPE=bash`
- **PowerShell syntax** when `SHELL_TYPE=powershell`

For PowerShell equivalents:
- `$(command)` → `$()`
- `VAR=value` → `$VAR = value`
- `if [ ]` → `if ()`
- `echo` → `Write-Host`

## Step 3: Ensure Auto-Approval Configuration

Ensure command auto-approval settings are configured (merges with existing settings):

```bash
# Run the settings merge script
python3 scripts/ensure-settings.py || echo "Warning: Could not update settings, continuing..."
```

This script intelligently merges required auto-approval patterns with any existing `.claude/settings.local.json`, preserving user customizations.

## Step 4: Verify Git Repository

```bash
git rev-parse --git-dir > /dev/null 2>&1 || { echo "ERROR: Not a git repository"; exit 1; }
```

## Step 5: Detect Default Branch

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

## Step 6: Set Variables

```bash
TASK_BRANCH="$TASK_NAME"
ORIGINAL_BRANCH=$(git branch --show-current)
ROOT_DIR=$(pwd)
WORKTREE_DIR="$ROOT_DIR/.worktree/$(basename $TASK_NAME)"
```

## Step 7: Determine PRD Mode

Check if PRD_ARG is an existing file:

```bash
if [ -f "$PRD_ARG" ]; then
    # User provided an existing PRD file
    PRD_PATH="$PRD_ARG"
    PRD_MODE="load"
    echo "Loading PRD from: $PRD_PATH"
else
    # User provided a task description
    TASK_DESC="$PRD_ARG"
    PRD_MODE="generate"
    echo "Will generate PRD from description"
fi
```

## Step 8: Check for Existing Worktree

```bash
if [ -d "$WORKTREE_DIR" ]; then
    echo "Worktree already exists: $WORKTREE_DIR"
    echo "Navigating to existing worktree..."
    cd "$WORKTREE_DIR"
    # Continue to PRD handling for existing worktree
else
    ## Step 9: Create Git Worktree (only if new)

    if git show-ref --verify --quiet refs/heads/"$TASK_BRANCH"; then
        echo "ERROR: Branch $TASK_BRANCH already exists"
        exit 1
    fi

    git worktree add -b "$TASK_BRANCH" "$WORKTREE_DIR" "$TARGET_BRANCH"
    echo "Created worktree: $WORKTREE_DIR"

    ## Step 10: Create Planning Configuration

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

    ## Step 11: Create Initial Files in Worktree

    cat > "$WORKTREE_DIR/findings.md" << 'EOF'
# Findings

Research and discovery notes will be accumulated here.

Use <!-- @tags: story-id --> to tag sections for specific stories.
EOF

    cat > "$WORKTREE_DIR/progress.txt" << 'EOF'
# Progress Log

Story execution progress will be tracked here.
EOF
fi
```

## Step 12: Navigate to Worktree

```bash
cd "$WORKTREE_DIR"
echo "Now working in: $(pwd)"
```

## Step 13: Handle PRD (Load or Generate)

### If PRD_MODE is "load" (user provided PRD file):

```bash
if [ "$PRD_MODE" = "load" ]; then
    # Copy PRD file to worktree
    cp "$PRD_PATH" prd.json
    echo "Loaded PRD from: $PRD_PATH"

    # Validate PRD
    if ! python3 -m json.tool prd.json > /dev/null 2>&1; then
        echo "ERROR: Invalid JSON in PRD file"
        exit 1
    fi

    PRD_SOURCE="Loaded from file: $PRD_PATH"
fi
```

### If PRD_MODE is "generate" (auto-generate from description):

Use the Task tool to automatically generate the PRD:

```
You are a PRD generation specialist. Your task is to:

1. ANALYZE the task description: "$TASK_DESC"
2. EXPLORE the codebase in the current directory to understand:
   - Existing patterns and conventions
   - Relevant code files
   - Architecture and structure
3. GENERATE a PRD (prd.json) with:
   - Clear goal statement
   - 3-7 user stories
   - Each story with: id, title, description, priority (high/medium/low), dependencies, acceptance_criteria, context_estimate (small/medium/large), tags
   - Dependencies between stories (where one story must complete before another)
4. SAVE the PRD to prd.json in the current directory

The PRD format must be:
{
  "metadata": {
    "created_at": "ISO-8601 timestamp",
    "version": "1.0.0",
    "description": "Task description"
  },
  "goal": "One sentence goal",
  "objectives": ["obj1", "obj2"],
  "stories": [
    {
      "id": "story-001",
      "title": "Story title",
      "description": "Detailed description",
      "priority": "high",
      "dependencies": [],
      "status": "pending",
      "acceptance_criteria": ["criterion1", "criterion2"],
      "context_estimate": "medium",
      "tags": ["feature", "api"]
    }
  ]
}

Work methodically and create a well-structured PRD.
```

Launch this as a background task with `run_in_background: true`:

```
IMPORTANT: After launching the background task, you MUST use the TaskOutput tool to wait for completion:

1. Launch the Task tool with run_in_background: true
2. Store the returned task_id
3. Immediately call TaskOutput with:
   - task_id: <the task_id from step 2>
   - block: true (wait for completion)
   - timeout: 600000 (10 minutes)

Example pattern:
```
Launch Task tool with run_in_background: true → Get task_id → TaskOutput(task_id, block=true)
```

DO NOT use sleep loops or polling. The TaskOutput tool with block=true will properly wait for the agent to complete.

After TaskOutput returns, the prd.json file will be ready. Continue to Step 12.

```bash
PRD_SOURCE="Auto-generated from description"
fi
```

## Step 14: Validate and Display PRD

After PRD is loaded or generated:

1. Read the `prd.json` file
2. Validate the structure (check for required fields)
3. Display a comprehensive PRD review showing:
   - Goal and objectives
   - All stories with IDs, titles, priorities
   - Dependency graph (ASCII)
   - Execution batches
   - Acceptance criteria for each story

## Step 15: Show Final Summary

```
============================================================
Hybrid Ralph Worktree Ready
============================================================

Worktree: $WORKTREE_DIR
Branch: $TASK_BRANCH
Target: $TARGET_BRANCH

✓ PRD Ready: $PRD_SOURCE

Stories: {count}
Batches: {batch_count}

## Execution Plan

{Show batches and story details}

============================================================

NEXT STEPS:

1. Review the PRD above
2. Edit if needed: /plan-cascade:edit
3. Approve to start execution: /plan-cascade:approve

When complete:
  /plan-cascade:hybrid-complete

To return to main project:
  cd $ROOT_DIR

Active Worktrees:
{Show git worktree list}
```

---

## Usage Examples

```bash
# Auto-generate PRD from description
/plan-cascade:hybrid-worktree fix-auth main "Fix authentication bug in login flow"

# Load existing PRD file
/plan-cascade:hybrid-worktree fix-auth main ./my-prd.json

# Load PRD from different location
/plan-cascade:hybrid-worktree fix-auth main ../prd-files/api-refactor.json
```

## Notes

- **File path mode**: If the third argument is an existing file, it's loaded as PRD
- **Description mode**: If the third argument is not a file, it's used to auto-generate PRD
- The entire process is automated: worktree creation → PRD loading/generation → review
- You can edit the PRD before approving: `/plan-cascade:edit`
- Multiple worktrees can run in parallel for different tasks

## Recovery

If execution is interrupted at any point:

```bash
# Resume from where it left off
/plan-cascade:hybrid-resume --auto
```

This will:
- Auto-detect current state from files
- Skip already-completed work
- Continue execution from incomplete stories
