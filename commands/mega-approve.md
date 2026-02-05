---
description: "Approve the mega-plan and start feature execution. Creates worktrees and generates PRDs for each feature. Usage: /plan-cascade:mega-approve [--flow <quick|standard|full>] [--tdd <off|on|auto>] [--confirm] [--no-confirm] [--spec <off|auto|on>] [--first-principles] [--max-questions N] [--auto-prd] [--agent <name>] [--prd-agent <name>] [--impl-agent <name>]"
---

# Approve Mega Plan and Start Execution

Approve the mega-plan and begin executing features in **batch-by-batch** order with **FULL AUTOMATION**.

## Execution Flow Parameters

This command accepts flow control parameters that propagate to ALL feature executions:

### Parameter Priority

Parameters are resolved in the following order when multiple sources exist:

1. **Command-line flags to THIS command** (highest priority)
   - Example: `/plan-cascade:mega-approve --flow full --tdd on`
   - Overrides all other sources

2. **mega-plan.json configuration** (saved by `/plan-cascade:mega-plan`)
   - `flow_config`, `tdd_config`, `spec_config`, `execution_config`
   - Used when no command-line flag is provided

3. **Default values** (lowest priority)

**Priority Resolution Example:**
```bash
# Scenario 1: Command-line overrides mega-plan
mega-plan.json contains: flow_config.level = "standard"
You run: /plan-cascade:mega-approve --flow full
Result: All features use FLOW = "full"

# Scenario 2: Use mega-plan config
mega-plan.json contains: tdd_config.mode = "on"
You run: /plan-cascade:mega-approve
Result: All features use TDD = "on"

# Scenario 3: Partial override
mega-plan.json contains: flow="full", tdd="on"
You run: /plan-cascade:mega-approve --flow standard
Result: All features use FLOW = "standard", TDD = "on"
```

**Parameter Propagation to Features:**
- Resolved parameters are passed to PRD generation agents via prompt
- PRD generation agents write these parameters to each feature's `prd.json`
- Story execution agents read parameters from their feature's `prd.json`

**Special Case - Spec Interview Parameters:**
- `--spec`, `--first-principles`, `--max-questions` are orchestrator-only
- Used in Step 6.0 (this command) to run spec interviews
- NOT propagated to feature agents (see Step 2.0.2 notes)

### `--flow <quick|standard|full>`

Override the execution flow depth for all feature approve phases.

| Flow | Gate Mode | AI Verification | Code Review | Test Enforcement |
|------|-----------|-----------------|-------------|------------------|
| `quick` | soft | disabled | no | no |
| `standard` | soft | enabled | no | no |
| `full` | **hard** | enabled | **required** | **required** |

**FULL Flow Enforcement** (propagates to all features):
- Quality gates BLOCK execution on failure
- Code review is REQUIRED after each story
- Test file changes are REQUIRED alongside code changes
- With `--confirm`: Batch-level confirmation before each feature batch (see below)

### `--tdd <off|on|auto>`

Control Test-Driven Development mode for all feature story executions.

| Mode | Description |
|------|-------------|
| `off` | TDD disabled |
| `on` | TDD enabled with prompts and compliance checks |
| `auto` | Automatically enable TDD for high-risk stories (default) |

### `--confirm`

Require explicit user confirmation before starting each **feature batch**.

**IMPORTANT: Batch-Level Confirmation Design**

In mega-plan execution, confirmation happens at the **batch level**, not inside sub-agents:

```
Batch 1: [feature-A, feature-B, feature-C]
    ↓
[CONFIRMATION POINT] ← User confirms here (Step 4.5)
    ↓
All 3 features execute in parallel (no further confirmation)
    ↓
Batch 2: [feature-D, feature-E]
    ↓
[CONFIRMATION POINT] ← User confirms here
    ↓
...
```

Why batch-level confirmation:
- Sub-agents run in background (`run_in_background=true`)
- Background agents cannot use `AskUserQuestion`
- Multiple parallel agents requesting confirmation would be chaotic
- Batch-level provides oversight while preserving parallelism

### `--no-confirm`

Disable batch confirmation, even when using FULL flow.

- Overrides `--confirm` flag
- Overrides mega-plan.json's `execution_config.require_batch_confirm`
- Overrides FULL flow's default confirmation behavior
- Useful for CI pipelines that want strict quality gates without interactive confirmation

### `--spec <off|auto|on>`

Run a planning-time **spec interview** per feature (in the orchestrator only) to produce `spec.json/spec.md`,
then compile to `prd.json` before story execution starts.

- `auto` (default): enabled when `--flow full`, otherwise disabled
- `on`: always run interview before PRD finalization
- `off`: never run interview

### `--first-principles`

Enable first-principles questions (only when spec interview runs).

### `--max-questions N`

Soft cap for interview length (recorded in `.state/spec-interview.json` per feature).

## Path Storage Modes

This command works with both new and legacy path storage modes:

### New Mode (Default)
Files are stored in user data directory:
- **Windows**: `%APPDATA%/plan-cascade/<project-id>/`
- **Unix/macOS**: `~/.plan-cascade/<project-id>/`

File locations:
- `mega-plan.json`: `<user-dir>/mega-plan.json`
- `.mega-status.json`: `<user-dir>/.state/.mega-status.json`
- Worktrees: `<user-dir>/.worktree/<feature-name>/`

### Legacy Mode
All files in project root:
- `mega-plan.json`: `<project-root>/mega-plan.json`
- `.mega-status.json`: `<project-root>/.mega-status.json`
- Worktrees: `<project-root>/.worktree/<feature-name>/`

The command uses PathResolver to determine correct paths automatically.

## Multi-Agent Collaboration

Mega-plan execution supports multiple AI agents at two levels:

1. **PRD Generation**: Agent that generates PRDs for each feature worktree
2. **Story Execution**: Agent that executes stories within each feature

### Supported Agents

| Agent | Type | Best For |
|-------|------|----------|
| `claude-code` | task-tool | General purpose (default, always available) |
| `codex` | cli | PRD generation, bug fixes |
| `aider` | cli | Refactoring, code improvements |
| `amp-code` | cli | Alternative implementations |

### Command Parameters

```
--auto-prd           Fully automatic mode (no pauses)
--agent <name>       Global agent override (all tasks use this agent)
--prd-agent <name>   Agent for PRD generation phase
--impl-agent <name>  Agent for story implementation phase
--no-fallback        Disable automatic fallback to claude-code
```

## Tool Usage Policy (CRITICAL)

**To avoid command confirmation prompts during automatic execution:**

1. **Use Read tool for file reading** - NEVER use `cat`, `head`, `tail` via Bash
   - ✅ `Read("mega-plan.json")`
   - ❌ `Bash("cat mega-plan.json")`

2. **Use Glob tool for file finding** - NEVER use `find`, `ls` via Bash
   - ✅ `Glob(".worktree/*/progress.txt")`
   - ❌ `Bash("ls .worktree/*/progress.txt")`

3. **Use Grep tool for content search** - NEVER use `grep` via Bash
   - ✅ `Grep("[FEATURE_COMPLETE]", path=".worktree/feature-x/progress.txt")`
   - ❌ `Bash("grep '[FEATURE_COMPLETE]' .worktree/feature-x/progress.txt")`

4. **Only use Bash for actual system commands:**
   - Git operations: `git checkout`, `git merge`, `git worktree add`
   - Directory creation: `mkdir -p`
   - File writing (when Write tool cannot be used): `echo "..." >> file`

5. **For monitoring loops:** Use Read tool to poll file contents, not Bash cat/grep

**CRITICAL**: With `--auto-prd`, this command runs the ENTIRE mega-plan to completion automatically:
1. Creates worktrees for current batch
2. Generates PRDs for each feature (via Task agents)
3. Executes all stories in each feature (via Task agents)
4. Monitors for completion
5. Merges completed batch to target branch
6. Automatically starts next batch
7. Repeats until ALL batches complete

**WITHOUT `--auto-prd`**: Pauses after PRD generation for manual review.

## Step 1: Verify Mega Plan Exists

```bash
# Get mega-plan path from PathResolver
MEGA_PLAN_PATH=$(uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_mega_plan_path())" 2>/dev/null || echo "mega-plan.json")

if [ ! -f "$MEGA_PLAN_PATH" ]; then
    echo "No mega-plan.json found at: $MEGA_PLAN_PATH"
    echo "Use /plan-cascade:mega-plan <description> to create one first."
    exit 1
fi
```

## Step 2: Parse Arguments and State

Parse all command arguments including flow control parameters:

```bash
# Mode flags
AUTO_PRD=false
NO_FALLBACK=false

# Flow control parameters (propagate to all features)
FLOW_LEVEL=""           # --flow <quick|standard|full>
TDD_MODE=""             # --tdd <off|on|auto>
CONFIRM_MODE=false      # --confirm
NO_CONFIRM_MODE=false   # --no-confirm
CONFIRM_EXPLICIT=false  # set when --confirm is provided
NO_CONFIRM_EXPLICIT=false  # set when --no-confirm is provided

# Spec interview parameters (orchestrator-only)
SPEC_MODE=""            # --spec <off|auto|on>
FIRST_PRINCIPLES=false  # --first-principles
MAX_QUESTIONS=""        # --max-questions N

# Agent parameters
GLOBAL_AGENT=""
PRD_AGENT=""
IMPL_AGENT=""

# Parse flags
NEXT_IS_FLOW=false
NEXT_IS_TDD=false
NEXT_IS_SPEC=false
NEXT_IS_MAXQ=false
NEXT_IS_AGENT=false
NEXT_IS_PRD_AGENT=false
NEXT_IS_IMPL_AGENT=false

# Parse arguments
for arg in $ARGUMENTS; do
    case "$arg" in
        # Flow control flags
        --flow=*) FLOW_LEVEL="${arg#*=}" ;;
        --flow) NEXT_IS_FLOW=true ;;
        --tdd=*) TDD_MODE="${arg#*=}" ;;
        --tdd) NEXT_IS_TDD=true ;;
        --confirm) CONFIRM_MODE=true; CONFIRM_EXPLICIT=true ;;
        --no-confirm) NO_CONFIRM_MODE=true; NO_CONFIRM_EXPLICIT=true ;;
        # Spec interview flags
        --spec=*) SPEC_MODE="${arg#*=}" ;;
        --spec) NEXT_IS_SPEC=true ;;
        --first-principles) FIRST_PRINCIPLES=true ;;
        --max-questions=*) MAX_QUESTIONS="${arg#*=}" ;;
        --max-questions) NEXT_IS_MAXQ=true ;;
        # Mode flags
        --auto-prd) AUTO_PRD=true ;;
        --no-fallback) NO_FALLBACK=true ;;
        # Agent flags
        --agent=*) GLOBAL_AGENT="${arg#*=}" ;;
        --agent) NEXT_IS_AGENT=true ;;
        --prd-agent=*) PRD_AGENT="${arg#*=}" ;;
        --prd-agent) NEXT_IS_PRD_AGENT=true ;;
        --impl-agent=*) IMPL_AGENT="${arg#*=}" ;;
        --impl-agent) NEXT_IS_IMPL_AGENT=true ;;
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
                GLOBAL_AGENT="$arg"
                NEXT_IS_AGENT=false
            elif [ "$NEXT_IS_PRD_AGENT" = true ]; then
                PRD_AGENT="$arg"
                NEXT_IS_PRD_AGENT=false
            elif [ "$NEXT_IS_IMPL_AGENT" = true ]; then
                IMPL_AGENT="$arg"
                NEXT_IS_IMPL_AGENT=false
            fi
            ;;
    esac
done

if [ "$AUTO_PRD" = true ]; then
    echo "============================================"
    echo "FULLY AUTOMATIC MODE ENABLED"
    echo "============================================"
    echo "Will execute ALL batches without stopping."
    echo "Only pauses on errors."
    echo "============================================"
fi

# Display configuration with parameter sources
echo ""
echo "============================================================"
echo "EXECUTION CONFIGURATION (with sources)"
echo "============================================================"
echo "Flow Level: ${FLOW_LEVEL:-"standard (default)"}"
echo "  Source: ${FLOW_LEVEL_SOURCE:-"[DEFAULT]"}"
echo ""
echo "TDD Mode: ${TDD_MODE:-"auto (default)"}"
echo "  Source: ${TDD_MODE_SOURCE:-"[DEFAULT]"}"
echo ""
echo "Batch Confirm: ${CONFIRM_MODE}"
echo "  No-Confirm Override: ${NO_CONFIRM_MODE}"
echo "  Source: ${CONFIRM_SOURCE:-"[DEFAULT]"}"
echo ""
echo "Spec Interview: ${SPEC_MODE:-"auto (default)"}"
echo "  First Principles: ${FIRST_PRINCIPLES}"
echo "  Max Questions: ${MAX_QUESTIONS:-"18 (default)"}"
echo "  Source: ${SPEC_MODE_SOURCE:-"[DEFAULT]"}"
echo ""
echo "Agent Configuration:"
echo "  Global Override: ${GLOBAL_AGENT:-"none (use defaults)"}"
echo "  PRD Generation: ${PRD_AGENT:-"claude-code"}"
echo "  Implementation: ${IMPL_AGENT:-"per-story resolution"}"
echo "  Fallback: ${NO_FALLBACK:+"disabled"}"
echo ""
echo "Parameter Sources Legend:"
echo "  [CLI]     - Command-line flag (highest priority)"
echo "  [MEGA]    - mega-plan.json configuration"
echo "  [DEFAULT] - Built-in default value"
echo ""
echo "These settings will propagate to all feature executions."
echo "============================================================"
echo ""
```

### 2.0.1: Check mega-plan.json for Flow Configuration

**CRITICAL**: If flow/tdd/confirm parameters were not specified on command line, check mega-plan.json for configuration from `/plan-cascade:mega-plan`.

**Parameter source tracking is added for debugging.**

```
# Read mega-plan.json once
mega_plan = Read("mega-plan.json")

# Resolve FLOW_LEVEL with source tracking
If FLOW_LEVEL is set from command-line:
    FLOW_LEVEL_SOURCE="[CLI] --flow ${FLOW_LEVEL}"
    echo "Note: Using flow level from command-line: ${FLOW_LEVEL}"
Elif mega_plan has "flow_config" field:
    FLOW_LEVEL = mega_plan.flow_config.level
    FLOW_LEVEL_SOURCE="[MEGA] flow_config.level"
    echo "Note: Using flow level from mega-plan: ${FLOW_LEVEL}"
Else:
    FLOW_LEVEL = "standard"
    FLOW_LEVEL_SOURCE="[DEFAULT]"

# Resolve TDD_MODE with source tracking
If TDD_MODE is set from command-line:
    TDD_MODE_SOURCE="[CLI] --tdd ${TDD_MODE}"
    echo "Note: Using TDD mode from command-line: ${TDD_MODE}"
Elif mega_plan has "tdd_config" field:
    TDD_MODE = mega_plan.tdd_config.mode
    TDD_MODE_SOURCE="[MEGA] tdd_config.mode"
    echo "Note: Using TDD mode from mega-plan: ${TDD_MODE}"
Else:
    TDD_MODE = "auto"
    TDD_MODE_SOURCE="[DEFAULT]"

# Resolve SPEC_MODE with source tracking
If SPEC_MODE is set from command-line:
    SPEC_MODE_SOURCE="[CLI] --spec ${SPEC_MODE}"
    echo "Note: Using spec interview config from command-line: ${SPEC_MODE}"
Elif mega_plan has "spec_config" field:
    SPEC_MODE = mega_plan.spec_config.mode
    FIRST_PRINCIPLES = mega_plan.spec_config.first_principles
    MAX_QUESTIONS = mega_plan.spec_config.max_questions
    SPEC_MODE_SOURCE="[MEGA] spec_config"
    echo "Note: Using spec interview config from mega-plan: ${SPEC_MODE}"
Else:
    SPEC_MODE = "auto"
    SPEC_MODE_SOURCE="[DEFAULT]"

# Resolve CONFIRM_MODE with source tracking (complex priority chain)
If NO_CONFIRM_MODE is true:  # --no-confirm from command line takes absolute precedence
    CONFIRM_MODE = false
    CONFIRM_SOURCE="[CLI] --no-confirm (overrides all)"
    echo "Note: Batch confirmation DISABLED by --no-confirm flag"
Elif CONFIRM_EXPLICIT is true:  # --confirm from command line
    CONFIRM_MODE = true
    CONFIRM_SOURCE="[CLI] --confirm"
    echo "Note: Batch confirmation enabled by --confirm flag"
Elif mega_plan has "execution_config" and execution_config.no_confirm_override == true:
    CONFIRM_MODE = false
    NO_CONFIRM_MODE = true
    CONFIRM_SOURCE="[MEGA] execution_config.no_confirm_override"
    echo "Note: Batch confirmation DISABLED by mega-plan config"
Elif mega_plan has "execution_config" and execution_config.require_batch_confirm == true:
    CONFIRM_MODE = true
    CONFIRM_SOURCE="[MEGA] execution_config.require_batch_confirm"
    echo "Note: Batch confirmation enabled by mega-plan config"
Elif FLOW_LEVEL == "full":  # FULL flow default (after flow config is resolved)
    CONFIRM_MODE = true
    CONFIRM_SOURCE="[DEFAULT] FULL flow default"
    echo "Note: Batch confirmation enabled by FULL flow default"
Else:
    CONFIRM_MODE = false
    CONFIRM_SOURCE="[DEFAULT]"
```

### 2.0.2: Build Parameter String for Feature Execution

**CRITICAL**: Build parameter string to pass to PRD generation and story execution agents for each feature.

**IMPORTANT**: Spec interview parameters (--spec, --first-principles, --max-questions) are NOT included in FEATURE_PARAMS because:
1. Spec interview is executed in the orchestrator (Step 6.0) BEFORE feature agents are launched
2. Feature agents receive PRDs that are already compiled from spec.json
3. Including these parameters would cause feature agents to attempt duplicate spec interviews
4. Spec interview parameters are recorded in SPEC_MODE, FIRST_PRINCIPLES, MAX_QUESTIONS variables for orchestrator use

```
FEATURE_PARAMS = ""

If FLOW_LEVEL is set:
    FEATURE_PARAMS = FEATURE_PARAMS + " --flow " + FLOW_LEVEL

If TDD_MODE is set:
    FEATURE_PARAMS = FEATURE_PARAMS + " --tdd " + TDD_MODE

# NOTE: Spec interview parameters are NOT propagated to feature agents
# They are used only in Step 6.0 (orchestrator-level spec interview)
# The variables SPEC_MODE, FIRST_PRINCIPLES, MAX_QUESTIONS are preserved for orchestrator use

# --no-confirm takes precedence over --confirm
If NO_CONFIRM_MODE is true:
    FEATURE_PARAMS = FEATURE_PARAMS + " --no-confirm"
Elif CONFIRM_MODE is true:
    FEATURE_PARAMS = FEATURE_PARAMS + " --confirm"

# Trim leading space
FEATURE_PARAMS = trim(FEATURE_PARAMS)

echo "Feature execution parameters: ${FEATURE_PARAMS}"
echo ""
echo "Orchestrator-only parameters (not propagated to features):"
echo "  Spec Mode: ${SPEC_MODE:-"none"}"
echo "  First Principles: ${FIRST_PRINCIPLES}"
echo "  Max Questions: ${MAX_QUESTIONS:-"none"}"
echo ""
```

### 2.1: Load Agent Configuration

```
If agents.json exists at project root:
    Load agent configuration:
    - agents: Map of agent definitions
    - phase_defaults: PRD generation and implementation defaults
    - story_type_defaults: Agent selection by story type
Else:
    Use defaults: claude-code for all tasks
```

Read current state from `.mega-status.json`:
- `current_batch`: Which batch is currently executing (0 = not started)
- `completed_batches`: List of completed batch numbers
- `features`: Status of each feature

## Step 2.5: Check for Design Document (Optional)

Check if a global `design_doc.json` exists at project root:

```
If design_doc.json exists:
    Read and display design document summary:

    ────────────────────────────────────────────────────────────────
    📐 GLOBAL DESIGN DOCUMENT DETECTED
    ────────────────────────────────────────────────────────────────
    Title: <overview.title>

    Components: N defined
      • <component1> - <description>
      • <component2> - <description>

    Architectural Patterns: M patterns
      • <pattern1>
      • <pattern2>

    Key Decisions: P ADRs
      • ADR-001: <title>
      • ADR-002: <title>

    This design document will be:
    ✓ Copied to each feature worktree
    ✓ Used to guide PRD generation
    ✓ Injected into story execution context
    ────────────────────────────────────────────────────────────────

    HAS_DESIGN_DOC=true
Else:
    Note: No global design document found.
          Consider creating one with /plan-cascade:design-generate
          for better architectural guidance across features.

    HAS_DESIGN_DOC=false
```

## Step 3: Calculate Batches and Determine State

Calculate all batches from mega-plan.json based on dependencies:
- **Batch 1**: Features with no dependencies
- **Batch 2**: Features depending only on Batch 1 features
- **Batch N**: Features depending only on Batch 1..N-1 features

Determine current state:

### Case A: No batch started yet (current_batch = 0 or missing)
→ Start Batch 1

### Case B: Current batch is in progress
→ Check if all features in current batch are complete
→ If not complete AND --auto-prd: Continue monitoring (don't exit)
→ If not complete AND no --auto-prd: Show status and exit
→ If complete: Merge current batch, then start next batch

### Case C: All batches complete
→ Run final cleanup and inform user

## Step 4: Main Execution Loop (AUTOMATIC)

**CRITICAL**: This is the main automation loop. With `--auto-prd`, this loop runs until ALL batches are complete.

```
TOTAL_BATCHES = <calculated from mega-plan.json>
CURRENT_BATCH = <from .mega-status.json or 1 if not started>

while CURRENT_BATCH <= TOTAL_BATCHES:

    # 4.1: Check if current batch features exist (worktrees created)
    if worktrees_not_created:
        create_worktrees_for_batch(CURRENT_BATCH)
        generate_prds_for_batch(CURRENT_BATCH)
        execute_stories_for_batch(CURRENT_BATCH)

    # 4.2: Monitor current batch until complete
    monitor_batch_until_complete(CURRENT_BATCH)

    # 4.3: Merge completed batch
    merge_batch_to_target(CURRENT_BATCH)

    # 4.4: Cleanup worktrees
    cleanup_batch_worktrees(CURRENT_BATCH)

    # 4.5: Move to next batch
    CURRENT_BATCH += 1

# All batches complete!
show_completion_status()
```

## Step 4.5: Batch-Level Confirmation (CRITICAL)

**IMPORTANT**: Confirmation happens HERE at the mega-approve level, NOT inside sub-agents.

Sub-agents run in background (`run_in_background=true`) and cannot interact with the user. Therefore:
- Batch confirmation occurs in the main agent before launching sub-agents
- Sub-agents execute autonomously without waiting for confirmation
- This preserves parallelism while providing human oversight

```
If CONFIRM_MODE is true AND NO_CONFIRM_MODE is false:
    # Display batch information
    echo ""
    echo "============================================================"
    echo "BATCH ${CURRENT_BATCH} OF ${TOTAL_BATCHES} - CONFIRMATION REQUIRED"
    echo "============================================================"
    echo ""
    echo "Features in this batch:"
    For each feature in current_batch:
        echo "  - ${feature.id}: ${feature.name}"
        echo "    Description: ${feature.description}"
        echo "    Priority: ${feature.priority}"
    echo ""
    echo "Execution Configuration:"
    echo "  Flow Level: ${FLOW_LEVEL}"
    echo "  TDD Mode: ${TDD_MODE}"
    echo "  Gate Mode: ${GATE_MODE}"
    echo ""
    echo "This will:"
    echo "  1. Create worktrees for ${batch_size} features"
    echo "  2. Generate PRDs in parallel"
    echo "  3. Execute all stories in parallel"
    echo "  4. Merge completed features to ${TARGET_BRANCH}"
    echo ""

    # Use AskUserQuestion for confirmation
    AskUserQuestion(
        questions=[{
            "question": "Proceed with Batch ${CURRENT_BATCH}?",
            "header": "Batch Confirm",
            "options": [
                {"label": "Yes, proceed", "description": "Start executing this batch"},
                {"label": "Skip this batch", "description": "Mark as skipped and continue to next batch"},
                {"label": "Abort", "description": "Stop mega-plan execution"}
            ],
            "multiSelect": false
        }]
    )

    # Handle response
    If response == "Skip this batch":
        echo "Batch ${CURRENT_BATCH} skipped by user"
        Update .mega-status.json: mark batch as skipped
        CURRENT_BATCH += 1
        Continue to next iteration
    Elif response == "Abort":
        echo "Mega-plan execution aborted by user"
        Exit
    # Else: proceed with batch execution

    echo ""
    echo "✓ Batch ${CURRENT_BATCH} confirmed. Starting execution..."
    echo ""
```

## Step 5: Create Worktrees for Batch

For each feature in the current batch:

### 5.1: Checkout Updated Target Branch

```bash
TARGET_BRANCH=$(jq -r '.target_branch' mega-plan.json)
git checkout "$TARGET_BRANCH"
git pull origin "$TARGET_BRANCH" 2>/dev/null || true
```

### 5.2: Create Worktree

```bash
# Get worktree base directory from PathResolver
WORKTREE_BASE=$(uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_worktree_dir())" 2>/dev/null || echo ".worktree")

# Ensure worktree base directory exists
mkdir -p "$WORKTREE_BASE"

FEATURE_NAME="<feature-name>"
BRANCH_NAME="mega-$FEATURE_NAME"
WORKTREE_PATH="$WORKTREE_BASE/$FEATURE_NAME"

# Create worktree from current HEAD (includes all previously merged batches)
git worktree add -b "$BRANCH_NAME" "$WORKTREE_PATH"
```

### 5.3: Initialize Worktree Files

Create in each worktree:
- `.planning-config.json` with feature metadata
- `findings.md` initialized with feature info
- `progress.txt` for tracking
- Copy `mega-findings.md` from project root
- Copy `design_doc.json` from project root (if exists)

## Step 6: Generate PRDs for Batch (Task Agents)

### 6.0: Optional Spec Interview (Orchestrator-only)

**CRITICAL**: Spec interview runs in the orchestrator (THIS command) BEFORE launching feature agents.

If spec interview is enabled (`SPEC_MODE == on` OR `SPEC_MODE == auto` and `FLOW_LEVEL == full`):

1. **Do NOT launch PRD generation sub-agents yet** (they must remain non-interactive)
2. For each feature in the batch, in the **main orchestrator** (current command context):
   ```bash
   # Build spec-plan command with orchestrator-only parameters
   SPEC_CMD="/plan-cascade:spec-plan \"<feature description>\""
   SPEC_CMD="${SPEC_CMD} --flow ${FLOW_LEVEL}"
   SPEC_CMD="${SPEC_CMD} --feature-slug <feature-name>"
   SPEC_CMD="${SPEC_CMD} --compile"  # CRITICAL: Compile to prd.json

   # Add TDD mode from orchestrator config
   If TDD_MODE is set:
       SPEC_CMD="${SPEC_CMD} --tdd ${TDD_MODE}"

   # Add confirm mode from orchestrator config
   If NO_CONFIRM_MODE is true:
       SPEC_CMD="${SPEC_CMD} --no-confirm"
   Elif CONFIRM_MODE is true:
       SPEC_CMD="${SPEC_CMD} --confirm"

   # Add first-principles flag (orchestrator-only parameter)
   If FIRST_PRINCIPLES is true:
       SPEC_CMD="${SPEC_CMD} --first-principles"

   # Add max-questions limit (orchestrator-only parameter)
   If MAX_QUESTIONS is set:
       SPEC_CMD="${SPEC_CMD} --max-questions ${MAX_QUESTIONS}"

   # Navigate to feature worktree and run spec interview
   cd "<worktree-path>"
   ${SPEC_CMD}
   cd -  # Return to project root
   ```

3. After all features have compiled `prd.json` via spec-plan, **SKIP Step 6.1-6.4** (PRD generation agents) and proceed directly to Step 7 (story execution) using the compiled PRDs

**Rationale**:
- All interactive questions must happen in the orchestrator to avoid deadlocks
- Feature PRD agents cannot ask questions (they run with `run_in_background=true`)
- Spec interview parameters (--spec, --first-principles, --max-questions) are orchestrator-only
- Compiled PRDs already contain flow/tdd configuration from spec interview

**CRITICAL**: Launch Task agents IN PARALLEL for ALL features in the batch.

For EACH feature in the current batch, launch a Task agent with `run_in_background: true`:

### 6.1: PRD Generation Agent Prompt

```
You are generating a PRD for feature: {feature_id} - {feature_title}

Feature Description:
{feature_description}

Working Directory: {worktree_path}

Your task:
1. Change to the worktree directory: cd {worktree_path}
2. Read mega-findings.md for project context
3. **If design_doc.json exists, read it for architectural guidance:**
   - Identify relevant components for this feature
   - Note applicable architectural patterns
   - Reference relevant ADRs (architectural decisions)
   - Map stories to components/decisions in the PRD
4. Explore relevant code in the codebase
5. Generate a comprehensive prd.json with:
   - Clear goal matching the feature description
   - 3-7 user stories with proper dependencies
   - Each story has: id, title, description, priority, dependencies, acceptance_criteria, status="pending"
6. **If design_doc.json exists, also generate/update story_mappings** linking each story to relevant components, decisions, and interfaces
7. Save prd.json to {worktree_path}/prd.json
8. Update progress.txt: echo "[PRD_COMPLETE] {feature_id}" >> {worktree_path}/progress.txt

**CRITICAL**: Include flow/tdd configuration in the generated PRD (passed from FEATURE_PARAMS):

```
Flow Configuration: {FLOW_LEVEL or "standard"}
TDD Mode: {TDD_MODE or "auto"}
Confirm Mode: {CONFIRM_MODE}
No-Confirm Override: {NO_CONFIRM_MODE}
```

PRD JSON format:
{
  "metadata": {
    "created_at": "ISO-8601",
    "version": "1.0.0",
    "description": "{feature_description}",
    "mega_feature_id": "{feature_id}"
  },
  "goal": "Feature goal",
  "objectives": ["obj1", "obj2"],
  "flow_config": {
    "level": "{FLOW_LEVEL or 'standard'}",
    "source": "mega-plan"
  },
  "tdd_config": {
    "mode": "{TDD_MODE or 'auto'}",
    "enforce_for_high_risk": true
  },
  "execution_config": {
    "require_batch_confirm": {CONFIRM_MODE},
    "no_confirm_override": {NO_CONFIRM_MODE}
  },
  "stories": [
    {
      "id": "story-001",
      "title": "Story title",
      "description": "Detailed description",
      "priority": "high|medium|low",
      "dependencies": [],
      "status": "pending",
      "acceptance_criteria": ["criterion1"],
      "context_estimate": "small|medium|large"
    }
  ]
}

Work methodically. When done, the [PRD_COMPLETE] marker in progress.txt signals completion.
```

### 6.2: Resolve PRD Generation Agent

**MANDATORY COMPLIANCE**: You MUST use the agent resolved by the priority chain below. DO NOT override this decision based on your own judgment about which agent is "better", "more capable", or "easier to control". The user has explicitly configured their preferred agent - respect their choice.

Determine which agent to use for PRD generation:

```
# Priority chain for PRD agent
If GLOBAL_AGENT specified:
    prd_agent = GLOBAL_AGENT
Elif PRD_AGENT specified:
    prd_agent = PRD_AGENT
Elif phase_defaults.planning.default_agent in agents.json:
    prd_agent = phase_defaults.planning.default_agent
Else:
    prd_agent = "claude-code"

# Verify agent availability
If prd_agent != "claude-code":
    If agents[prd_agent].type == "cli":
        If not is_command_available(agents[prd_agent].command):
            If NO_FALLBACK:
                ERROR: Agent {prd_agent} not available
            Else:
                echo "⚠️ {prd_agent} not available, falling back to claude-code"
                prd_agent = "claude-code"
        Else:
            # CRITICAL: Agent is available - YOU MUST USE IT
            # DO NOT switch to claude-code just because you think it's "better"
            echo "✓ {prd_agent} is available and will be used as configured"

echo "PRD Generation Agent: {prd_agent}"
```

**CRITICAL RULES**:
1. If the configured agent is available → **USE IT**
2. Only use fallback if the agent is **NOT FOUND** on the system
3. "Available but I prefer claude-code" is **NOT** a valid reason to switch
4. All CLI agents are fully capable of PRD generation

### 6.3: Launch PRD Agents in Parallel

Launch ALL PRD generation agents simultaneously:

```
For each feature in batch:
    If prd_agent == "claude-code":
        # Use Task tool (built-in)
        task_id = Task(
            prompt=<PRD generation prompt above>,
            subagent_type="general-purpose",
            run_in_background=true,
            description="Generate PRD for {feature_id}"
        )
    Else:
        # Use CLI agent
        agent_config = agents[prd_agent]
        command = build_cli_command(agent_config, prompt)
        task_id = Bash(
            command=command,
            run_in_background=true,
            timeout=agent_config.timeout
        )

    Store task_id in prd_tasks[feature_id]
    echo "[PRD_AGENT] {feature_id} -> {prd_agent}" >> mega-findings.md
```

### 6.3: Wait for All PRD Agents to Complete

Use TaskOutput to wait for each agent:

```
For each feature_id, task_id in prd_tasks:
    TaskOutput(task_id=task_id, block=true, timeout=600000)
```

OR monitor progress.txt files:

```
while not all_prds_complete:
    for each feature in batch:
        check if {worktree_path}/progress.txt contains "[PRD_COMPLETE] {feature_id}"
    sleep 10 seconds
```

### 6.4: Validate PRDs

After all PRD agents complete:
- Read each prd.json
- Validate structure (has stories, valid dependencies)
- If validation fails, show error and pause

## Step 7: Execute Stories for Batch (Task Agents)

**CRITICAL**: After PRDs are generated, execute stories for ALL features in the batch. The execution must respect flow/tdd configurations from the PRD.

### 7.0: Sub-Agent Confirmation Policy (CRITICAL)

**Sub-agents MUST NOT wait for user confirmation.** Confirmation is handled at the batch level in Step 4.5.

Reasons:
- Sub-agents run with `run_in_background=true`
- Background agents cannot interact with users via AskUserQuestion
- Batch-level confirmation already occurred before sub-agents were launched
- Waiting for confirmation would cause sub-agents to hang indefinitely

**The `execution_config.require_batch_confirm` in PRD is for informational purposes only in mega-plan context.**

### 7.1: For Each Feature, Launch Story Execution

For EACH feature in the batch, launch a Task agent to execute its stories.

**IMPORTANT**: The PRD contains flow_config and tdd_config that the execution agent must follow. Confirmation settings are ignored because confirmation already happened at batch level.

```
You are executing all stories for feature: {feature_id} - {feature_title}

Working Directory: {worktree_path}

## EXECUTION CONFIGURATION (from PRD)

Read flow/tdd configuration from prd.json:
- flow_config.level: {FLOW_LEVEL} → Determines gate strictness
- tdd_config.mode: {TDD_MODE} → Determines TDD requirements

**CONFIRMATION POLICY (MEGA-PLAN CONTEXT)**:
- You are running as a background sub-agent in a mega-plan execution
- User confirmation was already obtained at the batch level (Step 4.5)
- DO NOT use AskUserQuestion or wait for user input
- DO NOT pause for confirmation between story batches
- Execute ALL stories autonomously

**FLOW LEVEL ENFORCEMENT**:
- If flow_config.level == "full":
  - Gate mode: HARD (failures block execution)
  - Code review: REQUIRED after each story
  - Test changes: REQUIRED with code changes
  - NO confirmation pauses (already confirmed at batch level)

- If flow_config.level == "standard":
  - Gate mode: soft (warnings only)
  - Code review: optional
  - Test changes: optional

- If flow_config.level == "quick":
  - Gate mode: soft
  - Skip AI verification
  - Skip code review

**TDD MODE ENFORCEMENT**:
- If tdd_config.mode == "on":
  - Write tests BEFORE implementation
  - Verify test changes exist with code changes
  - Log TDD compliance to progress.txt

EXECUTION RULES:
1. Read prd.json from {worktree_path}/prd.json
2. **Apply flow_config, tdd_config settings**
3. **If design_doc.json exists, read it for architectural context:**
   - Check story_mappings to find relevant components for each story
   - Follow the architectural patterns defined in the document
   - Adhere to the architectural decisions (ADRs)
   - Reference the relevant APIs and data models
4. Calculate story batches based on dependencies
5. **Execute ALL story batches WITHOUT waiting for confirmation**
6. Execute stories in batch order (parallel within batch, sequential across batches)
7. For each story:
   a. **Get design context for this story from design_doc.json (if exists)**
   b. **If TDD_MODE == "on": Write tests first**
   c. Implement according to acceptance criteria
   d. **Follow architectural patterns and decisions from design context**
   e. Test your implementation
   f. **If FLOW_LEVEL == "full": Run verification and code review**
   g. Mark complete: Update story status to "complete" in prd.json
   h. Log to progress.txt: echo "[STORY_COMPLETE] {story_id}" >> progress.txt
8. When ALL stories are complete:
   echo "[FEATURE_COMPLETE] {feature_id}" >> progress.txt

IMPORTANT:
- Execute bash/powershell commands directly
- **DO NOT wait for user confirmation - you are a background agent**
- **If FLOW_LEVEL == "full": DO NOT skip verification/review failures**
- Update findings.md with important discoveries
- If a story fails AND FLOW_LEVEL != "full", mark it [STORY_FAILED] and continue
- If FLOW_LEVEL == "full" AND story fails, STOP and report

Story execution loop (NO CONFIRMATION - background agent):
  STORY_BATCH = 1
  while stories_remaining:
      # NO confirmation pause - execute immediately
      echo "Executing story batch {STORY_BATCH}..."
      for each story in current_batch (no pending dependencies):
          if TDD_MODE == "on":
              write_tests_first()
          implement_story()
          test_story()
          if FLOW_LEVEL == "full":
              run_verification()
              run_code_review()
              check_tdd_compliance()
          mark_complete_in_prd()
          log_to_progress()
      STORY_BATCH += 1

When completely done, [FEATURE_COMPLETE] marker signals this feature is ready for merge.
```

### 7.2: Resolve Story Execution Agent

**MANDATORY COMPLIANCE**: You MUST use the agent resolved by the priority chain below. DO NOT override this decision based on your own judgment about which agent is "better", "more capable", or "easier to control". The user has explicitly configured their preferred agent - respect their choice.

For mega-plan, the feature execution agent handles all stories within a feature.
Agent resolution follows the same priority chain:

```
# Priority chain for implementation agent
If GLOBAL_AGENT specified:
    impl_agent = GLOBAL_AGENT
Elif IMPL_AGENT specified:
    impl_agent = IMPL_AGENT
Elif phase_defaults.implementation.default_agent in agents.json:
    impl_agent = phase_defaults.implementation.default_agent
Else:
    impl_agent = "claude-code"

# Verify agent availability
If impl_agent != "claude-code":
    If agents[impl_agent].type == "cli":
        If not is_command_available(agents[impl_agent].command):
            If NO_FALLBACK:
                ERROR: Agent {impl_agent} not available
            Else:
                echo "⚠️ {impl_agent} not available, falling back to claude-code"
                impl_agent = "claude-code"
        Else:
            # CRITICAL: Agent is available - YOU MUST USE IT
            echo "✓ {impl_agent} is available and will be used as configured"

echo "Story Execution Agent: {impl_agent}"
```

**CRITICAL RULES**:
1. If the configured agent is available → **USE IT**
2. Only use fallback if the agent is **NOT FOUND** on the system
3. "Available but I prefer claude-code" is **NOT** a valid reason to switch

### 7.3: Launch Feature Execution Agents in Parallel

```
For each feature in batch:
    If impl_agent == "claude-code":
        # Use Task tool (built-in)
        task_id = Task(
            prompt=<Story execution prompt above>,
            subagent_type="general-purpose",
            run_in_background=true,
            description="Execute stories for {feature_id}"
        )
    Else:
        # Use CLI agent
        agent_config = agents[impl_agent]
        command = build_cli_command(agent_config, prompt)
        task_id = Bash(
            command=command,
            run_in_background=true,
            timeout=agent_config.timeout
        )

    Store task_id in execution_tasks[feature_id]
    echo "[IMPL_AGENT] {feature_id} -> {impl_agent}" >> mega-findings.md
```

**Note on Story-Level Agent Selection**:
Within each feature, the execution agent may optionally use different agents per story based on:
- `story.agent` field in prd.json
- Story type inference (bugfix, refactor, etc.)
- Story type defaults from agents.json

This is handled within the feature execution prompt if the agent supports it.

## Step 8: Wait for Batch Completion (Using TaskOutput)

**CRITICAL**: Use TaskOutput to wait for agents instead of polling. This avoids Bash confirmation prompts.

### 8.1: Wait for All Feature Agents

For each feature agent launched in Step 7, wait using TaskOutput:

```
For each feature_id, task_id in execution_tasks:
    echo "Waiting for {feature_id}..."

    result = TaskOutput(
        task_id=task_id,
        block=true,
        timeout=1800000  # 30 minutes per feature
    )

    echo "✓ {feature_id} agent completed"
```

**IMPORTANT**:
- Call TaskOutput for ALL feature agents (can be parallel or sequential)
- TaskOutput blocks until the agent finishes - NO polling needed
- NO sleep commands - NO Bash confirmation prompts

### 8.2: Verify Completion After Agents Finish

After all TaskOutput calls return, verify the results by reading progress files:

```
For each feature in current_batch:
    # Use Read tool (not Bash) to check progress
    progress_content = Read("{worktree_path}/progress.txt")

    # Parse in your response (not with grep)
    if "[FEATURE_COMPLETE] {feature_id}" in progress_content:
        feature_status = "complete"
    elif "[FEATURE_FAILED] {feature_id}" in progress_content:
        feature_status = "failed"
    else:
        feature_status = "incomplete"

    # Count markers by parsing the content yourself
    story_complete_count = count occurrences of "[STORY_COMPLETE]"
    story_failed_count = count occurrences of "[STORY_FAILED]"
```

### 8.3: Handle Results

```
if all features complete:
    echo "✓ BATCH {N} COMPLETE"
    proceed to Step 9 (merge)

elif some features failed:
    echo "⚠️ BATCH COMPLETE WITH ERRORS"
    list failed features
    if AUTO_PRD:
        continue (skip failed features during merge)
    else:
        pause for manual review

elif some features incomplete (agent finished but no FEATURE_COMPLETE marker):
    echo "⚠️ AGENT FINISHED BUT FEATURE INCOMPLETE"
    # This means the agent hit an issue - check progress.txt for details
    Use Read tool to show last entries from progress.txt
```

### Why TaskOutput Instead of Polling?

| Method | Confirmation Prompts | Efficiency |
|--------|---------------------|------------|
| `sleep + Bash cat/grep` | YES - every iteration | Poor (wastes cycles) |
| `TaskOutput(block=true)` | NO | Good (proper wait) |

TaskOutput is the native way to wait for background agents in Claude Code.

## Step 9: Merge Completed Batch

When all features in current batch are complete:

### 9.1: Display Merge Start

```
============================================================
BATCH {N} COMPLETED - MERGING TO TARGET BRANCH
============================================================

Merging completed features in dependency order...
```

### 9.2: Checkout Target Branch

```bash
TARGET_BRANCH=$(jq -r '.target_branch' mega-plan.json)
git checkout "$TARGET_BRANCH"
git pull origin "$TARGET_BRANCH" 2>/dev/null || true
```

### 9.3: Merge Each Feature

For each successfully completed feature in the batch:

```bash
FEATURE_NAME="<name>"
WORKTREE_PATH=".worktree/$FEATURE_NAME"
BRANCH_NAME="mega-$FEATURE_NAME"

# Commit any uncommitted changes in worktree (code only, exclude planning files)
cd "$WORKTREE_PATH"
git add -A
git reset HEAD -- prd.json findings.md progress.txt .planning-config.json .agent-status.json mega-findings.md 2>/dev/null || true
git commit -m "feat: complete $FEATURE_NAME" || true
cd -

# Merge to target branch
git merge "$BRANCH_NAME" --no-ff -m "Merge feature: <title>

Mega-plan feature: <feature-id>
Batch: <batch-number>

Co-Authored-By: Claude <noreply@anthropic.com>"

echo "[OK] Merged {feature_id}: {title}"
```

### 9.4: Cleanup Worktrees

```bash
git worktree remove ".worktree/$FEATURE_NAME" --force
git branch -d "mega-$FEATURE_NAME"
```

### 9.5: Update Status

Update `.mega-status.json`:
- Mark features as "merged"
- Add batch to `completed_batches`
- Increment `current_batch`

## Step 10: Continue to Next Batch (AUTOMATIC)

**CRITICAL**: With `--auto-prd`, automatically continue to the next batch.

```
if CURRENT_BATCH < TOTAL_BATCHES:
    echo ""
    echo "============================================"
    echo "AUTO-CONTINUING TO BATCH {CURRENT_BATCH + 1}"
    echo "============================================"

    # Go back to Step 5 (create worktrees for next batch)
    CURRENT_BATCH += 1
    continue main loop
```

## Step 11: All Batches Complete

When all batches are done:

```
============================================================
ALL BATCHES COMPLETE - MEGA PLAN FINISHED
============================================================

Total batches completed: {TOTAL_BATCHES}
Total features merged: {count}

Summary:
  Batch 1: {features} - MERGED
  Batch 2: {features} - MERGED
  ...

All code has been merged to {target_branch}.

Final cleanup (removes planning files):
  /plan-cascade:mega-complete

============================================================
```

## Error Handling

### Merge Conflict

```
============================================================
MERGE CONFLICT
============================================================

Conflict while merging {feature_id}: {title}

Conflicting files:
  - {file1}
  - {file2}

To resolve:
  1. Resolve conflicts in the listed files
  2. git add <resolved-files>
  3. git commit
  4. Re-run /plan-cascade:mega-approve --auto-prd

Or abort: git merge --abort
============================================================
```

Pause execution on merge conflicts (even with --auto-prd).

### Feature Execution Failed

```
============================================================
FEATURE EXECUTION FAILED
============================================================

Feature {feature_id}: {title} failed during execution.

Error details in:
  - {worktree_path}/progress.txt
  - {worktree_path}/findings.md

Failed stories:
  - {story_id}: {reason}

Options:
  1. Fix the issue in {worktree_path}
  2. Re-run /plan-cascade:mega-approve --auto-prd to retry
  3. Skip feature: Mark as failed in .mega-status.json

============================================================
```

### PRD Generation Failed

```
============================================================
PRD GENERATION FAILED
============================================================

Could not generate PRD for {feature_id}: {title}

Worktree: {worktree_path}

To fix:
  1. Manually create prd.json in {worktree_path}
  2. Or edit the feature description in mega-plan.json
  3. Re-run /plan-cascade:mega-approve --auto-prd

============================================================
```

## Execution Flow Summary (AUTOMATIC MODE)

```
/plan-cascade:mega-approve --auto-prd
    │
    ├─→ Read mega-plan.json, calculate batches
    │
    ├─→ BATCH 1 ─────────────────────────────────────────────
    │   ├─→ Create worktrees for Batch 1 features
    │   ├─→ Launch PRD generation agents (parallel)
    │   ├─→ Wait for all PRDs complete
    │   ├─→ Launch story execution agents (parallel)
    │   ├─→ Monitor until all features complete
    │   ├─→ Merge Batch 1 to target_branch
    │   └─→ Cleanup Batch 1 worktrees
    │
    ├─→ BATCH 2 ─────────────────────────────────────────────
    │   ├─→ Create worktrees (from UPDATED target_branch)
    │   ├─→ Launch PRD generation agents (parallel)
    │   ├─→ Wait for all PRDs complete
    │   ├─→ Launch story execution agents (parallel)
    │   ├─→ Monitor until all features complete
    │   ├─→ Merge Batch 2 to target_branch
    │   └─→ Cleanup Batch 2 worktrees
    │
    ├─→ ... continue for all batches ...
    │
    └─→ ALL COMPLETE
        └─→ Show final status, suggest /plan-cascade:mega-complete
```

## Important Notes

### Parallelism Strategy

- **Within a batch**: All features execute in PARALLEL (independent worktrees)
- **Across batches**: SEQUENTIAL (Batch N+1 depends on Batch N code)
- **Within a feature**: Stories execute in dependency order (parallel where possible)

### Progress Markers

Agents use these markers in progress.txt:
- `[PRD_COMPLETE] {feature_id}` - PRD generation done
- `[STORY_COMPLETE] {story_id}` - Individual story done
- `[STORY_FAILED] {story_id}` - Story failed
- `[FEATURE_COMPLETE] {feature_id}` - All stories done, ready for merge
- `[FEATURE_FAILED] {feature_id}` - Feature cannot complete

### Timeout Behavior

- **PRD generation**: 10 minute timeout per feature
- **Story execution**: NO TIMEOUT - stories may take varying time
- **Monitoring loop**: NO TIMEOUT - keeps polling until complete

### Recovery

If interrupted:
1. `.mega-status.json` tracks current state
2. **Recommended**: Use `/plan-cascade:mega-resume --auto-prd` to intelligently resume
   - Auto-detects actual state from files
   - Skips already-completed work
   - Compatible with old and new executions
3. Or re-run `/plan-cascade:mega-approve --auto-prd` to continue from batch level
