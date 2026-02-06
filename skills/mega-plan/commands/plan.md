---
description: "Generate a mega-plan for project-level multi-feature orchestration. Breaks a complex project into parallel features with dependencies. Usage: /mega:plan [--flow <quick|standard|full>] [--tdd <off|on|auto>] [--confirm] [--no-confirm] [--spec <off|auto|on>] [--first-principles] [--max-questions N] <project description> [design-doc-path]"
---

# Mega Plan - Project-Level Feature Orchestration

You are creating a **Mega Plan** - a project-level plan that orchestrates multiple features in parallel.

## Execution Flow Parameters

This command accepts flow control parameters that propagate to all feature executions:

### Parameter Priority

Parameters flow through three stages in mega-plan execution:

1. **Command-line flags to THIS command** (highest priority)
   - Example: `/mega:plan --flow full --tdd on "Build platform"`
   - Saved to `mega-plan.json` as `flow_config`, `tdd_config`, `spec_config`, etc.

2. **Command-line flags to `/mega:approve`**
   - Can override values saved in `mega-plan.json`
   - Propagated to all feature PRDs

3. **PRD-level overrides** (per feature, if needed)
   - Individual features can have custom parameters in their PRDs
   - Rarely used; usually all features use mega-plan settings

4. **Default values** (lowest priority)

**Parameter Propagation Chain:**
```bash
# Step 1: Create mega-plan with parameters
/mega:plan --flow full --tdd on --spec auto "Build e-commerce"
# → Saves to mega-plan.json:
#   flow_config: {level: "full", propagate_to_features: true}
#   tdd_config: {mode: "on", propagate_to_features: true}
#   spec_config: {mode: "auto", ...}

# Step 2: Execute with saved parameters
/mega:approve
# → Reads from mega-plan.json
# → For each feature: creates PRD with inherited flow/tdd settings
# → Sub-agents execute stories with these settings

# Step 3: Execute with override
/mega:approve --flow standard
# → Uses flow="standard" (overrides mega-plan.json)
# → All features get flow="standard", tdd="on" (from mega-plan.json)
```

**Note:** Spec interview parameters (--spec, --first-principles, --max-questions) are used by the orchestrator in `mega-approve` Step 6.0, NOT propagated to feature agents.

### `--flow <quick|standard|full>`

Override the execution flow depth for all feature approve phases.

| Flow | Gate Mode | AI Verification | Code Review | Test Enforcement |
|------|-----------|-----------------|-------------|------------------|
| `quick` | soft | disabled | no | no |
| `standard` | soft | enabled | no | no |
| `full` | **hard** | enabled | **required** | **required** |

### `--tdd <off|on|auto>`

Control Test-Driven Development mode for all feature story executions.

| Mode | Description |
|------|-------------|
| `off` | TDD disabled |
| `on` | TDD enabled with prompts and compliance checks |
| `auto` | Automatically decide based on risk assessment (default) |

### `--confirm`

Require confirmation before each **feature batch** execution.

**Batch-Level Confirmation**: In mega-plan execution, confirmation happens at the batch level (before launching parallel sub-agents), not inside individual feature executions. This allows human oversight while preserving parallelism.

### `--no-confirm`

Disable batch-level confirmation, even when using FULL flow.

- Overrides `--confirm` flag
- Overrides FULL flow's default confirmation behavior
- Useful for CI pipelines that want strict quality gates without interactive confirmation

### `--spec <off|auto|on>`

Record spec interview configuration for later execution in `mega-approve` (interviews must run in the orchestrator, never inside per-feature subagents).

- `auto` (default): enabled when `--flow full`, otherwise disabled
- `on`: always run spec interview per feature before PRD finalization
- `off`: never run spec interview

### `--first-principles`

Enable first-principles questions (only when spec interview runs).

### `--max-questions N`

Soft cap for interview length (recorded in `.state/spec-interview.json` per feature).

## Prerequisites Check

**CRITICAL**: If this is your first time using Plan Cascade, run `/plan-cascade:init` first to set up the environment.

```bash
# Quick check - if this fails, run /plan-cascade:init
uv run python -c "print('Environment OK')" 2>/dev/null || echo "Warning: Run /plan-cascade:init to set up environment"
```

## Path Storage Modes

Plan Cascade supports two path storage modes for runtime files:

### New Mode (Default)
Runtime files are stored in a user directory:
- **Windows**: `%APPDATA%/plan-cascade/<project-id>/`
- **Unix/macOS**: `~/.plan-cascade/<project-id>/`

File locations in new mode:
- `mega-plan.json`: `<user-dir>/mega-plan.json`
- `.mega-status.json`: `<user-dir>/.state/.mega-status.json`
- Feature worktrees: `<user-dir>/.worktree/<feature-name>/`
- `mega-findings.md`: `<project-root>/mega-findings.md` (user-visible, stays in project)

### Legacy Mode
All files in project root:
- `mega-plan.json`: `<project-root>/mega-plan.json`
- `.mega-status.json`: `<project-root>/.mega-status.json`
- Feature worktrees: `<project-root>/.worktree/<feature-name>/`

To check which mode is active:
```bash
uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; r=PathResolver(Path.cwd()); print('Mode:', 'legacy' if r.is_legacy_mode() else 'new'); print('Mega plan:', r.get_mega_plan_path())"
```

## Architecture

```
Level 1: Mega Plan (This level - Project)
    └── Level 2: Features (hybrid:worktree tasks)
              └── Level 3: Stories (hybrid-ralph internal parallelism)
```

## Step 0: Ensure .gitignore Configuration

**IMPORTANT**: Before creating any planning files, ensure the project's `.gitignore` is configured to ignore Plan Cascade temporary files:

```bash
# Check and update .gitignore for Plan Cascade entries
uv run python -c "from plan_cascade.utils.gitignore import ensure_gitignore; from pathlib import Path; ensure_gitignore(Path.cwd())" 2>/dev/null || echo "Note: Could not auto-update .gitignore"
```

This prevents planning files (mega-plan.json, .worktree/, etc.) from being accidentally committed.

## Step 1: Parse Arguments

Parse user arguments:
- **Project description**: First positional argument (required)
- **Design doc path**: Second positional argument (optional) - external design document to convert
- **--flow**: Execution flow depth (quick|standard|full) - propagates to all features
- **--tdd**: TDD mode (off|on|auto) - propagates to all features
- **--confirm**: Require batch confirmation - propagates to all features
- **--no-confirm**: Disable batch confirmation (overrides --confirm and FULL flow default) - propagates to all features
- **--spec**: Spec interview mode (off|auto|on) - recorded for mega-approve
- **--first-principles**: Enable first-principles questions (when spec interview runs)
- **--max-questions**: Soft cap for interview length (recorded)

```bash
PROJECT_DESC=""
DESIGN_DOC_ARG=""
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
            else
                # Positional arguments
                POS_INDEX=$((POS_INDEX + 1))
                case $POS_INDEX in
                    1) PROJECT_DESC="$arg" ;;
                    2) DESIGN_DOC_ARG="$arg" ;;
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

# Display parsed parameters
echo "Parsed Parameters:"
echo "  Project: ${PROJECT_DESC:-"(will prompt)"}"
echo "  Design Doc: ${DESIGN_DOC_ARG:-"(none)"}"
echo "  Flow: ${FLOW_LEVEL:-"(default)"}"
echo "  TDD: ${TDD_MODE:-"(default)"}"
echo "  Confirm: $CONFIRM_MODE"
echo "  No-Confirm: $NO_CONFIRM_MODE"
echo "  Spec: ${SPEC_MODE:-"(auto)"}"
echo "  First Principles: $FIRST_PRINCIPLES"
echo "  Max Questions: ${MAX_QUESTIONS:-"(default)"}"
```

If no description provided, ask the user:
```
Please provide a project description. What do you want to build?
Example: "Build an e-commerce platform with user authentication, product catalog, shopping cart, and order processing"

Optional arguments:
  - Flow control: /mega:plan --flow full "Build platform..."
  - TDD mode: /mega:plan --tdd on "Build platform..."
  - Confirm mode: /mega:plan --confirm "Build platform..."
  - No-confirm (CI): /mega:plan --no-confirm "Build platform..."
  - Spec interview: /mega:plan --flow full --spec on "Build platform..."
  - First principles: /mega:plan --flow full --spec on --first-principles "Build platform..."
  - Limit interview: /mega:plan --flow full --spec on --max-questions 12 "Build platform..."
  - Design document: /mega:plan "Build platform..." ./architecture.md

Example with full flow:
  /mega:plan --flow full --tdd on --confirm "Build e-commerce platform with users, products, cart, and orders"

Example with full flow in CI (no confirmation prompts):
  /mega:plan --flow full --tdd on --no-confirm "Build e-commerce platform with users, products, cart, and orders"
```

## Step 2: Check for Existing Mega Plan

```bash
# Get mega-plan path from PathResolver
MEGA_PLAN_PATH=$(uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_mega_plan_path())" 2>/dev/null || echo "mega-plan.json")

if [ -f "$MEGA_PLAN_PATH" ]; then
    echo "A mega-plan.json already exists at: $MEGA_PLAN_PATH"
fi
```

If exists, ask user:
- Overwrite the existing plan?
- Or use `/mega:edit` to modify it?

## Step 3: Analyze the Project

Analyze the project description and codebase to understand:
1. What features are needed
2. Dependencies between features
3. Priority of each feature
4. Appropriate breakdown granularity

Use the Explore agent or read relevant files to understand the existing codebase structure.

## Step 4: Generate the Mega Plan

Create `mega-plan.json` with 2-6 features:

```json
{
  "metadata": {
    "created_at": "<current ISO timestamp>",
    "version": "1.0.0"
  },
  "goal": "<one-sentence project goal>",
  "description": "<original user description>",
  "execution_mode": "auto",
  "target_branch": "main",
  "features": [
    {
      "id": "feature-001",
      "name": "feature-<name>",
      "title": "<Human Readable Title>",
      "description": "<detailed description for PRD generation>",
      "priority": "high",
      "dependencies": [],
      "status": "pending"
    }
  ]
}
```

**CRITICAL**: If flow/tdd/confirm parameters were specified, add them to the mega-plan:

```
# After generating basic mega-plan structure, add flow configuration
If FLOW_LEVEL is set:
    mega_plan["flow_config"] = {
        "level": FLOW_LEVEL,  # "quick", "standard", or "full"
        "source": "command-line",
        "propagate_to_features": true
    }

If TDD_MODE is set:
    mega_plan["tdd_config"] = {
        "mode": TDD_MODE,  # "off", "on", or "auto"
        "propagate_to_features": true
    }

# Record spec interview configuration for mega-approve
# Always save when FLOW_LEVEL == "full" (spec auto-enables in full flow)
If SPEC_MODE is set OR FIRST_PRINCIPLES is true OR MAX_QUESTIONS is set OR FLOW_LEVEL == "full":
    mega_plan["spec_config"] = {
        "mode": (SPEC_MODE or "auto"),  # "off", "auto", or "on" (defaults to auto)
        "first_principles": FIRST_PRINCIPLES,
        "max_questions": (MAX_QUESTIONS or 18),
        "propagate_to_features": true
    }

# --no-confirm takes precedence over --confirm
mega_plan["execution_config"] = mega_plan.get("execution_config", {})
mega_plan["execution_config"]["propagate_to_features"] = true
If NO_CONFIRM_MODE is true:
    mega_plan["execution_config"]["require_batch_confirm"] = false
    mega_plan["execution_config"]["no_confirm_override"] = true  # Explicit override marker
Elif CONFIRM_MODE is true:
    mega_plan["execution_config"]["require_batch_confirm"] = true
```

Example mega-plan.json with full flow configuration:
```json
{
  "metadata": { ... },
  "goal": "Build e-commerce platform",
  "flow_config": {
    "level": "full",
    "source": "command-line",
    "propagate_to_features": true
  },
  "tdd_config": {
    "mode": "on",
    "propagate_to_features": true
  },
  "spec_config": {
    "mode": "auto",
    "first_principles": false,
    "max_questions": 18,
    "propagate_to_features": true
  },
  "execution_config": {
    "require_batch_confirm": true,
    "propagate_to_features": true
  },
  "features": [ ... ]
}
```

Example mega-plan.json with CI mode (full flow, no confirmation):
```json
{
  "metadata": { ... },
  "goal": "Build e-commerce platform",
  "flow_config": {
    "level": "full",
    "source": "command-line",
    "propagate_to_features": true
  },
  "tdd_config": {
    "mode": "on",
    "propagate_to_features": true
  },
  "spec_config": {
    "mode": "auto",
    "first_principles": false,
    "max_questions": 18,
    "propagate_to_features": true
  },
  "execution_config": {
    "require_batch_confirm": false,
    "no_confirm_override": true,
    "propagate_to_features": true
  },
  "features": [ ... ]
}
```

**Feature Guidelines:**
- `name`: lowercase letters, numbers, hyphens only (e.g., "feature-auth", "feature-products")
- `description`: Detailed enough to generate a complete PRD with 3-7 stories
- `dependencies`: List feature IDs that must complete first
- `priority`: "high" (core), "medium" (important), "low" (nice-to-have)

## Step 5: Generate Project-Level Design Document

After generating mega-plan.json, automatically generate `design_doc.json` (project-level):

### 5.1: Check for User-Provided Design Document

```
If DESIGN_DOC_ARG is not empty and file exists:
    Read the external document at DESIGN_DOC_ARG
    Detect format:
      - .md files: Parse Markdown structure (headers -> sections)
      - .json files: Validate/map to our schema
      - .html files: Parse HTML structure
    Convert to our format:
      - Extract overview, architecture, patterns, decisions
      - Map to feature_mappings based on mega-plan features
    Save as design_doc.json
    DESIGN_SOURCE="Converted from: $DESIGN_DOC_ARG"
Else:
    Auto-generate based on mega-plan analysis
    DESIGN_SOURCE="Auto-generated from mega-plan"
```

### 5.2: Auto-Generate Project Design Document

Based on the features in mega-plan.json, generate `design_doc.json`:

```json
{
  "metadata": {
    "created_at": "<timestamp>",
    "version": "1.0.0",
    "source": "ai-generated",
    "level": "project",
    "mega_plan_reference": "mega-plan.json"
  },
  "overview": {
    "title": "<from mega-plan goal>",
    "summary": "<project summary>",
    "goals": ["<extracted goals>"],
    "non_goals": ["<identified non-goals>"]
  },
  "architecture": {
    "system_overview": "<high-level architecture based on features>",
    "components": [
      {
        "name": "ComponentName",
        "description": "Description",
        "responsibilities": ["resp1"],
        "dependencies": [],
        "features": ["feature-001", "feature-002"]
      }
    ],
    "data_flow": "<how data flows between features>",
    "patterns": [
      {
        "name": "PatternName",
        "description": "What it does",
        "rationale": "Why use it",
        "applies_to": ["feature-001", "all"]
      }
    ],
    "infrastructure": {}
  },
  "interfaces": {
    "api_standards": {
      "style": "RESTful",
      "versioning": "URL-based",
      "authentication": "<method>"
    },
    "shared_data_models": []
  },
  "decisions": [
    {
      "id": "ADR-001",
      "title": "Decision title",
      "context": "Background",
      "decision": "What we decided",
      "rationale": "Why",
      "alternatives_considered": [],
      "status": "accepted",
      "applies_to": ["all"]
    }
  ],
  "feature_mappings": {
    "feature-001": {
      "components": [],
      "patterns": [],
      "decisions": [],
      "description": "<from mega-plan>"
    }
  }
}
```

**Design Document Generation Guidelines:**
- Analyze feature descriptions to identify shared components
- Identify cross-cutting patterns (e.g., Repository, Service Layer)
- Create feature_mappings linking each feature to relevant patterns/decisions
- Mark patterns/decisions with `applies_to: ["all"]` if they apply universally

## Step 6: Create Supporting Files

Create `mega-findings.md`:

```markdown
# Mega Plan Findings

Project: <goal>
Created: <timestamp>

This file contains shared findings across all features.
Feature-specific findings should be in their respective worktrees.

---

## Project-Wide Decisions

<!-- Add project-wide architectural decisions here -->

## Shared Patterns

<!-- Add patterns that apply across features -->

## Integration Notes

<!-- Add notes about how features will integrate -->
```

Create `.mega-status.json` (in state directory for new mode, project root for legacy):

```bash
# Get status file path from PathResolver
MEGA_STATUS_PATH=$(uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_mega_status_path())" 2>/dev/null || echo ".mega-status.json")

# Ensure directory exists
mkdir -p "$(dirname "$MEGA_STATUS_PATH")"
```

```json
{
  "updated_at": "<timestamp>",
  "execution_mode": "auto",
  "target_branch": "main",
  "current_batch": 0,
  "features": {}
}
```

## Step 6: Calculate Execution Batches

Group features by dependencies:
- **Batch 1**: Features with no dependencies (run in parallel)
- **Batch 2**: Features depending only on Batch 1 (run in parallel after Batch 1)
- And so on...

**IMPORTANT**: Batches are executed sequentially:
1. Batch 1 features run in parallel
2. When ALL Batch 1 features complete, they are **merged to target_branch**
3. Batch 2 worktrees are created from the **updated** target_branch
4. This ensures Batch 2 features have access to Batch 1 code

## Step 7: Ask Execution Mode

Use AskUserQuestion to ask:

**How should feature batches progress?**

Options:
1. **Auto Mode (Recommended)** - Batches progress automatically when ready
2. **Manual Mode** - Confirm before starting each batch

Update `mega-plan.json` with the chosen mode.

## Step 8: Display Unified Review

**CRITICAL**: Use Bash to display the unified Mega-Plan + Design Document review:

```bash
uv run python "${CLAUDE_PLUGIN_ROOT}/skills/hybrid-ralph/scripts/unified-review.py" --mode mega
```

This displays:
- Mega-plan summary with features, priorities, and execution batches
- Project-level design document with system architecture, components, and patterns
- Feature-to-design mappings (showing which features use which components)
- Warnings for any unmapped features
- Available next steps

If the script is not available, display a manual summary:

```
============================================================
MEGA PLAN CREATED
============================================================

Goal: <goal>
Execution Mode: <auto|manual>
Target Branch: <target_branch>
Total Features: <count>

Feature Batches:

Batch 1 (Parallel - No Dependencies):
  - feature-001: <title> [high]
  - feature-002: <title> [high]

Batch 2 (After Batch 1):
  - feature-003: <title> [medium] (depends on: feature-001, feature-002)

============================================================
```

## Step 9: Show Files Created and Next Steps

After unified review, confirm created files and show execution configuration:

```
Files created:
  - mega-plan.json       (project plan - in user data dir or project root)
  - design_doc.json      (project-level technical design - in project root)
  - mega-findings.md     (shared findings - always in project root for visibility)
  - .mega-status.json    (execution status - in .state/ dir or project root)

Note: Use PathResolver to find exact file locations based on storage mode.

============================================================
EXECUTION CONFIGURATION
============================================================
  Flow Level: {FLOW_LEVEL or "standard (default)"}
  TDD Mode: {TDD_MODE or "auto (default)"}
  Batch Confirm: {CONFIRM_MODE}

  These settings will propagate to all feature executions.
============================================================

NEXT STEPS:

  Review and edit (optional):
    /mega:edit

  Approve and execute:
```

**CRITICAL**: Build the approve command with preserved parameters:

```
# Build mega-approve command with flow/tdd parameters
APPROVE_CMD = "/mega:approve"

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
- Standard flow: `/mega:approve`
- Full flow with TDD: `/mega:approve --flow full --tdd on`
- Full flow with confirm: `/mega:approve --flow full --tdd on --confirm`
- Full flow CI-friendly: `/mega:approve --flow full --tdd on --no-confirm`

## Example

User: `/mega:plan Build a blog platform with user accounts, post management, comments, and RSS feeds`

Generated features:
1. **feature-users** (high, no deps) - User accounts with registration, login, profiles
2. **feature-posts** (high, no deps) - Blog post CRUD with categories and tags
3. **feature-comments** (medium, depends on users+posts) - Comment system with moderation
4. **feature-rss** (low, depends on posts) - RSS/Atom feed generation

Batches:
- Batch 1: feature-users, feature-posts (parallel)
- Batch 2: feature-comments, feature-rss (after Batch 1)
