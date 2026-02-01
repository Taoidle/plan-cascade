---
description: "Resume an interrupted hybrid task (worktree or regular). Auto-detects state from files and continues execution. Usage: /plan-cascade:hybrid-resume [--auto]"
---

# Resume Interrupted Hybrid Task

Resume execution of an interrupted hybrid task by detecting current state from existing files.

## Path Storage Modes

This command works with both new and legacy path storage modes:

### New Mode (Default)
- Worktrees in: `~/.plan-cascade/<project-id>/.worktree/` (Unix) or `%APPDATA%/plan-cascade/<project-id>/.worktree/` (Windows)
- State files in: `~/.plan-cascade/<project-id>/.state/`
- PRD files in worktree directory

### Legacy Mode
- Worktrees in: `<project-root>/.worktree/`
- State/PRD files in project root or worktree

The command auto-detects which mode is active and scans the appropriate directories.

## Tool Usage Policy (CRITICAL)

**To avoid command confirmation prompts during automatic execution:**

1. **Use Read tool for file reading** - NEVER use `cat` via Bash
   - ✅ `Read("prd.json")`, `Read("progress.txt")`, `Read(".planning-config.json")`
   - ❌ `Bash("cat prd.json")`

2. **Use Glob tool for finding files** - NEVER use `ls` or `find` via Bash
   - ✅ `Glob(".worktree/*/.planning-config.json")`
   - ❌ `Bash("ls .worktree/")`

3. **Use Grep tool for content search** - NEVER use `grep` via Bash
   - ✅ `Grep("[COMPLETE]", path="progress.txt")`
   - ❌ `Bash("grep '[COMPLETE]' progress.txt")`

4. **Only use Bash for actual system commands:**
   - Git operations: `git status`, `git checkout`
   - Directory navigation: `cd` (only when necessary)
   - File writing: `echo "..." >> progress.txt`

**Works with both:**
- `hybrid-worktree` tasks (in `.worktree/` directories)
- `hybrid-auto` tasks (in regular directories)

## Arguments

- `--auto`: Continue in fully automatic mode (no confirmation prompts)

## Step 1: Detect Context

**IMPORTANT: Use Read/Glob tools for file detection, NOT Bash**

### 1.1: Check if in Worktree

**Use Read tool (NOT Bash) to check for worktree config:**

```
Read(".planning-config.json")
```

If Read succeeds:
- Parse the JSON content
- Check if `mode` equals "hybrid"
- If yes: `CONTEXT="worktree"`, extract `task_name`, `target_branch`
- Log "✓ Detected hybrid worktree: {task_name}"

### 1.2: Check for Regular Hybrid Task

If CONTEXT is not set yet:

**Use Read tool to check for PRD:**

```
Read("prd.json")
```

If Read succeeds:
- `CONTEXT="regular"`
- Log "✓ Detected regular hybrid task (prd.json found)"

### 1.3: Scan for Worktrees (if not in one)

If CONTEXT is still not set:

**Use Glob tool (NOT Bash `ls`) to find worktrees in both new and legacy locations:**

```
# Get worktree base directory from PathResolver
WORKTREE_BASE = python3 -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_worktree_dir())"

# Scan for worktrees (path depends on mode)
Glob("{WORKTREE_BASE}/*/.planning-config.json")
Glob("{WORKTREE_BASE}/*/prd.json")

# Also check legacy location if different
Glob(".worktree/*/.planning-config.json")
Glob(".worktree/*/prd.json")
```

Execute these Glob calls in parallel.

For each found file:
- **Use Read tool** to read the config/prd content
- Parse to get task info (mode, task_name, etc.)
- Build list of HYBRID_TASKS

If no tasks found:
```
============================================
No interrupted hybrid tasks found
============================================

To start a new task:
  /plan-cascade:hybrid-auto <description>
  /plan-cascade:hybrid-worktree <name> <branch> <description>
```

If tasks found, display them and use **AskUserQuestion** to let user select:

```
Found {count} interrupted hybrid task(s):

  [1] {task_name_1}
      Path: {path_1}
      PRD: {story_count} stories
      Progress: {complete_count} complete

  [2] {task_name_2}
      ...
```

After user selection, navigate to the selected worktree directory.

## Step 2: Analyze Current State

**IMPORTANT: Use Read/Grep tools for state analysis, NOT Bash**

### 2.1: Check PRD Status

**Use Read tool to get PRD content:**

```
Read("prd.json")
```

Based on result:
- If Read fails → `PRD_STATUS="missing"`
- If Read succeeds:
  - Parse JSON in the response
  - If JSON invalid → `PRD_STATUS="corrupted"`
  - If `stories` array is empty → `PRD_STATUS="empty"`
  - If `stories` array has items → `PRD_STATUS="valid"`, `TOTAL_STORIES=len(stories)`

Display: `PRD Status: {PRD_STATUS}` and `Stories: {TOTAL_STORIES}` if valid

### 2.2: Check Progress Status

**Use Read tool to get progress content:**

```
Read("progress.txt")
```

**Use Grep tool to count markers (NOT Bash grep):**

```
Grep("[COMPLETE]", path="progress.txt", output_mode="count")
Grep("[FAILED]", path="progress.txt", output_mode="count")
Grep("Batch [0-9]+", path="progress.txt", output_mode="content")
```

From results:
- `STORIES_COMPLETE` = count of [COMPLETE] or [STORY_COMPLETE] markers
- `STORIES_FAILED` = count of [FAILED] or [ERROR] markers
- `CURRENT_BATCH` = extract batch number from last "Batch N" line

Display progress status summary.

### 2.3: Check Story Statuses in PRD

If PRD is valid (from 2.1), analyze the prd.json content that was already read:
- Count stories with `status == "pending"` or no status
- Count stories with `status == "complete"`
- Count stories with `status == "in_progress"`

Display:
```
PRD Story Statuses:
  Pending: {PENDING_IN_PRD}
  In Progress: {IN_PROGRESS_IN_PRD}
  Complete: {COMPLETE_IN_PRD}
```

## Step 3: Determine Task State

Based on the analysis, determine the current state:

```
State Detection Logic:

1. NO prd.json OR corrupted:
   → State: "needs_prd"
   → Action: Generate PRD first

2. prd.json empty (no stories):
   → State: "needs_prd"
   → Action: Regenerate PRD

3. prd.json valid, 0 stories complete, no progress markers:
   → State: "needs_approval"
   → Action: Start story execution from beginning

4. prd.json valid, some stories complete (in PRD or progress.txt):
   → State: "executing"
   → Action: Resume from incomplete stories

5. All stories complete (TOTAL == COMPLETE):
   → State: "complete"
   → Action: Ready for completion (hybrid-complete)

COMPATIBILITY:
- Check both progress.txt markers AND prd.json status fields
- Use the higher of the two counts as "complete"
```

```bash
# Determine effective completion
EFFECTIVE_COMPLETE=$STORIES_COMPLETE
if [ "$COMPLETE_IN_PRD" -gt "$EFFECTIVE_COMPLETE" ]; then
    EFFECTIVE_COMPLETE=$COMPLETE_IN_PRD
fi

# Determine state
if [ "$PRD_STATUS" = "missing" ] || [ "$PRD_STATUS" = "corrupted" ] || [ "$PRD_STATUS" = "empty" ]; then
    TASK_STATE="needs_prd"
elif [ "$EFFECTIVE_COMPLETE" -eq 0 ] && [ "$STORIES_FAILED" -eq 0 ]; then
    TASK_STATE="needs_approval"
elif [ "$EFFECTIVE_COMPLETE" -ge "$TOTAL_STORIES" ]; then
    TASK_STATE="complete"
else
    TASK_STATE="executing"
fi

echo ""
echo "Detected State: $TASK_STATE"
```

## Step 4: Display State Summary

```
============================================================
HYBRID TASK RESUME - STATE DETECTION
============================================================

Context: {worktree | regular}
Task: {task_name if worktree}
Directory: {current directory}

PRD Status: {missing | corrupted | empty | valid}
  Stories: {count if valid}

Design Document: {present | not found}
  {If present: "✓ Architectural context will guide execution"}

Progress:
  Complete: {count} / {total}
  Failed: {count}
  Current Batch: {number}

Detected State: {state}

============================================================
RESUME PLAN
============================================================

{Based on state, show what will happen}

============================================================
```

## Step 5: Execute Resume Based on State

### State: needs_prd

```bash
if [ "$TASK_STATE" = "needs_prd" ]; then
    echo ""
    echo "============================================"
    echo "PRD GENERATION REQUIRED"
    echo "============================================"
    echo ""

    # Check if we have task description in config
    if [ -f ".planning-config.json" ]; then
        TASK_DESC=$(jq -r '.task_description // empty' .planning-config.json 2>/dev/null)
    fi

    if [ -z "$TASK_DESC" ]; then
        echo "No task description found."
        echo ""
        echo "Options:"
        echo "  1) Enter task description now"
        echo "  2) Provide PRD file path"
        echo "  3) Cancel"
        echo ""
        read -p "Choice [1/2/3]: " prd_choice

        case "$prd_choice" in
            1)
                read -p "Task description: " TASK_DESC
                ;;
            2)
                read -p "PRD file path: " PRD_PATH
                if [ -f "$PRD_PATH" ]; then
                    cp "$PRD_PATH" prd.json
                    echo "✓ PRD loaded from $PRD_PATH"
                    TASK_STATE="needs_approval"
                else
                    echo "File not found: $PRD_PATH"
                    exit 1
                fi
                ;;
            *)
                echo "Cancelled."
                exit 0
                ;;
        esac
    fi

    if [ "$TASK_STATE" = "needs_prd" ] && [ -n "$TASK_DESC" ]; then
        echo ""
        echo "Generating PRD..."
        echo ""

        # Launch PRD generation task (same as hybrid-auto)
        # Use Task tool with PRD generation prompt
        # ... (PRD generation agent prompt from hybrid-auto)

        echo ""
        echo "After PRD generation, run /plan-cascade:hybrid-resume again"
        echo "Or proceed to /plan-cascade:approve"
    fi
fi
```

### State: needs_approval

```bash
if [ "$TASK_STATE" = "needs_approval" ]; then
    echo ""
    echo "============================================"
    echo "PRD READY - STARTING EXECUTION"
    echo "============================================"
    echo ""

    # Display PRD summary
    echo "Goal: $(jq -r '.goal' prd.json)"
    echo "Stories: $TOTAL_STORIES"
    echo ""

    # Show stories
    echo "Stories to execute:"
    jq -r '.stories[] | "  - \(.id): \(.title)"' prd.json
    echo ""

    if [ "$AUTO_MODE" != "true" ]; then
        read -p "Start execution? [Y/n]: " confirm
        if [[ ! "$confirm" =~ ^[Yy]?$ ]]; then
            echo "Cancelled. Run /plan-cascade:approve when ready."
            exit 0
        fi
    fi

    echo ""
    echo "Launching story execution..."
    echo ""

    # Proceed to story execution (same as approve.md)
    # ... continue to Step 6
fi
```

### State: executing

```bash
if [ "$TASK_STATE" = "executing" ]; then
    echo ""
    echo "============================================"
    echo "RESUMING STORY EXECUTION"
    echo "============================================"
    echo ""
    echo "Progress: $EFFECTIVE_COMPLETE / $TOTAL_STORIES stories complete"
    echo ""

    # Identify incomplete stories
    echo "Incomplete stories:"

    # Build list of completed story IDs from both sources
    COMPLETED_IDS=()

    # From progress.txt markers
    if [ -f "progress.txt" ]; then
        while IFS= read -r line; do
            # Match [COMPLETE] story-xxx or [STORY_COMPLETE] story-xxx
            if [[ "$line" =~ \[(COMPLETE|STORY_COMPLETE)\].*([a-z]+-[0-9]+) ]]; then
                COMPLETED_IDS+=("${BASH_REMATCH[2]}")
            fi
        done < progress.txt
    fi

    # From prd.json status
    COMPLETED_FROM_PRD=$(jq -r '.stories[] | select(.status == "complete") | .id' prd.json 2>/dev/null)
    for id in $COMPLETED_FROM_PRD; do
        COMPLETED_IDS+=("$id")
    done

    # Remove duplicates
    COMPLETED_IDS=($(echo "${COMPLETED_IDS[@]}" | tr ' ' '\n' | sort -u | tr '\n' ' '))

    # Show incomplete stories
    jq -r --argjson completed "$(printf '%s\n' "${COMPLETED_IDS[@]}" | jq -R . | jq -s .)" \
        '.stories[] | select(.id as $id | $completed | index($id) | not) | "  - \(.id): \(.title) [\(.status // "pending")]"' \
        prd.json

    echo ""

    if [ "$AUTO_MODE" != "true" ]; then
        read -p "Resume execution? [Y/n]: " confirm
        if [[ ! "$confirm" =~ ^[Yy]?$ ]]; then
            echo "Cancelled."
            exit 0
        fi
    fi

    # Continue to Step 6 with resume context
fi
```

### State: complete

```bash
if [ "$TASK_STATE" = "complete" ]; then
    echo ""
    echo "============================================"
    echo "ALL STORIES COMPLETE"
    echo "============================================"
    echo ""
    echo "All $TOTAL_STORIES stories have been completed!"
    echo ""

    if [ "$CONTEXT" = "worktree" ]; then
        echo "Next step: Complete and merge the task"
        echo ""
        echo "  /plan-cascade:hybrid-complete"
        echo ""
        echo "This will:"
        echo "  - Commit code changes"
        echo "  - Merge to $TARGET_BRANCH"
        echo "  - Remove worktree"
    else
        echo "Task execution complete."
        echo ""
        echo "Review changes:"
        echo "  git status"
        echo "  git diff"
    fi

    exit 0
fi
```

## Step 6: Resume/Start Story Execution

For states `needs_approval` or `executing`:

### 6.1: Calculate Remaining Work

```bash
# Get all stories
ALL_STORIES=$(jq -r '.stories[].id' prd.json)

# Build completed set
COMPLETED_SET="${COMPLETED_IDS[*]}"

# Filter to incomplete
INCOMPLETE_STORIES=()
for story_id in $ALL_STORIES; do
    if [[ ! " $COMPLETED_SET " =~ " $story_id " ]]; then
        INCOMPLETE_STORIES+=("$story_id")
    fi
done

echo "Stories to execute: ${#INCOMPLETE_STORIES[@]}"
```

### 6.2: Calculate Batches for Remaining Stories

```bash
# Recalculate batches considering completed stories as "done"
# Stories depending only on completed stories can start

# For each incomplete story, check if dependencies are all complete
READY_STORIES=()
BLOCKED_STORIES=()

for story_id in "${INCOMPLETE_STORIES[@]}"; do
    deps=$(jq -r --arg id "$story_id" '.stories[] | select(.id == $id) | .dependencies[]?' prd.json)

    all_deps_complete=true
    for dep in $deps; do
        if [[ ! " $COMPLETED_SET " =~ " $dep " ]]; then
            all_deps_complete=false
            break
        fi
    done

    if [ "$all_deps_complete" = true ]; then
        READY_STORIES+=("$story_id")
    else
        BLOCKED_STORIES+=("$story_id")
    fi
done

echo ""
echo "Ready to execute (no blocking dependencies): ${#READY_STORIES[@]}"
echo "Blocked (waiting on dependencies): ${#BLOCKED_STORIES[@]}"
```

### 6.3: Launch Story Agents

For each ready story, launch a Task agent:

```
You are RESUMING execution for story: {story_id}

Story Details:
  Title: {title}
  Description: {description}

Acceptance Criteria:
{acceptance_criteria}

Dependencies: {dependencies} (all complete)

RESUME CONTEXT:
- This task was interrupted and is being resumed
- Check if any partial work exists for this story
- Do NOT redo work that appears complete

Your task:
1. Implement the story according to acceptance criteria
2. Test your implementation
3. Update findings.md with discoveries (use <!-- @tags: {story_id} -->)
4. When complete:
   a. Update story status to "complete" in prd.json (using jq or manual edit)
   b. Append to progress.txt: [COMPLETE] {story_id}

Execute bash/powershell commands directly. Work methodically.
```

Launch with `run_in_background: true` for parallel execution. Store all task_ids.

### 6.4: Wait for Agents Using TaskOutput

**CRITICAL**: Use TaskOutput to wait instead of polling. This avoids Bash confirmation prompts.

```
For each story_id, task_id in current_batch_tasks:
    echo "Waiting for {story_id}..."

    result = TaskOutput(
        task_id=task_id,
        block=true,
        timeout=600000  # 10 minutes per story
    )

    echo "✓ {story_id} agent completed"
```

### 6.5: Verify and Continue

After all TaskOutput calls return, verify using Read tool (NOT Bash):

```
# Use Read tool
progress_content = Read("progress.txt")

# Count markers yourself in your response
complete_count = count "[COMPLETE]" occurrences
error_count = count "[ERROR]" or "[FAILED]" occurrences

if error_count > 0:
    echo "⚠️ ERRORS DETECTED"
    # Show error details from progress_content
    if not AUTO_MODE:
        pause for review

# Check if more stories are now unblocked
# Recalculate ready stories based on completed dependencies
# If more ready stories exist, launch them and repeat TaskOutput wait
```

### 6.5: Handle Batch Progression

```bash
# After each batch completes
echo ""
echo "=========================================="
echo "Batch $CURRENT_BATCH complete!"
echo "=========================================="
echo ""
echo "Progress: $COMPLETE_COUNT / $TOTAL_STORIES"

# Check if more stories remain
REMAINING=$((TOTAL_STORIES - COMPLETE_COUNT))

if [ "$REMAINING" -gt 0 ]; then
    echo ""
    echo "Remaining stories: $REMAINING"
    echo "Calculating next batch..."

    # Recalculate ready stories (blocked stories whose deps are now complete)
    # Launch next batch

    if [ "$AUTO_MODE" != "true" ]; then
        read -p "Continue to next batch? [Y/n]: " continue_confirm
        if [[ ! "$continue_confirm" =~ ^[Yy]?$ ]]; then
            echo ""
            echo "Paused. Run /plan-cascade:hybrid-resume to continue."
            exit 0
        fi
    fi
fi
```

## Step 6.6: Update Execution Context File

After resuming execution, update the context file:

```bash
# Update .hybrid-execution-context.md for future recovery
python3 "${CLAUDE_PLUGIN_ROOT}/skills/hybrid-ralph/scripts/hybrid-context-reminder.py" update
```

This ensures the context file reflects the current resumed state.

## Step 7: Completion

When all stories are complete:

```
============================================================
ALL STORIES COMPLETE
============================================================

Total Stories: {total}
Completed: {complete}
Failed: {failed}

Execution time: {duration}

============================================================
NEXT STEPS
============================================================

{If worktree context:}
  Complete and merge the task:
    /plan-cascade:hybrid-complete

{If regular context:}
  Review your changes:
    git status
    git diff

  Commit when ready:
    git add .
    git commit -m "Complete: {task description}"

============================================================
```

## Compatibility

### Old-Style Progress Markers

| Old Marker | New Marker | Both Recognized |
|------------|------------|-----------------|
| `[COMPLETE] story-xxx` | `[STORY_COMPLETE] story-xxx` | ✅ |
| `[ERROR] story-xxx` | `[STORY_FAILED] story-xxx` | ✅ |
| `[FAILED] story-xxx` | `[STORY_FAILED] story-xxx` | ✅ |

### PRD Status Field

Stories can be marked complete either by:
1. Progress marker in `progress.txt`
2. `status: "complete"` in `prd.json`

Resume checks BOTH and uses the union.

### Missing .planning-config.json

If worktree exists but no config:
- Detect as "worktree-legacy"
- Use directory name as task name
- Try to infer target branch from git

## Error Recovery

### Corrupted prd.json

```
============================================
ERROR: Corrupted PRD
============================================

The prd.json file is not valid JSON.

Options:
  1) Delete and regenerate
  2) Edit manually to fix
  3) Provide new PRD file

============================================
```

### Stories Failed

```
============================================
WARNING: Failed Stories Detected
============================================

The following stories failed:
  - story-003: {error message from progress.txt}

Options:
  1) Retry failed stories
  2) Skip failed stories and continue
  3) Cancel and fix manually

============================================
```

## Usage Examples

```bash
# Resume with auto mode (no prompts)
/plan-cascade:hybrid-resume --auto

# Resume interactively
/plan-cascade:hybrid-resume

# Resume from specific worktree (if multiple exist)
cd .worktree/my-task
/plan-cascade:hybrid-resume
```

## Flow Summary

```
/plan-cascade:hybrid-resume [--auto]
    │
    ├─→ Detect context (worktree or regular)
    │   └─→ If neither, scan for interrupted tasks
    │
    ├─→ Analyze current state
    │   ├─→ Check prd.json (exists? valid? has stories?)
    │   ├─→ Check progress.txt (markers? batch number?)
    │   └─→ Check story statuses in prd.json
    │
    ├─→ Determine task state
    │   ├─→ needs_prd → Generate PRD first
    │   ├─→ needs_approval → Start execution
    │   ├─→ executing → Resume from incomplete
    │   └─→ complete → Suggest hybrid-complete
    │
    ├─→ Execute based on state
    │   ├─→ Calculate remaining work
    │   ├─→ Launch Task agents for ready stories
    │   ├─→ Monitor until batch complete
    │   ├─→ Progress to next batch
    │   └─→ Repeat until all complete
    │
    └─→ Show completion status
```
