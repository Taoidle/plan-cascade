---
name: hybrid:auto
description: Generate PRD from task description and enter review mode
---

# /hybrid:auto

Automatically generate a Product Requirements Document (PRD) from your task description and enter review mode.

## Usage

```
/hybrid:auto <task description>
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
/hybrid:auto Implement a user authentication system with login, registration, and password reset
```

## After PRD Generation

You'll see the PRD review with options to:
- `/approve` - Accept the PRD and start execution
- `/edit` - Open prd.json in your editor for manual changes
- `/hybrid:replan` - Regenerate the PRD with different parameters

## Notes

- The Planning Agent will analyze your codebase to understand existing patterns
- Stories are automatically prioritized (high/medium/low)
- Dependencies are detected between stories
- Context estimates help agents work efficiently

## See Also

- `/hybrid:manual` - Load an existing PRD file
- `/approve` - Approve the current PRD
- `/edit` - Edit the current PRD
