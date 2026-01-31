---
description: "AI auto strategy executor. Analyzes task and automatically selects and executes the best strategy: direct execution, hybrid-auto PRD generation, hybrid-worktree isolated development, or mega-plan multi-feature orchestration."
---

# Plan Cascade - Auto Strategy Executor

AI automatically analyzes the task and executes the optimal strategy without user confirmation.

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

## Step 1: Parse Task Description

Get the task description from user arguments:

```
TASK_DESC="{{args}}"
```

If no description provided, ask the user:
```
Please provide a task description. What do you want to accomplish?
Example: "Fix the login button styling" or "Build a user authentication system"
```

## Step 2: Gather Project Context

Collect project context for strategy analysis:

```bash
# Check if in Git repository
if git rev-parse --git-dir > /dev/null 2>&1; then
    GIT_REPO=true
    CURRENT_BRANCH=$(git branch --show-current)
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
else
    GIT_REPO=false
fi

# Check for existing planning files
HAS_PRD=false
HAS_MEGA_PLAN=false
[ -f "prd.json" ] && HAS_PRD=true
[ -f "mega-plan.json" ] && HAS_MEGA_PLAN=true

# Detect project type (optional context)
PROJECT_TYPE="unknown"
[ -f "package.json" ] && PROJECT_TYPE="nodejs"
[ -f "requirements.txt" ] || [ -f "pyproject.toml" ] && PROJECT_TYPE="python"
[ -f "Cargo.toml" ] && PROJECT_TYPE="rust"
[ -f "go.mod" ] && PROJECT_TYPE="go"
```

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
    "confidence": 0.0-1.0,
    "reasoning": "<explanation of why this strategy was chosen>"
  }
}
```

### 3.3: Strategy Selection Guide

Use this decision matrix based on your analysis:

| Analysis Result | Strategy | When to Use |
|-----------------|----------|-------------|
| 1 area, 1-2 steps, low risk, no parallelization | **DIRECT** | Quick fixes, config changes, simple updates |
| 2-3 areas, 3-7 steps, has dependencies | **HYBRID_AUTO** | Feature development with clear stories |
| HYBRID_AUTO conditions + high risk OR experimental | **HYBRID_WORKTREE** | Risky changes needing isolation |
| 4+ areas, 8+ steps, significant parallelization | **MEGA_PLAN** | Multiple independent features, platform work |

### 3.4: Strategy Decision Rules

**DIRECT** - Choose when ALL of these are true:
- Single functional area
- 1-2 implementation steps
- Low to medium risk
- No subtask dependencies
- No architecture decisions needed

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

## Step 4: Display Analysis Result and Execute

Display your structured analysis and selected strategy:

```
============================================================
AUTO STRATEGY ANALYSIS
============================================================
Task: {TASK_DESC}

Analysis:
  Functional Areas: {functional_areas}
  Estimated Stories: {estimated_stories}
  Has Dependencies: {has_dependencies}
  Architecture Decisions: {requires_architecture_decisions}
  Risk Level: {risk_level}
  Parallelization: {parallelization_benefit}

Decision:
  Strategy: {STRATEGY}
  Confidence: {confidence}
  Reasoning: {reasoning}
============================================================
Executing...
```

## Step 5: Route to Appropriate Strategy

Execute the selected strategy:

### If STRATEGY is "DIRECT":

Execute the task directly without creating planning files:

```
Direct execution mode selected.

The task is simple enough to execute without formal planning.
Proceeding to implement directly...
```

Then proceed to analyze the codebase and implement the requested changes directly.

### If STRATEGY is "HYBRID_AUTO":

Route to hybrid-auto command:

```
Routing to /plan-cascade:hybrid-auto...
```

Then invoke:
```
/plan-cascade:hybrid-auto "{TASK_DESC}"
```

### If STRATEGY is "HYBRID_WORKTREE":

Generate a task name from the description and route:

```bash
# Generate task name from description (first 3 significant words, lowercase, hyphenated)
TASK_NAME=$(echo "$TASK_DESC" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9 ]//g' | awk '{for(i=1;i<=3&&i<=NF;i++)printf "%s-",$i}' | sed 's/-$//')
if [ -z "$TASK_NAME" ]; then
    TASK_NAME="task-$(date +%Y%m%d-%H%M)"
fi
```

```
Routing to /plan-cascade:hybrid-worktree...
Task name: {TASK_NAME}
Target branch: {DEFAULT_BRANCH}
```

Then invoke:
```
/plan-cascade:hybrid-worktree {TASK_NAME} {DEFAULT_BRANCH} "{TASK_DESC}"
```

### If STRATEGY is "MEGA_PLAN":

Route to mega-plan command:

```
Routing to /plan-cascade:mega-plan...
```

Then invoke:
```
/plan-cascade:mega-plan "{TASK_DESC}"
```

## Strategy Summary Table

| Strategy | Trigger | Execution |
|----------|---------|-----------|
| **DIRECT** | Simple fixes, single-file changes, trivial tasks | Execute immediately |
| **HYBRID_AUTO** | Feature development keywords (no isolation) | Generate PRD, parallel stories |
| **HYBRID_WORKTREE** | Feature keywords + isolation keywords | Isolated worktree + PRD |
| **MEGA_PLAN** | Platform/system keywords or 3+ modules | Multi-feature orchestration |

## Examples

### Example 1: Direct Execution
```
/plan-cascade:auto "Fix the typo in the README"
```
**Detected**: "fix", "typo" -> DIRECT
**Action**: Execute fix directly

### Example 2: Hybrid Auto
```
/plan-cascade:auto "Implement user authentication with login and registration"
```
**Detected**: "implement", "authentication", "login", "registration" -> HYBRID_AUTO
**Action**: Route to `/plan-cascade:hybrid-auto`

### Example 3: Hybrid Worktree
```
/plan-cascade:auto "Experimental refactoring of the payment module"
```
**Detected**: "refactoring" (feature) + "experimental" (isolation) -> HYBRID_WORKTREE
**Action**: Route to `/plan-cascade:hybrid-worktree`

### Example 4: Mega Plan
```
/plan-cascade:auto "Build an e-commerce platform with users, products, cart, and orders"
```
**Detected**: "platform" + 4 modules listed -> MEGA_PLAN
**Action**: Route to `/plan-cascade:mega-plan`

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

- Strategy selection is fully automatic - no user confirmation required
- The AI performs structured complexity analysis, not keyword matching
- Analysis considers scope, complexity, risk, and parallelization benefit
- Existing planning files (prd.json, mega-plan.json) are detected but don't change strategy selection
- For ambiguous cases (low confidence < 0.7), the AI errs on the side of simpler strategies
- The JSON analysis is logged for debugging and can be reviewed in progress.txt
