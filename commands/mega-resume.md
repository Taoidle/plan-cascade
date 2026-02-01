---
description: "Resume an interrupted mega-plan execution. Detects current state from existing files and continues automatically. Usage: /plan-cascade:mega-resume [--auto-prd]"
---

# Resume Interrupted Mega Plan

Resume execution of an interrupted mega-plan by detecting the current state from existing files.

## Path Storage Modes

This command works with both new and legacy path storage modes:

### New Mode (Default)
Files are stored in user data directory:
- **Windows**: `%APPDATA%/plan-cascade/<project-id>/`
- **Unix/macOS**: `~/.plan-cascade/<project-id>/`

File locations:
- `mega-plan.json`: `<user-dir>/mega-plan.json`
- `.mega-status.json`: `<user-dir>/.state/.mega-status.json`
- Worktrees: `<user-dir>/.worktree/<feature-name>/`

### Legacy Mode
All files in project root:
- `mega-plan.json`: `<project-root>/mega-plan.json`
- `.mega-status.json`: `<project-root>/.mega-status.json`
- Worktrees: `<project-root>/.worktree/<feature-name>/`

The command auto-detects which mode is active and scans the appropriate directories.

## Tool Usage Policy (CRITICAL)

**To avoid command confirmation prompts during automatic execution:**

1. **Use Read tool for file reading** - NEVER use `cat` via Bash
   - ✅ `Read("mega-plan.json")`, `Read(".mega-status.json")`, `Read(".worktree/x/progress.txt")`
   - ❌ `Bash("cat mega-plan.json")`

2. **Use Glob tool for finding files** - NEVER use `ls` or `find` via Bash
   - ✅ `Glob(".worktree/*/prd.json")`
   - ❌ `Bash("ls .worktree/")`

3. **Use Grep tool for content search** - NEVER use `grep` via Bash
   - ✅ `Grep("[PRD_COMPLETE]", path=".worktree/x/progress.txt")`
   - ❌ `Bash("grep '[PRD_COMPLETE]' ...")`

4. **Only use Bash for actual system commands:**
   - Git operations: `git worktree add`, `git merge`
   - Directory creation: `mkdir -p`
   - File writing: `echo "..." >> progress.txt`

**Compatibility**: Works with both old-style (pre-4.1.1) and new-style mega-plan executions.

## Arguments

- `--auto-prd`: Continue in fully automatic mode (no manual intervention)

## Step 1: Verify Mega Plan Exists

```bash
# Get mega-plan path from PathResolver
MEGA_PLAN_PATH=$(python3 -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_mega_plan_path())" 2>/dev/null || echo "mega-plan.json")

if [ ! -f "$MEGA_PLAN_PATH" ]; then
    echo "============================================"
    echo "ERROR: No mega-plan.json found"
    echo "============================================"
    echo "Searched at: $MEGA_PLAN_PATH"
    echo "Nothing to resume."
    echo "Use /plan-cascade:mega-plan <description> to create a new plan."
    exit 1
fi
```

## Step 2: Detect Current State

Read all available state information:

### 2.1: Read Mega Plan

```bash
cat mega-plan.json
```

Extract:
- `goal`: Project goal
- `target_branch`: Target branch for merging
- `features[]`: All features with their dependencies
- `execution_mode`: auto or manual

### 2.2: Read Status File (if exists)

```bash
cat .mega-status.json 2>/dev/null || echo "{}"
```

Extract:
- `current_batch`: Current batch number (0 = not started)
- `completed_batches[]`: List of completed batch numbers
- `features{}`: Feature status map

### 2.3: Scan Worktrees

For each feature in mega-plan.json:

```bash
# Get worktree base directory from PathResolver
WORKTREE_BASE=$(python3 -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_worktree_dir())" 2>/dev/null || echo ".worktree")

FEATURE_NAME="<feature-name>"
WORKTREE_PATH="$WORKTREE_BASE/$FEATURE_NAME"

# Also check legacy location if worktree not found in new location
if [ ! -d "$WORKTREE_PATH" ] && [ -d ".worktree/$FEATURE_NAME" ]; then
    WORKTREE_PATH=".worktree/$FEATURE_NAME"
fi

# Check worktree existence
if [ -d "$WORKTREE_PATH" ]; then
    WORKTREE_EXISTS=true

    # Check for PRD
    if [ -f "$WORKTREE_PATH/prd.json" ]; then
        PRD_EXISTS=true
        # Count stories
        TOTAL_STORIES=$(jq '.stories | length' "$WORKTREE_PATH/prd.json")
    fi

    # Check progress.txt for markers
    if [ -f "$WORKTREE_PATH/progress.txt" ]; then
        # New-style markers
        PRD_COMPLETE=$(grep -c "\[PRD_COMPLETE\]" "$WORKTREE_PATH/progress.txt" 2>/dev/null || echo "0")
        STORIES_COMPLETE=$(grep -c "\[STORY_COMPLETE\]" "$WORKTREE_PATH/progress.txt" 2>/dev/null || echo "0")
        FEATURE_COMPLETE=$(grep -c "\[FEATURE_COMPLETE\]" "$WORKTREE_PATH/progress.txt" 2>/dev/null || echo "0")

        # Old-style markers (compatibility)
        OLD_COMPLETE=$(grep -c "\[COMPLETE\]" "$WORKTREE_PATH/progress.txt" 2>/dev/null || echo "0")
    fi
fi
```

## Step 3: Determine Feature States

For each feature, determine its state based on available evidence:

```
Feature State Detection Logic:

1. NO WORKTREE EXISTS:
   → State: "pending"
   → Action: Create worktree, generate PRD, execute stories

2. WORKTREE EXISTS, NO prd.json:
   → State: "worktree_created"
   → Action: Generate PRD, execute stories

3. WORKTREE EXISTS, prd.json EXISTS but EMPTY (no stories):
   → State: "prd_incomplete"
   → Action: Regenerate PRD, execute stories

4. WORKTREE EXISTS, prd.json HAS STORIES, NO completion markers:
   → State: "prd_generated"
   → Action: Execute stories (PRD might be from old version)

5. [PRD_COMPLETE] marker exists, NO [FEATURE_COMPLETE]:
   → State: "executing"
   → Action: Continue/resume story execution

6. [FEATURE_COMPLETE] marker exists:
   → State: "complete"
   → Action: Ready for merge

7. Status in .mega-status.json says "merged":
   → State: "merged"
   → Action: Skip (already done)

COMPATIBILITY: Old-style detection
- If prd.json has stories with status="complete", count them
- If all stories complete but no [FEATURE_COMPLETE] marker:
   → State: "complete" (old-style completion)
   → Action: Ready for merge
```

## Step 4: Display Detected State

```
============================================================
MEGA PLAN RESUME - STATE DETECTION
============================================================

Project: <goal>
Target Branch: <target_branch>
Total Features: <count>
Total Batches: <calculated>

Design Document: {present at project root | not found}
  {If present: "✓ Will be used for architectural guidance in all features"}

Detected State:
  Completed Batches: [1] (from .mega-status.json)
  Current Batch: 2

Feature Status (auto-detected):

  Batch 1 (MERGED):
    [M] feature-001: Backend API
        Status: merged (from .mega-status.json)

  Batch 2 (IN PROGRESS):
    [~] feature-002: Simple Mode UI
        Worktree: .worktree/feature-simple-mode/ ✓
        PRD: exists (5 stories)
        Progress: No completion markers found
        Detected State: prd_generated
        → Will: Execute stories

    [~] feature-003: Expert Mode UI
        Worktree: .worktree/feature-expert-mode/ ✓
        PRD: exists (7 stories)
        Progress: 3 stories complete (old-style markers)
        Detected State: executing
        → Will: Continue story execution

    [~] feature-004: Settings Page
        Worktree: .worktree/feature-settings-page/ ✓
        PRD: exists (4 stories)
        Progress: No markers
        Detected State: prd_generated
        → Will: Execute stories

  Batch 3 (PENDING):
    [ ] feature-005: Claude Code GUI Mode
        Worktree: not created
        Detected State: pending
        → Will: Wait for Batch 2

============================================================
```

## Step 5: Confirm Resume (if not --auto-prd)

If `--auto-prd` is NOT specified:

```
============================================================
RESUME PLAN
============================================================

The following actions will be taken:

Batch 2 Features:
  • feature-002: Execute all 5 stories (from beginning)
  • feature-003: Continue execution (3/7 stories done)
  • feature-004: Execute all 4 stories (from beginning)

After Batch 2 completes:
  • Merge all Batch 2 features to <target_branch>
  • Create worktrees for Batch 3
  • Continue until all batches complete

============================================================

? Proceed with resume? [Y/n]
```

If `--auto-prd` IS specified, skip confirmation and proceed directly.

## Step 6: Resume Execution

### 6.1: Handle Already-Complete Features

For features detected as "complete" (but not merged):

```bash
# Mark as complete in progress.txt if not already marked
if [ "$FEATURE_STATE" = "complete" ] && [ "$FEATURE_COMPLETE_MARKER" = "0" ]; then
    echo "[FEATURE_COMPLETE] $FEATURE_ID (detected from prd.json)" >> "$WORKTREE_PATH/progress.txt"
fi
```

### 6.2: Resume/Start Story Execution for In-Progress Features

For each feature in current batch that needs execution:

**Launch Task Agent with Resume Context:**

```
You are RESUMING execution for feature: {feature_id} - {feature_title}

Working Directory: {worktree_path}

RESUME CONTEXT:
- This feature was previously interrupted
- Check prd.json for existing story statuses
- Check progress.txt for completion markers
- DO NOT re-execute already completed stories

EXECUTION RULES:
1. cd {worktree_path}
2. Read prd.json - check each story's status field
3. Read progress.txt - check for [STORY_COMPLETE] or [COMPLETE] markers
4. For each story:
   - If status="complete" OR has completion marker → SKIP
   - If status="pending" or "in_progress" → EXECUTE
5. Execute remaining stories in dependency order
6. For each completed story:
   a. Update story status to "complete" in prd.json
   b. echo "[STORY_COMPLETE] {story_id}" >> progress.txt
7. When ALL stories are complete:
   echo "[FEATURE_COMPLETE] {feature_id}" >> progress.txt

IMPORTANT:
- Resume from where it left off - don't redo completed work
- Execute bash/powershell commands directly
- Update findings.md with discoveries
- If a story fails, mark [STORY_FAILED] and continue to next independent story

When done, [FEATURE_COMPLETE] marker signals ready for merge.
```

Launch with `run_in_background: true` for all features in parallel. Store all task_ids.

### 6.3: Wait for Agents Using TaskOutput

**CRITICAL**: Use TaskOutput to wait instead of polling. This avoids Bash confirmation prompts.

```
For each feature_id, task_id in resume_tasks:
    echo "Waiting for {feature_id} to complete..."

    result = TaskOutput(
        task_id=task_id,
        block=true,
        timeout=1800000  # 30 minutes
    )

    echo "✓ {feature_id} agent finished"
```

### 6.4: Verify Completion After Agents Finish

After all TaskOutput calls return, verify using Read tool (NOT Bash):

```
For each feature in current_batch:
    # Use Read tool
    progress_content = Read("{worktree_path}/progress.txt")
    prd_content = Read("{worktree_path}/prd.json")

    # Parse content yourself (no grep)
    if "[FEATURE_COMPLETE]" in progress_content:
        feature_status = "complete"
    elif all stories in prd_content have status="complete":
        feature_status = "complete"
        # Add marker for consistency (this one Bash is OK - it's a write)
        echo "[FEATURE_COMPLETE] {feature_id} (auto-detected)" >> progress.txt
    else:
        feature_status = "incomplete"
```

### 6.4: Merge and Continue

Once current batch is complete:

1. Merge all features in batch to target_branch (same as mega-approve Step 9)
2. Cleanup worktrees
3. Update .mega-status.json
4. If more batches remain AND --auto-prd: Continue to next batch
5. If all batches done: Show completion status

## Step 7: Update Context and Status Files

### 7.1: Update Execution Context File

After resuming, update the `.mega-execution-context.md` file for future recovery:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/mega-plan/scripts/mega-context-reminder.py" update
```

This ensures the context file reflects the current resumed state.

### 7.2: Update Status File

Ensure .mega-status.json is updated with detected/corrected state:

```json
{
  "updated_at": "<timestamp>",
  "resumed_at": "<timestamp>",
  "resume_count": 1,
  "execution_mode": "auto",
  "target_branch": "<branch>",
  "current_batch": 2,
  "completed_batches": [1],
  "features": {
    "feature-001": {
      "status": "merged",
      "batch": 1,
      "merged_at": "<timestamp>"
    },
    "feature-002": {
      "status": "in_progress",
      "batch": 2,
      "worktree": ".worktree/feature-simple-mode",
      "detected_state": "prd_generated",
      "resumed_at": "<timestamp>"
    }
  }
}
```

## Compatibility Matrix

| Scenario | Detection Method | Resume Action |
|----------|------------------|---------------|
| Old mega-approve stopped after worktree creation | Worktree exists, prd.json missing or empty | Generate PRD + Execute |
| Old mega-approve stopped after PRD generation | prd.json has stories, no progress markers | Execute all stories |
| Old mega-approve stopped mid-execution | prd.json has some stories complete | Resume from incomplete stories |
| New mega-approve interrupted | Progress markers exist | Resume based on markers |
| Batch already complete, not merged | All features complete | Merge + Continue next batch |
| Feature manually completed | prd.json all stories complete | Auto-detect + Merge |

## Error Handling

### Corrupted PRD

```
============================================================
WARNING: Corrupted PRD Detected
============================================================

Feature: feature-002
Issue: prd.json exists but is invalid JSON

Options:
  1. Delete and regenerate: rm .worktree/feature-simple-mode/prd.json
  2. Fix manually: edit .worktree/feature-simple-mode/prd.json
  3. Skip feature: Add to .mega-status.json as "skipped"

After fixing, re-run: /plan-cascade:mega-resume --auto-prd
============================================================
```

### Missing Worktree (but status says in_progress)

```
============================================================
WARNING: Missing Worktree
============================================================

Feature: feature-002
Status in .mega-status.json: in_progress
Worktree: .worktree/feature-simple-mode/ NOT FOUND

This can happen if:
  - Worktree was manually deleted
  - Git worktree prune was run

Action: Will recreate worktree and start fresh for this feature.

============================================================
```

### Merge Conflict on Resume

Same handling as mega-approve - pause and show resolution steps.

## Example Usage

### Resume with Full Automation

```bash
/plan-cascade:mega-resume --auto-prd
```

This will:
1. Detect current state from files
2. Resume all incomplete features in parallel
3. Continue through all remaining batches automatically

### Resume with Manual Control

```bash
/plan-cascade:mega-resume
```

This will:
1. Detect current state from files
2. Show detailed status
3. Ask for confirmation before proceeding
4. Pause between batches for review

## Flow Summary

```
/plan-cascade:mega-resume --auto-prd
    │
    ├─→ Read mega-plan.json, .mega-status.json
    │
    ├─→ Scan all worktrees for actual state
    │   ├─→ Check prd.json existence and content
    │   ├─→ Check progress.txt for markers (new + old style)
    │   └─→ Determine true state of each feature
    │
    ├─→ Display detected state summary
    │
    ├─→ Resume current batch
    │   ├─→ Skip already-complete features
    │   ├─→ Launch Task agents for incomplete features
    │   ├─→ Monitor until all complete
    │   ├─→ Merge batch to target_branch
    │   └─→ Cleanup worktrees
    │
    ├─→ Continue to next batch (if any)
    │   └─→ Loop until all batches done
    │
    └─→ Show final completion status
```

## Notes

### State Detection Priority

When detecting feature state, use this priority:
1. `.mega-status.json` status = "merged" → Trust it, skip feature
2. Physical worktree doesn't exist → State is "pending"
3. Progress markers in progress.txt → Use marker-based detection
4. prd.json story statuses → Fallback detection
5. `.mega-status.json` other statuses → Lowest priority (may be stale)

### Why This Works with Old Logs

The old mega-approve would:
1. Create worktrees ✓
2. Initialize planning files ✓
3. Update .mega-status.json with "in_progress" ✓
4. **STOP** (didn't launch Task agents)

So the state would be:
- Worktrees exist
- .mega-status.json says "in_progress"
- prd.json might be empty or have basic structure
- progress.txt has no completion markers

mega-resume detects this as "prd_incomplete" or "prd_generated" and picks up from there.
