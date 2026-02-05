---
description: "AI auto strategy executor. Analyzes task and automatically selects and executes the best strategy: direct execution, hybrid-auto PRD generation, hybrid-worktree isolated development, or mega-plan multi-feature orchestration."
---

# Plan Cascade - Auto Strategy Executor

AI automatically analyzes the task and executes the optimal strategy without user confirmation.

## Command-Line Flags

The auto command supports the following flags to customize execution:

### `--flow <quick|standard|full>`

Override the execution flow depth (**default: `full`**).

| Flow | Description | Gate Mode | AI Verification | Confirm Required |
|------|-------------|-----------|-----------------|------------------|
| `quick` | Fastest path, minimal gating | soft | disabled | no |
| `standard` | Balanced speed and quality | soft | enabled | no |
| `full` | Strict methodology + strict gating (default) | hard | enabled + review | yes |

**Usage:**
```
/plan-cascade:auto "Implement critical security feature"          # defaults to FULL flow (TDD on + confirm)
/plan-cascade:auto --flow full "Implement critical security feature"
/plan-cascade:auto --flow quick "Fix typo in documentation"
```

### `--explain`

Display detailed analysis results and decision rationale without executing.

This flag shows:
- **Key Factors**: scope, complexity, risk, parallelism assessment
- **Strategy Decision**: selected strategy, flow, confidence score
- **Confirmation Points**: questions for user review (if any)
- **TDD Recommendation**: test-driven development guidance
- **Estimates**: stories, features, duration, worktree usage

**Usage:**
```
/plan-cascade:auto --explain "Build user authentication system"
```

The output is human-readable by default. For machine-readable JSON output, combine with `--json`:
```
/plan-cascade:auto --explain --json "Build user authentication system"
```

### `--confirm`

Display confirmation points before execution and wait for user acknowledgment.

**Default**: enabled in `--flow full` (the default flow for `/plan-cascade:auto`) unless `--no-confirm` is set.

When this flag is set:
1. Analysis is performed and displayed
2. Confirmation points are shown (if any exist)
3. User must acknowledge before execution proceeds

Confirmation points are generated when:
- Low confidence (< 0.7): "Do you want to proceed with this strategy?"
- High risk: "Have you considered rollback procedures?"
- Architecture decisions needed: "Would you like to create a design document first?"

**Usage:**
```
/plan-cascade:auto --confirm "Major refactoring of payment module"
```

### `--no-confirm`

Explicitly disable batch confirmation during execution, even if FULL flow would normally require it.

This is useful for:
- CI/CD pipelines where interactive confirmation is not possible
- Automated testing environments
- When you want strict quality gates but uninterrupted execution

**Note**: `--no-confirm` only affects batch-level confirmation during story execution. It does NOT disable quality gates (verification, code review, TDD compliance) - those still run and can block on failures in FULL flow.

**Usage:**
```
# Strict quality gates but no batch confirmation prompts
/plan-cascade:auto --flow full --no-confirm "Implement critical feature"

# CI-friendly: strict gates, no interruptions
/plan-cascade:auto --flow full --tdd on --no-confirm "Security audit fixes"
```

**Precedence**: `--no-confirm` overrides `--confirm` and FULL flow's default confirmation requirement.

### `--tdd <off|on|auto>`

Control Test-Driven Development (TDD) mode for story execution.

| Mode | Description | When to Use |
|------|-------------|-------------|
| `off` | TDD disabled | Simple changes, documentation, non-code tasks |
| `on` | TDD enabled with prompts and compliance checks | Critical features, security-related code |
| `auto` | Automatically decide based on risk assessment | Mixed-risk tasks, faster iteration |

**Default**: `/plan-cascade:auto` uses `--tdd on` in FULL flow unless explicitly overridden via `--tdd`.

When TDD mode is enabled (on or auto-enabled):
1. **Red Phase**: AI writes failing tests first based on acceptance criteria
2. **Green Phase**: Minimal implementation to make tests pass
3. **Refactor Phase**: Improve code while keeping tests green

TDD compliance is checked via quality gates after story completion:
- Verifies test files were modified alongside code changes
- High-risk stories (security, auth, database) enforce test requirements
- Warnings generated for code changes without corresponding tests

**Usage:**
```
/plan-cascade:auto --tdd on "Implement payment processing module"
/plan-cascade:auto --tdd off "Update README documentation"
/plan-cascade:auto --tdd auto "Add user profile feature"
```

### Combining Flags

Flags can be combined for customized behavior:

```
# Analyze with full flow, show results, and wait for confirmation
/plan-cascade:auto --flow full --explain --confirm "Critical database migration"

# Quick analysis without execution
/plan-cascade:auto --flow quick --explain "Simple config update"

# Override defaults
/plan-cascade:auto --flow full --tdd off --no-confirm "Update README wording"
/plan-cascade:auto --flow standard --tdd auto "Add user profile feature"
```

## Path Storage Modes

Plan Cascade supports two path storage modes:

### New Mode (Default)
Runtime files are stored in a user directory:
- **Windows**: `%APPDATA%/plan-cascade/<project-id>/`
- **Unix/macOS**: `~/.plan-cascade/<project-id>/`

This keeps the project root clean and avoids polluting the codebase with planning files.

### Legacy Mode
Files are stored in project root for backward compatibility.

The auto command uses PathResolver to detect existing files in either location.

## Tool Usage Policy (CRITICAL)

**To avoid command confirmation prompts during automatic execution:**

1. **Use Read tool for file reading** - NEVER use `cat` via Bash
   - ✅ `Read("prd.json")`, `Read("mega-plan.json")`
   - ❌ `Bash("cat prd.json")`

2. **Use Glob tool for file finding** - NEVER use `ls` or `find` via Bash
   - ✅ `Glob("*.json")`, `Glob(".worktree/*")`
   - ❌ `Bash("ls *.json")`

3. **Only use Bash for actual system commands:**
   - Git operations: `git rev-parse`, `git branch`, `git show-ref`
   - File existence checks (when necessary)

4. **For strategy routing:** Use the Skill tool to invoke other commands

## Prerequisites Check

**CRITICAL**: If this is your first time using Plan Cascade, run `/plan-cascade:init` first to set up the environment.

```bash
# Quick check - if this fails, run /plan-cascade:init
uv run python -c "print('Environment OK')" 2>/dev/null || echo "Warning: Run /plan-cascade:init to set up environment"
```

## Step 0: Ensure .gitignore Configuration

**IMPORTANT**: Before any planning operations, ensure the project's `.gitignore` is configured:

```bash
uv run python -c "from plan_cascade.utils.gitignore import ensure_gitignore; from pathlib import Path; ensure_gitignore(Path.cwd())" 2>/dev/null || true
```

## Step 1: Parse Arguments and Flags

Parse user arguments to extract task description and optional flags:

```
ARGS="{{args}}"

# Parse flags from arguments
FLOW_OVERRIDE = null     # --flow <quick|standard|full>
EXPLAIN_MODE = false     # --explain
CONFIRM_MODE = false     # --confirm (default: enabled in FULL flow unless --no-confirm)
NO_CONFIRM_MODE = false  # --no-confirm (overrides --confirm and FULL flow default)
JSON_OUTPUT = false      # --json
TDD_OVERRIDE = null      # --tdd <off|on|auto> (default: on in FULL flow)
SPEC_MODE = null         # --spec <off|auto|on>
FIRST_PRINCIPLES = false # --first-principles
MAX_QUESTIONS = null     # --max-questions N

# Extract flags using pattern matching:
# --flow quick|standard|full -> FLOW_OVERRIDE = "quick" | "standard" | "full"
# --explain -> EXPLAIN_MODE = true
# --confirm -> CONFIRM_MODE = true
# --no-confirm -> NO_CONFIRM_MODE = true
# --json -> JSON_OUTPUT = true
# --tdd off|on|auto -> TDD_OVERRIDE = "off" | "on" | "auto"
# --spec off|auto|on -> SPEC_MODE = "off" | "auto" | "on"
# --first-principles -> FIRST_PRINCIPLES = true
# --max-questions N -> MAX_QUESTIONS = <int>

# Remove flags from ARGS to get TASK_DESC
TASK_DESC = (ARGS with flags removed)
```

Parse the arguments:
1. Check for `--flow` followed by `quick`, `standard`, or `full`
2. Check for `--explain` flag
3. Check for `--confirm` flag
4. Check for `--no-confirm` flag
5. Check for `--json` flag (only meaningful with --explain)
6. Check for `--tdd` followed by `off`, `on`, or `auto`
7. Check for `--spec` followed by `off`, `auto`, or `on`
8. Check for `--first-principles` flag
9. Check for `--max-questions` followed by an integer
10. Remaining text is the task description

**Note**: If both `--confirm` and `--no-confirm` are specified, `--no-confirm` takes precedence.

If no description provided, ask the user:
```
Please provide a task description. What do you want to accomplish?
Example: "Fix the login button styling" or "Build a user authentication system"
```

## Step 2: Gather Project Context

Collect project context for strategy analysis.

**IMPORTANT: Use the correct tools to avoid command confirmation prompts:**

### 2.1: Check Git Repository (Bash - unavoidable)

```bash
# Only git commands are acceptable for Bash here
git rev-parse --git-dir 2>/dev/null && git branch --show-current && git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@refs/remotes/origin/@@'
```

If git command fails, assume not in a git repository. If default branch detection fails, try:
```bash
git show-ref --verify --quiet refs/heads/main && echo "main" || git show-ref --verify --quiet refs/heads/master && echo "master" || echo "main"
```

### 2.2: Check Planning Files (Use Glob - NO Bash)

Use the **Glob** tool to check for existing planning files in both new mode and legacy locations:

```
# Get paths from PathResolver
PRD_PATH = uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_prd_path())"
MEGA_PLAN_PATH = uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_mega_plan_path())"
WORKTREE_BASE = uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_worktree_dir())"

# Check new mode paths
Glob(PRD_PATH)             -> HAS_PRD = (result count > 0)
Glob(MEGA_PLAN_PATH)       -> HAS_MEGA_PLAN = (result count > 0)
Glob(WORKTREE_BASE + "/*") -> HAS_WORKTREES = (result count > 0)

# Also check legacy paths if different
Glob("prd.json")           -> HAS_PRD |= (result count > 0)
Glob("mega-plan.json")     -> HAS_MEGA_PLAN |= (result count > 0)
Glob(".worktree/*")        -> HAS_WORKTREES |= (result count > 0)
```

### 2.3: Detect Project Type (Use Glob - NO Bash)

Use the **Glob** tool to detect project type by checking for marker files:

```
Glob("package.json")       -> PROJECT_TYPE = "nodejs"
Glob("pyproject.toml")     -> PROJECT_TYPE = "python"
Glob("requirements.txt")   -> PROJECT_TYPE = "python" (if no pyproject.toml)
Glob("Cargo.toml")         -> PROJECT_TYPE = "rust"
Glob("go.mod")             -> PROJECT_TYPE = "go"
```

Execute these Glob calls in parallel (single message with multiple tool calls) for efficiency.

**Example correct usage:**
```
// Call these 5 Glob patterns in parallel in a single message
Glob("prd.json")
Glob("mega-plan.json")
Glob("package.json")
Glob("Cargo.toml")
Glob("go.mod")
```

**DO NOT use Bash for file existence checks like:**
- ❌ `[ -f "prd.json" ]`
- ❌ `ls *.json`
- ❌ `test -f package.json`

## Step 3: AI Self-Assessment of Task Complexity

Instead of keyword matching, perform a structured analysis of the task to determine the appropriate strategy.

### 3.1: Task Decomposition Analysis

Analyze the task description across these dimensions:

1. **Scope Assessment**: How many functional areas does this task touch?
   - Single file or function
   - Single module/component
   - Multiple modules with clear boundaries
   - Cross-cutting concerns affecting the whole system

2. **Complexity Indicators**: What level of planning is needed?
   - Simple change (1-2 steps, obvious solution)
   - Moderate (3-5 steps, some decisions needed)
   - Complex (6+ steps, dependencies between subtasks)
   - Architectural (requires design decisions, patterns, interfaces)

3. **Risk Assessment**: What's the potential impact?
   - Low: Isolated change, easy to revert
   - Medium: Touches shared code, needs testing
   - High: Breaking changes, experimental, or affects critical paths

4. **Parallelization Benefit**: Can work be split effectively?
   - None: Sequential steps only
   - Some: 2-3 independent subtasks
   - Significant: 4+ independent features/stories

### 3.2: Output Structured Analysis

Produce a JSON analysis of your assessment:

```json
{
  "task_analysis": {
    "functional_areas": ["<area1>", "<area2>"],
    "estimated_stories": <number>,
    "has_dependencies": true|false,
    "requires_architecture_decisions": true|false,
    "risk_level": "low|medium|high",
    "parallelization_benefit": "none|some|significant"
  },
  "strategy_decision": {
    "strategy": "DIRECT|HYBRID_AUTO|HYBRID_WORKTREE|MEGA_PLAN",
    "flow": "quick|standard|full",
    "confidence": 0.0-1.0,
    "reasoning": "<explanation of why this strategy was chosen>",
    "confirm_points": ["<point1>", "<point2>"],
    "tdd_recommendation": "off|on|auto"
  }
}
```

### 3.2.1: Flow Selection Logic

**Default behavior:** Use **FULL** flow (strict gating) unless the user explicitly overrides via `--flow`.

| Condition | Flow | Rationale |
|-----------|------|-----------|
| `FLOW_OVERRIDE` is set (via `--flow`) | `FLOW_OVERRIDE` | User intent overrides defaults |
| Otherwise | **FULL** | Ensure strict methodology + maximum stability by default |

### 3.2.1.1: Default Safety Settings (Auto)

When running in FULL flow (default), enable the strictest safe defaults unless the user explicitly overrides:

```
# --no-confirm takes precedence over everything
If NO_CONFIRM_MODE is true:
    CONFIRM_MODE = false

# Default confirmations in FULL flow (needed for mega-plan batch confirmation too)
Elif FLOW == "full" AND CONFIRM_MODE is false:
    CONFIRM_MODE = true

# Default TDD in FULL flow
If FLOW == "full" AND TDD_OVERRIDE is null:
    TDD_OVERRIDE = "on"
```

### 3.2.2: Confirmation Points Generation

Generate 1-3 confirmation points when warranted:

1. **Low confidence (< 0.7)**: "The analysis confidence is X%. Do you want to proceed with [strategy], or would you prefer a different approach?"
2. **High risk**: "This task is identified as high-risk. [Worktree/rollback consideration]?"
3. **Architecture decisions needed**: "Architecture decisions are required. [Design document consideration]?"

### 3.3: Strategy Selection Guide

Use this decision matrix based on your analysis:

| Analysis Result | Strategy | When to Use |
|-----------------|----------|-------------|
| 1 area, 1-2 steps, low risk, no parallelization **AND** `FLOW != full` | **DIRECT** | Quick fixes, config changes, simple updates |
| 1-3 areas, 1-7 steps **OR** `FLOW == full` (default) | **HYBRID_AUTO** | Full pipeline with PRD + strict gates |
| HYBRID_AUTO conditions + high risk OR experimental | **HYBRID_WORKTREE** | Risky changes needing isolation |
| 4+ areas, 8+ steps, significant parallelization | **MEGA_PLAN** | Multiple independent features, platform work |

### 3.4: Strategy Decision Rules

**DIRECT** - Choose when ALL of these are true:
- Single functional area
- 1-2 implementation steps
- Low to medium risk
- No subtask dependencies
- No architecture decisions needed
 - **AND flow is NOT `full`** (DIRECT is only allowed in `quick`/`standard`)

**HYBRID_AUTO** - Choose when ANY of these are true:
- 2-3 functional areas involved
- 3-7 estimated implementation steps
- Dependencies between subtasks exist
- Moderate complexity requiring story breakdown
- BUT: Risk is low to medium and main branch is acceptable

**HYBRID_WORKTREE** - Choose when HYBRID_AUTO applies AND:
- High risk level (breaking changes, experimental)
- Need to preserve main branch integrity
- Parallel development with other features
- Major refactoring or restructuring
- Prototype or proof-of-concept work

**MEGA_PLAN** - Choose when ANY of these are true:
- 4+ distinct functional areas
- 8+ estimated stories
- Multiple independent features can be developed in parallel
- Project-level scope (platform, system, infrastructure)
- Significant parallelization benefit identified

## Step 4: Display Analysis Result

Display your structured analysis and selected strategy:

```
============================================================
AUTO STRATEGY ANALYSIS
============================================================
Task: {TASK_DESC}

Key Factors:
  Scope:        {scope}
  Complexity:   {complexity}
  Risk:         {risk_level}
  Parallelism:  {parallelization_benefit}

Strategy Decision:
  Strategy:     {STRATEGY}
  Flow:         {FLOW}
  Confidence:   {confidence}
  Reasoning:    {reasoning}

Confirmation Points:
  {confirm_points or "None"}

TDD Recommendation:
  {tdd_recommendation_description}

Estimates:
  Stories:      {estimated_stories}
  Duration:     ~{duration} hours
  Worktree:     {use_worktree ? "Yes" : "No"}
============================================================
```

### Step 4.1: Handle --explain Mode

If `EXPLAIN_MODE = true`:

1. Display the analysis output (as shown above)
2. If `JSON_OUTPUT = true`, also output the JSON format for machine parsing
3. **DO NOT EXECUTE** - exit after displaying analysis

```
Analysis complete. Use without --explain to execute.
```

### Step 4.2: Handle --confirm Mode

If `CONFIRM_MODE = true` AND there are confirmation points:

1. Display the analysis output
2. Display confirmation points prominently:
   ```
   ============================================================
   CONFIRMATION REQUIRED
   ============================================================
   Please review the following points before proceeding:

   1. {confirm_point_1}
   2. {confirm_point_2}
   3. {confirm_point_3}

   Reply with "proceed" or "yes" to continue, or provide feedback.
   ============================================================
   ```
3. **WAIT for user response** before proceeding

If user confirms, continue to Step 5. If user provides feedback, re-analyze with their input.

### Step 4.3: Normal Execution

If neither `EXPLAIN_MODE` nor `CONFIRM_MODE` (or no confirm points):

```
Executing...
```

Proceed to Step 5.

## Step 5: Route to Appropriate Strategy (MANDATORY SKILL TOOL USAGE)

**CRITICAL: For any strategy other than DIRECT, you MUST use the Skill tool to invoke the corresponding command. DO NOT attempt to execute the strategy logic yourself - let the specialized skill handle it.**

**IMPORTANT: Pass flow/tdd/confirm parameters to sub-commands to ensure strict execution mode is enforced.**

### Build Parameter String for Sub-Commands

Before routing, build the parameter string to pass to sub-commands:

```
# Build parameter string
PARAM_STRING = ""

# Add --flow parameter (use FLOW from analysis, which may be overridden by FLOW_OVERRIDE)
If FLOW is set:
    PARAM_STRING = PARAM_STRING + " --flow " + FLOW

# Add --tdd parameter
If TDD_OVERRIDE is set:
    PARAM_STRING = PARAM_STRING + " --tdd " + TDD_OVERRIDE
Elif tdd_recommendation from analysis is "on" or starts with "on ":
    PARAM_STRING = PARAM_STRING + " --tdd on"

# Add spec interview parameters
If SPEC_MODE is set:
    PARAM_STRING = PARAM_STRING + " --spec " + SPEC_MODE
Elif FLOW is "full":
    # Default behavior: FULL flow shifts-left via spec interview
    PARAM_STRING = PARAM_STRING + " --spec auto"

If FIRST_PRINCIPLES is true:
    PARAM_STRING = PARAM_STRING + " --first-principles"

If MAX_QUESTIONS is set:
    PARAM_STRING = PARAM_STRING + " --max-questions " + MAX_QUESTIONS

# Add --confirm or --no-confirm parameter
# --no-confirm takes precedence over --confirm
If NO_CONFIRM_MODE is true:
    PARAM_STRING = PARAM_STRING + " --no-confirm"
Elif CONFIRM_MODE is true:
    PARAM_STRING = PARAM_STRING + " --confirm"

# Trim leading space
PARAM_STRING = trim(PARAM_STRING)
```

### FULL Flow Default Guardrails

**CRITICAL**: FULL flow is the default and is intended to run the **complete methodology** (planning artifacts + strict gates).

If the analysis selected `DIRECT` while `FLOW == "full"`, upgrade to `HYBRID_AUTO`:
```
If STRATEGY == "DIRECT" AND FLOW == "full":
    echo "Note: FULL flow default requires full pipeline. Upgrading DIRECT -> HYBRID_AUTO."
    STRATEGY = "HYBRID_AUTO"
```

Execute the selected strategy:

### If STRATEGY is "DIRECT":

Execute the task directly without creating planning files:

```
Direct execution mode selected.

The task is simple enough to execute without formal planning.
Proceeding to implement directly...
```

Then proceed to analyze the codebase and implement the requested changes directly.
(This is the ONLY strategy where you execute the task yourself.)

### If STRATEGY is "HYBRID_AUTO":

**MANDATORY: Use the Skill tool to invoke the hybrid-auto command with flow/tdd/confirm parameters.**

Display:
```
Routing to /plan-cascade:hybrid-auto...
Flow: {FLOW}
TDD: {TDD_OVERRIDE or tdd_recommendation}
Confirm: {CONFIRM_MODE}
```

Then you MUST call the Skill tool with parameters:
```
Skill(skill="plan-cascade:hybrid-auto", args="{PARAM_STRING} {TASK_DESC}")
```

Example with full flow:
```
Skill(skill="plan-cascade:hybrid-auto", args="--flow full --tdd on --confirm Implement user authentication")
```

**DO NOT proceed to read files or implement the task yourself. The hybrid-auto skill will generate the PRD and handle execution.**

### If STRATEGY is "HYBRID_WORKTREE":

Generate a task name from the description:

```bash
# Generate task name from description (first 3 significant words, lowercase, hyphenated)
TASK_NAME=$(echo "$TASK_DESC" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9 ]//g' | awk '{for(i=1;i<=3&&i<=NF;i++)printf "%s-",$i}' | sed 's/-$//')
if [ -z "$TASK_NAME" ]; then
    TASK_NAME="task-$(date +%Y%m%d-%H%M)"
fi
```

Display:
```
Routing to /plan-cascade:hybrid-worktree...
Task name: {TASK_NAME}
Target branch: {DEFAULT_BRANCH}
Flow: {FLOW}
TDD: {TDD_OVERRIDE or tdd_recommendation}
Confirm: {CONFIRM_MODE}
```

**MANDATORY: Use the Skill tool to invoke the hybrid-worktree command with parameters.**

```
Skill(skill="plan-cascade:hybrid-worktree", args="{PARAM_STRING} {TASK_NAME} {DEFAULT_BRANCH} {TASK_DESC}")
```

Example with full flow:
```
Skill(skill="plan-cascade:hybrid-worktree", args="--flow full --tdd on fix-auth main Fix authentication bug")
```

**DO NOT proceed to create worktrees or implement the task yourself. The hybrid-worktree skill will handle it.**

### If STRATEGY is "MEGA_PLAN":

Display:
```
Routing to /plan-cascade:mega-plan...
Flow: {FLOW}
TDD: {TDD_OVERRIDE or tdd_recommendation}
Confirm: {CONFIRM_MODE}
```

**MANDATORY: Use the Skill tool to invoke the mega-plan command with parameters.**

```
Skill(skill="plan-cascade:mega-plan", args="{PARAM_STRING} {TASK_DESC}")
```

Example with full flow:
```
Skill(skill="plan-cascade:mega-plan", args="--flow full --tdd on --confirm Build e-commerce platform")
```

**DO NOT proceed to create mega-plan.json or implement the task yourself. The mega-plan skill will handle it.**

---

**IMPORTANT REMINDER**: After determining the strategy:
- For DIRECT: Execute the task yourself
- For HYBRID_AUTO, HYBRID_WORKTREE, MEGA_PLAN: You MUST use the Skill tool with parameters. This ensures:
  1. The correct PRD/plan files are generated
  2. The proper review/approval workflow is followed
  3. Parallel execution is handled correctly
  4. **Flow/TDD/Confirm parameters are propagated for strict execution mode**

**CRITICAL**: Always pass `{PARAM_STRING}` to sub-commands. The `--flow full --tdd on --confirm` parameters must be propagated to ensure:
- Hard quality gates (blocking instead of warnings)
- Mandatory code review
- Mandatory test changes
- TDD compliance checking
- Batch confirmation prompts

**If you find yourself reading code files or implementing after selecting HYBRID_AUTO/WORKTREE/MEGA_PLAN, STOP and use the Skill tool instead.**

## Strategy Summary Table

| Strategy | Trigger | Execution | Default Flow |
|----------|---------|-----------|--------------|
| **DIRECT** | Simple fixes when explicitly using `--flow quick` or `--flow standard` | Execute immediately | QUICK / STANDARD (explicit) |
| **HYBRID_AUTO** | Default for most tasks (including simple tasks under FULL flow default) | Generate PRD, parallel stories | FULL (default) |
| **HYBRID_WORKTREE** | Feature keywords + isolation keywords | Isolated worktree + PRD | STANDARD/FULL |
| **MEGA_PLAN** | Platform/system keywords or 3+ modules | Multi-feature orchestration | STANDARD/FULL |

## Flow Summary Table

| Flow | Gate Mode | AI Verification | Code Review | Test Enforcement | Use When |
|------|-----------|-----------------|-------------|------------------|----------|
| **QUICK** | soft | disabled | no | no | Low-risk, high-confidence, small tasks |
| **STANDARD** | soft | enabled | no | no | Faster iteration when explicitly requested |
| **FULL** | hard | enabled | required | required | Default: strict methodology, maximum safety/stability |

## Examples

### Example 1: Direct Execution
```
/plan-cascade:auto --flow quick "Fix the typo in the README"
```
**Analysis**: scope=single_file, risk=low, confidence=0.9
**Decision**: Strategy=DIRECT, Flow=QUICK
**Action**: Execute fix directly

### Example 2: Hybrid Auto
```
/plan-cascade:auto "Implement user authentication with login and registration"
```
**Analysis**: scope=multiple_modules, risk=medium, confidence=0.8
**Decision**: Strategy=HYBRID_AUTO, Flow=FULL (default)
**Action**: Route to `/plan-cascade:hybrid-auto`

### Example 3: Hybrid Worktree (High Risk)
```
/plan-cascade:auto "Experimental refactoring of the payment module"
```
**Analysis**: scope=single_module, risk=high, confidence=0.7
**Decision**: Strategy=HYBRID_WORKTREE, Flow=FULL
**Confirm Points**: "This task is high-risk. Have you considered rollback procedures?"
**Action**: Route to `/plan-cascade:hybrid-worktree`

### Example 4: Mega Plan
```
/plan-cascade:auto "Build an e-commerce platform with users, products, cart, and orders"
```
**Analysis**: scope=cross_cutting, risk=medium, confidence=0.85
**Decision**: Strategy=MEGA_PLAN, Flow=FULL (default)
**Action**: Route to `/plan-cascade:mega-plan`

### Example 5: Using --explain Flag
```
/plan-cascade:auto --explain "Migrate database schema to new format"
```
**Output**: Full analysis displayed (key factors, strategy, flow, confirm points)
**Action**: No execution - analysis only

### Example 6: Using --flow Override
```
/plan-cascade:auto --flow standard "Add new API endpoint for user preferences"
```
**Analysis**: Would normally run FULL flow by default
**Decision**: Strategy=HYBRID_AUTO, Flow=STANDARD (overridden)
**Action**: Execute with faster, softer gating

### Example 7: Using --confirm for Critical Changes
```
/plan-cascade:auto --confirm "Refactor authentication to use OAuth2"
```
**Analysis**: scope=multiple_modules, risk=high, requires_architecture=true
**Decision**: Strategy=HYBRID_AUTO, Flow=FULL
**Confirm Points**:
  1. "Architecture decisions are required. Would you like to create a design document first?"
  2. "This task is high-risk. Have you considered rollback procedures?"
**Action**: Wait for user confirmation before executing

## Recovery

If execution is interrupted at any point:

```bash
# Universal resume - auto-detects which strategy was used and resumes
/plan-cascade:resume
```

This will:
- Auto-detect whether it was mega-plan, hybrid-worktree, or hybrid-auto
- Route to the appropriate resume command
- Continue from where execution stopped

## Notes

- Strategy selection is automatic; **FULL flow (default)** may prompt for batch confirmations during execution unless `--no-confirm` is used
- The AI performs structured complexity analysis, not keyword matching
- Analysis considers scope, complexity, risk, and parallelization benefit
- Existing planning files (prd.json, mega-plan.json) are detected but don't change strategy selection
- For ambiguous cases (low confidence < 0.7), the AI errs on the side of simpler strategies
- The JSON analysis is logged for debugging and can be reviewed in progress.txt

### Flow Behavior Notes

- **Default flow**: `full` (strict methodology + strict gating)
- **--flow override**: Users can explicitly select `quick` or `standard` for faster/less strict execution
- **Confirmation points**: Generated automatically for low-confidence, high-risk, or architecture-heavy tasks
- **--explain mode**: Useful for understanding what the AI would do without actually executing
- **--confirm mode**: Recommended for critical changes to ensure human oversight

### TDD Recommendations

The `tdd_recommendation` field in the analysis output suggests whether TDD should be used:

- **off**: For simple, low-risk changes (DIRECT strategy) - no TDD overhead needed
- **on**: For high-risk tasks or FULL flow - write tests before implementation to ensure correctness
- **auto**: Let the executing agent decide based on story context (default for STANDARD flow)

#### Auto Mode Risk Detection

When `--tdd auto` (or no flag), TDD is automatically enabled for stories that:
- Have high-risk tags: `security`, `auth`, `database`, `payment`, `migration`
- Contain high-risk keywords in title/description: `authentication`, `authorization`, `encrypt`, `credential`, `delete`, etc.
- Have `test_expectations.required = true` in the story definition
- Have context_estimate of `large` or `xlarge`

#### TDD Compliance Gate

When TDD is enabled, a `TDD_COMPLIANCE` quality gate runs after story completion:
- Checks if test files were modified alongside code changes
- For high-risk stories: errors if no test changes detected
- For other stories: warnings for missing test coverage
- Provides suggestions for following TDD workflow

Example gate output:
```
[PASSED] tdd_compliance
  Test files changed: 2
  Code files changed: 3
  Message: TDD compliance check passed

[FAILED] tdd_compliance
  ERROR: Story story-001: Code changes detected but no test files modified.
  Suggestion: Follow TDD workflow: 1) Write failing test, 2) Implement to pass, 3) Refactor
```
