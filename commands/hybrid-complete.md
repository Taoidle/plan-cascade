---
name: planning-with-files:hybrid-complete
description: Complete Hybrid Ralph task in worktree, verify all stories complete, merge to target branch, and cleanup worktree directory. Validates PRD completion, removes planning files, and performs git merge automatically.
disable-model-invocation: true
---

# /planning-with-files:hybrid-complete

Complete the current Hybrid Ralph task in a worktree, verify all stories are complete, merge the task branch to target, and cleanup the worktree.

## Usage

```
/planning-with-files:hybrid-complete [target-branch]
```

### Arguments

- `target-branch` (optional): Branch to merge into (default: reads from `.planning-config.json` or auto-detects main/master)

### Examples

```bash
# Complete and merge to auto-detected target branch
/planning-with-files:hybrid-complete

# Complete and merge to specific branch
/planning-with-files:hybrid-complete develop

# Complete and merge to main
/planning-with-files:hybrid-complete main
```

## What Happens After Approval

1. **Creates execution plan** - Analyzes dependencies and creates batches
2. **Shows execution summary** - Displays batches and execution strategy
3. **Starts Batch 1** - Launches parallel agents for first batch of stories
4. **Monitors progress** - Tracks story completion in progress.txt

## Completion Flow

```
┌─────────────────────────────────────────────────────────┐
│  /planning-with-files:hybrid-complete main                   │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Phase 1: Verification                                   │
│  - Check if in worktree                                 │
│  - Read .planning-config.json                           │
│  - Verify all stories complete                          │
│  - Show completion summary                              │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Phase 2: Cleanup in Worktree                            │
│  - Delete prd.json                                      │
│  - Delete findings.md                                   │
│  - Delete progress.txt                                  │
│  - Delete .planning-config.json                         │
│  - Delete .agent-outputs/                               │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Phase 3: Navigate and Merge                             │
│  - cd to project root                                   │
│  - Merge task branch to target                          │
│  - Remove worktree                                      │
│  - Delete task branch                                   │
└─────────────────────────────────────────────────────────┘
```

## Before Running

Ensure all stories are complete:

```bash
# Check status
/planning-with-files:hybrid-status

# All stories should show ● (complete)
```

If any stories are incomplete, complete them first before running `/planning-with-files:hybrid-complete`.

## Completion Summary Example

```
============================================================
COMPLETION SUMMARY
============================================================

Task: feature-auth
Branch: task-20260126-1430
Target: main

Stories: 4 total
  ✓ story-001: Design database schema
  ✓ story-002: Implement user registration
  ✓ story-003: Implement user login
  ✓ story-004: Implement password reset

All stories complete!

Changes to merge:
  - Modified: src/database/schema.sql
  - Modified: src/api/auth.py
  - Modified: src/api/users.py
  - Created: tests/test_auth.py

Ready to merge to main...
============================================================
```

## Safety Checks

The command performs several safety checks:

1. **Not in worktree?** - Prompts to use `/planning-with-files:complete` instead
2. **Incomplete stories?** - Shows which stories need completion
3. **Merge conflicts?** - Pauses for manual conflict resolution
4. **Target branch missing?** - Prompts for target branch

## Merge Conflicts

If merge conflicts occur:

1. **Command pauses** - Worktree is not removed
2. **Manual resolution** - Resolve conflicts in task branch
3. **Re-run complete** - Run `/planning-with-files:hybrid-complete` again after resolution

## Cleanup Details

Files deleted in worktree:
- `prd.json`
- `findings.md`
- `progress.txt`
- `.planning-config.json`
- `.agent-outputs/` (entire directory)

Git operations:
- Merge task branch to target
- Remove worktree with `git worktree remove`
- Delete task branch with `git branch -D`

## After Completion

After successful completion:

```
✓ Task complete!
✓ Changes merged to main
✓ Worktree removed
✓ Task branch deleted

Main directory is now on 'main' branch with merged changes.
```

You can now:
- Start a new worktree task with `/planning-with-files:hybrid-worktree`
- Continue working in main directory
- Push changes with `git push origin main`

## See Also

- `/planning-with-files:hybrid-worktree` - Start a new worktree + hybrid task
- `/planning-with-files:hybrid-status` - Check if all stories are complete
- `/planning-with-files:show-dependencies` - Review dependency graph before completing
