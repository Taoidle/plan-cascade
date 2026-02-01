---
description: "Display the dependency graph for all stories in the PRD. Shows visual ASCII graph, dependency details, critical path analysis, and detects issues like circular dependencies or bottlenecks."
---

# Hybrid Ralph - Show Dependencies

You are displaying the dependency graph and analysis for all stories in the PRD.

## Path Storage Modes

PRD file location depends on the storage mode:
- **New Mode**: `~/.plan-cascade/<project-id>/prd.json` or in worktree directory
- **Legacy Mode**: `prd.json` in project root or worktree

## Step 1: Verify PRD Exists

```bash
# Get PRD path from PathResolver
PRD_PATH=$(python3 -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_prd_path())" 2>/dev/null || echo "prd.json")

# Also check local prd.json (in worktree)
if [ -f "prd.json" ]; then
    PRD_PATH="prd.json"
elif [ ! -f "$PRD_PATH" ]; then
    echo "ERROR: No PRD found at: $PRD_PATH"
    exit 1
fi
```

## Step 2: Read PRD

Read the PRD file at `$PRD_PATH` to get all stories and their dependencies.

## Step 3: Build Dependency Graph

Create a mapping of:
- Story → Dependencies (what this story depends on)
- Story → Dependents (what depends on this story)
- Story → Depth (how many levels from root)

## Step 4: Detect Issues

### Check for Circular Dependencies

Use depth-first search to detect cycles:

```
For each story:
  Mark as visiting
  For each dependency:
    If dependency is visiting → CYCLE DETECTED
    If dependency not visited → recurse
  Mark as visited
```

### Check for Orphan Stories

Find stories with:
- No dependencies
- No dependents

### Check for Bottlenecks

Find stories with many dependents (threshold: 4+).

## Step 5: Display Dependency Report

```
============================================================
DEPENDENCY GRAPH
============================================================

## Visual Graph

{ASCII tree showing dependencies}

Example:
story-001 (Database Schema)
    │
    ├─── story-002 (User Registration)
    │        │
    │        └─── story-004 (Password Reset)
    │
    └─── story-003 (User Login)
             │
             └─── story-004 (Password Reset)

## Dependency Details

{For each story, show:
  - ID and title
  - Dependencies (if any)
  - Depth level
  - Dependents (if any)
  - Type (root, intermediate, endpoint, orphan, bottleneck)
}

## Analysis

- Maximum depth: {depth} levels
- Critical path length: {count} stories
- Bottleneck stories: {list}
- Circular dependencies: {count or "none detected"}
- Orphan stories: {count or "none detected"}

============================================================
```

## Step 6: Show Warnings (if any)

If issues detected, show them:

### Circular Dependency Warning
```
⚠️ Circular dependency detected:
  story-001 → story-002 → story-003 → story-001

This will prevent any story from starting. Break the cycle by removing a dependency.
```

### Bottleneck Warning
```
⚠️ Bottleneck warning:
  story-001 has 5 dependents
  Consider breaking this story into smaller pieces
```

### Orphan Story Info
```
ℹ️ Orphan story:
  story-005 has no dependencies and nothing depends on it
  This story may not be part of the main workflow
```

## Step 7: Show Usage Tips

```
Using This Information:

For Parallel Execution:
  Root stories (depth 0) can all run in parallel.

For Optimization:
  Bottleneck stories should be kept small to avoid blocking others.

For Planning:
  The critical path shows the minimum number of sequential steps.

Next steps:
  - /plan-cascade:approve - See execution plan with batches
  - /plan-cascade:edit - Modify dependencies
  - /plan-cascade:hybrid-status - Check execution progress
```
