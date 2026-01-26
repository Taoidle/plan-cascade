---
description: "Approve the current PRD and begin parallel story execution. Analyzes dependencies, creates execution batches, launches background Task agents for each story, and monitors progress."
---

# Hybrid Ralph - Approve PRD and Execute

You are approving the PRD and starting parallel execution of user stories.

## Step 1: Verify PRD Exists

Check if `prd.json` exists:

```bash
if [ ! -f "prd.json" ]; then
    echo "ERROR: No PRD found. Please generate one first with:"
    echo "  /planning-with-files:hybrid-auto <description>"
    echo "  /planning-with-files:hybrid-manual <path>"
    exit 1
fi
```

## Step 2: Read and Validate PRD

Read `prd.json` and validate:
- Has `metadata`, `goal`, `objectives`, `stories`
- Each story has `id`, `title`, `description`, `priority`, `dependencies`, `acceptance_criteria`
- All dependency references exist

If validation fails, show errors and suggest `/planning-with-files:edit`.

## Step 3: Calculate Execution Batches

Analyze story dependencies and create parallel execution batches:

- **Batch 1**: Stories with no dependencies (can run in parallel)
- **Batch 2**: Stories that depend only on Batch 1 stories
- **Batch 3+**: Continue until all stories are batched

Display the execution plan:
```
=== Execution Plan ===

Total Stories: X
Total Batches: Y

Batch 1 (can run in parallel):
  - story-001: Title
  - story-002: Title

Batch 2:
  - story-003: Title (depends on: story-001)

...
```

## Step 4: Initialize Progress Tracking

Create/initialize `progress.txt`:

```bash
cat > progress.txt << 'EOF'
# Hybrid Ralph Progress

Started: $(date -u +%Y-%m-%dT%H:%M:%SZ)
PRD: prd.json
Total Stories: X
Total Batches: Y

## Batch 1 (In Progress)

EOF
```

Create `.agent-outputs/` directory for agent logs.

## Step 5: Launch Batch 1 Agents

For each story in Batch 1, launch a background Task agent:

For each story:
```
You are executing story {story_id}: {title}

Description:
{description}

Acceptance Criteria:
- {criterion1}
- {criterion2}

Dependencies: None

Your task:
1. Read relevant code and documentation
2. Implement the story according to acceptance criteria
3. Test your implementation
4. Update findings.md with discoveries (use <!-- @tags: {story_id} -->)
5. Mark complete by appending to progress.txt: [COMPLETE] {story_id}

Work methodically and document your progress.
```

Launch each agent with `run_in_background: true`.

Store each task_id for monitoring.

## Step 6: Monitor Progress

After launching Batch 1 agents, display:

```
=== Batch 1 Started ===

Launched X parallel agents:
  - story-001: task_id-xxx (running in background)
  - story-002: task_id-yyy (running in background)

Monitor with: /planning-with-files:hybrid-status

Agent logs: .agent-outputs/
Progress log: progress.txt
```

## Step 7: Show Next Steps

```
Next steps:
  - /planning-with-files:hybrid-status - Check execution progress
  - /planning-with-files:show-dependencies - View dependency graph
  - Wait for Batch 1 to complete, then approve Batch 2
```

## Notes

- Each agent runs in the background with its own task_id
- Agents write their findings to `findings.md` tagged with their story ID
- Progress is tracked in `progress.txt` with `[COMPLETE]` markers
- Agent outputs are logged to `.agent-outputs/{story_id}.log`
- You must manually approve each subsequent batch (for human oversight)
