---
description: "Approve the mega-plan and start feature execution. Creates worktrees and generates PRDs for each feature. Usage: /planning-with-files:mega-approve [--auto-prd]"
---

# Approve Mega Plan and Start Execution

Approve the mega-plan and begin executing features in parallel batches.

## Arguments

- `--auto-prd`: Automatically approve all generated PRDs (skip manual review)

## Step 1: Verify Mega Plan Exists

```bash
if [ ! -f "mega-plan.json" ]; then
    echo "No mega-plan.json found."
    echo "Use /planning-with-files:mega-plan <description> to create one first."
    exit 1
fi
```

## Step 2: Parse Arguments

Check if `--auto-prd` was specified:

```bash
AUTO_PRD=false
if [[ "$ARGUMENTS" == *"--auto-prd"* ]]; then
    AUTO_PRD=true
    echo "Auto-PRD mode enabled: PRDs will be automatically approved"
fi
```

## Step 3: Validate Mega Plan

Read and validate the mega-plan:
- Check required fields (metadata, goal, features)
- Verify feature names are valid
- Check dependencies exist
- Detect circular dependencies

If validation fails, show errors and exit.

## Step 4: Display Execution Plan

```
============================================================
MEGA PLAN APPROVAL
============================================================

Goal: <goal>
Execution Mode: <auto|manual>
PRD Approval: <auto|manual>
Target Branch: <branch>

Execution Plan:

Batch 1 (Starting Now):
  - feature-001: <title>
    → Worktree: .worktree/<name>/
    → Branch: mega-<name>

  - feature-002: <title>
    → Worktree: .worktree/<name>/
    → Branch: mega-<name>

Batch 2 (After Batch 1):
  - feature-003: <title>
    → Depends on: feature-001, feature-002

============================================================

Proceed with execution? This will:
1. Create Git worktrees for Batch 1 features
2. Generate PRDs in each worktree
3. <Auto-approve PRDs | Wait for manual approval>
4. Execute stories in each feature
```

## Step 5: Create Batch 1 Worktrees

For each feature in Batch 1:

```bash
# Create worktree
FEATURE_NAME="<feature-name>"
BRANCH_NAME="mega-$FEATURE_NAME"
WORKTREE_PATH=".worktree/$FEATURE_NAME"

git worktree add -b "$BRANCH_NAME" "$WORKTREE_PATH"
echo "Created worktree: $WORKTREE_PATH"
```

## Step 6: Initialize Each Worktree

For each worktree:

1. Create `.planning-config.json`:
```json
{
  "task_name": "<feature-name>",
  "target_branch": "<target-branch>",
  "mega_feature_id": "<feature-id>",
  "created_at": "<timestamp>"
}
```

2. Create `findings.md`:
```markdown
# Findings: <feature-title>

Feature: <feature-name> (<feature-id>)

---

```

3. Create `progress.txt`:
```
[<timestamp>] Feature <feature-id> initialized
```

4. Copy `mega-findings.md` (read-only):
```bash
cp mega-findings.md "$WORKTREE_PATH/mega-findings.md"
# Add read-only header
```

## Step 7: Generate PRDs

For each feature in Batch 1, use a Task agent to generate the PRD:

```
Launch a background Task agent for each feature:

subagent_type: general-purpose
description: Generate PRD for <feature-name>
prompt: |
  You are in worktree: .worktree/<feature-name>/

  Generate a PRD (prd.json) for this feature:

  Feature: <title>
  Description: <description>

  Break this feature into 3-7 user stories with:
  - Clear acceptance criteria
  - Appropriate dependencies
  - Priority levels

  Save as prd.json in the current directory.
run_in_background: true
```

Wait for all PRD generation tasks to complete using TaskOutput.

## Step 8: Update Feature Statuses

Update mega-plan.json to set Batch 1 features to `prd_generated`.

## Step 9: Handle PRD Approval

### If `--auto-prd` (Auto Mode):

```
============================================================
AUTO-APPROVING PRDs
============================================================

[OK] feature-001: PRD approved (5 stories)
[OK] feature-002: PRD approved (4 stories)

Starting story execution...
```

Update statuses to `approved`, then `in_progress`.

For each feature, launch a Task agent to execute stories:

```
subagent_type: general-purpose
description: Execute <feature-name> stories
prompt: |
  You are in worktree: .worktree/<feature-name>/
  Execution mode: <auto|manual>

  Execute the stories in prd.json according to execution batches.

  For each story:
  1. Read story details
  2. Implement the story
  3. Update progress.txt
  4. Mark story complete

  Update findings.md with discoveries.
run_in_background: true
```

### If Manual Mode:

```
============================================================
PRD REVIEW REQUIRED
============================================================

PRDs have been generated for Batch 1 features.
Please review and approve each one:

Feature 1: <title>
  cd .worktree/<name>
  cat prd.json
  /planning-with-files:approve

Feature 2: <title>
  cd .worktree/<name>
  cat prd.json
  /planning-with-files:approve

============================================================

Run /planning-with-files:mega-status to monitor progress.
When all PRDs are approved, story execution will begin.
```

## Step 10: Show Status

```
============================================================
BATCH 1 EXECUTION STARTED
============================================================

Features in progress:
  [>] feature-001: <title>
      Worktree: .worktree/<name>/
      Stories: 0/5 complete

  [>] feature-002: <title>
      Worktree: .worktree/<name>/
      Stories: 0/4 complete

============================================================

Monitor progress: /planning-with-files:mega-status

When Batch 1 completes, Batch 2 will start automatically.
When all features complete: /planning-with-files:mega-complete

============================================================
```

## Error Handling

### Worktree Creation Fails

```
Error: Could not create worktree for <feature>
Try: git worktree prune
Then re-run /planning-with-files:mega-approve
```

### Branch Already Exists

```
Error: Branch mega-<name> already exists
Options:
  1. Delete branch: git branch -D mega-<name>
  2. Use different feature name: /planning-with-files:mega-edit
```
