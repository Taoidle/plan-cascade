---
description: "Resume an interrupted hybrid task (worktree or regular). Auto-detects state from files and continues execution. Usage: /plan-cascade:hybrid-resume [--auto]"
---

# Resume Interrupted Hybrid Task

Resume execution of an interrupted hybrid task by detecting current state from existing files.

**Works with both:**
- `hybrid-worktree` tasks (in `.worktree/` directories)
- `hybrid-auto` tasks (in regular directories)

## Arguments

- `--auto`: Continue in fully automatic mode (no confirmation prompts)

## Step 1: Detect Context

### 1.1: Check if in Worktree

```bash
if [ -f ".planning-config.json" ]; then
    MODE=$(jq -r '.mode // empty' .planning-config.json 2>/dev/null)
    if [ "$MODE" = "hybrid" ]; then
        CONTEXT="worktree"
        TASK_NAME=$(jq -r '.task_name' .planning-config.json)
        TARGET_BRANCH=$(jq -r '.target_branch' .planning-config.json)
        WORKTREE_DIR=$(pwd)
        echo "✓ Detected hybrid worktree: $TASK_NAME"
    fi
fi
```

### 1.2: Check for Regular Hybrid Task

```bash
if [ -z "$CONTEXT" ]; then
    if [ -f "prd.json" ]; then
        CONTEXT="regular"
        echo "✓ Detected regular hybrid task (prd.json found)"
    fi
fi
```

### 1.3: Scan for Worktrees (if not in one)

```bash
if [ -z "$CONTEXT" ]; then
    # Look for hybrid worktrees
    echo "Scanning for interrupted hybrid tasks..."

    HYBRID_TASKS=()

    # Check .worktree directory
    if [ -d ".worktree" ]; then
        for dir in .worktree/*/; do
            if [ -f "${dir}.planning-config.json" ]; then
                mode=$(jq -r '.mode // empty' "${dir}.planning-config.json" 2>/dev/null)
                if [ "$mode" = "hybrid" ]; then
                    task_name=$(jq -r '.task_name' "${dir}.planning-config.json" 2>/dev/null)
                    HYBRID_TASKS+=("$dir|$task_name|worktree")
                fi
            elif [ -f "${dir}prd.json" ]; then
                # Worktree with PRD but no config
                HYBRID_TASKS+=("$dir|$(basename $dir)|worktree-legacy")
            fi
        done
    fi

    if [ ${#HYBRID_TASKS[@]} -eq 0 ]; then
        echo ""
        echo "============================================"
        echo "No interrupted hybrid tasks found"
        echo "============================================"
        echo ""
        echo "To start a new task:"
        echo "  /plan-cascade:hybrid-auto <description>"
        echo "  /plan-cascade:hybrid-worktree <name> <branch> <description>"
        exit 0
    fi

    echo ""
    echo "Found ${#HYBRID_TASKS[@]} interrupted hybrid task(s):"
    echo ""

    for i in "${!HYBRID_TASKS[@]}"; do
        IFS='|' read -r path name type <<< "${HYBRID_TASKS[$i]}"
        echo "  [$((i+1))] $name"
        echo "      Path: $path"
        echo "      Type: $type"

        # Show state preview
        if [ -f "${path}prd.json" ]; then
            stories=$(jq '.stories | length' "${path}prd.json" 2>/dev/null || echo "?")
            echo "      PRD: $stories stories"
        else
            echo "      PRD: not generated"
        fi

        if [ -f "${path}progress.txt" ]; then
            complete=$(grep -c "\[COMPLETE\]" "${path}progress.txt" 2>/dev/null || echo "0")
            echo "      Progress: $complete stories complete"
        fi
        echo ""
    done

    read -p "Select task to resume [1-${#HYBRID_TASKS[@]}]: " selection

    if [ "$selection" -lt 1 ] || [ "$selection" -gt ${#HYBRID_TASKS[@]} ]; then
        echo "Invalid selection."
        exit 1
    fi

    selected="${HYBRID_TASKS[$((selection-1))]}"
    IFS='|' read -r SELECTED_PATH TASK_NAME TYPE <<< "$selected"

    cd "$SELECTED_PATH"
    CONTEXT="worktree"
    WORKTREE_DIR=$(pwd)
    echo ""
    echo "✓ Now in: $(pwd)"
fi
```

## Step 2: Analyze Current State

### 2.1: Check PRD Status

```bash
PRD_STATUS="missing"
TOTAL_STORIES=0

if [ -f "prd.json" ]; then
    # Validate JSON
    if python3 -m json.tool prd.json > /dev/null 2>&1 || jq '.' prd.json > /dev/null 2>&1; then
        TOTAL_STORIES=$(jq '.stories | length' prd.json 2>/dev/null || echo "0")

        if [ "$TOTAL_STORIES" -gt 0 ]; then
            PRD_STATUS="valid"
        else
            PRD_STATUS="empty"
        fi
    else
        PRD_STATUS="corrupted"
    fi
fi

echo "PRD Status: $PRD_STATUS"
if [ "$PRD_STATUS" = "valid" ]; then
    echo "  Stories: $TOTAL_STORIES"
fi
```

### 2.2: Check Progress Status

```bash
STORIES_COMPLETE=0
STORIES_FAILED=0
STORIES_IN_PROGRESS=0
CURRENT_BATCH=0

if [ -f "progress.txt" ]; then
    # Count completion markers (both old and new style)
    STORIES_COMPLETE=$(grep -cE "\[COMPLETE\]|\[STORY_COMPLETE\]" progress.txt 2>/dev/null || echo "0")
    STORIES_FAILED=$(grep -cE "\[FAILED\]|\[STORY_FAILED\]|\[ERROR\]" progress.txt 2>/dev/null || echo "0")

    # Try to detect current batch
    CURRENT_BATCH=$(grep -oE "Batch [0-9]+" progress.txt | tail -1 | grep -oE "[0-9]+" || echo "0")

    # Check execution mode
    EXECUTION_MODE=$(grep -oE "execution_mode: (auto|manual)" progress.txt | cut -d' ' -f2 || echo "auto")
fi

echo "Progress Status:"
echo "  Complete: $STORIES_COMPLETE"
echo "  Failed: $STORIES_FAILED"
echo "  Current Batch: $CURRENT_BATCH"
```

### 2.3: Check Story Statuses in PRD

```bash
if [ "$PRD_STATUS" = "valid" ]; then
    # Count stories by status in prd.json
    PENDING_IN_PRD=$(jq '[.stories[] | select(.status == "pending" or .status == null)] | length' prd.json 2>/dev/null || echo "0")
    COMPLETE_IN_PRD=$(jq '[.stories[] | select(.status == "complete")] | length' prd.json 2>/dev/null || echo "0")
    IN_PROGRESS_IN_PRD=$(jq '[.stories[] | select(.status == "in_progress")] | length' prd.json 2>/dev/null || echo "0")

    echo "PRD Story Statuses:"
    echo "  Pending: $PENDING_IN_PRD"
    echo "  In Progress: $IN_PROGRESS_IN_PRD"
    echo "  Complete: $COMPLETE_IN_PRD"
fi
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

Launch with `run_in_background: true` for parallel execution.

### 6.4: Monitor Execution

Same monitoring loop as approve.md:

```
while true:
    # Count completions
    complete_count = count("[COMPLETE]" in progress.txt)

    # Check for errors
    error_count = count("[ERROR]" or "[FAILED]" in progress.txt)

    if error_count > 0:
        echo "⚠️ ERRORS DETECTED"
        echo "Review progress.txt and fix issues"
        if not AUTO_MODE:
            exit 1
        # With AUTO_MODE, continue anyway

    # Check if current batch complete
    if all ready_stories complete:
        # Recalculate next batch
        # Some blocked stories may now be ready
        recalculate_ready_stories()

        if no more stories:
            break  # All done!

        # Launch next batch
        launch_ready_stories()

    sleep 10 seconds
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
