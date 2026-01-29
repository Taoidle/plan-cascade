---
description: "Approve the mega-plan and start feature execution. Creates worktrees and generates PRDs for each feature. Usage: /plan-cascade:mega-approve [--auto-prd]"
---

# Approve Mega Plan and Start Execution

Approve the mega-plan and begin executing features in **batch-by-batch** order with **FULL AUTOMATION**.

**CRITICAL**: With `--auto-prd`, this command runs the ENTIRE mega-plan to completion automatically:
1. Creates worktrees for current batch
2. Generates PRDs for each feature (via Task agents)
3. Executes all stories in each feature (via Task agents)
4. Monitors for completion
5. Merges completed batch to target branch
6. Automatically starts next batch
7. Repeats until ALL batches complete

**WITHOUT `--auto-prd`**: Pauses after PRD generation for manual review.

## Arguments

- `--auto-prd`: **FULLY AUTOMATIC MODE** - No manual intervention required until completion or error

## Step 1: Verify Mega Plan Exists

```bash
if [ ! -f "mega-plan.json" ]; then
    echo "No mega-plan.json found."
    echo "Use /plan-cascade:mega-plan <description> to create one first."
    exit 1
fi
```

## Step 2: Parse Arguments and State

Check if `--auto-prd` was specified:

```bash
AUTO_PRD=false
if [[ "$ARGUMENTS" == *"--auto-prd"* ]]; then
    AUTO_PRD=true
    echo "============================================"
    echo "FULLY AUTOMATIC MODE ENABLED"
    echo "============================================"
    echo "Will execute ALL batches without stopping."
    echo "Only pauses on errors."
    echo "============================================"
fi
```

Read current state from `.mega-status.json`:
- `current_batch`: Which batch is currently executing (0 = not started)
- `completed_batches`: List of completed batch numbers
- `features`: Status of each feature

## Step 3: Calculate Batches and Determine State

Calculate all batches from mega-plan.json based on dependencies:
- **Batch 1**: Features with no dependencies
- **Batch 2**: Features depending only on Batch 1 features
- **Batch N**: Features depending only on Batch 1..N-1 features

Determine current state:

### Case A: No batch started yet (current_batch = 0 or missing)
→ Start Batch 1

### Case B: Current batch is in progress
→ Check if all features in current batch are complete
→ If not complete AND --auto-prd: Continue monitoring (don't exit)
→ If not complete AND no --auto-prd: Show status and exit
→ If complete: Merge current batch, then start next batch

### Case C: All batches complete
→ Run final cleanup and inform user

## Step 4: Main Execution Loop (AUTOMATIC)

**CRITICAL**: This is the main automation loop. With `--auto-prd`, this loop runs until ALL batches are complete.

```
TOTAL_BATCHES = <calculated from mega-plan.json>
CURRENT_BATCH = <from .mega-status.json or 1 if not started>

while CURRENT_BATCH <= TOTAL_BATCHES:

    # 4.1: Check if current batch features exist (worktrees created)
    if worktrees_not_created:
        create_worktrees_for_batch(CURRENT_BATCH)
        generate_prds_for_batch(CURRENT_BATCH)
        execute_stories_for_batch(CURRENT_BATCH)

    # 4.2: Monitor current batch until complete
    monitor_batch_until_complete(CURRENT_BATCH)

    # 4.3: Merge completed batch
    merge_batch_to_target(CURRENT_BATCH)

    # 4.4: Cleanup worktrees
    cleanup_batch_worktrees(CURRENT_BATCH)

    # 4.5: Move to next batch
    CURRENT_BATCH += 1

# All batches complete!
show_completion_status()
```

## Step 5: Create Worktrees for Batch

For each feature in the current batch:

### 5.1: Checkout Updated Target Branch

```bash
TARGET_BRANCH=$(jq -r '.target_branch' mega-plan.json)
git checkout "$TARGET_BRANCH"
git pull origin "$TARGET_BRANCH" 2>/dev/null || true
```

### 5.2: Create Worktree

```bash
FEATURE_NAME="<feature-name>"
BRANCH_NAME="mega-$FEATURE_NAME"
WORKTREE_PATH=".worktree/$FEATURE_NAME"

# Create worktree from current HEAD (includes all previously merged batches)
git worktree add -b "$BRANCH_NAME" "$WORKTREE_PATH"
```

### 5.3: Initialize Worktree Files

Create in each worktree:
- `.planning-config.json` with feature metadata
- `findings.md` initialized with feature info
- `progress.txt` for tracking
- Copy `mega-findings.md` from project root

## Step 6: Generate PRDs for Batch (Task Agents)

**CRITICAL**: Launch Task agents IN PARALLEL for ALL features in the batch.

For EACH feature in the current batch, launch a Task agent with `run_in_background: true`:

### 6.1: PRD Generation Agent Prompt

```
You are generating a PRD for feature: {feature_id} - {feature_title}

Feature Description:
{feature_description}

Working Directory: {worktree_path}

Your task:
1. Change to the worktree directory: cd {worktree_path}
2. Read mega-findings.md for project context
3. Explore relevant code in the codebase
4. Generate a comprehensive prd.json with:
   - Clear goal matching the feature description
   - 3-7 user stories with proper dependencies
   - Each story has: id, title, description, priority, dependencies, acceptance_criteria, status="pending"
5. Save prd.json to {worktree_path}/prd.json
6. Update progress.txt: echo "[PRD_COMPLETE] {feature_id}" >> {worktree_path}/progress.txt

PRD JSON format:
{
  "metadata": {
    "created_at": "ISO-8601",
    "version": "1.0.0",
    "description": "{feature_description}",
    "mega_feature_id": "{feature_id}"
  },
  "goal": "Feature goal",
  "objectives": ["obj1", "obj2"],
  "stories": [
    {
      "id": "story-001",
      "title": "Story title",
      "description": "Detailed description",
      "priority": "high|medium|low",
      "dependencies": [],
      "status": "pending",
      "acceptance_criteria": ["criterion1"],
      "context_estimate": "small|medium|large"
    }
  ]
}

Work methodically. When done, the [PRD_COMPLETE] marker in progress.txt signals completion.
```

### 6.2: Launch PRD Agents in Parallel

Launch ALL PRD generation agents simultaneously using the Task tool:

```
For each feature in batch:
    task_id = Task(
        prompt=<PRD generation prompt above>,
        subagent_type="general-purpose",
        run_in_background=true,
        description="Generate PRD for {feature_id}"
    )
    Store task_id in prd_tasks[feature_id]
```

### 6.3: Wait for All PRD Agents to Complete

Use TaskOutput to wait for each agent:

```
For each feature_id, task_id in prd_tasks:
    TaskOutput(task_id=task_id, block=true, timeout=600000)
```

OR monitor progress.txt files:

```
while not all_prds_complete:
    for each feature in batch:
        check if {worktree_path}/progress.txt contains "[PRD_COMPLETE] {feature_id}"
    sleep 10 seconds
```

### 6.4: Validate PRDs

After all PRD agents complete:
- Read each prd.json
- Validate structure (has stories, valid dependencies)
- If validation fails, show error and pause

## Step 7: Execute Stories for Batch (Task Agents)

**CRITICAL**: After PRDs are generated, execute stories for ALL features in the batch.

### 7.1: For Each Feature, Launch Story Execution

For EACH feature in the batch, launch a Task agent to execute its stories:

```
You are executing all stories for feature: {feature_id} - {feature_title}

Working Directory: {worktree_path}

EXECUTION RULES:
1. Read prd.json from {worktree_path}/prd.json
2. Calculate story batches based on dependencies
3. Execute stories in batch order (parallel within batch, sequential across batches)
4. For each story:
   a. Implement according to acceptance criteria
   b. Test your implementation
   c. Mark complete: Update story status to "complete" in prd.json
   d. Log to progress.txt: echo "[STORY_COMPLETE] {story_id}" >> progress.txt
5. When ALL stories are complete:
   echo "[FEATURE_COMPLETE] {feature_id}" >> progress.txt

IMPORTANT:
- Execute bash/powershell commands directly
- Do NOT wait for user confirmation between stories
- Update findings.md with important discoveries
- If a story fails, mark it [STORY_FAILED] and continue to next independent story
- Only stop on blocking errors

Story execution loop:
  STORY_BATCH = 1
  while stories_remaining:
      for each story in current_batch (no pending dependencies):
          implement_story()
          test_story()
          mark_complete_in_prd()
          log_to_progress()
      STORY_BATCH += 1

When completely done, [FEATURE_COMPLETE] marker signals this feature is ready for merge.
```

### 7.2: Launch Feature Execution Agents in Parallel

```
For each feature in batch:
    task_id = Task(
        prompt=<Story execution prompt above>,
        subagent_type="general-purpose",
        run_in_background=true,
        description="Execute stories for {feature_id}"
    )
    Store task_id in execution_tasks[feature_id]
```

## Step 8: Monitor Batch Until Complete

**CRITICAL**: This is the monitoring loop. Keep polling until ALL features in batch are complete.

```
while true:
    all_complete = true
    has_errors = false

    for each feature in current_batch:
        progress_file = "{worktree_path}/progress.txt"

        # Check for completion
        if "[FEATURE_COMPLETE] {feature_id}" in progress_file:
            feature_status = "complete"
        elif "[FEATURE_FAILED] {feature_id}" in progress_file:
            feature_status = "failed"
            has_errors = true
        else:
            feature_status = "in_progress"
            all_complete = false

        # Count story progress
        complete_count = count("[STORY_COMPLETE]" in progress_file)
        failed_count = count("[STORY_FAILED]" in progress_file)

        # Update .mega-status.json with progress
        update_feature_status(feature_id, feature_status, complete_count, failed_count)

    # Check if batch is done
    if all_complete:
        if has_errors:
            echo "⚠️  BATCH COMPLETE WITH ERRORS"
            echo "Some features failed. Review and fix before continuing."
            if not AUTO_PRD:
                exit 1  # Pause for manual review
            # With AUTO_PRD, continue anyway (failed features won't merge)
        break

    # Show progress
    echo "Batch {CURRENT_BATCH} progress: {completed}/{total} features"
    for each feature:
        echo "  {feature_id}: {stories_complete}/{stories_total} stories"

    # Wait before next check
    sleep 30 seconds

    # IMPORTANT: NO TIMEOUT - keep monitoring until complete
```

## Step 9: Merge Completed Batch

When all features in current batch are complete:

### 9.1: Display Merge Start

```
============================================================
BATCH {N} COMPLETED - MERGING TO TARGET BRANCH
============================================================

Merging completed features in dependency order...
```

### 9.2: Checkout Target Branch

```bash
TARGET_BRANCH=$(jq -r '.target_branch' mega-plan.json)
git checkout "$TARGET_BRANCH"
git pull origin "$TARGET_BRANCH" 2>/dev/null || true
```

### 9.3: Merge Each Feature

For each successfully completed feature in the batch:

```bash
FEATURE_NAME="<name>"
WORKTREE_PATH=".worktree/$FEATURE_NAME"
BRANCH_NAME="mega-$FEATURE_NAME"

# Commit any uncommitted changes in worktree (code only, exclude planning files)
cd "$WORKTREE_PATH"
git add -A
git reset HEAD -- prd.json findings.md progress.txt .planning-config.json .agent-status.json mega-findings.md 2>/dev/null || true
git commit -m "feat: complete $FEATURE_NAME" || true
cd -

# Merge to target branch
git merge "$BRANCH_NAME" --no-ff -m "Merge feature: <title>

Mega-plan feature: <feature-id>
Batch: <batch-number>

Co-Authored-By: Claude <noreply@anthropic.com>"

echo "[OK] Merged {feature_id}: {title}"
```

### 9.4: Cleanup Worktrees

```bash
git worktree remove ".worktree/$FEATURE_NAME" --force
git branch -d "mega-$FEATURE_NAME"
```

### 9.5: Update Status

Update `.mega-status.json`:
- Mark features as "merged"
- Add batch to `completed_batches`
- Increment `current_batch`

## Step 10: Continue to Next Batch (AUTOMATIC)

**CRITICAL**: With `--auto-prd`, automatically continue to the next batch.

```
if CURRENT_BATCH < TOTAL_BATCHES:
    echo ""
    echo "============================================"
    echo "AUTO-CONTINUING TO BATCH {CURRENT_BATCH + 1}"
    echo "============================================"

    # Go back to Step 5 (create worktrees for next batch)
    CURRENT_BATCH += 1
    continue main loop
```

## Step 11: All Batches Complete

When all batches are done:

```
============================================================
ALL BATCHES COMPLETE - MEGA PLAN FINISHED
============================================================

Total batches completed: {TOTAL_BATCHES}
Total features merged: {count}

Summary:
  Batch 1: {features} - MERGED
  Batch 2: {features} - MERGED
  ...

All code has been merged to {target_branch}.

Final cleanup (removes planning files):
  /plan-cascade:mega-complete

============================================================
```

## Error Handling

### Merge Conflict

```
============================================================
MERGE CONFLICT
============================================================

Conflict while merging {feature_id}: {title}

Conflicting files:
  - {file1}
  - {file2}

To resolve:
  1. Resolve conflicts in the listed files
  2. git add <resolved-files>
  3. git commit
  4. Re-run /plan-cascade:mega-approve --auto-prd

Or abort: git merge --abort
============================================================
```

Pause execution on merge conflicts (even with --auto-prd).

### Feature Execution Failed

```
============================================================
FEATURE EXECUTION FAILED
============================================================

Feature {feature_id}: {title} failed during execution.

Error details in:
  - {worktree_path}/progress.txt
  - {worktree_path}/findings.md

Failed stories:
  - {story_id}: {reason}

Options:
  1. Fix the issue in {worktree_path}
  2. Re-run /plan-cascade:mega-approve --auto-prd to retry
  3. Skip feature: Mark as failed in .mega-status.json

============================================================
```

### PRD Generation Failed

```
============================================================
PRD GENERATION FAILED
============================================================

Could not generate PRD for {feature_id}: {title}

Worktree: {worktree_path}

To fix:
  1. Manually create prd.json in {worktree_path}
  2. Or edit the feature description in mega-plan.json
  3. Re-run /plan-cascade:mega-approve --auto-prd

============================================================
```

## Execution Flow Summary (AUTOMATIC MODE)

```
/plan-cascade:mega-approve --auto-prd
    │
    ├─→ Read mega-plan.json, calculate batches
    │
    ├─→ BATCH 1 ─────────────────────────────────────────────
    │   ├─→ Create worktrees for Batch 1 features
    │   ├─→ Launch PRD generation agents (parallel)
    │   ├─→ Wait for all PRDs complete
    │   ├─→ Launch story execution agents (parallel)
    │   ├─→ Monitor until all features complete
    │   ├─→ Merge Batch 1 to target_branch
    │   └─→ Cleanup Batch 1 worktrees
    │
    ├─→ BATCH 2 ─────────────────────────────────────────────
    │   ├─→ Create worktrees (from UPDATED target_branch)
    │   ├─→ Launch PRD generation agents (parallel)
    │   ├─→ Wait for all PRDs complete
    │   ├─→ Launch story execution agents (parallel)
    │   ├─→ Monitor until all features complete
    │   ├─→ Merge Batch 2 to target_branch
    │   └─→ Cleanup Batch 2 worktrees
    │
    ├─→ ... continue for all batches ...
    │
    └─→ ALL COMPLETE
        └─→ Show final status, suggest /plan-cascade:mega-complete
```

## Important Notes

### Parallelism Strategy

- **Within a batch**: All features execute in PARALLEL (independent worktrees)
- **Across batches**: SEQUENTIAL (Batch N+1 depends on Batch N code)
- **Within a feature**: Stories execute in dependency order (parallel where possible)

### Progress Markers

Agents use these markers in progress.txt:
- `[PRD_COMPLETE] {feature_id}` - PRD generation done
- `[STORY_COMPLETE] {story_id}` - Individual story done
- `[STORY_FAILED] {story_id}` - Story failed
- `[FEATURE_COMPLETE] {feature_id}` - All stories done, ready for merge
- `[FEATURE_FAILED] {feature_id}` - Feature cannot complete

### Timeout Behavior

- **PRD generation**: 10 minute timeout per feature
- **Story execution**: NO TIMEOUT - stories may take varying time
- **Monitoring loop**: NO TIMEOUT - keeps polling until complete

### Recovery

If interrupted:
1. `.mega-status.json` tracks current state
2. **Recommended**: Use `/plan-cascade:mega-resume --auto-prd` to intelligently resume
   - Auto-detects actual state from files
   - Skips already-completed work
   - Compatible with old and new executions
3. Or re-run `/plan-cascade:mega-approve --auto-prd` to continue from batch level
