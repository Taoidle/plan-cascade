---
description: "AI auto strategy executor. Analyzes task and automatically selects and executes the best strategy: direct execution, hybrid-auto PRD generation, hybrid-worktree isolated development, or mega-plan multi-feature orchestration."
---

# Plan Cascade - Auto Strategy Executor

AI automatically analyzes the task and executes the optimal strategy without user confirmation.

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

## Step 3: AI Strategy Analysis

Analyze the task description using keyword-based detection (not word count):

### Strategy Detection Rules

**Priority Order** (check in this order, first match wins):

### 1. MEGA_PLAN Detection

**Keywords** (any match triggers mega-plan):
- Scale keywords: `platform`, `system`, `architecture`, `infrastructure`, `framework`
- Multi-feature keywords: `multiple features`, `several modules`, `various components`
- Completeness keywords: `complete`, `comprehensive`, `full`, `entire`, `whole`, `end-to-end`, `e2e`
- Architecture keywords: `microservices`, `monorepo`, `multi-tenant`, `distributed`

**Structure patterns**:
- Lists 3+ independent functional modules (e.g., users, products, orders)
- Contains enumeration patterns like "A, B, C, and D"

### 2. HYBRID_WORKTREE Detection

**Requires BOTH conditions**:
- Contains feature development keywords (from hybrid-auto list below)
- **AND** contains isolation keywords:
  - `experimental`, `experiment`, `prototype`, `poc`, `proof of concept`
  - `parallel`, `isolation`, `isolated`, `separate`, `independently`
  - `refactor`, `refactoring`, `rewrite`, `restructure`, `reorganize`
  - `risky`, `breaking`, `major change`, `don't affect`, `without affecting`

### 3. HYBRID_AUTO Detection

**Keywords** (any match without isolation keywords):
- Action keywords: `implement`, `create`, `build`, `develop`, `design`, `integrate`
- Feature keywords: `feature`, `function`, `module`, `component`, `api`, `endpoint`, `service`, `handler`
- Technical keywords: `authentication`, `authorization`, `login`, `registration`, `crud`, `database`, `cache`

### 4. DIRECT (Default)

**Keywords that suggest direct execution**:
- Action keywords: `fix`, `typo`, `update`, `modify`, `change`, `rename`, `remove`, `delete`, `add` (alone)
- Scope keywords: `minor`, `simple`, `quick`, `small`, `single`, `one`, `only`, `just`, `trivial`, `tiny`
- Target keywords: `file`, `line`, `button`, `text`, `string`, `config`, `setting`, `style`, `css`

**Default**: If no other strategy matches, use DIRECT.

## Step 4: Display Analysis Result and Execute

Display the selected strategy and reasoning:

```
============================================================
AUTO STRATEGY: {STRATEGY}
============================================================
Task: {TASK_DESC}
Reasoning: {REASONING}
============================================================
Executing...
```

Where `{REASONING}` explains:
- Which keywords were detected
- Why this strategy was chosen
- For mega-plan: list the identified modules

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

## Notes

- Strategy selection is fully automatic - no user confirmation required
- The AI analyzes keywords and patterns, not task description length
- Existing planning files (prd.json, mega-plan.json) are detected but don't change strategy selection
- For ambiguous cases, the AI errs on the side of simpler strategies (DIRECT > HYBRID_AUTO > HYBRID_WORKTREE > MEGA_PLAN)
