---
name: hybrid:worktree
description: Create a new Git worktree and initialize Hybrid Ralph mode for parallel multi-task development with PRD-based story execution
---

# /hybrid:worktree

Create a new Git worktree with isolated environment and initialize Hybrid Ralph mode for PRD-based parallel story execution.

## Usage

```
/hybrid:worktree <task-name> <target-branch> <task-description>
```

### Arguments

- `task-name` (required): Name for the task/worktree (e.g., "feature-auth", "fix-api-bug")
- `target-branch` (optional): Branch to merge into when complete (default: auto-detects main/master)
- `task-description` (optional): Description for auto-generating PRD

### Examples

```bash
# Create worktree and generate PRD from description
/hybrid:worktree feature-auth main "Implement user authentication with login and registration"

# Create worktree, will prompt for PRD description later
/hybrid:worktree refactor-api main

# Create worktree with custom target branch
/hybrid:worktree feature-payment develop "Add payment processing"
```

## What It Does

### Phase 1: Worktree Creation

1. **Creates Git worktree** at `.worktree/<task-name>/`
2. **Creates task branch** with format `task-YYYY-MM-DD-HHMM`
3. **Initializes planning files** in the worktree
4. **Creates `.planning-config.json`** with task metadata
5. **Main directory remains untouched** on original branch

### Phase 2: Hybrid Ralph Initialization

1. **Changes to worktree directory**
2. **Generates PRD** from task description (or prompts for it)
3. **Enters review mode** for PRD approval
4. **Ready for parallel story execution** with `/approve`

## Workflow

```
┌─────────────────────────────────────────────────────────┐
│  /hybrid:worktree feature-auth main "Implement auth"    │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Phase 1: Worktree Creation                              │
│  - git worktree add .worktree/feature-auth -b task-*    │
│  - Create planning files in worktree                    │
│  - Create .planning-config.json                         │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Phase 2: Navigate to Worktree                           │
│  - cd .worktree/feature-auth/                           │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Phase 3: Hybrid Ralph Initialization                     │
│  - Generate PRD from description                         │
│  - Show PRD for review                                   │
│  - Wait for /approve                                    │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Ready for Execution                                     │
│  - /approve to start parallel story execution           │
│  - /status to monitor progress                          │
└─────────────────────────────────────────────────────────┘
```

## Worktree Structure

```
.worktree/feature-auth/
├── [git worktree files]
├── prd.json              # Generated PRD
├── findings.md           # Research findings (tagged)
├── progress.txt          # Progress tracking
├── .planning-config.json # Worktree metadata
└── .agent-outputs/       # Agent logs
```

## After PRD Approval

Once you approve the PRD with `/approve`:

1. **Stories execute in parallel batches**
2. **Progress tracked in progress.txt**
3. **Findings tagged by story ID**
4. **Each story gets filtered context**

## Completion

When all stories complete:

```
/hybrid:complete [target-branch]
```

This will:
1. Verify all stories are complete
2. Navigate to root directory
3. Merge task branch to target
4. Remove worktree directory
5. Delete task branch
6. Clean up planning files

## Multi-Task Parallel Development

Create multiple worktrees for parallel tasks:

```bash
# Terminal 1: Start authentication feature
/hybrid:worktree feature-auth main
cd .worktree/feature-auth

# Terminal 2: Start API refactoring (parallel!)
/hybrid:worktree refactor-api main
cd .worktree/refactor-api

# Each worktree has:
# - Its own branch and directory
# - Its own PRD and stories
# - Its own execution context
# No conflicts, no branch switching!
```

## Advantages Over Standard Hybrid

| Feature | Standard Hybrid | Worktree + Hybrid |
|---------|----------------|-------------------|
| Branch isolation | ❌ Works on current branch | ✅ Isolated task branch |
| Main directory safety | ❌ Files modified directly | ✅ Main directory untouched |
| Parallel tasks | ❌ Only one at a time | ✅ Multiple simultaneously |
| Clean merge | ❌ Manual cleanup | ✅ Automatic merge on complete |
| Experiment safety | ❌ Changes affect main | ✅ Isolated, easy to discard |

## File Locations

| File | Location in Worktree |
|------|---------------------|
| `prd.json` | `.worktree/<task-name>/prd.json` |
| `findings.md` | `.worktree/<task-name>/findings.md` |
| `progress.txt` | `.worktree/<task-name>/progress.txt` |
| `.planning-config.json` | `.worktree/<task-name>/.planning-config.json` |
| `.agent-outputs/` | `.worktree/<task-name>/.agent-outputs/` |

## Troubleshooting

### Worktree Already Exists

```
Error: Worktree already exists at .worktree/feature-auth
```

Solution: Use a different task name or complete existing worktree first.

### Not a Git Repository

```
Error: Not a git repository
```

Solution: Initialize git first with `git init`

### Target Branch Not Found

```
Error: Target branch 'feature-x' not found
```

Solution: Use `git branch -a` to see available branches

## See Also

- `/hybrid:complete` - Complete worktree and merge changes
- `/hybrid:auto` - Standard hybrid mode (no worktree)
- `/hybrid:manual` - Load existing PRD in worktree
- `/status` - Check execution status
- `/show-dependencies` - View dependency graph
