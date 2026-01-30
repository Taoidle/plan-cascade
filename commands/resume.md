---
description: "Auto-detect and resume any interrupted Plan Cascade task. Detects mega-plan, hybrid-worktree, or hybrid-auto context and routes to the appropriate resume command."
---

# Plan Cascade - Universal Resume

Automatically detect and resume any interrupted Plan Cascade task.

## Tool Usage Policy (CRITICAL)

**To avoid command confirmation prompts:**

1. **Use Read tool for file reading** - NEVER use `cat` via Bash
2. **Use Glob tool for finding files** - NEVER use `ls` or `find` via Bash
3. **Only use Bash for git commands**

## Step 1: Detect Task Context

Check for various planning files to determine what type of task was interrupted.

### 1.1: Check for Mega Plan

```
Use Read tool to check if mega-plan.json exists:
  Read("mega-plan.json")

If file exists and is valid JSON:
  CONTEXT = "mega-plan"
  Extract: goal, target_branch, features count
```

### 1.2: Check for Hybrid Worktree

```
Use Read tool to check for worktree config:
  Read(".planning-config.json")

If file exists and mode = "hybrid":
  CONTEXT = "hybrid-worktree"
  Extract: task_name, target_branch
```

### 1.3: Check for Worktrees Directory

```
Use Glob tool to find worktrees:
  Glob(".worktree/*/.planning-config.json")
  Glob(".worktree/*/prd.json")

If any worktrees found:
  CONTEXT = "has-worktrees"
  List all found worktrees
```

### 1.4: Check for Regular Hybrid Task

```
Use Read tool to check for PRD:
  Read("prd.json")

If file exists and has stories:
  CONTEXT = "hybrid-auto"
  Extract: goal, stories count
```

### 1.5: No Task Found

```
If none of the above:
  CONTEXT = "none"
```

## Step 2: Display Detection Result

```
============================================================
PLAN CASCADE - RESUME DETECTION
============================================================

{Based on CONTEXT, show what was found}

============================================================
```

### If CONTEXT is "mega-plan":

```
Detected: MEGA PLAN

Project: {goal}
Target Branch: {target_branch}
Features: {count}

Status file: .mega-status.json {exists/missing}
Worktrees: {list any in .worktree/}

→ Will route to: /plan-cascade:mega-resume --auto-prd
```

### If CONTEXT is "hybrid-worktree":

```
Detected: HYBRID WORKTREE

Task: {task_name}
Target Branch: {target_branch}
PRD: {exists/missing}

Current directory is a worktree.

→ Will route to: /plan-cascade:hybrid-resume --auto
```

### If CONTEXT is "has-worktrees":

```
Detected: WORKTREES EXIST (but not currently in one)

Found worktrees:
  - .worktree/{name1}/ (PRD: {exists/missing})
  - .worktree/{name2}/ (PRD: {exists/missing})

Options:
  1. Resume mega-plan (if mega-plan.json exists)
  2. Navigate to a specific worktree and resume

→ Checking for mega-plan.json...
```

### If CONTEXT is "hybrid-auto":

```
Detected: HYBRID AUTO (regular directory)

Goal: {goal}
Stories: {total} ({complete} complete, {pending} pending)

→ Will route to: /plan-cascade:hybrid-resume --auto
```

### If CONTEXT is "none":

```
No interrupted task detected.

Checked for:
  ✗ mega-plan.json (not found)
  ✗ .planning-config.json (not found)
  ✗ .worktree/ directory (not found or empty)
  ✗ prd.json (not found)

To start a new task:
  /plan-cascade:auto "your task description"
```

## Step 3: Route to Appropriate Resume Command

Based on detected context, automatically invoke the correct resume command:

### If CONTEXT is "mega-plan":

```
Routing to mega-resume...
```

Then invoke:
```
/plan-cascade:mega-resume --auto-prd
```

### If CONTEXT is "hybrid-worktree" OR "hybrid-auto":

```
Routing to hybrid-resume...
```

Then invoke:
```
/plan-cascade:hybrid-resume --auto
```

### If CONTEXT is "has-worktrees" (with mega-plan):

```
Routing to mega-resume (worktrees belong to mega-plan)...
```

Then invoke:
```
/plan-cascade:mega-resume --auto-prd
```

### If CONTEXT is "has-worktrees" (without mega-plan):

Show options and ask user:

```
Multiple worktrees found but no mega-plan.json.

These may be standalone hybrid-worktree tasks.

Options:
  [1] Resume worktree: {name1}
  [2] Resume worktree: {name2}
  [3] Cancel

Select which worktree to resume.
```

Use AskUserQuestion to get selection, then:
```
cd .worktree/{selected}
/plan-cascade:hybrid-resume --auto
```

### If CONTEXT is "none":

Do not route anywhere. Show the "no task detected" message and exit.

## Detection Priority

```
1. mega-plan.json exists → MEGA PLAN
2. .planning-config.json with mode=hybrid → HYBRID WORKTREE
3. .worktree/ has subdirectories → HAS WORKTREES
   a. If mega-plan.json also exists → MEGA PLAN
   b. If no mega-plan.json → Ask user which worktree
4. prd.json exists → HYBRID AUTO
5. None of above → NO TASK
```

## Arguments

- `--auto` or `--auto-prd`: Pass through to the routed resume command (automatic mode)
- No arguments: Will still auto-detect and route, using auto mode by default

## Examples

```bash
# Universal resume - auto-detects and routes
/plan-cascade:resume

# Same as above (--auto is default for resume)
/plan-cascade:resume --auto
```

## Flow Summary

```
/plan-cascade:resume
    │
    ├─→ Check mega-plan.json
    │   └─→ Found? → /plan-cascade:mega-resume --auto-prd
    │
    ├─→ Check .planning-config.json (hybrid worktree)
    │   └─→ Found? → /plan-cascade:hybrid-resume --auto
    │
    ├─→ Check .worktree/ directory
    │   ├─→ Has mega-plan.json? → /plan-cascade:mega-resume --auto-prd
    │   └─→ No mega-plan? → Ask user which worktree
    │
    ├─→ Check prd.json (hybrid auto)
    │   └─→ Found? → /plan-cascade:hybrid-resume --auto
    │
    └─→ Nothing found → Show "no task detected" message
```

## Notes

- This command is the recommended way to resume after using `/plan-cascade:auto`
- It handles all three main workflows: mega-plan, hybrid-worktree, hybrid-auto
- Detection is based on file existence, not execution history
- If multiple contexts are detected, priority order is used (mega-plan > worktree > hybrid-auto)
