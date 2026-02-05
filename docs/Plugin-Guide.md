[中文版](Plugin-Guide_zh.md)

# Plan Cascade - Claude Code Plugin Guide

**Version**: 4.4.0
**Last Updated**: 2026-02-05

This document provides detailed instructions for using Plan Cascade as a Claude Code plugin.

---

## Installation

```bash
# Install from GitHub
claude plugins install Taoidle/plan-cascade

# Or clone and install locally
git clone https://github.com/Taoidle/plan-cascade.git
claude plugins install ./plan-cascade
```

---

## First-Time Setup

Before using Plan Cascade commands, run the initialization command to ensure your environment is properly configured:

```bash
/plan-cascade:init
```

This command:
- Detects your operating system
- Installs `uv` (fast Python package manager) if needed
- Verifies Python execution works correctly
- Confirms Plan Cascade module is accessible

**Note**: This is especially important on Windows, where Python execution aliases can interfere with direct `python3` calls.

---

## Command Overview

Plan Cascade provides five main entry commands, suitable for development scenarios of different scales:

| Entry Command | Use Case | Features |
|--------------|----------|----------|
| `/plan-cascade:auto` | Any task (AI auto-selects strategy) | Automatic strategy selection + ExecutionFlow depth |
| `/plan-cascade:mega-plan` | Large projects (multiple related features) | Feature-level parallel + Story-level parallel |
| `/plan-cascade:hybrid-worktree` | Single complex feature | Worktree isolation + Story parallel |
| `/plan-cascade:hybrid-auto` | Simple features | Quick PRD generation + Story parallel |
| `/plan-cascade:dashboard` | Status monitoring | Aggregated status view across all executions |

---

## Specification Interview (Spec)

Plan Cascade can run a short, resumable **spec interview** (planning-time) to reduce ambiguity before `prd.json` is finalized.

- Enabled via `--spec <off|auto|on>` on `/plan-cascade:auto`, `/plan-cascade:hybrid-auto`, `/plan-cascade:hybrid-worktree`, `/plan-cascade:mega-plan`, and `/plan-cascade:mega-approve`.
- In Mega execution, interviews run in the **orchestrator** (mega-approve), not inside per-feature subagents.

### Trigger Rules

| Mode | Behavior |
|------|----------|
| `--spec auto` | Runs only when `--flow full` (default in `/plan-cascade:auto`) |
| `--spec on` | Always run the interview |
| `--spec off` | Never run the interview |

### Outputs

- `spec.json` (structured spec) + `spec.md` (rendered spec)
- `.state/spec-interview.json` (resume state)
- Optional compile to `prd.json`

### Commands

```bash
/plan-cascade:spec-plan "<desc>" [--compile] [--output-dir <dir>] [--flow <quick|standard|full>] [--first-principles] [--max-questions N] [--feature-slug <slug>]
/plan-cascade:spec-resume [--output-dir <dir>] [--flow <quick|standard|full>]
/plan-cascade:spec-cleanup [--output-dir <dir>] [--all]
```

---

## Design Document System

Plan Cascade automatically generates technical design documents (`design_doc.json`) to provide architectural context during story execution.

### Two-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Level 1: Project Design (from mega-plan.json)               │
│ ─────────────────────────────────────────────────────────── │
│ • Global architecture and system overview                   │
│ • Cross-feature components and patterns                     │
│ • Project-wide ADRs (architectural decisions)               │
│ • Feature mappings (which patterns/decisions apply where)   │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ inheritance
                          ▼
┌─────────────────────────────────────────────────────────────┐
│ Level 2: Feature Design (from prd.json)                     │
│ ─────────────────────────────────────────────────────────── │
│ • Feature-specific components                               │
│ • Feature-specific APIs and data models                     │
│ • Feature-specific ADRs (prefixed ADR-F###)                 │
│ • Story mappings (which components/decisions per story)     │
└─────────────────────────────────────────────────────────────┘
```

### Auto-Generation Flow

| Command | Design Doc Generated | Level |
|---------|---------------------|-------|
| `mega-plan` | Automatic after mega-plan.json | Project |
| `hybrid-worktree` | Automatic after prd.json | Feature (inherits from project) |
| `hybrid-auto` | Automatic after prd.json | Feature |

### External Design Documents

All three main commands support optional external design documents:

```bash
# mega-plan: 2nd argument
/plan-cascade:mega-plan "Build e-commerce platform" ./project-architecture.md

# hybrid-auto: 2nd argument
/plan-cascade:hybrid-auto "Implement user auth" ./auth-design.md

# hybrid-worktree: 4th argument
/plan-cascade:hybrid-worktree fix-auth main "Fix auth" ./architecture.md

# Supported formats: Markdown (.md), JSON (.json), HTML (.html)
```

External documents are automatically converted to the `design_doc.json` format.

### Design Document Commands

```bash
/plan-cascade:design-generate    # Manually generate design document
/plan-cascade:design-review      # Review design document
/plan-cascade:design-import      # Import external document
```

---

## External Framework Skills

Plan Cascade includes built-in framework-specific skills that are automatically detected and injected into story execution context.

### Supported Frameworks

| Framework | Skills | Auto-Detection |
|-----------|--------|----------------|
| React/Next.js | `react-best-practices`, `web-design-guidelines` | `package.json` contains `react` or `next` |
| Vue/Nuxt | `vue-best-practices`, `vue-router-best-practices`, `vue-pinia-best-practices` | `package.json` contains `vue` or `nuxt` |
| Rust | `rust-coding-guidelines`, `rust-ownership`, `rust-error-handling`, `rust-concurrency` | `Cargo.toml` exists |

### How It Works

1. **Detection**: When a story is executed, Plan Cascade scans the project for framework indicators (`package.json`, `Cargo.toml`)
2. **Loading**: Matching skills are loaded from Git submodules in `external-skills/`
3. **Injection**: Skill content is injected into the agent's context during implementation and retry phases

### Initialization

External skills are included as Git submodules. Initialize them after cloning:

```bash
git submodule update --init --recursive
```

### Skill Sources

| Source | Repository | Skills |
|--------|------------|--------|
| Vercel | [vercel-labs/agent-skills](https://github.com/vercel-labs/agent-skills) | React, Web Design |
| Vue.js | [vuejs-ai/skills](https://github.com/vuejs-ai/skills) | Vue, Pinia, Router |
| Rust | [actionbook/rust-skills](https://github.com/actionbook/rust-skills) | Coding Guidelines, Ownership, Error Handling, Concurrency |

---

## Three-Tier External Skills System

Plan Cascade uses a three-tier skill system that allows built-in, external, and user-defined skills to coexist with clear priority rules.

### Tier Overview

| Tier | Source Type | Priority Range | Description |
|------|-------------|----------------|-------------|
| 1 | Builtin | 1-50 | Built-in with Plan Cascade (Python, TypeScript, Go, Java) |
| 2 | Submodule | 51-100 | From Git submodules (Vercel, Vue, Rust skills) |
| 3 | User | 101-200 | User-defined, highest priority |

Higher priority skills override lower priority skills with the same base name.

### How Skills Are Detected and Injected

1. **Detection**: When a story executes, Plan Cascade scans project files for framework indicators
2. **Matching**: Skills with matching `detect.files` and `detect.patterns` are selected
3. **Deduplication**: If multiple skills share the same base name, only the highest priority is kept
4. **Injection**: Up to 3 skills are injected into the agent's context during `implementation` and `retry` phases

### CLI Commands

```bash
# List all available skills
plan-cascade skills list

# List skills grouped by source type
plan-cascade skills list --group

# Show skills applicable to current project
plan-cascade skills detect

# Show skills with override details
plan-cascade skills detect --overrides

# Add a user skill from local path
plan-cascade skills add my-skill --path ./my-skills/SKILL.md

# Add a user skill from remote URL
plan-cascade skills add remote-skill --url https://example.com/skills/SKILL.md

# Add with custom options
plan-cascade skills add my-skill --path ./SKILL.md --priority 150 --level project \
  --detect-files package.json,tsconfig.json --detect-patterns typescript \
  --inject-into implementation,retry

# Remove a user skill
plan-cascade skills remove my-skill

# Remove from specific level
plan-cascade skills remove my-skill --level user

# Validate all skill configurations
plan-cascade skills validate

# Validate with verbose output
plan-cascade skills validate --verbose

# Refresh cached remote skills (re-download)
plan-cascade skills refresh --all

# Refresh specific skill
plan-cascade skills refresh remote-skill

# Clear cache without re-downloading
plan-cascade skills refresh --all --clear

# Show cache statistics
plan-cascade skills cache
```

### Configuration File

User skills are configured in `.plan-cascade/skills.json` (project-level) or `~/.plan-cascade/skills.json` (user-level).

**Project-level configuration takes precedence over user-level.**

```json
{
  "version": "1.0.0",
  "skills": [
    {
      "name": "my-custom-skill",
      "path": "./my-skills/custom/SKILL.md",
      "detect": {
        "files": ["package.json"],
        "patterns": ["my-framework"]
      },
      "priority": 150,
      "inject_into": ["implementation"]
    },
    {
      "name": "company-coding-standards",
      "path": "../shared-skills/coding-standards/SKILL.md",
      "detect": {
        "files": ["pyproject.toml", "package.json", "Cargo.toml"],
        "patterns": []
      },
      "priority": 180,
      "inject_into": ["implementation", "retry"]
    },
    {
      "name": "remote-skill",
      "url": "https://raw.githubusercontent.com/example/skills/main/advanced/SKILL.md",
      "detect": {
        "files": ["config.json"],
        "patterns": ["advanced-feature"]
      },
      "priority": 160,
      "inject_into": ["implementation"]
    }
  ]
}
```

**Configuration Fields:**

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Unique skill name |
| `path` | Yes* | Local path to SKILL.md (relative to config file) |
| `url` | Yes* | Remote URL to skill directory |
| `detect.files` | No | Files that trigger this skill (e.g., `["package.json"]`) |
| `detect.patterns` | No | Patterns to match in detected files (e.g., `["react"]`) |
| `priority` | No | Priority 101-200 (default: 150) |
| `inject_into` | No | Phases to inject: `implementation`, `retry` (default: both) |

*Either `path` or `url` is required, but not both.

### Creating Custom Skills

Custom skills are defined in SKILL.md files with YAML frontmatter.

**SKILL.md Format:**

```markdown
---
name: my-custom-skill
description: Brief description of what this skill provides.
license: MIT
metadata:
  author: your-name
  version: "1.0.0"
---

# My Custom Skill

## When to Apply

Describe when this skill should be used...

## Guidelines

| Rule | Guideline |
|------|-----------|
| Rule 1 | Description |
| Rule 2 | Description |

## Code Examples

\`\`\`typescript
// Example code...
\`\`\`

## Anti-Patterns

| Avoid | Use Instead |
|-------|-------------|
| Bad pattern | Good pattern |
```

**Frontmatter Fields:**

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Skill identifier |
| `description` | Yes | When to use this skill |
| `license` | No | License type |
| `metadata.author` | No | Skill author |
| `metadata.version` | No | Skill version |

### Priority and Override Rules

1. **Higher priority wins**: When skills share the same base name, the highest priority skill is used
2. **User skills can override**: A user skill with priority 150 overrides a builtin skill with priority 30
3. **Project > User**: Project-level `.plan-cascade/skills.json` takes precedence over `~/.plan-cascade/skills.json`

**Example Override:**

```
builtin/typescript (priority: 30) <- OVERRIDDEN
submodule/vercel-react (priority: 75) <- ACTIVE
user/my-typescript (priority: 150) <- ACTIVE (overrides builtin/typescript)
```

**Check effective skills:**

```bash
# See which skills will actually be used
plan-cascade skills detect --overrides

# Output shows:
# - Total matched: 5
# - Effective after dedup: 3
# - Override details: "user/my-typescript (150) overrides builtin/typescript (30)"
```

### Remote Skill Caching

Remote URL skills are cached locally for performance and offline access.

**Cache Details:**
- Location: `~/.plan-cascade/cache/skills/`
- Default TTL: 7 days
- Graceful degradation: Uses expired cache if network fails

**Cache Commands:**

```bash
# View cache statistics
plan-cascade skills cache

# Force refresh all cached skills
plan-cascade skills refresh --all

# Clear all cache
plan-cascade skills refresh --all --clear
```

### Best Practices

1. **Use project-level config** for team-shared skills
2. **Use user-level config** for personal coding style preferences
3. **Set priority carefully**: 150 for general overrides, 180+ for critical rules
4. **Keep skills focused**: One skill per concern (e.g., testing, error handling)
5. **Include detection patterns**: More specific patterns reduce false matches
6. **Test with `skills detect`**: Verify skills are correctly detected before execution

---

## `/plan-cascade:auto` - AI Auto Strategy

The easiest entry point. AI analyzes your task description and automatically selects the best strategy.

### How It Works

1. You provide a task description
2. AI performs structured self-assessment analyzing:
   - **Scope**: How many functional areas are involved?
   - **Complexity**: Are there sub-task dependencies? Architecture decisions needed?
   - **Risk**: Could it break existing functionality? Needs isolation?
   - **Parallelization**: Can work be parallelized for efficiency?
3. AI outputs structured analysis with confidence score
4. AI selects optimal strategy and ExecutionFlow depth, then executes

### ExecutionFlow Depth

Plan Cascade uses three workflow depth levels that control gating strictness:

| Flow | Description | Gate Mode | AI Verification | Confirm Required |
|------|-------------|-----------|-----------------|------------------|
| `quick` | Fastest path, minimal gating | soft | disabled | no |
| `standard` | Balanced speed and quality | soft | enabled | no |
| `full` | Strict methodology + strict gating (default) | hard | enabled + review | yes |

### Strategy Selection

| Analysis Result | Strategy | Example |
|----------------|----------|---------|
| 1 area, 1-2 steps, low risk | **direct** | "Fix the login button styling" |
| 2-3 areas, 3-7 steps, has dependencies | **hybrid-auto** | "Implement user authentication" |
| hybrid-auto + high risk or experimental | **hybrid-worktree** | "Experimental refactoring of payment module" |
| 4+ areas, multiple independent features | **mega-plan** | "Build e-commerce platform with users, products, orders" |

### Command-Line Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--flow <quick\|standard\|full>` | Override execution flow depth (default: full) | `--flow full` |
| `--tdd <off\|on\|auto>` | Control TDD mode (default: on in `/plan-cascade:auto`) | `--tdd on` |
| `--spec <off\|auto\|on>` | Planning-time spec interview | `--spec auto` |
| `--first-principles` | Spec: ask first-principles questions | `--first-principles` |
| `--max-questions N` | Spec: soft cap interview length | `--max-questions 12` |
| `--confirm` | Ask for confirmation before execution (default in Full flow) | `--confirm` |
| `--no-confirm` | Disable batch confirmations (even in Full flow) | `--no-confirm` |
| `--explain` | Show analysis without executing | `--explain` |
| `--json` | JSON output for `--explain` | `--explain --json` |

### Usage Example

```bash
# AI automatically determines strategy
/plan-cascade:auto "Fix the typo in README"
# → Defaults to full flow; simple tasks are upgraded to hybrid-auto for full pipeline
# → Also defaults to `--tdd on` and `--confirm` (use `--tdd off` / `--no-confirm` to override)
# → Use `--flow quick` for direct execution

/plan-cascade:auto "Implement user login with OAuth"
# → Uses hybrid-auto strategy with full flow (default) + `--tdd on` + `--confirm`

/plan-cascade:auto "Experimental refactoring of the API layer"
# → Uses hybrid-worktree strategy (full flow by default)

/plan-cascade:auto "Build a blog platform with users, posts, comments, and RSS"
# → Uses mega-plan strategy (full flow by default)

# With flags
/plan-cascade:auto --flow full --tdd on --no-confirm "Implement payment processing"
/plan-cascade:auto --explain --json "Build user authentication"
/plan-cascade:auto --flow full --spec auto --first-principles "Critical database migration"
```

---

## `/plan-cascade:mega-plan` Workflow

Suitable for large project development containing multiple related feature modules.

### Use Cases

| Type | Scenario | Example |
|------|----------|---------|
| ✅ Suitable | Multi-module new project development | Build SaaS platform (user + subscription + billing + admin) |
| ✅ Suitable | Large-scale refactoring involving multiple subsystems | Monolith to microservices architecture |
| ✅ Suitable | Feature group development | E-commerce platform (users, products, cart, orders) |
| ❌ Not suitable | Single feature development | Only implement user authentication (use Hybrid Ralph) |
| ❌ Not suitable | Bug fixes | Fix login page form validation issue |

### Sequential Execution Between Batches

Mega-plan uses **sequential execution between batches** mode, ensuring each batch creates worktree from the updated target branch:

```
mega-approve (1st time) → Start Batch 1
    ↓ Batch 1 complete
mega-approve (2nd time) → Merge Batch 1 → Create Batch 2 from updated branch
    ↓ Batch 2 complete
mega-approve (3rd time) → Merge Batch 2 → ...
    ↓ All batches complete
mega-complete → Clean up planning files
```

Key points:
- Without `--auto-prd`, call `mega-approve` once per batch; with `--auto-prd`, it runs all batches automatically
- Each batch creates worktree from **updated target branch**
- Planning files are not committed (added to .gitignore)

### Command Reference

```bash
/plan-cascade:mega-plan <description>        # Generate project plan
/plan-cascade:mega-edit                      # Edit plan
/plan-cascade:mega-approve --auto-prd        # Approve and auto-execute all batches
/plan-cascade:mega-resume --auto-prd         # Resume interrupted execution
/plan-cascade:mega-status                    # View progress
/plan-cascade:mega-complete [branch]         # Merge and cleanup
```

### Full Automation with --auto-prd

With `--auto-prd`, mega-approve runs the ENTIRE mega-plan automatically:
1. Creates worktrees for current batch
2. Generates PRDs for each feature (via Task agents)
3. Executes all stories (via Task agents)
4. Monitors until batch complete
5. Merges batch to target branch
6. Automatically continues to next batch
7. Only pauses on errors or merge conflicts

### Resume Interrupted Execution

If execution is interrupted:
```bash
/plan-cascade:mega-resume --auto-prd
```

This will:
- Auto-detect current state from files (mega-plan.json, .mega-status.json, worktrees)
- Skip already-completed features and stories
- Resume from where it left off

### Usage Example

```bash
# Scenario: Build e-commerce platform
/plan-cascade:mega-plan "Build e-commerce platform: user authentication, product management, shopping cart, order processing"

# View generated plan
/plan-cascade:mega-status

# Edit plan (optional)
/plan-cascade:mega-edit

# Start execution (auto-runs all batches)
/plan-cascade:mega-approve --auto-prd

# View execution progress
/plan-cascade:mega-status

# Cleanup after all complete
/plan-cascade:mega-complete main
```

---

## `/plan-cascade:hybrid-worktree` Workflow

Suitable for single complex feature development requiring branch isolation.

### Use Cases

| Type | Scenario | Example |
|------|----------|---------|
| ✅ Suitable | Complete feature with multiple subtasks | User authentication (registration + login + password reset) |
| ✅ Suitable | Experimental feature requiring branch isolation | New payment channel integration test |
| ✅ Suitable | Medium-scale refactoring (5-20 files) | API layer unified error handling |
| ❌ Not suitable | Simple single-file modification | Modify a component's style |
| ❌ Not suitable | Quick prototype validation | Verify if a library is usable |

### Command Reference

```bash
/plan-cascade:hybrid-worktree <name> <branch> <desc> [options]  # Create worktree + PRD
/plan-cascade:hybrid-auto <desc> [options]                      # Generate PRD (no worktree)
/plan-cascade:edit                                              # Edit PRD
/plan-cascade:approve [options]                                 # Execute PRD stories
/plan-cascade:hybrid-resume [--auto]                            # Resume interrupted execution
/plan-cascade:hybrid-status                                     # View status
/plan-cascade:hybrid-complete [branch] [--force]                # Complete and merge (--force skips uncommitted check)
```

| Parameter | Description |
|-----------|-------------|
| `<name>` | Task name (used for worktree and branch) |
| `<branch>` | Target branch to merge into when complete |
| `<desc>` | Task description OR path to existing PRD file |
| `options` | Common: `--flow`, `--tdd`, `--confirm/--no-confirm`, `--spec`, `--first-principles`, `--max-questions`, `--agent` |

### Usage Example

```bash
# Create isolated development environment (uses default agent from agents.json)
/plan-cascade:hybrid-worktree feature-auth main "Implement user authentication: login, registration, password reset"

# Create worktree with specific agent for PRD generation
/plan-cascade:hybrid-worktree feature-auth main "Implement user authentication" --agent=codex

# Generate PRD (without worktree)
/plan-cascade:hybrid-auto "Implement user authentication feature"

# View and edit PRD
/plan-cascade:edit

# Approve and auto-execute
/plan-cascade:approve --auto-run

# View execution progress
/plan-cascade:hybrid-status

# Merge to main after completion
/plan-cascade:hybrid-complete main
```

---

## `/plan-cascade:hybrid-auto` Workflow

Suitable for quick development of simple features without Worktree isolation.

### Command Reference

```bash
/plan-cascade:hybrid-auto <desc> [options]         # Generate PRD
/plan-cascade:approve [options]                    # Execute
/plan-cascade:edit                                 # Edit PRD
/plan-cascade:show-dependencies                    # View dependency graph
```

### Usage Example

```bash
# Quick PRD generation
/plan-cascade:hybrid-auto "Add password reset functionality"

# Approve and auto-execute
/plan-cascade:approve --auto-run
```

---

## `/plan-cascade:dashboard` - Status Monitoring

Provides an aggregated status view across all Plan Cascade executions.

### What It Shows

- **Execution Status**: Current state of mega-plan, hybrid, or direct executions
- **Story Progress**: Completion percentage, batch progress, pending/failed stories
- **Recent Activity**: Timeline of recent actions with timestamps
- **Suggested Actions**: Context-aware recommendations for next steps

### Usage

```bash
# Show aggregated status
/plan-cascade:dashboard

# Show verbose output with details
/plan-cascade:dashboard --verbose
```

### Output Example

```
============================================================
PLAN CASCADE DASHBOARD
============================================================

Current Execution: hybrid-auto
Status: IN_PROGRESS (62%)
Flow: standard

Story Progress:
  ✓ story-001: Completed
  ✓ story-002: Completed
  → story-003: In Progress (claude-code)
  ○ story-004: Pending
  ○ story-005: Pending

Recent Activity:
  [10:30] story-002 completed via claude-code
  [10:28] story-002 started via claude-code
  [10:25] story-001 completed via claude-code

Suggested Actions:
  • Continue: Wait for story-003 to complete
  • Review: Check progress.txt for details
============================================================
```

---

## DoR/DoD Gates (Definition of Ready/Done)

Plan Cascade provides validation gates to ensure quality at execution boundaries.

### Definition of Ready (DoR)

DoR gates run **before** story/feature execution to validate prerequisites:

| Check | Description | Mode |
|-------|-------------|------|
| Acceptance Criteria | Verifies criteria are testable and clear | SOFT/HARD |
| Dependencies Valid | Ensures all dependencies are resolved | SOFT/HARD |
| Risks Explicit | Validates risk assessment is documented | SOFT/HARD |
| Verification Prompt | Checks AI verification hints are present | SOFT/HARD |

**Gate Modes:**
- **SOFT**: Warnings only, execution continues
- **HARD**: Blocking, execution halts on failures (used in Full flow)

### Definition of Done (DoD)

DoD gates run **after** story/feature execution to validate completion:

| Level | Checks | When Used |
|-------|--------|-----------|
| **STANDARD** | Quality gates pass, AI verification, change summary | Default |
| **FULL** | Above + code review, test changes, deployment notes | Full flow |

**DoD Checks:**
- Quality gates (typecheck, test, lint) passed
- No skeleton code detected
- Acceptance criteria verified
- Change summary generated
- Code review passed (Full level)

### Gate Configuration in PRD

```json
{
  "flow_config": { "level": "standard" },
  "tdd_config": { "mode": "auto" },
  "execution_config": { "require_batch_confirm": false }
}
```

---

## TDD Support

Plan Cascade supports optional Test-Driven Development (TDD) rhythm at the story level.

### TDD Modes

| Mode | Description | When to Use |
|------|-------------|-------------|
| `off` | TDD disabled | Simple changes, documentation |
| `on` | TDD with prompts and compliance checks | Critical features, security code |
| `auto` | Auto-enable based on risk assessment | Most development tasks (default when `--tdd` not set) |

> **Note**: `/plan-cascade:auto` defaults to `--tdd on` in FULL flow. Use `--tdd auto` to opt into risk-based auto-enabling.

### TDD Workflow

When TDD is enabled:

1. **Red Phase**: Write failing tests based on acceptance criteria
2. **Green Phase**: Minimal implementation to pass tests
3. **Refactor Phase**: Improve code while keeping tests green

### TDD Compliance Checking

After story completion, quality gates verify:
- Test files were modified alongside code changes
- High-risk stories have corresponding tests
- Test coverage requirements met (if configured)

### Usage

```bash
# Default in /plan-cascade:auto (FULL flow): TDD is on
/plan-cascade:auto "Add user profile feature"

# Disable TDD for documentation
/plan-cascade:auto --tdd off "Update README"

# Let risk-based auto-detection decide
/plan-cascade:auto --tdd auto "Add user profile feature"
```

---

## Auto Execution and Quality Gates

Plan Cascade executes stories batch-by-batch with quality gates and (optionally) automatic retries.

### Full Auto Execution (Recommended)

For unattended runs (CI-friendly), use Full Auto mode from `/plan-cascade:approve`, or run the helper script directly:

```bash
uv run python scripts/auto-execute.py --prd prd.json --flow full --tdd on
```

Common script flags:
- `--max-retries N` / `--no-retry`
- `--batch N` (execute only one batch)
- `--parallel` + `--max-concurrency N`
- `--state-file <path>` (defaults to `.iteration-state.json`)

### Quality Gate Configuration

Configure in `prd.json`:

```json
{
  "quality_gates": {
    "enabled": true,
    "fail_fast": false,
    "gates": [
      {"name": "format", "type": "format", "required": false, "check_only": false},
      {"name": "typecheck", "type": "typecheck", "required": true},
      {"name": "tests", "type": "test", "required": true},
      {"name": "lint", "type": "lint", "required": false},
      {"name": "tdd", "type": "tdd_compliance", "required": false},
      {"name": "code-review", "type": "code_review", "required": false, "min_score": 0.7, "block_on_critical": true},
      {"name": "implementation_verify", "type": "implementation_verify", "required": false}
    ]
  }
}
```

**Gate Execution Order:**
1. **PRE_VALIDATION**: FORMAT (auto-format code)
2. **VALIDATION**: TYPECHECK, TEST, LINT (parallel)
3. **POST_VALIDATION**: CODE_REVIEW, IMPLEMENTATION_VERIFY (parallel)

**Gate Types:**

| Type | Description | Options |
|------|-------------|---------|
| `format` | Auto-format code | `check_only`: only check, don't modify |
| `typecheck` | Type checking (mypy/tsc) | - |
| `test` | Run tests (pytest/jest) | - |
| `lint` | Linting (ruff/eslint) | - |
| `tdd_compliance` | TDD compliance check | - |
| `code_review` | AI code review | `min_score`, `block_on_critical` |
| `implementation_verify` | AI implementation verification | - |
| `custom` | Custom script | `command` |

### Monitoring

Use `/plan-cascade:dashboard` (aggregated), `/plan-cascade:hybrid-status`, `/plan-cascade:mega-status`, and `progress.txt`.

---

## Multi-Agent Collaboration

### Supported Agents

| Agent | Type | Description |
|-------|------|-------------|
| `claude-code` | task-tool | Claude Code Task tool (built-in, always available) |
| `codex` | cli | OpenAI Codex CLI |
| `amp-code` | cli | Amp Code CLI |
| `aider` | cli | Aider AI pair programming assistant |
| `cursor-cli` | cli | Cursor CLI |

### Specifying Agents

**For hybrid-auto (PRD generation):**
```bash
# Use default agent (claude-code)
/plan-cascade:hybrid-auto "Implement user authentication"

# Specify agent for PRD generation
/plan-cascade:hybrid-auto "Implement user authentication" --agent=codex
```

**For approve (story execution):**
```bash
# Global agent override (all stories)
/plan-cascade:approve --agent=codex

# Phase-specific agents
/plan-cascade:approve --impl-agent=claude-code --retry-agent=aider

# Disable auto-fallback
/plan-cascade:approve --agent=codex --no-fallback

# Disable AI verification gate (enabled by default)
/plan-cascade:approve --no-verify

# Disable AI code review (enabled by default)
/plan-cascade:approve --no-review

# Specify code review agent
/plan-cascade:approve --review-agent=claude-code
```

**For mega-approve (feature execution):**
```bash
# Specify agents for different phases
/plan-cascade:mega-approve --auto-prd --prd-agent=codex --impl-agent=aider

# Global override
/plan-cascade:mega-approve --auto-prd --agent=claude-code
```

### Specifying Agent in PRD

```json
{
  "stories": [
    {
      "id": "story-001",
      "agent": "aider",
      "title": "Refactor data layer",
      ...
    }
  ]
}
```

### Agent Configuration File (agents.json)

```json
{
  "default_agent": "claude-code",
  "agents": {
    "claude-code": {"type": "task-tool"},
    "codex": {"type": "cli", "command": "codex"},
    "aider": {"type": "cli", "command": "aider"}
  },
  "phase_defaults": {
    "implementation": {
      "default_agent": "claude-code",
      "fallback_chain": ["codex", "aider"],
      "story_type_overrides": {
        "refactor": "aider",
        "bugfix": "codex"
      }
    }
  }
}
```

### Agent Priority

```
1. Command parameter --agent              # Highest priority
2. Phase override --impl-agent etc.
3. Story-level agent field
4. Story type override               # bugfix -> codex, refactor -> aider
5. Phase default Agent
6. Fallback chain
7. claude-code                       # Final fallback
```

---

## Complete Command Reference

### Setup

```bash
/plan-cascade:init                     # Environment setup (uv/Python checks)
/plan-cascade:check-gitignore           # Ensure .gitignore entries
```

### Auto Strategy

```bash
/plan-cascade:auto <description> [options]

Options:
  --flow <quick|standard|full>    Override execution flow depth
  --tdd <off|on|auto>             Control TDD mode
  --confirm | --no-confirm        Confirmation behavior
  --spec <off|auto|on>            Planning-time spec interview
  --first-principles              Spec: ask first-principles questions
  --max-questions N               Spec: soft cap interview length
  --explain [--json]              Show analysis without executing
```

### Spec Interview

```bash
/plan-cascade:spec-plan "<desc>" [--compile] [--output-dir <dir>] [--flow <quick|standard|full>] [--first-principles] [--max-questions N] [--feature-slug <slug>]
/plan-cascade:spec-resume [--output-dir <dir>] [--flow <quick|standard|full>]
/plan-cascade:spec-cleanup [--output-dir <dir>] [--all]
```

### Status Monitoring

```bash
/plan-cascade:dashboard [--verbose]
```

### Project-Level (Mega Plan)

```bash
/plan-cascade:mega-plan <description> [options]
/plan-cascade:mega-edit
/plan-cascade:mega-approve [--auto-prd] [options]
/plan-cascade:mega-resume [--auto-prd]
/plan-cascade:mega-status
/plan-cascade:mega-complete [branch]
```

### Feature-Level (Hybrid)

```bash
/plan-cascade:hybrid-worktree <name> <branch> <desc> [options]
/plan-cascade:hybrid-auto <desc> [options]
/plan-cascade:edit
/plan-cascade:approve [options]
/plan-cascade:show-dependencies
/plan-cascade:hybrid-status
/plan-cascade:hybrid-resume [--auto]
/plan-cascade:hybrid-manual
/plan-cascade:hybrid-complete [branch] [--force]
```

### Design Documents

```bash
/plan-cascade:design-generate
/plan-cascade:design-import <path>
/plan-cascade:design-review
```

### Basic Planning

```bash
/plan-cascade:start
/plan-cascade:resume
/plan-cascade:worktree <name> <branch>
/plan-cascade:complete [branch] [--force]
```

---

## Status File Reference

State files may be stored in the project root (legacy mode) or under `<user-dir>/.state/` (new mode).

| File | Type | Description |
|------|------|-------------|
| `prd.json` | Planning | PRD document |
| `mega-plan.json` | Planning | Project plan |
| `design_doc.json` | Planning | Technical design document |
| `spec.json` | Planning | Structured planning spec (optional) |
| `spec.md` | Planning | Rendered spec generated from `spec.json` (optional) |
| `agents.json` | Configuration | Agent configuration |
| `findings.md` | Shared | Findings record |
| `mega-findings.md` | Shared | Project-level findings (mega-plan) |
| `progress.txt` | Shared | Progress log |
| `.mega-status.json` / `.state/.mega-status.json` | Status | Mega-plan execution status |
| `.agent-status.json` / `.state/agent-status.json` | Status | Agent status |
| `.iteration-state.json` / `.state/iteration-state.json` | Status | Iteration state |
| `.retry-state.json` / `.state/retry-state.json` | Status | Retry record |
| `.state/spec-interview.json` | Status | Spec interview resume state (optional) |
| `.state/stage-state.json` | Status | Stage state machine state (v4.4.0+) |
| `.hybrid-execution-context.md` | Context | Hybrid task context for AI recovery |
| `.mega-execution-context.md` | Context | Mega-plan context for AI recovery |
| `.agent-outputs/` | Output | Agent logs |

---

## Troubleshooting

### Agent Unavailable

```
[AgentExecutor] Agent 'codex' unavailable (CLI 'codex' not found in PATH)
```

Solution: Install the corresponding Agent or use `--no-fallback` to disable fallback.

### Quality Gate Failure

```
[QualityGate] typecheck failed: error TS2304
```

Solution: Fix type errors and retry, or disable that gate in prd.json.

### Worktree Conflict

```
fatal: 'feature-xxx' is already checked out
```

Solution: Use `/plan-cascade:hybrid-complete` to clean up existing worktree.

### Interrupted Execution

If a mega-plan or hybrid task was interrupted (e.g., connection lost, Claude Code crashed):

```bash
# For mega-plan
/plan-cascade:mega-resume --auto-prd

# For hybrid task
/plan-cascade:hybrid-resume --auto
```

These commands will:
- Auto-detect current state from existing files
- Skip already-completed work
- Resume from where execution stopped
- Support both old and new progress marker formats

### Context Recovery After Long Sessions

Plan Cascade automatically generates context files that help recover execution state after:
- Context compression (AI summarizes old messages)
- Context truncation (old messages deleted)
- New conversation session
- Claude Code restart

**Context files generated:**
| File | Mode | Description |
|------|------|-------------|
| `.hybrid-execution-context.md` | Hybrid | Current batch, pending stories, progress summary |
| `.mega-execution-context.md` | Mega Plan | Active worktrees, parallel execution state |

These files are automatically updated via hooks during execution. If you notice the AI has lost context:

```bash
# Universal recovery command (auto-detects mode)
/plan-cascade:resume

# Or check the context file directly
cat .hybrid-execution-context.md
cat .mega-execution-context.md
```
