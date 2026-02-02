---
description: "Generate PRD from task description and enter review mode. Auto-generates user stories with priorities, dependencies, and acceptance criteria for parallel execution. Usage: /plan-cascade:hybrid-auto <task description> [design-doc-path] [--agent <name>]"
---

# Hybrid Ralph - Auto Generate PRD

You are automatically generating a Product Requirements Document (PRD) from the task description.

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
python3 -c "from plan_cascade.utils.gitignore import ensure_gitignore; from pathlib import Path; ensure_gitignore(Path.cwd())" 2>/dev/null || echo "Note: Could not auto-update .gitignore"
```

This prevents planning files from being accidentally committed to version control.

## Step 1: Parse Arguments

Parse user arguments:
- **Task description**: First argument (required)
- **Design doc path**: Second argument (optional) - external design document to convert
- **--agent**: Optional agent override for PRD generation

```
TASK_DESC="{{args|arg 1}}"
DESIGN_DOC_ARG="{{args|arg 2 or empty}}"
PRD_AGENT=""

# Parse --agent flag
for arg in $ARGUMENTS; do
    case "$arg" in
        --agent=*) PRD_AGENT="${arg#*=}" ;;
    esac
done
```

If no description provided, ask the user:
```
Please provide a task description.

Optional arguments:
  - Design document path (2nd arg): /plan-cascade:hybrid-auto "task" ./design.md
  - Agent override: /plan-cascade:hybrid-auto "task" --agent=codex
```

### 1.1: Resolve PRD Generation Agent

**CRITICAL**: You MUST read `agents.json` to get the configured default agent:

```
# Step 1: Read agents.json configuration
Read("agents.json")

# Step 2: Parse and select agent
agents_config = parse_json(agents.json content)

If PRD_AGENT specified (from --agent flag):
    agent = PRD_AGENT
Elif agents_config.phase_defaults.planning.default_agent exists:
    agent = agents_config.phase_defaults.planning.default_agent
Else:
    agent = "claude-code"

# Step 3: Get agent configuration
agent_config = agents_config.agents[agent]
agent_type = agent_config.type  # "task-tool" or "cli"

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

echo "PRD Generation Agent: {agent} (type: {agent_type})"
```

**Example**: If `agents.json` has `phase_defaults.planning.default_agent: "codex"`, then:
- Agent will be `codex`
- Type will be `cli`
- Command will be `codex` (from `agents.codex.command`)

## Step 2: Generate PRD with Selected Agent

Use the resolved agent to generate the PRD.

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

## Step 5: Display Unified Review

**CRITICAL**: Use Bash to display the unified PRD + Design Document review:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/hybrid-ralph/scripts/unified-review.py" --mode hybrid
```

This displays:
- PRD summary with stories, priorities, and execution batches
- Design document with components, patterns, and architectural decisions
- Story-to-design mappings (showing which stories are linked to which components)
- Warnings for any unmapped stories
- Available next steps

If the script is not available, display a manual summary showing:
- Goal and objectives
- All stories with IDs, titles, priorities
- Design document summary (components, patterns, decisions)

## Step 6: Confirm Generation Complete

After displaying the unified review, confirm:

```
PRD and Design Document generated successfully!

Files created:
  - prd.json          (product requirements document)
  - design_doc.json   (technical design document)
```

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
