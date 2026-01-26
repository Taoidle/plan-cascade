---
name: planning-with-files:approve
description: Approve the current PRD and begin parallel story execution. Analyzes dependencies, creates execution batches, launches background Task agents for each story, and monitors progress.
disable-model-invocation: true
---

# /planning-with-files:approve

Approve the current PRD and begin parallel execution of stories.

## Usage

```
/planning-with-files:approve
```

## What Happens After Approval

1. **Creates execution plan** - Analyzes dependencies and creates batches
2. **Shows execution summary** - Displays batches and execution strategy
3. **Starts Batch 1** - Launches parallel agents for first batch of stories
4. **Monitors progress** - Tracks story completion in progress.txt

## Context Filtering

Each agent receives only relevant context:
- Their story description and acceptance criteria
- Summaries of completed dependencies
- Findings tagged with their story ID

This keeps context focused and efficient.

## Progress Tracking

Monitor execution with:
- `/planning-with-files:hybrid-status` - Show current batch and story statuses
- `progress.txt` - Detailed progress log
- `.agent-outputs/` - Individual agent logs

## Completion

When all stories complete:
- `[COMPLETE]` markers appear in progress.txt
- Worktree is merged to target branch (if applicable)
- Summary shows successful and failed stories

## See Also

- `/planning-with-files:hybrid-status` - Check execution status
- `/planning-with-files:edit` - Modify PRD before approval
- `/planning-with-files:show-dependencies` - View dependency graph
