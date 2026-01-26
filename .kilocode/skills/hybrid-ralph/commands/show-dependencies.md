---
name: hybrid:show-dependencies
description: Display the dependency graph for all stories in the PRD
---

# /show-dependencies

Display a visual representation of the dependency graph for all stories in the PRD.

## Usage

```
/show-dependencies
```

## What It Shows

### ASCII Dependency Graph
Visual representation of how stories depend on each other using ASCII art.

### Dependency Details
- Each story's dependencies
- Dependency depth (how many levels deep)
- Critical path (stories that block others)

### Potential Issues
- Circular dependencies
- Orphan stories (no dependencies, nothing depends on them)
- Bottleneck stories (many stories depend on them)

## Example Output

```
============================================================
DEPENDENCY GRAPH
============================================================

## Visual Graph

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

story-001: Design database schema
  Dependencies: None
  Depth: 0 (Root)
  Dependents: story-002, story-003
  └─ Critical path story

story-002: Implement user registration
  Dependencies: story-001
  Depth: 1
  Dependents: story-004

story-003: Implement user login
  Dependencies: story-001
  Depth: 1
  Dependents: story-004

story-004: Implement password reset
  Dependencies: story-002, story-003
  Depth: 2
  Dependents: None
  └─ Endpoint story

## Analysis

- Maximum depth: 2 levels
- Critical path length: 3 stories
- Bottleneck stories: story-001 (2 dependents)
- No circular dependencies detected
- No orphan stories detected

============================================================
```

## Reading the Graph

### Arrows (│, ├───, └───)
Show the dependency relationship:
- `│` - Vertical connection
- `├───` - Horizontal branch
- `└───` - Last branch

### Depth Levels
- **Depth 0**: Root stories (no dependencies)
- **Depth 1**: Stories that depend on root stories
- **Depth 2+**: Stories with chained dependencies

### Story Types

| Type | Description |
|------|-------------|
| Root story | No dependencies, can start immediately |
| Intermediate story | Has dependencies and dependents |
| Endpoint story | Has dependencies, no dependents |
| Orphan story | No dependencies, nothing depends on it |
| Bottleneck story | Many stories depend on it |

## Issue Detection

### Circular Dependencies
```
⚠️ Circular dependency detected:
  story-001 → story-002 → story-003 → story-001
```

This will prevent any story from starting. Break the cycle by removing a dependency.

### Bottleneck Warnings
```
⚠️ Bottleneck warning:
  story-001 has 5 dependents
  Consider breaking this story into smaller pieces
```

### Orphan Stories
```
ℹ️ Orphan story:
  story-005 has no dependencies and nothing depends on it
  This story may not be part of the main workflow
```

## Using This Information

### For Parallel Execution
Root stories (depth 0) can all run in parallel.

### For Optimization
Bottleneck stories should be kept small to avoid blocking others.

### For Planning
The critical path shows the minimum number of sequential steps.

## See Also

- `/approve` - See execution plan with batches
- `/status` - Check execution progress
- `/edit` - Modify dependencies
