---
name: approve
description: Approve the current PRD and begin execution
---

# /approve

Approve the current PRD and begin parallel execution of stories.

## Usage

```
/approve
```

## What Happens After Approval

1. **Creates execution plan** - Analyzes dependencies and creates batches
2. **Shows execution summary** - Displays batches and execution strategy
3. **Starts Batch 1** - Launches parallel agents for first batch of stories
4. **Monitors progress** - Tracks story completion in progress.txt

## Execution Flow

```
┌─────────────────────────────────────────────────────────┐
│  PRD Approved                                            │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Generate Execution Batches                              │
│  - Batch 1: Stories with no dependencies (parallel)      │
│  - Batch 2: Stories whose Batch 1 deps are complete      │
│  - Batch N: Continue until all stories complete          │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Execute Batch 1                                         │
│  - Launch background Task agents for each story         │
│  - Each agent gets filtered context                     │
│  - Agents write findings with story tags                │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Wait for Batch Completion                               │
│  - Monitor progress.txt for completion markers          │
│  - Check /status for real-time updates                  │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
                    ┌─────────┐
                    │ More    │──── Yes ────▶ Start Next Batch
                    │ Batches?│
                    └─────────┘
                          │ No
                          ▼
┌─────────────────────────────────────────────────────────┐
│  All Stories Complete                                    │
│  - Merge worktree if in worktree mode                   │
│  - Show completion summary                              │
└─────────────────────────────────────────────────────────┘
```

## Context Filtering

Each agent receives only relevant context:
- Their story description and acceptance criteria
- Summaries of completed dependencies
- Findings tagged with their story ID

This keeps context focused and efficient.

## Progress Tracking

Monitor execution with:
- `/status` - Show current batch and story statuses
- `progress.txt` - Detailed progress log
- `.agent-outputs/` - Individual agent logs

## Completion

When all stories complete:
- `[COMPLETE]` markers appear in progress.txt
- Worktree is merged to target branch (if applicable)
- Summary shows successful and failed stories

## See Also

- `/status` - Check execution status
- `/edit` - Modify PRD before approval
- `/show-dependencies` - View dependency graph
