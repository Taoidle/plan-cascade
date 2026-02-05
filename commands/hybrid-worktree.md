---
description: "Start a new task in an isolated Git worktree with Hybrid Ralph PRD mode. Creates worktree, branch, loads existing PRD or auto-generates from description. Usage: /plan-cascade:hybrid-worktree [--flow <quick|standard|full>] [--tdd <off|on|auto>] [--confirm] [--no-confirm] [--spec <off|auto|on>] [--first-principles] [--max-questions N] [--agent <name>] <task-name> <target-branch> <prd-path-or-description> [design-doc-path]"
---

# Hybrid Ralph + Worktree Mode (Fully Automated)

You are starting a task in **Git Worktree + Hybrid Ralph mode**. This will create the worktree and handle the PRD automatically.

## Execution Flow Parameters

This command accepts flow control parameters that affect story execution:

### Parameter Priority

Parameters are saved to `prd.json` in the worktree and later used by `/plan-cascade:approve`. Priority order:

1. **Command-line flags to THIS command** (highest priority)
   - Example: `/plan-cascade:hybrid-worktree --flow full fix-auth main "Fix bug"`
   - Saved to `<worktree>/prd.json` as `flow_config`, `tdd_config`, etc.

2. **Command-line flags to `/plan-cascade:approve`** (when run in worktree)
   - Can override values saved in `prd.json`

3. **Default values** (lowest priority)

**Worktree Isolation:** Parameters are stored in the worktree's `prd.json`, ensuring complete isolation from other tasks. When you run `/plan-cascade:approve` in the worktree, it automatically finds and uses this PRD.

**Example:**
```bash
# Create worktree with strict flow
/plan-cascade:hybrid-worktree --flow full --tdd on fix-auth main "Fix authentication bug"
# → Creates worktree at .worktree/fix-auth/
# → Saves prd.json with flow_config.level="full", tdd_config.mode="on"

# Later, in the worktree directory:
cd .worktree/fix-auth/
/plan-cascade:approve
# → Uses flow="full", tdd="on" from prd.json in current worktree
```

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

Explicitly disable batch confirmation, even if FULL flow would normally require it. Useful for CI/automated environments.

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

## Path Storage Modes

Plan Cascade supports two path storage modes for runtime files:

### New Mode (Default)
Runtime files are stored in a user directory, keeping the project root clean:
- **Windows**: `%APPDATA%/plan-cascade/<project-id>/`
- **Unix/macOS**: `~/.plan-cascade/<project-id>/`

Where `<project-id>` is a unique identifier based on the project name and path hash (e.g., `my-project-a1b2c3d4`).

File locations in new mode:
- Worktrees: `<user-dir>/.worktree/<task-name>/`
- PRD: `<worktree>/.prd.json` or `<user-dir>/prd.json` for non-worktree
- State files: `<user-dir>/.state/`

### Legacy Mode
All files stored in project root (backward compatible):
- Worktrees: `<project-root>/.worktree/<task-name>/`
- PRD: `<worktree>/prd.json`

**Note**: User-visible files like `findings.md`, `progress.txt`, and `mega-findings.md` always remain in the worktree/project root for easy access.

## Tool Usage Policy (CRITICAL)

**To avoid command confirmation prompts:**

1. **Use Read tool for file reading** - NEVER use `cat` via Bash
   - ✅ `Read("prd.json")`, `Read(".planning-config.json")`
   - ❌ `Bash("cat prd.json")`

2. **Only use Bash for actual system commands:**
   - Git operations: `git worktree add`, `git show-ref`
   - OS detection: `uname -s`
   - File writing when creating new files

3. **Use Write tool for creating structured files** - When possible
   - ✅ `Write("prd.json", content)` for JSON files

## Step 1: Parse Parameters

Parse user arguments:
- **Task name**: First positional arg (or `task-YYYY-MM-DD-HHMM`)
- **Target branch**: Second positional arg (or auto-detect `main`/`master`)
- **PRD path OR description**: Third positional arg
  - If it's an existing file path → Load that PRD
  - Otherwise → Use as task description to auto-generate PRD
- **Design doc path**: Fourth positional arg (optional)
  - If provided → Convert external doc to design_doc.json format
- **--flow**: Execution flow depth (quick|standard|full)
- **--tdd**: TDD mode (off|on|auto)
- **--confirm**: Require batch confirmation
- **--no-confirm**: Disable batch confirmation (overrides --confirm and FULL flow default)
- **--spec**: Spec interview mode (off|auto|on)
- **--first-principles**: Enable first-principles questions (when spec interview runs)
- **--max-questions**: Soft cap for interview length
- **--agent**: Optional agent override for PRD generation

```bash
TASK_NAME=""
TARGET_BRANCH=""
PRD_ARG=""
DESIGN_ARG=""
PRD_AGENT=""
FLOW_LEVEL=""           # --flow <quick|standard|full>
TDD_MODE=""             # --tdd <off|on|auto>
CONFIRM_MODE=false      # --confirm
NO_CONFIRM_MODE=false   # --no-confirm
CONFIRM_EXPLICIT=false  # set when --confirm is provided
NO_CONFIRM_EXPLICIT=false  # set when --no-confirm is provided
SPEC_MODE=""            # --spec <off|auto|on>
FIRST_PRINCIPLES=false  # --first-principles
MAX_QUESTIONS=""        # --max-questions N

# Track positional argument index
POS_INDEX=0
NEXT_IS_FLOW=false
NEXT_IS_TDD=false
NEXT_IS_AGENT=false
NEXT_IS_SPEC=false
NEXT_IS_MAXQ=false

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
            else
                # Positional arguments
                POS_INDEX=$((POS_INDEX + 1))
                case $POS_INDEX in
                    1) TASK_NAME="$arg" ;;
                    2) TARGET_BRANCH="$arg" ;;
                    3) PRD_ARG="$arg" ;;
                    4) DESIGN_ARG="$arg" ;;
                esac
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

# Default task name if not provided
if [ -z "$TASK_NAME" ]; then
    TASK_NAME="task-$(date +%Y%m%d-%H%M)"
fi

# Display parsed parameters
echo "Parsed Parameters:"
echo "  Task Name: $TASK_NAME"
echo "  Target Branch: ${TARGET_BRANCH:-"(auto-detect)"}"
echo "  PRD/Description: ${PRD_ARG:-"(will prompt)"}"
echo "  Flow: ${FLOW_LEVEL:-"(default)"}"
echo "  TDD: ${TDD_MODE:-"(default)"}"
echo "  Confirm: $CONFIRM_MODE"
echo "  No-Confirm: $NO_CONFIRM_MODE"
echo "  Spec: ${SPEC_MODE:-"(auto)"}"
echo "  First Principles: $FIRST_PRINCIPLES"
echo "  Max Questions: ${MAX_QUESTIONS:-"(default)"}"
echo "  Agent: ${PRD_AGENT:-"(default)"}"
```

## Step 2: Detect Operating System and Shell

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

**Important**: Throughout this command, use:
- **Bash syntax** when `SHELL_TYPE=bash`
- **PowerShell syntax** when `SHELL_TYPE=powershell`

For PowerShell equivalents:
- `$(command)` → `$()`
- `VAR=value` → `$VAR = value`
- `if [ ]` → `if ()`
- `echo` → `Write-Host`

## Step 2.5: Ensure .gitignore Configuration

**IMPORTANT**: Before creating any planning files, ensure the project's `.gitignore` is configured to ignore Plan Cascade temporary files:

```bash
# Check and update .gitignore for Plan Cascade entries
uv run python -c "from plan_cascade.utils.gitignore import ensure_gitignore; from pathlib import Path; ensure_gitignore(Path.cwd())" 2>/dev/null || echo "Note: Could not auto-update .gitignore"
```

This ensures that planning files (prd.json, .worktree/, etc.) won't be accidentally committed to version control.

## Step 3: Ensure Auto-Approval Configuration

Ensure command auto-approval settings are configured (merges with existing settings):

```bash
# Run the settings merge script
uv run python "${CLAUDE_PLUGIN_ROOT}/scripts/ensure-settings.py" 2>/dev/null || uv run python scripts/ensure-settings.py 2>/dev/null || echo "Warning: Could not update settings, continuing..."
```

This script intelligently merges required auto-approval patterns with any existing `.claude/settings.local.json`, preserving user customizations.

## Step 4: Verify Git Repository

```bash
git rev-parse --git-dir > /dev/null 2>&1 || { echo "ERROR: Not a git repository"; exit 1; }
```

## Step 5: Detect Default Branch

```bash
DEFAULT_BRANCH=$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@refs/remotes/origin/@@')
if [ -z "$DEFAULT_BRANCH" ]; then
    if git show-ref --verify --quiet refs/heads/main; then
        DEFAULT_BRANCH="main"
    elif git show-ref --verify --quiet refs/heads/master; then
        DEFAULT_BRANCH="master"
    else
        DEFAULT_BRANCH="main"
    fi
fi
TARGET_BRANCH="${TARGET_BRANCH:-$DEFAULT_BRANCH}"
```

## Step 6: Set Variables

```bash
TASK_BRANCH="$TASK_NAME"
ORIGINAL_BRANCH=$(git branch --show-current)
ROOT_DIR=$(pwd)

# Resolve worktree directory using PathResolver
# New mode: ~/.plan-cascade/<project-id>/.worktree/<task-name>
# Legacy mode: <project-root>/.worktree/<task-name>
WORKTREE_BASE=$(uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_worktree_dir())" 2>/dev/null || echo "$ROOT_DIR/.worktree")
WORKTREE_DIR="$WORKTREE_BASE/$(basename $TASK_NAME)"

# Ensure worktree base directory exists
mkdir -p "$WORKTREE_BASE"
```

## Step 6.5: Check for Design Document

**Use Read tool (NOT Bash) to check for design document:**

```
Read("design_doc.json")  # in ROOT_DIR
```

- If Read succeeds → `HAS_DESIGN_DOC=true`, log "✓ Design document detected at project root"
- If Read fails (file not found) → `HAS_DESIGN_DOC=false`, log "ℹ No design document found (optional)"

## Step 7: Determine PRD Mode

**Use Read tool (NOT Bash) to check if PRD_ARG is an existing file:**

```
Read("$PRD_ARG")  # Try to read the path as a file
```

- If Read succeeds → `PRD_MODE="load"`, `PRD_PATH="$PRD_ARG"`, log "Loading PRD from: $PRD_PATH"
- If Read fails (file not found) → `PRD_MODE="generate"`, `TASK_DESC="$PRD_ARG"`, log "Will generate PRD from description"

## Step 8: Check for Existing Worktree

```bash
if [ -d "$WORKTREE_DIR" ]; then
    echo "Worktree already exists: $WORKTREE_DIR"
    echo "Navigating to existing worktree..."
    cd "$WORKTREE_DIR"
    # Continue to PRD handling for existing worktree
else
    ## Step 9: Create Git Worktree (only if new)

    if git show-ref --verify --quiet refs/heads/"$TASK_BRANCH"; then
        echo "ERROR: Branch $TASK_BRANCH already exists"
        exit 1
    fi

    git worktree add -b "$TASK_BRANCH" "$WORKTREE_DIR" "$TARGET_BRANCH"
    echo "Created worktree: $WORKTREE_DIR"

    ## Step 10: Create Planning Configuration

    cat > "$WORKTREE_DIR/.planning-config.json" << EOF
{
  "mode": "hybrid",
  "task_name": "$TASK_NAME",
  "task_branch": "$TASK_BRANCH",
  "target_branch": "$TARGET_BRANCH",
  "worktree_dir": "$WORKTREE_DIR",
  "original_branch": "$ORIGINAL_BRANCH",
  "root_dir": "$ROOT_DIR",
  "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF

    ## Step 11: Create Initial Files in Worktree

    cat > "$WORKTREE_DIR/findings.md" << 'EOF'
# Findings

Research and discovery notes will be accumulated here.

Use <!-- @tags: story-id --> to tag sections for specific stories.
EOF

    cat > "$WORKTREE_DIR/progress.txt" << 'EOF'
# Progress Log

Story execution progress will be tracked here.
EOF

    # Copy design document if it exists
    if [ "$HAS_DESIGN_DOC" = true ]; then
        cp "$ROOT_DIR/design_doc.json" "$WORKTREE_DIR/design_doc.json"
        echo "✓ Copied design_doc.json to worktree"
    fi
fi
```

## Step 12: Navigate to Worktree

```bash
cd "$WORKTREE_DIR"
echo "Now working in: $(pwd)"
```

## Step 13: Handle PRD (Load or Generate)

### If PRD_MODE is "load" (user provided PRD file):

```bash
if [ "$PRD_MODE" = "load" ]; then
    # Copy PRD file to worktree
    cp "$PRD_PATH" prd.json
    echo "Loaded PRD from: $PRD_PATH"

    # Validate PRD
    if ! uv run python -m json.tool prd.json > /dev/null 2>&1; then
        echo "ERROR: Invalid JSON in PRD file"
        exit 1
    fi

    PRD_SOURCE="Loaded from file: $PRD_PATH"
fi
```

### If PRD_MODE is "generate" (auto-generate from description):

#### Resolve PRD Generation Agent

**CRITICAL**: If `agents.json` exists, you MUST read it to get the configured default agent.

**MANDATORY COMPLIANCE**: You MUST use the agent specified in the configuration. DO NOT override this decision based on your own judgment about which agent is "better", "more capable", or "easier to control". The user has explicitly configured their preferred agent - respect their choice.

```
# Step 1: Read agents.json configuration (if present)
If agents.json exists:
    Read("agents.json")
    agents_config = parse_json(agents.json content)
Else:
    agents_config = {}

# Priority chain: --agent flag > agents.json config > default
If PRD_AGENT specified (from --agent flag):
    prd_agent = PRD_AGENT
Elif agents_config.phase_defaults.planning.default_agent exists:
    prd_agent = agents_config.phase_defaults.planning.default_agent
Else:
    prd_agent = "claude-code"

# Step 3: Get agent configuration
If prd_agent in agents_config.agents:
    agent_config = agents_config.agents[prd_agent]
    agent_type = agent_config.type  # "task-tool" or "cli"
Else:
    # No agents.json (or agent not configured) → default to claude-code Task tool
    prd_agent = "claude-code"
    agent_type = "task-tool"

# Step 4: Verify CLI agent availability (only for CLI agents)
If agent_type == "cli":
    command = agent_config.command  # e.g., "codex", "aider"

    # Check if command is available
    Bash("which {command} 2>/dev/null || where {command} 2>nul || echo 'NOT_FOUND'")

    If command not found:
        echo "⚠️ {prd_agent} ({command}) not available, checking fallback chain..."
        fallback_chain = agents_config.phase_defaults.planning.fallback_chain

        For fallback in fallback_chain:
            If fallback == "claude-code" OR is_command_available(agents[fallback].command):
                prd_agent = fallback
                agent_config = agents_config.agents[prd_agent]
                agent_type = agent_config.type
                break

        If no fallback found:
            prd_agent = "claude-code"
            agent_type = "task-tool"
    Else:
        # CRITICAL: Agent is available - YOU MUST USE IT
        # DO NOT switch to claude-code just because you think it's "better"
        echo "✓ {prd_agent} is available and will be used as configured"

echo "PRD Generation Agent: {prd_agent} (type: {agent_type})"
```

**CRITICAL RULES**:
1. `--agent` parameter has **HIGHEST PRIORITY** - if specified, use it
2. If the configured agent (e.g., `codex`) is available → **USE IT**
3. Only use fallback if the configured agent is **NOT FOUND** on the system
4. "Available but I prefer claude-code" is **NOT** a valid reason to switch
5. All CLI agents (codex, aider, etc.) are fully capable of exploring codebases

**Example**: `/plan-cascade:hybrid-worktree fix-auth main "Fix auth" --agent=codex`
- Agent will be `codex` regardless of agents.json config

#### Optional: Spec Interview (Shift-left)

If spec interview is enabled, run it in the worktree **before** finalizing the PRD:

- Effective spec mode:
  - If `SPEC_MODE` is set: use it
  - Else default `auto`
  - `auto` ⇒ enabled when `FLOW_LEVEL == full`

If enabled:
1. `cd` into the worktree directory
2. Run `/plan-cascade:spec-plan` with `--compile` to produce `spec.json/spec.md` and compile `prd.json`
3. Skip the LLM PRD-generation step below (the PRD is already compiled)

#### Generate PRD with Selected Agent

**If prd_agent == "claude-code" (Task tool)**:

Use the Task tool to automatically generate the PRD:

```
You are a PRD generation specialist. Your task is to:

1. ANALYZE the task description: "$TASK_DESC"
2. **If design_doc.json exists in the current directory:**
   - Read it for architectural guidance
   - Identify relevant components for this task
   - Note applicable architectural patterns and decisions (ADRs)
   - Use this context to create well-aligned stories
3. EXPLORE the codebase in the current directory to understand:
   - Existing patterns and conventions
   - Relevant code files
   - Architecture and structure
4. GENERATE a PRD (prd.json) with:
   - Clear goal statement
   - 3-7 user stories
   - Each story with: id, title, description, priority (high/medium/low), dependencies, acceptance_criteria, context_estimate (small/medium/large), tags
   - Dependencies between stories (where one story must complete before another)
5. **If design_doc.json exists, update its story_mappings section** to link each new story to relevant components, decisions, and interfaces
6. SAVE the PRD to prd.json in the current directory

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

Launch this as a background task with `run_in_background: true`:

```
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

After TaskOutput returns, the prd.json file will be ready. Continue to Step 13.5.

**If prd_agent is CLI (codex, aider, etc.)**:

Based on `agent_config` from agent resolution, build and execute the CLI command:

```
# Get agent configuration (already loaded in agent resolution step)
command = agent_config.command      # e.g., "codex"
args = agent_config.args            # e.g., ["--prompt", "{prompt}"]
working_dir = agent_config.working_dir or "."
timeout = agent_config.timeout or 600  # seconds

# Build the prompt for PRD generation
prd_prompt = """
You are a PRD generation specialist. Analyze the task and generate a PRD.

Task: $TASK_DESC
Working Directory: $WORKTREE_DIR

If design_doc.json exists, read it for architectural context.

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

# Wait for completion
TaskOutput(task_id=task_id, block=true, timeout=600000)
```

After the CLI agent completes, verify prd.json was created. Continue to Step 13.5.

```bash
PRD_SOURCE="Auto-generated from description"
fi
```

## Step 13.5: Auto-Generate Feature Design Document

After PRD is ready (loaded or generated), automatically generate or update `design_doc.json`:

### 13.5.1: Check for User-Provided Design Document

```
If DESIGN_ARG is not empty and file exists:
    Read the external document at DESIGN_ARG
    Detect format and convert:
      - .md files: Parse Markdown structure (headers → sections)
      - .json files: Validate/map to our schema
      - .html files: Parse HTML structure
    Extract: overview, architecture, patterns, decisions
    Save as design_doc.json (overwrite copied project-level doc)
    DESIGN_SOURCE="Converted from: $DESIGN_ARG"
Elif design_doc.json already exists (copied from project root):
    # Update it to become feature-level with story_mappings
    DESIGN_SOURCE="Derived from project-level design"
Else:
    Auto-generate based on PRD analysis
    DESIGN_SOURCE="Auto-generated from PRD"
```

### 13.5.2: Generate/Update Feature Design Document

Use the Task tool to generate or update `design_doc.json`:

```
You are a technical design specialist. Your task is to generate a feature-level design_doc.json.

CONTEXT:
- Working directory: $WORKTREE_DIR
- PRD file: prd.json (already exists)
- Existing design doc: ${HAS_DESIGN_DOC ? "yes (from project root)" : "no"}

1. Read prd.json to understand:
   - The goal and objectives
   - All stories with their requirements
   - Dependencies between stories

2. If design_doc.json exists from project root:
   - Read it for inherited context (patterns, decisions, shared_models)
   - Create a NEW feature-level design doc that:
     - References the parent in metadata.parent_design_doc: "../design_doc.json"
     - Includes inherited_context section with relevant patterns/decisions
     - Adds feature-specific components, APIs, data_models
     - Uses ADR-F### prefix for feature-specific decisions
     - Creates story_mappings for all stories

3. If NO design_doc.json exists:
   - EXPLORE the codebase to understand existing patterns
   - Generate a standalone feature-level design doc

4. Generate/update design_doc.json with this structure:
{
  "metadata": {
    "created_at": "<ISO-8601>",
    "version": "1.0.0",
    "source": "ai-generated",
    "level": "feature",
    "prd_reference": "prd.json",
    "parent_design_doc": "../design_doc.json",
    "feature_id": "$TASK_NAME"
  },
  "overview": {
    "title": "<from PRD goal>",
    "summary": "<brief description>",
    "goals": ["<from PRD objectives>"],
    "non_goals": ["<identified non-goals>"]
  },
  "inherited_context": {
    "description": "Context inherited from project-level design document",
    "patterns": ["PatternName"],
    "decisions": ["ADR-001"],
    "shared_models": ["SharedModel"]
  },
  "architecture": {
    "components": [...],
    "data_flow": "<feature-specific data flow>",
    "patterns": [...]
  },
  "interfaces": {
    "apis": [...],
    "data_models": [...]
  },
  "decisions": [
    {
      "id": "ADR-F001",
      "title": "Feature-specific decision",
      ...
    }
  ],
  "story_mappings": {
    "story-001": {
      "components": ["ComponentA"],
      "decisions": ["ADR-F001"],
      "interfaces": ["API-001"]
    }
  }
}

5. Create complete story_mappings for ALL stories in the PRD
6. SAVE to design_doc.json (overwrite if exists)
```

Launch as background task with `run_in_background: true`, then use TaskOutput to wait.

## Step 13.6: Update Execution Context File

After PRD and design document are ready, generate the execution context file:

```bash
# Generate .hybrid-execution-context.md for context recovery
uv run python "${CLAUDE_PLUGIN_ROOT}/skills/hybrid-ralph/scripts/hybrid-context-reminder.py" update
```

This file helps AI recover execution context after context compression/truncation.

## Step 13.7: Write Flow and TDD Configuration to PRD

**CRITICAL**: If flow or TDD parameters were specified, add them to the PRD for the approve phase.

```
# Read the generated/loaded prd.json
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
            "require_test_changes": (TDD_MODE == "on"),
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

## Step 14: Display Unified Review

**CRITICAL**: Use Bash to display the unified PRD + Design Document review:

```bash
uv run python "${CLAUDE_PLUGIN_ROOT}/skills/hybrid-ralph/scripts/unified-review.py" --mode hybrid
```

This displays:
- PRD summary with stories, priorities, and execution batches
- Design document with components, patterns, and architectural decisions
- Story-to-design mappings (showing which stories are linked to which components)
- Warnings for any unmapped stories
- Available next steps

If the script is not available, manually display:
1. Read the `prd.json` file
2. Validate the structure (check for required fields)
3. Display PRD review with stories, dependency graph, execution batches

## Step 15: Show Worktree Summary and Next Steps

After unified review, show worktree-specific information with execution configuration:

```
============================================================
Hybrid Ralph Worktree Ready
============================================================

Worktree: $WORKTREE_DIR
Branch: $TASK_BRANCH
Target: $TARGET_BRANCH

✓ PRD Ready: $PRD_SOURCE
✓ Design Document: $DESIGN_SOURCE

============================================================
EXECUTION CONFIGURATION
============================================================
  Flow Level: {FLOW_LEVEL or "standard (default)"}
  TDD Mode: {TDD_MODE or "auto (default)"}
  Batch Confirm: {CONFIRM_MODE}
  No-Confirm Override: {NO_CONFIRM_MODE}
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
- Full flow without confirm (CI mode): `/plan-cascade:approve --flow full --tdd on --no-confirm`

```
============================================================
WORKTREE-SPECIFIC COMMANDS:

When complete:
  /plan-cascade:hybrid-complete

To return to main project:
  cd $ROOT_DIR

Active Worktrees:
{Show git worktree list}
```

---

## Usage Examples

```bash
# Auto-generate PRD and design doc from description
/plan-cascade:hybrid-worktree fix-auth main "Fix authentication bug in login flow"

# Load existing PRD file (design doc auto-generated)
/plan-cascade:hybrid-worktree fix-auth main ./my-prd.json

# Load PRD and use external design document
/plan-cascade:hybrid-worktree fix-auth main ./my-prd.json ./design-spec.md

# Auto-generate PRD, use external design document
/plan-cascade:hybrid-worktree fix-auth main "Fix auth flow" ./architecture.md

# Specify agent for PRD generation (overrides agents.json config)
/plan-cascade:hybrid-worktree --agent codex fix-auth main "Fix auth"

# Full flow with TDD enabled (strict quality gates)
/plan-cascade:hybrid-worktree --flow full --tdd on fix-auth main "Implement OAuth2 authentication"

# Full flow with confirmation required before each batch
/plan-cascade:hybrid-worktree --flow full --tdd on --confirm critical-refactor main "Refactor payment processing"

# Combine all options
/plan-cascade:hybrid-worktree --flow full --tdd on --confirm --agent codex refactor-db main "Migrate database schema" ./design.md
```

## Notes

- **File path mode**: If the third argument is an existing file, it's loaded as PRD
- **Description mode**: If the third argument is not a file, it's used to auto-generate PRD
- The entire process is automated: worktree creation → PRD loading/generation → design doc generation → review
- You can edit the PRD before approving: `/plan-cascade:edit`
- Multiple worktrees can run in parallel for different tasks
- **Design Document Auto-Generation**:
  - If project-level `design_doc.json` exists at root: It's used as inheritance source
  - Feature-level `design_doc.json` is auto-generated after PRD with story_mappings
  - Story execution receives filtered design context per story
  - User-provided external design docs (4th arg) are automatically converted

## Recovery

If execution is interrupted at any point:

```bash
# Resume from where it left off
/plan-cascade:hybrid-resume --auto
```

This will:
- Auto-detect current state from files
- Skip already-completed work
- Continue execution from incomplete stories
