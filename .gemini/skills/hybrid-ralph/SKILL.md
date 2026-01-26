---
name: hybrid-ralph
version: "2.7.7"
description: Hybrid architecture combining Ralph's PRD format with Planning-with-Files' structured approach. Auto-generates PRDs from task descriptions, manages parallel story execution with dependency resolution, and provides context-filtered agents for efficient multi-story development.
user-invocable: true
allowed-tools:
  - Read
  - Write
  - Edit
  - Bash
  - Task
  - Glob
  - Grep
  - AskUserQuestion
hooks:
  PreToolUse:
    # Show current story context for relevant tools
    - matcher: "Write|Edit|Bash"
      hooks:
        - type: command
          command: |
            # Show current story context if we're executing a story
            if [ -f ".current-story" ]; then
              STORY_ID=$(cat .current-story 2>/dev/null || echo "")
              if [ -n "$STORY_ID" ]; then
                echo ""
                echo "=== Current Story: $STORY_ID ==="
                # Show story details from prd.json if available
                if command -v python3 &> /dev/null; then
                  python3 "${CLAUDE_PLUGIN_ROOT}/skills/hybrid-ralph/core/context_filter.py" get-story "$STORY_ID" 2>/dev/null | head -20 || true
                fi
                echo ""
              fi
            fi
  PostToolUse:
    # Remind to update findings after significant operations
    - matcher: "Write|Edit"
      hooks:
        - type: command
          command: |
            if [ -f ".current-story" ]; then
              STORY_ID=$(cat .current-story 2>/dev/null || echo "")
              if [ -n "$STORY_ID" ]; then
                echo "[hybrid-ralph] Consider updating findings.md with this discovery (tag with <!-- @tags: $STORY_ID -->)"
              fi
            fi
  Stop:
    # Verify story completion before stopping
    - hooks:
        - type: command
          command: |
            if [ -f ".current-story" ]; then
              STORY_ID=$(cat .current-story 2>/dev/null || echo "")
              if [ -n "$STORY_ID" ]; then
                echo ""
                echo "=== Story Completion Check ==="
                echo "Current story: $STORY_ID"
                echo "Mark complete in progress.txt with: [COMPLETE] $STORY_ID"
                echo ""
              fi
            fi
---

# Hybrid Ralph + Planning-with-Files

A hybrid architecture combining the best of three approaches:

- **Ralph**: Structured PRD format (prd.json), progress tracking patterns, small task philosophy
- **Planning-with-Files**: 3-file planning pattern (task_plan.md, findings.md, progress.txt), Git Worktree support
- **Claude Code Native**: Task tool with subagents for parallel story execution

## Quick Start

### Automatic PRD Generation

Generate a PRD from your task description:

```
/hybrid:auto Implement a user authentication system with login, registration, and password reset
```

This will:
1. Launch a Planning Agent to analyze your task
2. Generate a PRD with user stories
3. Show the PRD for review
4. Wait for your approval

### Manual PRD Loading

Load an existing PRD file:

```
/hybrid:manual path/to/prd.json
```

### Approval and Execution

After reviewing the PRD:

```
/approve
```

This begins parallel execution of stories according to the dependency graph.

## Architecture

### File Structure

```
project-root/
├── prd.json                 # Product Requirements Document
├── findings.md              # Research findings (tagged by story)
├── progress.txt             # Progress tracking
├── .current-story           # Currently executing story
├── .locks/                  # File locks for concurrent access
└── .agent-outputs/          # Individual agent logs
```

### The PRD Format

The `prd.json` file contains:

- **metadata**: Creation date, version, description
- **goal**: One-sentence project goal
- **objectives**: List of specific objectives
- **stories**: Array of user stories with:
  - `id`: Unique story identifier (story-001, story-002, etc.)
  - `title`: Short story title
  - `description`: Detailed story description
  - `priority`: high, medium, or low
  - `dependencies`: Array of story IDs this story depends on
  - `status`: pending, in_progress, or complete
  - `acceptance_criteria`: List of completion criteria
  - `context_estimate`: small, medium, large, or xlarge
  - `tags`: Array of tags for categorization

### Parallel Execution Model

Stories are organized into **execution batches**:

1. **Batch 1**: Stories with no dependencies (run in parallel)
2. **Batch 2+**: Stories whose dependencies are complete (run in parallel)

```
Batch 1 (Parallel):
  - story-001: Design database schema
  - story-002: Design API endpoints

Batch 2 (After story-001 complete):
  - story-003: Implement database schema

Batch 3 (After story-002, story-003 complete):
  - story-004: Implement API endpoints
```

### Context Filtering

Each agent receives **only relevant context**:

- Their story description and acceptance criteria
- Summaries of completed dependencies
- Findings tagged with their story ID

This keeps context windows focused and efficient.

## Core Python Modules

### context_filter.py

Filters and extracts relevant context for specific stories.

```bash
# Get story details
python3 context_filter.py get-story story-001

# Get context for a story
python3 context_filter.py get-context story-001

# Get execution batch
python3 context_filter.py get-batch 1

# Show full execution plan
python3 context_filter.py plan-batches
```

### state_manager.py

Thread-safe file operations with platform-specific locking.

```bash
# Read PRD
python3 state_manager.py read-prd

# Mark story complete
python3 state_manager.py mark-complete story-001

# Get all story statuses
python3 state_manager.py get-statuses
```

### prd_generator.py

Generates PRD from task descriptions and manages story dependencies.

```bash
# Validate PRD
python3 prd_generator.py validate

# Show execution batches
python3 prd_generator.py batches

# Create sample PRD
python3 prd_generator.py sample
```

### orchestrator.py

Manages parallel execution of stories.

```bash
# Show execution plan
python3 orchestrator.py plan

# Show execution status
python3 orchestrator.py status

# Execute a batch
python3 orchestrator.py execute-batch 1
```

## Commands Reference

### /hybrid:auto

Generate PRD from task description.

```
/hybrid:auto <task description>
```

### /hybrid:manual

Load existing PRD file.

```
/hybrid:manual [path/to/prd.json]
```

### /approve

Approve PRD and begin execution.

```
/approve
```

### /edit

Edit the PRD in your default editor.

```
/edit
```

### /status

Show execution status.

```
/status
```

## Workflows

### Complete Workflow

```
1. /hybrid:auto "Implement feature X"
   ↓
2. Review generated PRD
   ↓
3. /edit (if needed) or /approve
   ↓
4. Agents execute stories in parallel batches
   ↓
5. Monitor with /status
   ↓
6. All stories complete
```

### Manual PRD Workflow

```
1. Create prd.json (or use template)
   ↓
2. /hybrid:manual prd.json
   ↓
3. Review and edit as needed
   ↓
4. /approve
   ↓
5. Execution begins
```

## Findings Tagging

When updating `findings.md`, tag sections with relevant story IDs:

```markdown
<!-- @tags: story-001,story-002 -->

## Database Schema Discovery

The existing schema uses UUIDs for primary keys...
```

This allows agents to receive only relevant findings.

## Progress Tracking

Track progress in `progress.txt`:

```
[2024-01-15 10:00:00] story-001: [IN_PROGRESS] story-001
[2024-01-15 10:15:00] story-001: [COMPLETE] story-001
[2024-01-15 10:15:00] story-002: [IN_PROGRESS] story-002
```

## File Locking

The state manager uses platform-specific locking:

- **Linux/Mac**: fcntl for advisory file locking
- **Windows**: msvcrt for file locking
- **Fallback**: PID-based lock files

Lock files are stored in `.locks/` directory.

## Error Handling

### Validation Errors

If PRD validation fails:

1. Check for duplicate story IDs
2. Verify all dependencies exist
3. Ensure required fields are present
4. Use `/edit` to fix issues

### Execution Failures

If a story fails:

1. Check `.agent-outputs/<story-id>.log`
2. Review progress.txt for error messages
3. Fix issues manually or with agent help
4. Re-run the story

### Dependency Cycles

If dependency cycles are detected:

1. Review `/show-dependencies` output
2. Use `/edit` to break cycles
3. Re-validate with `/hybrid:manual`

## Best Practices

1. **Write Clear Descriptions**: Agents work better with specific, detailed descriptions
2. **Use Acceptance Criteria**: Define what "done" means for each story
3. **Tag Findings**: Always tag findings with relevant story IDs
4. **Update Progress**: Mark stories complete promptly
5. **Review Before Approval**: Always review the PRD before approving

## Integration with Planning-with-Files

This skill integrates seamlessly with planning-with-files:

- **Standard Mode**: Use alongside existing task_plan.md, findings.md, progress.md
- **Worktree Mode**: Each worktree can have its own PRD and story execution
- **Session Recovery**: Resume work after /clear with `/hybrid:manual`

## Templates

Use the provided templates as starting points:

- `templates/prd.json.example` - Example PRD structure
- `templates/prd_review.md` - PRD review display template
- `templates/findings.md` - Structured findings template

## Troubleshooting

### PRD Not Found

```
Error: No PRD found in current directory
```

Solution: Use `/hybrid:auto` to generate or `/hybrid:manual` to load.

### Lock Timeout

```
TimeoutError: Could not acquire lock within 30s
```

Solution: Run `python3 state_manager.py cleanup-locks` to remove stale locks.

### Dependency Not Found

```
Validation Error: Unknown dependency 'story-005'
```

Solution: Edit PRD to fix dependency reference or add missing story.

## See Also

- [planning-with-files](../planning-with-files/SKILL.md) - Base planning skill
- [Ralph Loop](https://github.com/anthropics/ralph-loop) - Original Ralph concept
