---
description: "Generate PRD from task description and enter review mode. Auto-generates user stories with priorities, dependencies, and acceptance criteria for parallel execution."
---

# Hybrid Ralph - Auto Generate PRD

You are automatically generating a Product Requirements Document (PRD) from the task description.

## Step 1: Parse Task Description

Get the task description from user arguments:
```
TASK_DESC="{{args}}"
```

If no description provided, ask the user for it.

## Step 2: Use Task Tool to Generate PRD

Use Claude Code's Task tool with a planning agent to analyze and generate the PRD:

```
You are a PRD generation specialist. Your task is to:

1. ANALYZE the task description: "$TASK_DESC"
2. EXPLORE the codebase to understand:
   - Existing patterns and conventions
   - Relevant code files
   - Architecture and structure
3. GENERATE a PRD (prd.json) with:
   - Clear goal statement
   - 3-7 user stories
   - Each story with: id, title, description, priority (high/medium/low), dependencies, acceptance_criteria, context_estimate (small/medium/large)
   - Dependencies between stories (where one story must complete before another)
4. SAVE the PRD to prd.json in the current directory

The PRD format must be:
{
  "metadata": {
    "created_at": "ISO-8601 timestamp",
    "version": "1.0.0",
    "description": "Task description"
  },
  "goal": "One sentence goal",
  "objectives": ["obj1", "obj2"],
  "stories": [
    {
      "id": "story-001",
      "title": "Story title",
      "description": "Detailed description",
      "priority": "high",
      "dependencies": [],
      "status": "pending",
      "acceptance_criteria": ["criterion1", "criterion2"],
      "context_estimate": "medium",
      "tags": ["feature", "api"]
    }
  ]
}

Work methodically and create a well-structured PRD.
```

Launch this as a background task with `run_in_background: true`.

## Step 3: Wait for PRD Generation

IMPORTANT: After launching the background task, you MUST use the TaskOutput tool to wait for completion:

1. Launch the Task tool with run_in_background: true
2. Store the returned task_id
3. Immediately call TaskOutput with:
   - task_id: <the task_id from step 2>
   - block: true (wait for completion)
   - timeout: 600000 (10 minutes)

Example pattern:
```
Launch Task tool with run_in_background: true → Get task_id → TaskOutput(task_id, block=true)
```

DO NOT use sleep loops or polling. The TaskOutput tool with block=true will properly wait for the agent to complete.

## Step 4: Validate and Display PRD

Once the task completes:

1. Read the generated `prd.json` file
2. Validate the structure (check for required fields)
3. Display a PRD review summary showing:
   - Goal and objectives
   - All stories with IDs, titles, priorities
   - Dependency graph (ASCII)
   - Execution batches

## Step 5: Show Next Steps

After displaying the PRD review, tell the user their options:

```
PRD generated successfully!

Next steps:
  - /planning-with-files:approve - Approve PRD and start parallel execution
  - /planning-with-files:edit - Edit PRD manually
  - /planning-with-files:show-dependencies - View dependency graph
```

## Notes

- If PRD validation fails, show errors and suggest `/planning-with-files:edit` to fix manually
- The planning agent may take time to explore the codebase - be patient
- Generated PRD is a draft - user should review and can edit before approving
