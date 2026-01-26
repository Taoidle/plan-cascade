---
name: planning-with-files:hybrid-manual
description: Load an existing PRD file and enter review mode. Supports JSON format with stories, priorities, dependencies, and acceptance criteria.
disable-model-invocation: true
---

# /planning-with-files:hybrid-manual

Load an existing Product Requirements Document (PRD) from a file and enter review mode.

## Usage

```
/planning-with-files:hybrid-manual [path/to/prd.json]
```

If no path is provided, looks for `prd.json` in the current directory.

## What It Does

1. **Reads the PRD file** - Loads and parses the JSON file
2. **Validates structure** - Checks for required fields
3. **Shows PRD review** - Displays the plan for your approval

## Example

```
/planning-with-files:hybrid-manual ./prd.json
```

or simply:

```
/planning-with-files:hybrid-manual
```

## PRD Format

The PRD file should follow this structure:

```json
{
  "metadata": {
    "created_at": "2024-01-15T10:00:00",
    "version": "1.0.0",
    "description": "Task description"
  },
  "goal": "One sentence goal",
  "objectives": ["Objective 1", "Objective 2"],
  "stories": [
    {
      "id": "story-001",
      "title": "Story title",
      "description": "Story description",
      "priority": "high",
      "dependencies": [],
      "status": "pending",
      "acceptance_criteria": ["Criteria 1", "Criteria 2"],
      "context_estimate": "medium",
      "tags": ["tag1", "tag2"]
    }
  ]
}
```

## After Loading

You'll see the PRD review with options to:
- `/planning-with-files:approve` - Accept the PRD and start execution
- `/planning-with-files:edit` - Open prd.json in your editor for changes
- `/planning-with-files:show-dependencies` - View the dependency graph

## Validation

The command will check for:
- Required metadata fields
- Valid story IDs
- Existing dependencies
- Proper priority values (high/medium/low)

## See Also

- `/planning-with-files:hybrid-auto` - Generate PRD from description
- `/planning-with-files:approve` - Approve the current PRD
- `/planning-with-files:show-dependencies` - View dependency graph
