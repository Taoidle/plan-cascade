---
description: "Approve the current PRD and begin parallel story execution. Analyzes dependencies, creates execution batches, launches background Task agents for each story, and monitors progress."
---

# Hybrid Ralph - Approve PRD and Execute

You are approving the PRD and starting parallel execution of user stories.

## Step 1: Detect Operating System and Shell

Detect the current operating system to use appropriate commands:

```bash
# Detect OS
OS_TYPE="$(uname -s 2>/dev/null || echo Windows)"
case "$OS_TYPE" in
    Linux*|Darwin*|MINGW*|MSYS*)
        SHELL_TYPE="bash"
        echo "✓ Detected Unix-like environment (bash)"
        ;;
    *)
        # Check if PowerShell is available on Windows
        if command -v pwsh >/dev/null 2>&1 || command -v powershell >/dev/null 2>&1; then
            SHELL_TYPE="powershell"
            echo "✓ Detected Windows environment (PowerShell)"
        else
            SHELL_TYPE="bash"
            echo "✓ Using bash (default)"
        fi
        ;;
esac
```

## Step 2: Ensure Auto-Approval Configuration

Ensure command auto-approval settings are configured (merges with existing settings):

```bash
# Run the settings merge script from project root
python3 ../scripts/ensure-settings.py 2>/dev/null || python3 scripts/ensure-settings.py || echo "Warning: Could not update settings, continuing..."
```

This script intelligently merges required auto-approval patterns with any existing `.claude/settings.local.json`, preserving user customizations.

## Step 3: Verify PRD Exists

Check if `prd.json` exists:

```bash
if [ ! -f "prd.json" ]; then
    echo "ERROR: No PRD found. Please generate one first with:"
    echo "  /planning-with-files:hybrid-auto <description>"
    echo "  /planning-with-files:hybrid-manual <path>"
    exit 1
fi
```

## Step 4: Read and Validate PRD

Read `prd.json` and validate:
- Has `metadata`, `goal`, `objectives`, `stories`
- Each story has `id`, `title`, `description`, `priority`, `dependencies`, `acceptance_criteria`
- All dependency references exist

If validation fails, show errors and suggest `/planning-with-files:edit`.

## Step 5: Calculate Execution Batches

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

## Step 6: Choose Execution Mode

Ask the user to choose between automatic and manual batch progression:

```bash
echo ""
echo "=========================================="
echo "Select Execution Mode"
echo "=========================================="
echo ""
echo "  [1] Auto Mode  - Automatically progress through batches"
echo "                    Pause only on errors"
echo ""
echo "  [2] Manual Mode - Require approval before each batch"
echo "                    Full control and review"
echo ""
echo "=========================================="
read -p "Enter choice [1/2] (default: 1): " MODE_CHOICE
MODE_CHOICE="${MODE_CHOICE:-1}"

if [ "$MODE_CHOICE" = "2" ]; then
    EXECUTION_MODE="manual"
    echo ""
    echo "✓ Manual mode selected"
    echo "  You will be prompted before each batch starts"
else
    EXECUTION_MODE="auto"
    echo ""
    echo "✓ Auto mode selected"
    echo "  Batches will progress automatically (pause on errors)"
fi

# Save mode to config for reference
echo "execution_mode: $EXECUTION_MODE" >> progress.txt
```

## Step 7: Initialize Progress Tracking

Create/initialize `progress.txt`:

```bash
cat > progress.txt << 'EOF'
# Hybrid Ralph Progress

Started: $(date -u +%Y-%m-%dT%H:%M:%SZ)
PRD: prd.json
Total Stories: X
Total Batches: Y
Execution Mode: EXECUTION_MODE

## Batch 1 (In Progress)

EOF
```

Create `.agent-outputs/` directory for agent logs.

## Step 8: Launch Batch 1 Agents

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

Execute all necessary bash/powershell commands directly to complete the story.
Work methodically and document your progress.
```

Launch each agent with `run_in_background: true`.

Store each task_id for monitoring.

## Step 9: Monitor and Progress Through Batches

After launching Batch 1 agents, display:

```
=== Batch 1 Started ===

Launched X parallel agents:
  - story-001: task_id-xxx (running in background)
  - story-002: task_id-yyy (running in background)

Monitoring progress...
Agent logs: .agent-outputs/
Progress log: progress.txt
```

Then execute based on the selected mode:

### If EXECUTION_MODE is "auto":

```bash
# Wait for current batch to complete before launching next batch
CURRENT_BATCH=1
TOTAL_BATCHES=Y

while [ $CURRENT_BATCH -le $TOTAL_BATCHES ]; do
    echo "Waiting for Batch $CURRENT_BATCH to complete..."

    # Poll progress.txt for completion
    while true; do
        COMPLETE_COUNT=$(grep -c "\[COMPLETE\]" progress.txt 2>/dev/null || echo "0")
        ERROR_COUNT=$(grep -c "\[ERROR\]" progress.txt 2>/dev/null || echo "0")
        FAILED_COUNT=$(grep -c "\[FAILED\]" progress.txt 2>/dev/null || echo "0")

        # Calculate expected complete count for this batch
        EXPECTED_COUNT=<calculate from PRD>

        # Check for errors or failures in current batch
        if [ "$ERROR_COUNT" -gt 0 ] || [ "$FAILED_COUNT" -gt 0 ]; then
            echo ""
            echo "⚠️  ISSUES DETECTED IN BATCH $CURRENT_BATCH"
            echo "Errors: $ERROR_COUNT | Failures: $FAILED_COUNT"
            echo ""
            echo "Please review:"
            echo "  - progress.txt for error details"
            echo "  - .agent-outputs/ for agent logs"
            echo ""
            echo "Execution PAUSED. Fix issues and run /planning-with-files:approve to continue."
            exit 1
        fi

        if [ "$COMPLETE_COUNT" -ge "$EXPECTED_COUNT" ]; then
            echo "✓ Batch $CURRENT_BATCH complete!"
            break
        fi

        # Show progress every 10 seconds
        echo "Progress: $COMPLETE_COUNT stories completed..."
        sleep 10

        # IMPORTANT: Do NOT add any timeout or iteration limit
        # Keep polling until all stories in the batch are complete
        # Stories may take varying amounts of time depending on complexity
    done

    # Move to next batch
    CURRENT_BATCH=$((CURRENT_BATCH + 1))

    if [ $CURRENT_BATCH -le $TOTAL_BATCHES ]; then
        echo "=== Auto-launching Batch $CURRENT_BATCH ==="

        # Launch agents for stories in this batch
        # Use the SAME agent prompt format as Batch 1, including:
        # - Story details (id, title, description, acceptance criteria)
        # - Clear instruction to execute bash/powershell commands directly
        # Launch each agent with run_in_background: true
    fi
done

echo ""
echo "=== ALL BATCHES COMPLETE ==="
```

### If EXECUTION_MODE is "manual":

```bash
CURRENT_BATCH=1
TOTAL_BATCHES=Y

while [ $CURRENT_BATCH -le $TOTAL_BATCHES ]; do
    echo "Waiting for Batch $CURRENT_BATCH to complete..."

    # Poll progress.txt for completion
    while true; do
        COMPLETE_COUNT=$(grep -c "\[COMPLETE\]" progress.txt 2>/dev/null || echo "0")
        ERROR_COUNT=$(grep -c "\[ERROR\]" progress.txt 2>/dev/null || echo "0")
        FAILED_COUNT=$(grep -c "\[FAILED\]" progress.txt 2>/dev/null || echo "0")

        EXPECTED_COUNT=<calculate from PRD>

        # Check for errors or failures
        if [ "$ERROR_COUNT" -gt 0 ] || [ "$FAILED_COUNT" -gt 0 ]; then
            echo ""
            echo "⚠️  ISSUES DETECTED IN BATCH $CURRENT_BATCH"
            echo "Errors: $ERROR_COUNT | Failures: $FAILED_COUNT"
            echo ""
            echo "Please review and fix issues before continuing."
            exit 1
        fi

        if [ "$COMPLETE_COUNT" -ge "$EXPECTED_COUNT" ]; then
            echo "✓ Batch $CURRENT_BATCH complete!"
            break
        fi

        echo "Progress: $COMPLETE_COUNT stories completed..."
        sleep 10

        # IMPORTANT: Do NOT add any timeout or iteration limit
        # Keep polling until all stories in the batch are complete
    done

    # Move to next batch
    CURRENT_BATCH=$((CURRENT_BATCH + 1))

    if [ $CURRENT_BATCH -le $TOTAL_BATCHES ]; then
        echo ""
        echo "=========================================="
        echo "Batch $CURRENT_BATCH Ready"
        echo "=========================================="
        echo ""
        echo "Stories in this batch:"
        <list stories>
        echo ""
        read -p "Launch Batch $CURRENT_BATCH? [Y/n]: " CONFIRM
        CONFIRM="${CONFIRM:-Y}"

        if [[ ! "$CONFIRM" =~ ^[Yy]$ ]]; then
            echo "Paused. Run /planning-with-files:approve to continue."
            exit 0
        fi

        echo "=== Launching Batch $CURRENT_BATCH ==="

        # Launch agents for stories in this batch
        # Use the SAME agent prompt format as Batch 1, including:
        # - Story details (id, title, description, acceptance criteria)
        # - Clear instruction to execute bash/powershell commands directly
        # Note: MANUAL mode only controls batch progression, not individual command execution
        # Launch each agent with run_in_background: true
    fi
done

echo ""
echo "=== ALL BATCHES COMPLETE ==="
```

## Step 10: Show Final Status

```
=== All Batches Complete ===

Total Stories: X
Completed: X

All batches have been executed successfully.

Next steps:
  - /planning-with-files:hybrid-status - Verify completion
  - /planning-with-files:hybrid-complete - Finalize and merge
```

## Notes

### Execution Modes

**Auto Mode (default)**:
- Batches progress automatically when previous batch completes
- No manual intervention needed between batches
- Pauses only on errors or failures
- Best for: routine tasks, trusted PRDs, faster execution

**Manual Mode**:
- Requires user approval before launching each batch
- Batch completes → Review → Approve → Next batch starts
- Full control to review between batches
- Best for: critical tasks, complex PRDs, careful oversight

**Note**: In BOTH modes, agents execute bash/powershell commands directly without waiting for confirmation. The execution mode ONLY controls batch-to-batch progression.

### Shared Features

- Each agent runs in the background with its own task_id
- Agents write their findings to `findings.md` tagged with their story ID
- Progress is tracked in `progress.txt` with `[COMPLETE]`, `[ERROR]`, or `[FAILED]` markers
- Agent outputs are logged to `.agent-outputs/{story_id}.log`
- **Pause on errors**: Both modes pause if any agent reports `[ERROR]` or `[FAILED]`
- **Real-time monitoring**: Progress is polled every 10 seconds and displayed
- **Error markers**: Agents should use `[ERROR]` for recoverable issues, `[FAILED]` for blocking problems
- **Resume capability**: After fixing errors, run `/planning-with-files:approve` to continue
