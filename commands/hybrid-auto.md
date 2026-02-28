---
description: "Generate PRD from task description and enter review mode. Auto-generates user stories with priorities, dependencies, and acceptance criteria for parallel execution. Usage: /plan-cascade:hybrid-auto [--flow <quick|standard|full>] [--tdd <off|on|auto>] [--confirm] [--no-confirm] [--spec <off|auto|on>] [--first-principles] [--max-questions N] [--agent <name>] <task description> [design-doc-path]"
---

# Hybrid Ralph - Auto Generate PRD

You are automatically generating a Product Requirements Document (PRD) from the task description.

## Execution Flow Parameters

This command accepts flow control parameters that affect story execution:

### Parameter Priority

Parameters are saved to `prd.json` and later used by `/plan-cascade:approve`. Priority order:

1. **Command-line flags to THIS command** (highest priority)
   - Example: `/plan-cascade:hybrid-auto --flow full "task"`
   - Saved to `prd.json` as `flow_config`, `tdd_config`, etc.

2. **Command-line flags to `/plan-cascade:approve`**
   - Can override values saved in `prd.json`
   - Example: Even if prd.json has `flow_config.level="standard"`, running `/plan-cascade:approve --flow full` will use `full`

3. **Default values** (lowest priority)
   - Used when no flags provided to either command

**Example Flow:**
```bash
# Step 1: Generate PRD with parameters
/plan-cascade:hybrid-auto --flow full --tdd on "Build auth"
# → Saves to prd.json: flow_config.level="full", tdd_config.mode="on"

# Step 2a: Approve with saved parameters
/plan-cascade:approve
# → Uses flow="full", tdd="on" from prd.json

# Step 2b: Approve with override
/plan-cascade:approve --flow standard
# → Uses flow="standard" (overrides prd.json), tdd="on" (from prd.json)
```

**Note:** This command writes parameters to `prd.json` in Step 5. The recommended `/plan-cascade:approve` command (shown in Step 6) includes the same parameters for clarity, but they're already in prd.json.

### `--flow <quick|standard|full>`

Override the execution flow depth for the approve phase.

| Flow | Gate Mode | AI Verification | Code Review | Test Enforcement |
|------|-----------|-----------------|-------------|------------------|
| `quick` | soft | disabled | no | no |
| `standard` | soft | enabled | no | no |
| `full` | **hard** | enabled | **required** | **required** |

### `--tdd <off|on|auto>`

Control Test-Driven Development mode for story execution.

| Mode | Description |
|------|-------------|
| `off` | TDD disabled |
| `on` | TDD enabled with prompts and compliance checks |
| `auto` | Automatically decide based on risk assessment (default) |

### `--confirm`

Require confirmation before each batch execution in the approve phase.

### `--no-confirm`

Explicitly disable batch confirmation, even if FULL flow would normally require it. Useful for CI/automated environments where you want strict quality gates but no interactive prompts.

**Precedence**: `--no-confirm` overrides `--confirm` and FULL flow's default confirmation requirement.

### `--spec <off|auto|on>`

Enable a planning-time **spec interview** to produce `spec.json/spec.md` before finalizing `prd.json`.

- `auto` (default): enabled when `--flow full`, otherwise disabled
- `on`: always run interview before PRD finalization
- `off`: never run interview

### `--first-principles`

Ask 3–5 first-principles questions before detailed spec questions (only when spec interview runs).

### `--max-questions N`

Soft cap for interview length (recorded in `.state/spec-interview.json`).

## Prerequisites Check

**CRITICAL**: If this is your first time using Plan Cascade, run `/plan-cascade:init` first to set up the environment.

```bash
# Quick check - if this fails, run /plan-cascade:init
uv run python -c "print('Environment OK')" 2>/dev/null || echo "Warning: Run /plan-cascade:init to set up environment"
```

## Path Storage Modes

Plan Cascade supports two path storage modes:

### New Mode (Default)
Runtime files stored in user directory:
- **Windows**: `%APPDATA%/plan-cascade/<project-id>/`
- **Unix/macOS**: `~/.plan-cascade/<project-id>/`

Files created:
- `prd.json`: `<user-dir>/prd.json` (or in worktree if in worktree mode)
- `design_doc.json`: In project root (user-visible)
- State files: `<user-dir>/.state/`

### Legacy Mode
All files in project root or worktree directory.

User-visible files (`progress.txt`, `findings.md`) always remain in the working directory.

## Multi-Agent Support

PRD generation can use different AI agents:

| Agent | Type | Notes |
|-------|------|-------|
| `claude-code` | task-tool | Default, always available |
| `codex` | cli | Good for structured PRD generation |
| `aider` | cli | Alternative for PRD generation |

### Command Parameters

```
--agent <name>    Agent to use for PRD and design doc generation
```

## Tool Usage Policy (CRITICAL)

**To avoid command confirmation prompts:**

1. **Use Read tool for file reading** - NEVER use `cat` via Bash
   - ✅ `Read("prd.json")`
   - ❌ `Bash("cat prd.json")`

2. **Use Write tool for file creation** - When creating prd.json
   - ✅ `Write("prd.json", content)`
   - Or have the Task agent write it directly

## Step 0: Ensure .gitignore Configuration

**IMPORTANT**: Before creating any planning files, ensure the project's `.gitignore` is configured to ignore Plan Cascade temporary files:

```bash
# Check and update .gitignore for Plan Cascade entries
uv run python -c "from plan_cascade.utils.gitignore import ensure_gitignore; from pathlib import Path; ensure_gitignore(Path.cwd())" 2>/dev/null || echo "Note: Could not auto-update .gitignore"
```

This prevents planning files from being accidentally committed to version control.

## Step 1: Parse Arguments

Parse user arguments:
- **Task description**: First positional argument (required)
- **Design doc path**: Second positional argument (optional) - external design document to convert
- **--flow**: Execution flow depth (quick|standard|full)
- **--tdd**: TDD mode (off|on|auto)
- **--confirm**: Require batch confirmation
- **--no-confirm**: Disable batch confirmation (overrides --confirm and FULL flow default)
- **--spec**: Spec interview mode (off|auto|on)
- **--first-principles**: Enable first-principles questions (when spec interview runs)
- **--max-questions**: Soft cap for interview length
- **--agent**: Optional agent override for PRD generation

```
TASK_DESC=""
DESIGN_DOC_ARG=""
PRD_AGENT=""
FLOW_LEVEL=""           # --flow <quick|standard|full>
TDD_MODE=""             # --tdd <off|on|auto>
CONFIRM_MODE=false      # --confirm
NO_CONFIRM_MODE=false   # --no-confirm (overrides --confirm and FULL flow default)
CONFIRM_EXPLICIT=false  # set when --confirm is provided
NO_CONFIRM_EXPLICIT=false  # set when --no-confirm is provided
SPEC_MODE=""            # --spec <off|auto|on>
FIRST_PRINCIPLES=false  # --first-principles
MAX_QUESTIONS=""        # --max-questions N

# Parse flags and positional arguments
for arg in $ARGUMENTS; do
    case "$arg" in
        --flow=*) FLOW_LEVEL="${arg#*=}" ;;
        --flow) NEXT_IS_FLOW=true ;;
        --tdd=*) TDD_MODE="${arg#*=}" ;;
        --tdd) NEXT_IS_TDD=true ;;
        --confirm) CONFIRM_MODE=true; CONFIRM_EXPLICIT=true ;;
        --no-confirm) NO_CONFIRM_MODE=true; NO_CONFIRM_EXPLICIT=true ;;
        --spec=*) SPEC_MODE="${arg#*=}" ;;
        --spec) NEXT_IS_SPEC=true ;;
        --first-principles) FIRST_PRINCIPLES=true ;;
        --max-questions=*) MAX_QUESTIONS="${arg#*=}" ;;
        --max-questions) NEXT_IS_MAXQ=true ;;
        --agent=*) PRD_AGENT="${arg#*=}" ;;
        --agent) NEXT_IS_AGENT=true ;;
        *)
            # Handle space-separated flag values
            if [ "$NEXT_IS_FLOW" = true ]; then
                FLOW_LEVEL="$arg"
                NEXT_IS_FLOW=false
            elif [ "$NEXT_IS_TDD" = true ]; then
                TDD_MODE="$arg"
                NEXT_IS_TDD=false
            elif [ "$NEXT_IS_SPEC" = true ]; then
                SPEC_MODE="$arg"
                NEXT_IS_SPEC=false
            elif [ "$NEXT_IS_MAXQ" = true ]; then
                MAX_QUESTIONS="$arg"
                NEXT_IS_MAXQ=false
            elif [ "$NEXT_IS_AGENT" = true ]; then
                PRD_AGENT="$arg"
                NEXT_IS_AGENT=false
            elif [ -z "$TASK_DESC" ]; then
                TASK_DESC="$arg"
            elif [ -z "$DESIGN_DOC_ARG" ]; then
                DESIGN_DOC_ARG="$arg"
            fi
            ;;
    esac
done

# --no-confirm takes precedence
If NO_CONFIRM_MODE is true:
    CONFIRM_MODE = false
Elif FLOW_LEVEL == "full" AND CONFIRM_EXPLICIT is false:
    # Default confirmations in FULL flow
    CONFIRM_MODE = true

# Display parsed parameters with sources
echo "============================================================"
echo "PARSED PARAMETERS (all from command-line)"
echo "============================================================"
echo "Task: $TASK_DESC"
echo "Flow: ${FLOW_LEVEL:-"(not specified - will use default)"}"
echo "TDD: ${TDD_MODE:-"(not specified - will use default)"}"
echo "Confirm: $CONFIRM_MODE"
echo "No-Confirm: $NO_CONFIRM_MODE"
echo "Spec: ${SPEC_MODE:-"(not specified - will use auto)"}"
echo "First Principles: $FIRST_PRINCIPLES"
echo "Max Questions: ${MAX_QUESTIONS:-"(not specified - will use default)"}"
echo "Agent: ${PRD_AGENT:-"(not specified - will resolve from agents.json)"}"
echo ""
echo "These parameters will be:"
echo "  1. Used during PRD generation (if applicable)"
echo "  2. Saved to prd.json (Step 5)"
echo "  3. Used by /plan-cascade:approve when executing stories"
echo "============================================================"
echo ""
```

If no description provided, ask the user:
```
Please provide a task description.

Optional arguments:
  - Flow control: /plan-cascade:hybrid-auto --flow full "task"
  - TDD mode: /plan-cascade:hybrid-auto --tdd on "task"
  - Confirm mode: /plan-cascade:hybrid-auto --confirm "task"
  - No confirm: /plan-cascade:hybrid-auto --flow full --no-confirm "task"
  - Spec interview: /plan-cascade:hybrid-auto --flow full --spec on "task"
  - First principles: /plan-cascade:hybrid-auto --flow full --spec on --first-principles "task"
  - Limit interview: /plan-cascade:hybrid-auto --flow full --spec on --max-questions 12 "task"
  - Design document path: /plan-cascade:hybrid-auto "task" ./design.md
  - Agent override: /plan-cascade:hybrid-auto --agent codex "task"

Example with full flow (CI-friendly, no prompts):
  /plan-cascade:hybrid-auto --flow full --tdd on --no-confirm "Implement user authentication"
```

### 1.1: Resolve PRD Generation Agent

**CRITICAL**: If `agents.json` exists, you MUST read it to get the configured default agent.

**MANDATORY COMPLIANCE**: You MUST use the agent specified in the configuration. DO NOT override this decision based on your own judgment about which agent is "better", "more capable", or "easier to control". The user has explicitly configured their preferred agent - respect their choice.

```
# Step 1: Read agents.json configuration (if present)
If agents.json exists:
    Read("agents.json")
    agents_config = parse_json(agents.json content)
Else:
    agents_config = {}

If PRD_AGENT specified (from --agent flag):
    agent = PRD_AGENT
Elif agents_config.phase_defaults.planning.default_agent exists:
    agent = agents_config.phase_defaults.planning.default_agent
Else:
    agent = "claude-code"

# Step 3: Get agent configuration
If agent in agents_config.agents:
    agent_config = agents_config.agents[agent]
    agent_type = agent_config.type  # "task-tool" or "cli"
Else:
    # No agents.json (or agent not configured) → default to claude-code Task tool
    agent = "claude-code"
    agent_type = "task-tool"

# Step 4: Verify CLI agent availability (only for CLI agents)
If agent_type == "cli":
    command = agent_config.command  # e.g., "codex", "aider"

    # Check if command is available
    Bash("which {command} 2>/dev/null || where {command} 2>nul || echo 'NOT_FOUND'")

    If command not found:
        echo "⚠️ {agent} ({command}) not available, checking fallback chain..."
        fallback_chain = agents_config.phase_defaults.planning.fallback_chain

        For fallback in fallback_chain:
            If fallback == "claude-code" OR is_command_available(agents[fallback].command):
                agent = fallback
                agent_config = agents_config.agents[agent]
                agent_type = agent_config.type
                break

        If no fallback found:
            agent = "claude-code"
            agent_type = "task-tool"
    Else:
        # CRITICAL: Agent is available - YOU MUST USE IT
        # DO NOT switch to claude-code just because you think it's "better"
        echo "✓ {agent} is available and will be used as configured"

echo "PRD Generation Agent: {agent} (type: {agent_type})"
```

**CRITICAL RULES**:
1. If the configured agent (e.g., `codex`) is available → **USE IT**
2. Only use fallback if the configured agent is **NOT FOUND** on the system
3. "Available but I prefer claude-code" is **NOT** a valid reason to switch
4. All CLI agents (codex, aider, etc.) are fully capable of exploring codebases

**Example**: If `agents.json` has `phase_defaults.planning.default_agent: "codex"` and `codex` command exists:
- Agent MUST be `codex` (not claude-code)
- Type will be `cli`
- Command will be `codex` (from `agents.codex.command`)

## Step 1.5: Optional Spec Interview (Shift-left)

Determine effective spec mode (handles empty default):
```
If SPEC_MODE is set (non-empty):
    EFFECTIVE_SPEC_MODE = SPEC_MODE
Else:
    EFFECTIVE_SPEC_MODE = "auto"  # Default when not specified
```

**CRITICAL**: If the conditions below are met, you MUST run the spec interview. Do NOT skip it based on your own judgment of task complexity, description detail, or perceived redundancy. The spec interview is a mandatory shift-left gate when enabled.

If spec interview is enabled:
- `EFFECTIVE_SPEC_MODE == "on"`, OR
- `EFFECTIVE_SPEC_MODE == "auto" AND FLOW_LEVEL == "full"`

Then run a spec interview **before** finalizing PRD:
1. Run `/plan-cascade:spec-plan "$TASK_DESC" --flow <FLOW_LEVEL> [--first-principles] [--max-questions N] --compile --tdd <TDD_MODE> [--confirm|--no-confirm]`
2. Use the compiled `prd.json` and skip Step 2 PRD generation below.

## Step 2: Generate PRD with Selected Agent

**CRITICAL**: Use the agent resolved in Step 1.1. DO NOT substitute a different agent here.

- If Step 1.1 resolved to `codex` → Execute the CLI section below
- If Step 1.1 resolved to `claude-code` → Execute the Task tool section below
- **NEVER** use claude-code just because "it's easier" or "better for exploration"

### If agent == "claude-code" (Task tool):

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

Launch with Task tool:
```
task_id = Task(
    prompt=<above prompt>,
    subagent_type="general-purpose",
    run_in_background=true,
    allowed_tools=["Write", "Edit", "Read", "Glob", "Grep"]
)
```

**IMPORTANT**: The `allowed_tools` parameter grants the subagent permission to write files. Without this, the agent can only read files and the main agent must write the output.

### If agent is CLI (codex, aider, etc.):

Based on `agent_config` from Step 1.1, build and execute the CLI command:

```
# Get agent configuration (already loaded in Step 1.1)
command = agent_config.command      # e.g., "codex"
args = agent_config.args            # e.g., ["--prompt", "{prompt}"]
working_dir = agent_config.working_dir or "."
timeout = agent_config.timeout or 600  # seconds

# Build the prompt for PRD generation
prd_prompt = """
You are a PRD generation specialist. Analyze the task and generate a PRD.

Task: $TASK_DESC

Generate prd.json with:
- metadata (created_at, version, description)
- goal (one sentence)
- objectives (list)
- stories (3-7 stories with id, title, description, priority, dependencies, status, acceptance_criteria, context_estimate, tags)

Save the result to prd.json in the current directory.
"""

# Substitute {prompt} in args with actual prompt
full_args = []
for arg in args:
    if "{prompt}" in arg:
        full_args.append(arg.replace("{prompt}", prd_prompt))
    else:
        full_args.append(arg)

# Build full command
full_command = command + " " + " ".join(full_args)

# Example for codex: codex --prompt "..."
# Example for aider: aider --message "..." --yes

task_id = Bash(
    command=full_command,
    run_in_background=true,
    timeout=timeout * 1000  # Convert to milliseconds
)
```

**CLI Agent Examples**:
- `codex`: `codex --prompt "Generate PRD for: ..."`
- `aider`: `aider --message "Generate PRD for: ..." --yes`
- `claude-cli`: `claude -p "Generate PRD for: ..."`

## Step 3: Wait for PRD Generation

IMPORTANT: After launching the background task, use TaskOutput to wait for completion:

```
TaskOutput(task_id=task_id, block=true, timeout=600000)
```

DO NOT use sleep loops or polling. The TaskOutput tool with block=true will properly wait for the agent to complete.

## Step 4: Validate PRD

Once the task completes:

1. Read the generated `prd.json` file
2. Validate the structure (check for required fields)

## Step 4.5: Auto-Generate Feature Design Document

After PRD is validated, automatically generate `design_doc.json`:

### 4.5.1: Check for User-Provided Design Document

```
If DESIGN_DOC_ARG is not empty and file exists:
    Read the external document at DESIGN_DOC_ARG
    Detect format and convert:
      - .md files: Parse Markdown structure (headers → sections)
      - .json files: Validate/map to our schema
      - .html files: Parse HTML structure
    Extract: overview, architecture, patterns, decisions
    Save as design_doc.json
    DESIGN_SOURCE="Converted from: $DESIGN_DOC_ARG"
Else:
    Auto-generate based on PRD analysis
    DESIGN_SOURCE="Auto-generated from PRD"
```

### 4.5.2: Auto-Generate Feature Design Document

Use the Task tool to generate `design_doc.json` from the PRD:

```
You are a technical design specialist. Your task is to generate a design_doc.json based on the PRD.

1. Read prd.json to understand:
   - The goal and objectives
   - All stories with their requirements
   - Dependencies between stories

2. EXPLORE the codebase to understand:
   - Existing architecture and patterns
   - Relevant code files
   - Current conventions

3. Generate design_doc.json with this structure:
{
  "metadata": {
    "created_at": "<ISO-8601>",
    "version": "1.0.0",
    "source": "ai-generated",
    "level": "feature",
    "prd_reference": "prd.json"
  },
  "overview": {
    "title": "<from PRD goal>",
    "summary": "<brief description>",
    "goals": ["<from PRD objectives>"],
    "non_goals": ["<identified non-goals>"]
  },
  "architecture": {
    "components": [
      {
        "name": "ComponentName",
        "description": "Description",
        "responsibilities": ["resp1"],
        "dependencies": ["OtherComponent"],
        "files": ["src/path/to/file.py"]
      }
    ],
    "data_flow": "<how data flows>",
    "patterns": [
      {
        "name": "PatternName",
        "description": "What this pattern does",
        "rationale": "Why we use it"
      }
    ]
  },
  "interfaces": {
    "apis": [
      {
        "id": "API-001",
        "method": "POST",
        "path": "/api/v1/endpoint",
        "description": "What it does",
        "request_body": {},
        "response": {}
      }
    ],
    "data_models": [
      {
        "name": "ModelName",
        "description": "Description",
        "fields": {"field": "type"}
      }
    ]
  },
  "decisions": [
    {
      "id": "ADR-F001",
      "title": "Decision title",
      "context": "Background",
      "decision": "What we decided",
      "rationale": "Why",
      "alternatives_considered": ["alt1"],
      "status": "accepted"
    }
  ],
  "story_mappings": {
    "story-001": {
      "components": ["ComponentA"],
      "decisions": ["ADR-F001"],
      "interfaces": ["API-001", "ModelName"]
    }
  }
}

4. Create story_mappings linking each story to relevant components/decisions/interfaces
5. SAVE to design_doc.json in the current directory
```

Launch with Task tool (with write permissions):
```
task_id = Task(
    prompt=<above prompt>,
    subagent_type="general-purpose",
    run_in_background=true,
    allowed_tools=["Write", "Edit", "Read", "Glob", "Grep"]
)

TaskOutput(task_id=task_id, block=true, timeout=600000)
```

**IMPORTANT**: The `allowed_tools` parameter is required for the subagent to write `design_doc.json` directly. Without it, you must write the file yourself after the agent returns the content.

## Step 5: Write Flow and TDD Configuration to PRD

**CRITICAL**: If flow or TDD parameters were specified, add them to the PRD for the approve phase.

```
# Read the generated prd.json
prd_content = Read("prd.json")
prd = parse_json(prd_content)

# Add flow configuration if specified
If FLOW_LEVEL is set:
    prd["flow_config"] = {
        "level": FLOW_LEVEL,  # "quick", "standard", or "full"
        "source": "command-line"
    }

    # For FULL flow, set strict gate settings
    If FLOW_LEVEL == "full":
        prd["verification_gate"] = {"enabled": true, "required": true}
        prd["code_review"] = {"enabled": true, "required": true}

# Add TDD configuration if specified
If TDD_MODE is set:
    prd["tdd_config"] = {
        "mode": TDD_MODE,  # "off", "on", or "auto"
        "enforce_for_high_risk": true,
        "test_requirements": {
            # In ON mode, always require tests
            # In AUTO mode, require tests for high-risk stories
            "require_test_changes": (TDD_MODE == "on"),
            "require_test_for_high_risk": true,  # Always require tests for high-risk stories
            "minimum_coverage_delta": 0.0,
            "test_patterns": ["test_", "_test.", ".test.", "tests/", "test/", "spec/"]
        }
    }

# Add confirm mode flag (--no-confirm takes precedence)
prd["execution_config"] = prd.get("execution_config", {})
If NO_CONFIRM_MODE is true:
    prd["execution_config"]["require_batch_confirm"] = false
    prd["execution_config"]["no_confirm_override"] = true  # Explicit override marker
Elif CONFIRM_MODE is true:
    prd["execution_config"]["require_batch_confirm"] = true

# Write updated PRD
Write("prd.json", json.dumps(prd, indent=2))

echo "✓ Flow/TDD configuration written to PRD"
```

## Step 5.4: Decision Conflict Check (Passive)

After design_doc.json is generated, check new decisions against existing project decisions:

```bash
uv run python "${CLAUDE_PLUGIN_ROOT}/skills/hybrid-ralph/scripts/memory-doctor.py" \
  --mode passive \
  --new-decisions design_doc.json \
  --project-root "$(pwd)"
```

If the script exits with code 1 (issues found):
1. Display the diagnosis report to the user
2. Use `AskUserQuestion` to ask the user how to resolve each issue:
   - **Deprecate** — Mark the older/conflicting decision as deprecated
   - **Merge** — Combine duplicate decisions into one
   - **Skip** — Keep both decisions as-is
3. Apply the user's choices by modifying the relevant `design_doc.json` files

If the script exits with code 0 (no issues or no existing decisions to compare), proceed silently to Step 5.5.

## Step 5.5: Display Unified Review

**CRITICAL**: Use Bash to display the unified PRD + Design Document review:

```bash
uv run python "${CLAUDE_PLUGIN_ROOT}/skills/hybrid-ralph/scripts/unified-review.py" --mode hybrid
```

This displays:
- PRD summary with stories, priorities, and execution batches
- Design document with components, patterns, and architectural decisions
- Story-to-design mappings (showing which stories are linked to which components)
- Warnings for any unmapped stories
- Flow/TDD configuration summary (if set)
- Available next steps

If the script is not available, display a manual summary showing:
- Goal and objectives
- All stories with IDs, titles, priorities
- Design document summary (components, patterns, decisions)
- Flow configuration: {FLOW_LEVEL or "standard (default)"}
- TDD mode: {TDD_MODE or "auto (default)"}

## Step 6: Confirm Generation Complete and Show Next Steps

After displaying the unified review, confirm and show how to proceed:

```
PRD and Design Document generated successfully!

Files created:
  - prd.json          (product requirements document)
  - design_doc.json   (technical design document)

============================================================
EXECUTION CONFIGURATION
============================================================
  Flow Level: {FLOW_LEVEL or "standard (default)"}
  TDD Mode: {TDD_MODE or "auto (default)"}
  Batch Confirm: {NO_CONFIRM_MODE ? "disabled (--no-confirm)" : (CONFIRM_MODE ? "enabled" : "default")}
============================================================

NEXT STEPS:

  Review and edit (optional):
    /plan-cascade:edit

  Approve and execute:
```

**CRITICAL**: Build the approve command with preserved parameters:

```
# Build approve command with flow/tdd parameters
APPROVE_CMD = "/plan-cascade:approve"

If FLOW_LEVEL is set:
    APPROVE_CMD = APPROVE_CMD + " --flow " + FLOW_LEVEL

If TDD_MODE is set:
    APPROVE_CMD = APPROVE_CMD + " --tdd " + TDD_MODE

# --no-confirm takes precedence over --confirm
If NO_CONFIRM_MODE is true:
    APPROVE_CMD = APPROVE_CMD + " --no-confirm"
Elif CONFIRM_MODE is true:
    APPROVE_CMD = APPROVE_CMD + " --confirm"

echo "    " + APPROVE_CMD
```

Example outputs:
- Standard flow: `/plan-cascade:approve`
- Full flow with TDD: `/plan-cascade:approve --flow full --tdd on`
- Full flow with confirm: `/plan-cascade:approve --flow full --tdd on --confirm`
- Full flow CI-friendly: `/plan-cascade:approve --flow full --tdd on --no-confirm`

## Notes

- If PRD validation fails, show errors and suggest `/plan-cascade:edit` to fix manually
- The planning agent may take time to explore the codebase - be patient
- Generated PRD is a draft - user should review and can edit before approving
- **Design Document Auto-Generation**:
  - A feature-level `design_doc.json` is automatically generated after PRD
  - Provides architectural context for story execution agents
  - Contains story_mappings linking each story to relevant components/decisions
  - User can provide external design doc which will be converted to our format

## Recovery

If the task is interrupted at any point (PRD generation, story execution):

```bash
# Resume from where it left off
/plan-cascade:hybrid-resume --auto
```

This will:
- Auto-detect current state from prd.json and progress.txt
- Skip already-completed work
- Resume execution from incomplete stories
