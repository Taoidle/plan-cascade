---
description: "Generate PRD from task description and enter review mode. Auto-generates user stories with priorities, dependencies, and acceptance criteria for parallel execution. Usage: /plan-cascade:hybrid-auto <task description> [design-doc-path] [--agent <name>]"
---

# Hybrid Ralph - Auto Generate PRD

You are automatically generating a Product Requirements Document (PRD) from the task description.

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

```
If PRD_AGENT specified:
    agent = PRD_AGENT
Elif phase_defaults.planning.default_agent in agents.json:
    agent = phase_defaults.planning.default_agent
Else:
    agent = "claude-code"

# Verify CLI agent availability
If agent != "claude-code" AND agents[agent].type == "cli":
    If not is_command_available(agents[agent].command):
        echo "⚠️ {agent} not available, falling back to claude-code"
        agent = "claude-code"

echo "PRD Generation Agent: {agent}"
```

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
    run_in_background=true
)
```

### If agent is CLI (codex, aider, etc.):

```
agent_config = agents[agent]
command = agent_config.command  # e.g., "codex"
args = agent_config.args        # e.g., ["--prompt", "{prompt}"]

# Build command with prompt substitution
full_command = build_cli_command(agent_config, prompt)

task_id = Bash(
    command=full_command,
    run_in_background=true,
    timeout=agent_config.timeout or 600000
)
```

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

Launch this as a background task with `run_in_background: true`, then use TaskOutput to wait.

## Step 5: Display PRD Review

Display a PRD review summary showing:
- Goal and objectives
- All stories with IDs, titles, priorities
- Dependency graph (ASCII)
- Execution batches
- Design document summary (components, patterns, decisions)

## Step 6: Show Next Steps

After displaying the PRD review, tell the user their options:

```
PRD and Design Document generated successfully!

Files created:
  - prd.json          (product requirements document)
  - design_doc.json   (technical design document)

Next steps:
  - /plan-cascade:approve - Approve PRD and start parallel execution
  - /plan-cascade:edit - Edit PRD manually
  - /plan-cascade:design-review - Review design document
  - /plan-cascade:show-dependencies - View dependency graph
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
