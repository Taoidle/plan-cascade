---
description: "Load an existing PRD file and enter review mode. Supports JSON format with stories, priorities, dependencies, and acceptance criteria."
---

# Hybrid Ralph - Load Manual PRD

You are loading an existing Product Requirements Document (PRD) from a file.

## Step 1: Parse PRD Path

Get the PRD path from arguments:
```
PRD_PATH="{{args|first arg or 'prd.json'}}"
```

## Step 2: Verify PRD Exists

```bash
if [ ! -f "$PRD_PATH" ]; then
    echo "ERROR: PRD file not found: $PRD_PATH"
    echo "Please provide a valid path to a prd.json file"
    exit 1
fi
```

## Step 3: Read and Validate PRD

Read the PRD file and validate structure:

Required fields:
- `metadata.description`
- `goal`
- `stories` array

Each story must have:
- `id`
- `title`
- `description`
- `priority` (high/medium/low)
- `dependencies` (array of story IDs)
- `acceptance_criteria` (array)

If validation fails, show specific errors and suggest fixes.

## Step 4: Copy PRD to Current Directory (if needed)

If the PRD is not already `prd.json` in the current directory:

```bash
if [ "$PRD_PATH" != "prd.json" ]; then
    cp "$PRD_PATH" prd.json
    echo "Copied PRD to prd.json"
fi
```

## Step 5: Initialize Supporting Files (if missing)

Create `findings.md` if it doesn't exist:

```bash
if [ ! -f "findings.md" ]; then
    cat > findings.md << 'EOF'
# Findings

Research and discovery notes will be accumulated here.

Use <!-- @tags: story-id --> to tag sections for specific stories.
EOF
fi
```

Create `progress.txt` if it doesn't exist:

```bash
if [ ! -f "progress.txt" ]; then
    cat > progress.txt << 'EOF'
# Progress Log

Story execution progress will be tracked here.
EOF
fi
```

## Step 6: Display PRD Review

Show a comprehensive PRD review:

```
============================================================
PRD REVIEW
============================================================

## Goal

{goal from PRD}

## Objectives

- {objective 1}
- {objective 2}
...

## Stories Summary

Total Stories: {count}
By Priority:
  High: {count}
  Medium: {count}
  Low: {count}

## All Stories

### story-001: {title} [High]
Description: {description}
Dependencies: {none or list}
Acceptance Criteria:
  - {criterion 1}
  - {criterion 2}
...

### story-002: {title} [Medium]
...

## Execution Batches

Batch 1 (parallel):
  - story-001
  - story-002

Batch 2:
  - story-003 (depends on: story-001)

...

============================================================
```

## Step 7: Show Next Steps

```
PRD loaded successfully!

Next steps:
  - /approve - Approve PRD and start execution
  - /edit - Edit PRD manually
  - /show-dependencies - View dependency graph
```

## Notes

- Validates all dependency references exist
- Shows warnings for orphan stories (no dependencies, nothing depends on them)
- Detects circular dependencies if any exist
