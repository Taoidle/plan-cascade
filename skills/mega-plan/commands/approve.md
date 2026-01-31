---
name: mega:approve
description: Approve the mega-plan and begin feature execution
arguments:
  - name: auto-prd
    description: Automatically approve all generated PRDs (--auto-prd)
    required: false
---

# /mega:approve

Approve the mega-plan and begin executing features in parallel batches.

## Arguments

- `--auto-prd`: Automatically approve all generated PRDs without manual review

## Your Task

### Step 1: Check for Mega Plan

**Use Read tool (NOT Bash) to check if mega-plan.json exists:**

```
Read("mega-plan.json")
```

If the file doesn't exist (Read returns error):
```
No mega-plan.json found.
Use /mega:plan <description> to create one first.
```

### Step 2: Validate the Mega Plan

Validate before proceeding:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/mega-plan/core/mega_generator.py" validate
```

If validation fails, show errors and exit.

### Step 3: Parse Arguments

Check if `--auto-prd` was specified in `$ARGUMENTS`.

```
Arguments: $ARGUMENTS
Auto-PRD Mode: <yes if --auto-prd, no otherwise>
```

### Step 4: Ask Execution Mode (if not set)

Read the mega-plan and check execution_mode. If needed, ask the user:

Use AskUserQuestion:

**Question: How should feature batches progress?**

Options:
1. **Auto Mode (Recommended)** - Batches progress automatically when ready
2. **Manual Mode** - Confirm before starting each batch

Update mega-plan.json with their choice.

### Step 5: Display Execution Plan

Show what will happen:

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
```

### Step 6: Create Batch 1 Worktrees

For each feature in Batch 1:

1. **Create Git worktree:**
```bash
git worktree add -b mega-<feature-name> .worktree/<feature-name>
```

2. **Initialize planning files:**
```bash
mkdir -p .worktree/<feature-name>
```

3. **Copy mega-findings:**
```bash
cp mega-findings.md .worktree/<feature-name>/mega-findings.md
# Add read-only header
```

4. **Update feature status to `prd_generated`**

### Step 7: Generate PRDs

For each feature in Batch 1, use a Task agent to generate the PRD:

```
<Task tool call>
subagent_type: general-purpose
description: Generate PRD for <feature-name>
prompt: |
  You are in worktree: .worktree/<feature-name>/

  Generate a PRD (prd.json) for this feature:

  Feature: <title>
  Description: <description>

  Break this feature into 3-7 user stories. Each story should be:
  - Small enough to complete independently
  - Have clear acceptance criteria
  - Have appropriate dependencies

  Create prd.json with the standard hybrid-ralph format.
  Also initialize findings.md and progress.txt.

  After creating the PRD, display it for review.
run_in_background: true
</Task>
```

### Step 8: Handle PRD Approval

**If `--auto-prd` specified:**

```
Auto-approving PRDs for all features in Batch 1...

[OK] feature-001: PRD approved (5 stories)
[OK] feature-002: PRD approved (4 stories)

Starting story execution...
```

Update each feature status to `approved`, then `in_progress`.
Trigger story execution in each worktree.

**If manual PRD approval:**

```
============================================================
PRD REVIEW REQUIRED
============================================================

PRDs have been generated for Batch 1 features.
Please review and approve each one:

Feature 1: <title>
  cd .worktree/<name>
  cat prd.json
  /approve  # When ready

Feature 2: <title>
  cd .worktree/<name>
  cat prd.json
  /approve  # When ready

============================================================

Run /mega:status to monitor PRD approval progress.
When all PRDs are approved, story execution will begin.
```

### Step 9: Start Story Execution (Auto-PRD mode)

If auto-prd mode, immediately start executing stories in each worktree:

For each approved feature, launch a Task agent:

```
<Task tool call>
subagent_type: general-purpose
description: Execute <feature-name> stories
prompt: |
  You are in worktree: .worktree/<feature-name>/
  Execution mode: <auto|manual>

  Execute the stories in prd.json according to the execution batches.

  For each story:
  1. Read story details from prd.json
  2. Implement the story
  3. Update progress.txt
  4. Mark story complete in prd.json

  In auto mode, progress through story batches automatically.
  In manual mode, pause after each batch for confirmation.

  Update findings.md with any discoveries.
run_in_background: true
</Task>
```

### Step 10: Show Final Status

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

Monitor progress: /mega:status

When Batch 1 completes, Batch 2 will start automatically.
When all features complete: /mega:complete

============================================================
```

## PRD Approval Modes Summary

| Mode | Command | Behavior |
|------|---------|----------|
| Manual | `/mega:approve` | PRDs generated, user reviews in each worktree, runs `/approve` |
| Auto | `/mega:approve --auto-prd` | PRDs generated and auto-approved, stories execute immediately |

## Error Handling

### Worktree Creation Fails

```
Error: Could not create worktree for feature-001
Reason: <git error>

Try:
  git worktree prune
  /mega:approve
```

### PRD Generation Fails

```
Error: PRD generation failed for feature-001

Check:
  cd .worktree/<name>
  cat progress.txt

Retry:
  /hybrid:auto "<feature description>"
```

### Branch Already Exists

```
Error: Branch mega-<name> already exists

Options:
1. Delete existing branch: git branch -D mega-<name>
2. Use different feature name: /mega:edit
```
