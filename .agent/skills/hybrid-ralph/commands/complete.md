---
name: hybrid:complete
description: Complete Hybrid Ralph task in worktree, merge to target branch, and cleanup
---

# /hybrid:complete

Complete the current Hybrid Ralph task in a worktree, verify all stories are complete, merge the task branch to target, and cleanup the worktree.

## Usage

```
/hybrid:complete [target-branch]
```

### Arguments

- `target-branch` (optional): Branch to merge into (default: reads from `.planning-config.json` or auto-detects main/master)

### Examples

```bash
# Complete and merge to auto-detected target branch
/hybrid:complete

# Complete and merge to specific branch
/hybrid:complete develop

# Complete and merge to main
/hybrid:complete main
```

## What It Does

### Phase 1: Verification

1. **Verifies worktree mode** - Checks if currently in a worktree
2. **Reads planning config** - Gets task metadata from `.planning-config.json`
3. **Verifies PRD completion** - Checks all stories are marked complete
4. **Shows completion summary** - Displays what was accomplished

### Phase 2: Cleanup

1. **Removes planning files** - Deletes `prd.json`, `findings.md`, `progress.txt`
2. **Removes planning config** - Deletes `.planning-config.json`
3. **Cleans agent outputs** - Removes `.agent-outputs/` directory

### Phase 3: Merge

1. **Navigates to root directory** - Returns to project root
2. **Merges task branch** - Merges task branch to target branch
3. **Removes worktree** - Deletes the worktree directory
4. **Deletes task branch** - Removes the task branch after merge

## Workflow

```
┌─────────────────────────────────────────────────────────┐
│  /hybrid:complete main                                   │
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
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Complete!                                               │
│  - All changes merged to target branch                  │
│  - Worktree cleaned up                                  │
│  - Ready for next task                                  │
└─────────────────────────────────────────────────────────┘
```

## Before Running

Ensure all stories are complete:

```bash
# Check status
/status

# All stories should show ● (complete)
```

If any stories are incomplete, complete them first before running `/hybrid:complete`.

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
3. **Re-run complete** - Run `/hybrid:complete` again after resolution

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
- Start a new worktree task with `/hybrid:worktree`
- Continue working in main directory
- Push changes with `git push origin main`

## Troubleshooting

### Not in Worktree

```
Error: Not currently in a worktree
```

Solution: Use `/planning-with-files:complete [target-branch]` instead for standard worktree completion.

### Incomplete Stories

```
Error: Not all stories are complete:
  - story-004: Implement password reset (pending)
```

Solution: Complete remaining stories first, then run `/hybrid:complete` again.

### Merge Conflict

```
Error: Merge conflicts detected
```

Solution:
1. Resolve conflicts manually
2. Complete the merge
3. Re-run `/hybrid:complete`

### Target Branch Not Found

```
Error: Target branch 'feature-x' not found
```

Solution: Specify correct target branch: `/hybrid:complete main`

## Comparison with Standard Complete

| Feature | `/hybrid:complete` | `/planning-with-files:complete` |
|---------|-------------------|-------------------------------|
| Verifies PRD stories | ✅ Yes | ❌ No |
| Shows completion summary | ✅ Yes | ❌ No |
| Removes PRD files | ✅ Yes | ❌ No |
| Standard worktree cleanup | ✅ Yes | ✅ Yes |

## See Also

- `/hybrid:worktree` - Start a new worktree + hybrid task
- `/status` - Check if all stories are complete
- `/show-dependencies` - Review dependency graph before completing
- `/planning-with-files:complete` - Complete standard worktree task
