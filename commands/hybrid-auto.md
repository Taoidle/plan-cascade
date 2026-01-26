---
name: planning-with-files:hybrid-auto
description: Generate PRD from task description and enter review mode. Auto-generates user stories with priorities, dependencies, and acceptance criteria for parallel execution.
disable-model-invocation: true
---

# /planning-with-files:hybrid-auto

Automatically generate a Product Requirements Document (PRD) from your task description and enter review mode.

## Usage

```
/planning-with-files:hybrid-auto <task description>
```

## What It Does

1. **Parses your task description** - Takes your natural language task description
2. **Launches Planning Agent** - Uses Claude Code's Task tool to analyze and plan
3. **Generates PRD draft** - Creates prd.json with:
   - Goal and objectives
   - User stories with priorities
   - Dependency analysis
   - Context size estimates
4. **Enters review mode** - Shows the PRD for your approval

## Example

```
/planning-with-files:hybrid-auto Implement a user authentication system with login, registration, and password reset
```

## After PRD Generation

You'll see the PRD review with options to:
- `/planning-with-files:approve` - Accept the PRD and start execution
- `/planning-with-files:edit` - Open prd.json in your editor for manual changes
- `/planning-with-files:replan` - Regenerate the PRD with different parameters

## Notes

- The Planning Agent will analyze your codebase to understand existing patterns
- Stories are automatically prioritized (high/medium/low)
- Dependencies are detected between stories
- Context estimates help agents work efficiently

## See Also

- `/planning-with-files:hybrid-manual` - Load an existing PRD file
- `/planning-with-files:approve` - Approve the current PRD
- `/planning-with-files:edit` - Edit the current PRD
