---
description: "Approve the current PRD and begin parallel story execution. Analyzes dependencies, creates execution batches, launches background Task agents for each story, and monitors progress. Usage: /plan-cascade:approve [--agent <name>] [--impl-agent <name>] [--retry-agent <name>] [--no-fallback] [--auto-run]"
---

# Hybrid Ralph - Approve PRD and Execute

You are approving the PRD and starting parallel execution of user stories.

## Path Storage Modes

This command works with both new and legacy path storage modes:

### New Mode (Default)
- PRD file: In worktree directory or `~/.plan-cascade/<project-id>/prd.json`
- State files: `~/.plan-cascade/<project-id>/.state/`
- Agent outputs: In worktree or project root `.agent-outputs/`

### Legacy Mode
- PRD file: In worktree or project root `prd.json`
- State files: In project root
- Agent outputs: In project root `.agent-outputs/`

User-visible files (`progress.txt`, `findings.md`) always remain in the working directory.

## Multi-Agent Collaboration

This command supports multiple AI agents for story execution. The system automatically selects the best agent based on:

1. Command-line parameters (highest priority)
2. Story-level `agent` field in PRD
3. Story type inference (bugfix→codex, refactor→aider)
4. Phase defaults from agents.json
5. Fallback to claude-code (always available)

### Supported Agents

| Agent | Type | Best For |
|-------|------|----------|
| `claude-code` | task-tool | General purpose (default, always available) |
| `codex` | cli | Bug fixes, quick implementations |
| `aider` | cli | Refactoring, code improvements |
| `amp-code` | cli | Alternative implementations |
| `cursor-cli` | cli | IDE-integrated tasks |

### Command Parameters

```
--agent <name>       Global agent override (all stories use this agent)
--impl-agent <name>  Agent for implementation phase
--retry-agent <name> Agent for retry phase (after failures)
--no-fallback        Disable automatic fallback to claude-code
--auto-run           Start execution immediately after approval
```

## Tool Usage Policy (CRITICAL)

**To avoid command confirmation prompts during automatic execution:**

1. **Use Read tool for file reading** - NEVER use `cat` via Bash
   - ✅ `Read("prd.json")`, `Read("progress.txt")`
   - ❌ `Bash("cat prd.json")`

2. **Use Grep tool for content search** - NEVER use `grep` via Bash
   - ✅ `Grep("[COMPLETE]", path="progress.txt")`
   - ❌ `Bash("grep -c '[COMPLETE]' progress.txt")`

3. **Only use Bash for actual system commands:**
   - Git operations
   - File writing: `echo "..." >> progress.txt`
   - Running tests or build commands

4. **For monitoring loops:** Use Read tool to poll `progress.txt`, then parse the content in your response to count markers

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

## Step 2: Parse Agent Parameters

Parse agent-related arguments:

```bash
# Parse arguments
GLOBAL_AGENT=""
IMPL_AGENT=""
RETRY_AGENT=""
NO_FALLBACK=false
AUTO_RUN=false

for arg in $ARGUMENTS; do
    case "$arg" in
        --agent=*) GLOBAL_AGENT="${arg#*=}" ;;
        --impl-agent=*) IMPL_AGENT="${arg#*=}" ;;
        --retry-agent=*) RETRY_AGENT="${arg#*=}" ;;
        --no-fallback) NO_FALLBACK=true ;;
        --auto-run) AUTO_RUN=true ;;
    esac
done

# Also support space-separated format: --agent codex
# Parse with Read tool logic in Claude's response
```

Display agent configuration:
```
Agent Configuration:
  Global Override: ${GLOBAL_AGENT:-"none (use priority chain)"}
  Implementation: ${IMPL_AGENT:-"default"}
  Retry: ${RETRY_AGENT:-"default"}
  Fallback: ${NO_FALLBACK:+"disabled" || "enabled"}
```

## Step 2.5: Load Agent Configuration

Read `agents.json` if it exists to get agent definitions and phase defaults:

```
If agents.json exists:
    Load agent configuration:
    - agents: Map of agent_name → {type, command, args, ...}
    - phase_defaults: {implementation: {...}, retry: {...}, ...}
    - story_type_defaults: {bugfix: "codex", refactor: "aider", ...}
Else:
    Use default: claude-code only
```

## Step 3: Ensure Auto-Approval Configuration

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
    echo "  /plan-cascade:hybrid-auto <description>"
    echo "  /plan-cascade:hybrid-manual <path>"
    exit 1
fi
```

## Step 4: Read and Validate PRD

Read `prd.json` and validate:
- Has `metadata`, `goal`, `objectives`, `stories`
- Each story has `id`, `title`, `description`, `priority`, `dependencies`, `acceptance_criteria`
- All dependency references exist

If validation fails, show errors and suggest `/plan-cascade:edit`.

## Step 4.5: Check for Design Document (Optional)

Check if `design_doc.json` exists and display a summary:

```
If design_doc.json exists:
    Read and display design document summary:

    ────────────────────────────────────────────────────────────────
    📐 DESIGN DOCUMENT DETECTED
    ────────────────────────────────────────────────────────────────
    Title: <overview.title>

    Components: N defined
      • <component1>
      • <component2>
      ...

    Architectural Patterns: M patterns
      • <pattern1> - <rationale>
      • <pattern2> - <rationale>

    Key Decisions: P ADRs
      • ADR-001: <title>
      • ADR-002: <title>

    Story Mappings: Q stories mapped
      ✓ Mapped: story-001, story-002, ...
      ⚠ Unmapped: story-005, story-006, ... (if any)

    Agents will receive relevant design context during execution.
    ────────────────────────────────────────────────────────────────

Else:
    Note: No design document found.
          Consider generating one with /plan-cascade:design-generate
          for better architectural guidance during execution.
```

This summary helps reviewers understand the architectural context that will guide story execution.

## Step 4.6: Detect External Framework Skills

Check for applicable external framework skills based on the project type:

```bash
# Detect and display loaded skills
if command -v python3 &> /dev/null; then
    python3 -c "
import sys
sys.path.insert(0, '${CLAUDE_PLUGIN_ROOT}/src')
from plan_cascade.core.external_skill_loader import ExternalSkillLoader
from pathlib import Path

loader = ExternalSkillLoader(Path('.'))
skills = loader.detect_applicable_skills(verbose=True)
if skills:
    loader.display_skills_summary('implementation')
else:
    print('[ExternalSkillLoader] No matching framework skills detected')
    print('                      (Skills are auto-loaded based on package.json/Cargo.toml)')
" 2>/dev/null || echo "Note: External skills detection skipped"
fi
```

This will display:
```
┌──────────────────────────────────────────────────────────┐
│  EXTERNAL FRAMEWORK SKILLS LOADED                        │
├──────────────────────────────────────────────────────────┤
│  ✓ React Best Practices (source: vercel)                │
│  ✓ Web Design Guidelines (source: vercel)               │
├──────────────────────────────────────────────────────────┤
│  Phase: implementation | Total: 2 skill(s)              │
└──────────────────────────────────────────────────────────┘
```

If no skills are detected, this is normal for projects without matching frameworks.

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

## Step 8: Launch Batch Agents with Multi-Agent Support

For each story in the current batch, resolve the agent and launch execution.

### 8.1: Agent Resolution for Each Story

For each story, resolve agent using this priority chain:

```
1. GLOBAL_AGENT (--agent parameter)     → Use if specified
2. IMPL_AGENT (--impl-agent parameter)  → Use for implementation phase
3. story.agent (from PRD)               → Use if specified in story
4. Story type inference:
   - Check story.tags for: bugfix, refactor, test, feature
   - Check story.title for keywords:
     - "fix", "bug", "error" → bugfix → prefer codex
     - "refactor", "cleanup", "optimize" → refactor → prefer aider
     - "test", "spec" → test → prefer claude-code
     - "add", "create", "implement" → feature → prefer claude-code
5. Phase default from agents.json       → implementation.default_agent
6. Fallback chain from agents.json      → implementation.fallback_chain
7. claude-code                          → Ultimate fallback (always available)
```

### 8.2: Agent Availability Check

Before using a CLI agent, verify it's available:

```
For agent_name in resolved_chain:
    If agent_name == "claude-code":
        AVAILABLE = true (always)
    Elif agent.type == "cli":
        Check if agent.command exists in PATH:
        - Unix: which {command}
        - Windows: where {command}
        If not found AND NO_FALLBACK == false:
            Continue to next agent in chain
        Elif not found AND NO_FALLBACK == true:
            ERROR: Agent {agent_name} not available and fallback disabled

    If AVAILABLE:
        RESOLVED_AGENT = agent_name
        Break
```

### 8.3: Launch Story Execution

For each story in the batch:

```
# Resolve agent for this story
RESOLVED_AGENT = resolve_agent(story, phase="implementation")

# Log agent selection
echo "[{story_id}] Using agent: {RESOLVED_AGENT}"
echo "[AGENT] {story_id} -> {RESOLVED_AGENT}" >> progress.txt

# Build prompt
PROMPT = """
You are executing story {story_id}: {title}

Description:
{description}

Acceptance Criteria:
- {criterion1}
- {criterion2}

Dependencies: {dependencies or "None"}

{If design_doc.json exists and has story_mappings[story_id]:}
## Technical Design Context

Components to use:
{list of relevant components from design_doc}

Patterns to follow:
{list of relevant patterns}

Relevant decisions:
{list of relevant ADRs}
{End if}

{If external skills were detected for the project:}
## Framework-Specific Best Practices

{Inject skill content from ExternalSkillLoader.get_skill_context("implementation")}

(Note: Follow these framework-specific guidelines when implementing)
{End if}

Your task:
1. Read relevant code and documentation
2. Implement the story according to acceptance criteria
3. Test your implementation
4. Update findings.md with discoveries (use <!-- @tags: {story_id} -->)
5. Mark complete by appending to progress.txt: [COMPLETE] {story_id}

Execute all necessary bash/powershell commands directly to complete the story.
Work methodically and document your progress.
"""

# Execute based on agent type
If RESOLVED_AGENT == "claude-code":
    # Use Task tool (built-in)
    Task(
        prompt=PROMPT,
        subagent_type="general-purpose",
        run_in_background=true
    )
    Store task_id for monitoring

Elif agents[RESOLVED_AGENT].type == "cli":
    # Use CLI agent via Bash
    agent_config = agents[RESOLVED_AGENT]
    command = agent_config.command  # e.g., "aider", "codex"
    args = agent_config.args        # e.g., ["--message", "{prompt}"]

    # Replace {prompt} placeholder with actual prompt
    # Replace {working_dir} with current directory
    full_command = f"{command} {' '.join(args)}"

    Bash(
        command=full_command,
        run_in_background=true,
        timeout=agent_config.timeout or 600000
    )
    Store task_id for monitoring
```

### 8.4: Display Agent Launch Summary

```
=== Batch {N} Launched ===

Stories and assigned agents:
  - story-001: claude-code (task-tool)
  - story-002: aider (cli) [refactor detected]
  - story-003: codex (cli) [bugfix detected]

{If any fallbacks occurred:}
⚠️ Agent fallbacks:
  - story-002: aider → claude-code (aider CLI not found)
{End if}

Waiting for completion...
```

## Step 9: Wait for Batch Completion (Using TaskOutput)

After launching batch agents, wait for completion:

**CRITICAL**: Use TaskOutput to wait instead of sleep+grep polling. This avoids Bash confirmation prompts.

### 9.1: Wait for Current Batch Agents

```
For each story_id, task_id in current_batch_tasks:
    echo "Waiting for {story_id}..."

    result = TaskOutput(
        task_id=task_id,
        block=true,
        timeout=600000  # 10 minutes per story
    )

    echo "✓ {story_id} agent completed"
```

### 9.2: Verify Batch Completion

After all TaskOutput calls return, verify using Read tool (NOT Bash grep):

```
# Use Read tool to get progress.txt content
progress_content = Read("progress.txt")

# Parse content yourself in your response
complete_count = count occurrences of "[COMPLETE]" in progress_content
error_count = count occurrences of "[ERROR]" in progress_content
failed_count = count occurrences of "[FAILED]" in progress_content

if error_count > 0 or failed_count > 0:
    echo "⚠️ ISSUES DETECTED IN BATCH"
    # Show which stories failed
    # Offer retry with different agent (see Step 9.2.5)

if complete_count >= expected_count:
    echo "✓ Batch complete!"
```

### 9.2.5: Retry Failed Stories with Different Agent

When a story fails, offer to retry with a different agent:

```
For each failed_story in failed_stories:
    # Determine retry agent
    If RETRY_AGENT specified:
        retry_agent = RETRY_AGENT
    Elif phase_defaults.retry.default_agent in agents.json:
        retry_agent = phase_defaults.retry.default_agent
    Else:
        retry_agent = "claude-code"

    # Check if retry agent is different from original
    original_agent = get_original_agent(failed_story)

    If retry_agent != original_agent AND is_agent_available(retry_agent):
        echo "Retrying {story_id} with {retry_agent} (was: {original_agent})"

        # Build retry prompt with error context
        RETRY_PROMPT = """
        You are RETRYING story {story_id}: {title}

        PREVIOUS ATTEMPT FAILED. Error details:
        {error_message from progress.txt}

        Please analyze what went wrong and try a different approach.
        {rest of original prompt}
        """

        # Launch retry agent
        Launch agent (retry_agent) with RETRY_PROMPT
        echo "[RETRY] {story_id} -> {retry_agent}" >> progress.txt
    Else:
        echo "Cannot retry {story_id}: no alternative agent available"
        # Pause for manual intervention
```

### 9.3: Progress to Next Batch

**AUTO MODE**: Automatically launch next batch

```
if more batches remain:
    echo "=== Auto-launching Batch {N+1} ==="

    # Launch agents for next batch stories
    # Store new task_ids
    # Go back to Step 9.1 to wait for them
```

**MANUAL MODE**: Ask before launching next batch

```
if more batches remain:
    echo "Batch {N+1} Ready"
    echo "Stories: {list}"

    # Use AskUserQuestion tool to ask for confirmation
    # If confirmed, launch next batch agents
    # Go back to Step 9.1
```

### Why TaskOutput Instead of Polling?

| Method | Confirmation Prompts | How it works |
|--------|---------------------|--------------|
| `sleep + grep` loop | YES - every iteration | Bash commands need confirmation |
| `TaskOutput(block=true)` | NO | Native wait for agent completion |

TaskOutput is the correct way to wait for background agents.

## Step 10: Show Final Status

```
=== All Batches Complete ===

Total Stories: X
Completed: X

All batches have been executed successfully.

Next steps:
  - /plan-cascade:hybrid-status - Verify completion
  - /plan-cascade:hybrid-complete - Finalize and merge
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
- **Resume capability**: After fixing errors or interruption, run `/plan-cascade:hybrid-resume --auto` to intelligently resume
  - Auto-detects completed stories and skips them
  - Works with both old and new progress markers
  - Or run `/plan-cascade:approve` to restart current batch
