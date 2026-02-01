# Changelog

All notable changes to this project will be documented in this file.

## [4.2.4] - 2026-02-01

### Added

- **Runtime Files Migration to User Directory** - Planning files now stored in centralized user directory
  - **Windows**: `%APPDATA%/plan-cascade/<project-id>/`
  - **Unix/macOS**: `~/.plan-cascade/<project-id>/`
  - Project ID format: `<sanitized-name>-<SHA256-8chars>` for uniqueness
  - Keeps project root clean and avoids polluting codebase with planning files

- **PathResolver Module** - Unified path resolution for all Plan Cascade files
  - `get_data_dir()` - User data directory
  - `get_project_id()` - Unique project identifier
  - `get_project_dir()` - Project-specific data directory
  - `get_prd_path()`, `get_mega_plan_path()` - Planning file paths
  - `get_worktree_dir()`, `get_locks_dir()`, `get_state_dir()` - Runtime directories
  - 45 comprehensive tests

- **ConfigManager Module** - Hierarchical configuration system
  - Priority: Environment variables > Project config > Global config > Defaults
  - `PLAN_CASCADE_DATA_DIR` - Custom data directory
  - `PLAN_CASCADE_LEGACY_MODE` - Enable legacy mode (files in project root)
  - 46 comprehensive tests

- **ProjectLinkManager Module** - Project discovery via `.plan-cascade-link.json`
  - Lightweight link file in project root for project discovery
  - Contains project ID and data directory path
  - Enables finding all Plan Cascade projects
  - 38 comprehensive tests

- **MigrationManager Module** - `plan-cascade migrate` command
  - `migrate detect` - Scan for legacy files
  - `migrate run` - Migrate to new mode with `--dry-run` support
  - `migrate rollback` - Revert to legacy mode
  - Automatic backup creation during migration
  - 39 comprehensive tests

- **GitignoreManager Module** - Automatic `.gitignore` configuration
  - Auto-checks and updates `.gitignore` when Plan Cascade commands start
  - Prevents planning files from being committed to version control
  - New command: `/plan-cascade:check-gitignore`
  - Idempotent: Safe to run multiple times
  - 21 comprehensive tests

- **New CLI Commands:**
  - `migrate detect` - Detect legacy files in project
  - `migrate run [--dry-run]` - Migrate project to new path mode
  - `migrate rollback` - Revert to legacy mode

### Changed

- **StateManager Updated** - Now uses PathResolver for all path operations
  - Supports both new mode (user directory) and legacy mode (project root)
  - Backward compatible with existing projects

- **MegaStateManager Updated** - Uses PathResolver for mega-plan state files
  - `.mega-status.json` now in state directory
  - `mega-plan.json` in project data directory

- **ContextRecoveryManager Updated** - Uses PathResolver for context files
  - `.hybrid-execution-context.md` and `.mega-execution-context.md` paths resolved dynamically

- **All 21 Command Files Updated** - Path Storage Modes documentation added
  - `hybrid-auto.md`, `mega-plan.md`, `auto.md` - Auto gitignore check at start
  - All commands document new mode vs legacy mode file locations

- **User-Visible Files** - Always remain in project root for visibility
  - `findings.md`, `progress.txt`, `mega-findings.md`
  - These files are useful for users and should not be hidden

### Technical Details

**Path Resolution:**
```python
from plan_cascade.state.path_resolver import PathResolver
from pathlib import Path

resolver = PathResolver(Path.cwd())
print(resolver.get_prd_path())        # ~/.plan-cascade/<project-id>/prd.json
print(resolver.get_worktree_dir())    # ~/.plan-cascade/<project-id>/.worktree/
print(resolver.is_legacy_mode())      # False (default)
```

**Configuration:**
```bash
# Use custom data directory
export PLAN_CASCADE_DATA_DIR=/custom/path

# Enable legacy mode (files in project root)
export PLAN_CASCADE_LEGACY_MODE=true

# Or in .plan-cascade/config.json
{
  "data_dir": "/custom/path",
  "legacy_mode": false
}
```

**Migration:**
```bash
# Detect legacy files
plan-cascade migrate detect

# Migrate with dry-run preview
plan-cascade migrate run --dry-run

# Perform actual migration
plan-cascade migrate run

# Rollback if needed
plan-cascade migrate rollback
```

**Gitignore Entries:**
```
# Plan Cascade - Runtime directories
.worktree/
.locks/
.state/

# Plan Cascade - Planning documents
prd.json
mega-plan.json
design_doc.json

# Plan Cascade - Status files
.mega-status.json
.planning-config.json
.agent-status.json
.iteration-state.json
.retry-state.json

# Plan Cascade - Context recovery
.hybrid-execution-context.md
.mega-execution-context.md

# Plan Cascade - New mode files
.plan-cascade-link.json
.plan-cascade-backup/
.plan-cascade.json

# Plan Cascade - Agent outputs
.agent-outputs/
```

**New Files:**
| File | Description |
|------|-------------|
| `src/plan_cascade/state/path_resolver.py` | Unified path resolution |
| `src/plan_cascade/state/config_manager.py` | Hierarchical configuration |
| `src/plan_cascade/state/project_link.py` | Project link management |
| `src/plan_cascade/cli/migrate.py` | Migration tool |
| `src/plan_cascade/utils/gitignore.py` | Gitignore management |
| `commands/check-gitignore.md` | New command for manual gitignore check |
| `tests/test_path_resolver.py` | 45 tests |
| `tests/test_config_manager.py` | 46 tests |
| `tests/test_project_link.py` | 38 tests |
| `tests/test_migrate.py` | 39 tests |
| `tests/test_gitignore.py` | 21 tests |

**Test Summary:**
- Total tests: 465 (was 276 in v4.2.3)
- New tests: 189 (path_resolver: 45, config: 46, link: 38, migrate: 39, gitignore: 21)

---

## [4.2.3] - 2026-02-01

### Added

- **Three-Tier Skill Priority System** - Comprehensive skill management with priority-based override
  - **Builtin skills** (priority 1-50): Python, Go, Java, TypeScript best practices bundled with Plan Cascade
  - **External skills** (priority 51-100): Framework skills from git submodules (React, Vue, Rust)
  - **User skills** (priority 101-200): Custom skills via `.plan-cascade/skills.json` with highest priority
  - Same-name skills are deduplicated with higher priority winning

- **User Skill Configuration** - Project and user-level custom skill support
  - Project config: `.plan-cascade/skills.json`
  - User config: `~/.plan-cascade/skills.json`
  - Configuration cascade: project > user > builtin defaults
  - Support for local paths and remote URLs

- **Remote Skill Caching** - Persistent cache for URL-based skills
  - Cache location: `~/.plan-cascade/cache/skills/`
  - Default TTL: 7 days with auto-expiry
  - Graceful degradation: uses expired cache on network errors
  - Cache metadata tracking (download time, content hash, size)

- **New CLI Commands** for skill management:
  - `skills list --group` - List skills grouped by source type
  - `skills detect --overrides` - Show skill override analysis
  - `skills add <name> --path|--url` - Add custom skill
  - `skills remove <name>` - Remove custom skill
  - `skills refresh` - Refresh cached remote skills
  - `skills cache` - Show cache statistics

### Changed

- **ExternalSkillLoader Refactored** - Multi-source loading with priority sorting
  - `LoadedSkill` now includes `source_type` and `origin` fields
  - Automatic deduplication of same-name skills by priority
  - Enhanced verbose output showing skill sources

- **external-skills.json Updated** - Version 1.1.0 with three-tier support
  - Added `priority_ranges` section defining valid ranges per type
  - Added builtin source pointing to `builtin-skills/`
  - Builtin skills for Python, Go, Java, TypeScript (priority 30-36)

### Documentation

- Updated Plugin-Guide.md with comprehensive skill system documentation
- Updated Plugin-Guide_zh.md with Chinese translation
- Added `.plan-cascade/skills.example.json` for reference

---

## [4.2.2] - 2026-01-31

### Fixed

- **Strategy Routing in Auto Command** - Fixed `/plan-cascade:auto` not properly routing to selected strategy
  - When HYBRID_AUTO, HYBRID_WORKTREE, or MEGA_PLAN strategy was selected, AI would directly execute tasks instead of calling the appropriate skill
  - Added **MANDATORY Skill tool usage** instructions in `commands/auto.md`
  - Now explicitly requires `Skill(skill="plan-cascade:xxx", args="...")` calls for non-DIRECT strategies

- **Resume Command Routing** - Fixed `/plan-cascade:resume` not properly routing to resume commands
  - Same issue as auto command: AI would try to execute resume logic directly
  - Added **MANDATORY Skill tool usage** instructions in `commands/resume.md`
  - Now correctly routes to `mega-resume` or `hybrid-resume` via Skill tool

### Technical Details

**Root Cause:** The command instructions used ambiguous "Then invoke:" phrasing which AI interpreted as "execute directly" rather than "call via Skill tool".

**Fix Applied:** Added explicit instructions:
- `CRITICAL` warnings about mandatory Skill tool usage
- `MANDATORY` markers before each routing step
- Clear `Skill(skill="...", args="...")` syntax examples
- Warnings to stop and use Skill tool if AI starts reading files after strategy selection

---

## [4.2.1] - 2026-01-31

### Added

- **Context Recovery System** - Auto-generated context files for AI state recovery after session interruption
  - `.hybrid-execution-context.md` - Hybrid task context (current batch, pending stories, progress)
  - `.mega-execution-context.md` - Mega-plan context (active worktrees, parallel execution state)
  - Context files auto-updated via PreToolUse/PostToolUse hooks
  - New scripts: `hybrid-context-reminder.py`, `mega-context-reminder.py`
  - Auto-Recovery Protocol in SKILL.md guides AI to read context on session start

- **Enhanced External Skill Detection** - Improved visibility of loaded framework skills
  - `detect_applicable_skills(verbose=True)` - Print detection results to stdout
  - `get_skills_summary(phase)` - Generate ASCII table summary of loaded skills
  - `display_skills_summary(phase)` - Display skills summary during execution
  - Skills now logged when loaded: `[ExternalSkillLoader] ✓ Loaded: xxx (N lines)`

### Changed

- **Hooks Enhancement** - PreToolUse/PostToolUse hooks now trigger context file updates
  - `hybrid-ralph` hooks extended to match `Write|Edit|Bash|Task` (was `Write|Edit|Bash`)
  - `mega-plan` hooks extended similarly
  - Hooks display context reminder before relevant tool calls

- **Command Updates:**
  - `approve.md` - Added Step 4.6 for external skill detection and display
  - `hybrid-resume.md` - Added Step 6.6 for context file update after resume
  - `hybrid-worktree.md` - Added Step 13.6 for context file generation
  - `mega-resume.md` - Added Step 7.1 for context file update (split from Step 7)
  - `mega-plan/approve.md` - Added context file update and skill detection steps

- **Wrong Branch Detection** - Mega-plan hooks now warn if executing on main/master while worktrees exist

### Technical Details

**Context File Content:**
- Current execution mode and task name
- Batch progress (current/total)
- Pending stories in current batch with status
- Critical execution rules
- Recovery command reference

**New Files:**
| File | Description |
|------|-------------|
| `.hybrid-execution-context.md` | Hybrid task context for AI recovery |
| `.mega-execution-context.md` | Mega-plan context for AI recovery |
| `skills/hybrid-ralph/scripts/hybrid-context-reminder.py` | Context generator script |
| `skills/mega-plan/scripts/mega-context-reminder.py` | Context generator script |

---

## [4.2.0] - 2026-01-31

### Added

- **External Framework Skills** - Auto-detected framework-specific best practices injected into story execution
  - Git submodules for skill sources: vercel, vue, rust
  - Auto-detection based on project files (`package.json`, `Cargo.toml`)
  - Skills loaded and injected during implementation and retry phases
  - Supported frameworks:
    | Framework | Skills | Detection |
    |-----------|--------|-----------|
    | React/Next.js | `react-best-practices`, `web-design-guidelines` | `package.json` contains `react`/`next` |
    | Vue/Nuxt | `vue-best-practices`, `vue-router-best-practices`, `vue-pinia-best-practices` | `package.json` contains `vue`/`nuxt` |
    | Rust | `rust-coding-guidelines`, `rust-ownership`, `rust-error-handling`, `rust-concurrency` | `Cargo.toml` exists |
  - Skill sources:
    - [vercel-labs/agent-skills](https://github.com/vercel-labs/agent-skills) — React/Next.js
    - [vuejs-ai/skills](https://github.com/vuejs-ai/skills) — Vue.js
    - [actionbook/rust-skills](https://github.com/actionbook/rust-skills) — Rust meta-cognition framework
  - New module: `src/plan_cascade/core/external_skill_loader.py`
  - Configuration: `external-skills.json`

- **Design Document System** - Auto-generated technical design documents alongside PRDs
  - Two-level hierarchy: Project-level (from mega-plan.json) and Feature-level (from prd.json)
  - Feature design docs inherit from project-level context
  - `story_mappings`: Links stories to relevant components/decisions/interfaces
  - `feature_mappings`: Links features to patterns/decisions in project-level docs
  - ADR (Architecture Decision Record) support with prefixes: `ADR-###` (project), `ADR-F###` (feature)
  - Automatic generation after PRD creation in all three modes

- **External Design Document Import** - Convert and use existing design documents
  - Supported formats: Markdown (.md), JSON (.json), HTML (.html)
  - All three main commands support external design docs:
    - `/plan-cascade:mega-plan "desc" ./architecture.md`
    - `/plan-cascade:hybrid-auto "desc" ./design.md`
    - `/plan-cascade:hybrid-worktree name branch "desc" ./design.md`
  - Automatic conversion to `design_doc.json` format

- **Design Document Commands:**
  - `/plan-cascade:design-generate` - Manually generate design document
  - `/plan-cascade:design-import <path>` - Import external design document
  - `/plan-cascade:design-review` - Review current design document

- **Context Injection** - Design context provided to agents during story execution
  - ContextFilter extracts relevant components/decisions per story via `story_mappings`
  - AgentExecutor builds design-aware prompts with architectural patterns
  - Reduces AI hallucination by maintaining architectural context

### Changed

- **Auto Strategy Selection** - AI self-assessment replaces keyword matching
  - Analyzes task across 4 dimensions: scope, complexity, risk, parallelization benefit
  - Outputs structured JSON with `task_analysis` and `strategy_decision`
  - Includes confidence score and reasoning
  - More accurate strategy selection based on actual task complexity

- **Strategy Selection Guidelines:**
  | Analysis Result | Strategy |
  |----------------|----------|
  | 1 area, 1-2 steps, low risk | DIRECT |
  | 2-3 areas, 3-7 steps, has dependencies | HYBRID_AUTO |
  | HYBRID_AUTO + high risk or experimental | HYBRID_WORKTREE |
  | 4+ areas, multiple independent features | MEGA_PLAN |

- **Multi-Agent Command Parameters** - Full integration in all command files
  - `/plan-cascade:approve`: `--agent`, `--impl-agent`, `--retry-agent`, `--no-fallback`
  - `/plan-cascade:mega-approve`: `--agent`, `--prd-agent`, `--impl-agent`, `--auto-prd`
  - `/plan-cascade:hybrid-auto`: `--agent`

- **Documentation Updates:**
  - README.md/README_zh.md - Restructured with professional open source format, added External Framework Skills section
  - Plugin-Guide.md/Plugin-Guide_zh.md - Added design document, multi-agent, and External Framework Skills sections
  - System-Architecture.md/System-Architecture_zh.md - Complete rewrite with new sections:
    - Section 5: Design Document System with External Framework Skills subsection
    - Updated Section 3: Complete Workflow with context loading step
    - Updated Section 9: Auto-Iteration Workflow with skill injection
    - Added ExternalSkillLoader to Core Components diagram
    - Updated Section 4: AI self-assessment flow
    - Updated Section 10: Data flow with design_doc.json
    - Updated Section 12: Multi-agent command parameters

### Technical Details

**Design Document Schema:**
```json
{
  "metadata": {
    "source": "ai-generated|user-provided|converted",
    "prd_reference": "prd.json",
    "parent_design_doc": "path/to/project/design_doc.json"
  },
  "overview": { "title", "summary", "goals", "non_goals" },
  "architecture": {
    "components": [{ "name", "responsibilities", "dependencies", "files" }],
    "patterns": [{ "name", "description", "rationale" }]
  },
  "decisions": [{ "id", "title", "context", "decision", "status" }],
  "story_mappings": { "story-001": { "components", "decisions", "interfaces" } },
  "feature_mappings": { "feature-001": { "patterns", "decisions" } }
}
```

**AI Self-Assessment Output:**
```json
{
  "task_analysis": {
    "functional_areas": ["auth", "api", "frontend"],
    "estimated_stories": 5,
    "has_dependencies": true,
    "requires_architecture_decisions": true,
    "risk_level": "medium",
    "parallelization_benefit": "significant"
  },
  "strategy_decision": {
    "strategy": "HYBRID_AUTO",
    "confidence": 0.85,
    "reasoning": "Task involves 3 functional areas with dependencies..."
  }
}
```

**New Files:**
| File | Description |
|------|-------------|
| `design_doc.json` | Technical design document |
| `mega-findings.md` | Project-level findings (mega-plan) |
| `.mega-status.json` | Mega-plan execution status |
| `external-skills.json` | External skill mapping configuration |
| `external-skills/vercel/` | Git submodule for React/Next.js skills |
| `external-skills/vue/` | Git submodule for Vue.js skills |
| `external-skills/rust/` | Git submodule for Rust skills |

**External Skill Loader:**
```python
# Usage in ContextFilter
from ..core.external_skill_loader import ExternalSkillLoader

loader = ExternalSkillLoader(project_root, plugin_root)
skills = loader.detect_applicable_skills()  # ['react-best-practices', 'web-design-guidelines']
context = loader.get_skill_context("implementation")  # Formatted markdown for agent prompt
```

---

## [4.1.1] - 2026-01-30

### Added

- **Mega-Resume Command** - New `/plan-cascade:mega-resume` command to resume interrupted mega-plan executions
  - Auto-detects current state from existing files (worktrees, prd.json, progress.txt)
  - Compatible with both old-style (pre-4.1.1) and new-style executions
  - Skips already-completed work, resumes from where it left off
  - Supports `--auto-prd` flag for fully automatic resumption
  - Handles edge cases: missing worktrees, corrupted PRDs, partial completions

- **Hybrid-Resume Command** - New `/plan-cascade:hybrid-resume` command to resume interrupted hybrid tasks
  - Works with both `hybrid-worktree` and `hybrid-auto` tasks
  - Auto-detects context (worktree directory vs regular directory)
  - Scans for interrupted tasks if not in a task directory
  - Determines task state: needs_prd, needs_approval, executing, complete
  - Supports both old-style `[COMPLETE]` and new-style `[STORY_COMPLETE]` markers
  - Calculates remaining work and resumes from incomplete stories
  - `--auto` flag for fully automatic execution

### Fixed

- **Mega-Approve Full Automation** - Fixed `/plan-cascade:mega-approve --auto-prd` not running to completion automatically
  - Previously would stop after creating worktrees and require manual intervention
  - Now runs the ENTIRE mega-plan automatically with `--auto-prd` flag:
    1. Creates worktrees for current batch
    2. Launches PRD generation Task agents in parallel
    3. Executes all stories via Task agents in parallel
    4. Monitors until batch complete
    5. Merges completed batch to target branch
    6. Automatically continues to next batch
    7. Repeats until ALL batches complete
  - Only pauses on errors or merge conflicts

### Changed

- **mega-approve.md** - Complete rewrite with explicit automation instructions
  - Added Step 4: Main Execution Loop with pseudocode
  - Added Step 6: PRD Generation with Task agent prompts
  - Added Step 7: Story Execution with Task agent prompts
  - Added Step 8: Monitoring loop with progress markers
  - Added Step 10: Automatic batch continuation
  - Clear separation of `--auto-prd` (fully automatic) vs manual mode

- **mega-status.md** - Updated to use new progress markers
  - Added documentation for `[PRD_COMPLETE]`, `[STORY_COMPLETE]`, `[FEATURE_COMPLETE]` markers
  - Added section on automated execution mode

### Added

- **Progress Markers** - Standardized markers for mega-plan execution tracking:
  - `[PRD_COMPLETE] {feature_id}` - PRD generation done
  - `[STORY_COMPLETE] {story_id}` - Individual story done
  - `[STORY_FAILED] {story_id}` - Story failed
  - `[FEATURE_COMPLETE] {feature_id}` - All stories done, ready for merge
  - `[FEATURE_FAILED] {feature_id}` - Feature cannot complete

---

## [4.1.0] - 2026-01-29

### Added

- **Auto Strategy Command** - New `/plan-cascade:auto` command for AI-driven automatic strategy selection
  - AI automatically analyzes task description and selects optimal strategy
  - Four strategies supported: direct, hybrid-auto, hybrid-worktree, mega-plan
  - Keyword-based detection (not word count based)
  - No user confirmation required - direct execution
  - Strategy routing to corresponding commands

- **Strategy Detection Rules:**
  | Strategy | Trigger Keywords | Example |
  |----------|------------------|---------|
  | direct | fix, typo, update, simple, single | "Fix the login button styling" |
  | hybrid-auto | implement, create, feature, api | "Implement user authentication" |
  | hybrid-worktree | (feature keywords) + experimental, refactor | "Experimental refactoring of payment module" |
  | mega-plan | platform, system, architecture, 3+ modules | "Build e-commerce platform" |

### Changed

- **Command Prefix Standardization** - All command prefixes updated from `planning-with-files:` to `plan-cascade:`
  - Affects 16 command files: approve, complete, edit, hybrid-auto, hybrid-complete, hybrid-manual, hybrid-status, hybrid-worktree, mega-approve, mega-complete, mega-edit, mega-plan, mega-status, show-dependencies, start, worktree
  - Documentation updated accordingly

- Updated version to 4.1.0
- Updated System-Architecture documentation with Auto Strategy Workflow flowchart
- Updated Plugin-Guide documentation with auto command section

---

## [4.0.1] - 2026-01-29

### Added

- **Bilingual Documentation** - Added Chinese translations for all documentation files
  - `README_zh.md` - Chinese README
  - `docs/Plugin-Guide_zh.md` - Chinese Plugin Guide
  - `docs/System-Architecture_zh.md` - Chinese System Architecture
  - Language switcher links between English and Chinese versions

### Changed

- Renamed plugin from `planning-with-files` to `plan-cascade` in plugin.json
- Updated settings and configuration for clarity

---

## [4.0.0] - 2026-01-29

### Added

- **Standalone CLI Complete** - Independent command-line tool fully functional
  - Simple mode for beginners
  - Expert mode for advanced users
  - Interactive REPL chat mode with session management
  - AI automatic strategy selection based on task analysis

- **Multi-LLM Backend Support** - Support for 5 LLM providers
  - **Claude Max** - No API key required (uses Claude Code)
  - **Claude API** - Direct Anthropic API access
  - **OpenAI** - GPT-4 and other OpenAI models
  - **DeepSeek** - DeepSeek API support
  - **Ollama** - Local model support for offline usage

- **Independent ReAct Engine** - Complete agentic execution
  - Think → Act → Observe loop implementation
  - Tool execution with result parsing
  - Multi-turn conversation support

- **Per-Story Agent Selection** - Specify different agents for different stories
  - `--agent` flag for global agent selection
  - `--impl-agent` for implementation phase
  - Story-level `agent` field in PRD

- **Quality Gates Configuration** - Configurable quality checks
  - TypeCheck, Test, Lint gates
  - Per-project configuration in `agents.json`
  - Automatic retry on failure

- **New CLI Commands:**
  - `plan-cascade agents` - List available agents with status
  - `plan-cascade chat` - Interactive REPL mode
  - `plan-cascade run` - Execute task with strategy selection

### Changed

- **Documentation Restructure** - Split into separate focused guides
  - `docs/Plugin-Guide.md` - Claude Code plugin usage
  - `docs/System-Architecture.md` - Technical architecture details
  - Separate CLI documentation

- Updated orchestrator with quality gate integration
- Enhanced agent executor with fallback chain support

### Technical Details

**Dual-Mode Architecture:**
```
┌─────────────────────────────────────────────────────────────┐
│                     Plan Cascade                             │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   ┌─────────────────────┐     ┌─────────────────────┐       │
│   │    Simple Mode      │     │    Expert Mode      │       │
│   │                     │     │                     │       │
│   │  AI auto-select     │     │  Manual strategy    │       │
│   │  strategy           │     │  selection          │       │
│   └─────────────────────┘     └─────────────────────┘       │
│                                                              │
│                    Shared Core Engine                        │
│   ┌─────────────────────────────────────────────────────┐   │
│   │  PRDGenerator │ Orchestrator │ QualityGate │ ...    │   │
│   └─────────────────────────────────────────────────────┘   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**LLM Backend Selection:**
| Backend | API Key Required | Offline | Best For |
|---------|-----------------|---------|----------|
| Claude Max | No | No | Claude Code users |
| Claude API | Yes | No | Direct API access |
| OpenAI | Yes | No | GPT-4 preference |
| DeepSeek | Yes | No | Cost-effective |
| Ollama | No | Yes | Privacy/offline |

---

## [3.1.0] - 2026-01-28

### Added

- **Multi-Agent Collaboration** - Support for using different AI agents to execute stories
  - Support for Codex, Amp Code, Aider, Cursor CLI, Claude CLI
  - Automatic fallback: CLI agents unavailable → fallback to claude-code
  - Agent wrapper script for unified process management and status tracking
  - Agent monitor for polling status and reading results

- **New Core Modules:**
  - `agent_executor.py` - Agent execution abstraction layer with automatic fallback
  - `agent_monitor.py` - Monitor for checking agent status and reading results
  - `agent-wrapper.py` - Wrapper script for CLI agent execution with proper status tracking

- **New MCP Tools (9):**
  - `get_agent_status` - Get status of running agents
  - `get_available_agents` - List configured agents with availability
  - `set_default_agent` - Set default agent for execution
  - `execute_story_with_agent` - Execute a story with specific agent
  - `get_agent_result` - Get result of completed agent
  - `get_agent_output` - Get output log of agent
  - `wait_for_agent` - Wait for specific agent to complete
  - `stop_agent` - Stop a running CLI agent
  - `check_agents` - Poll all running agents and update status

- **New Commands:**
  - `/plan-cascade:agent-status` - View status of running agents

- **New Configuration:**
  - `agents.json` - Agent configuration file for defining CLI agents

- **Agent Priority Chain:**
  1. `--agent` command argument (highest priority)
  2. `story.agent` field in PRD
  3. `metadata.default_agent` in PRD
  4. `default_agent` in agents.json
  5. `claude-code` (always available fallback)

- **Status Tracking Files:**
  - `.agent-status.json` - Agent running/completed/failed status
  - `.agent-outputs/story-xxx.log` - Agent output logs
  - `.agent-outputs/story-xxx.prompt.txt` - Prompt sent to agent
  - `.agent-outputs/story-xxx.result.json` - Execution result (exit code, success/fail)

### Changed

- Updated `orchestrator.py` to integrate AgentExecutor
- Updated `state_manager.py` with agent status tracking methods
- Updated `execution_tools.py` with agent management tools
- Updated `/hybrid:auto` command to support `--agent` parameter
- Updated `/hybrid:approve` command to support `--agent` parameter
- Updated `/mega:plan` command to support `--prd-agent` and `--story-agent` parameters
- Updated `/hybrid:status` to show agent information
- Enhanced `progress.txt` format to include agent info

### Technical Details

**Agent Wrapper Architecture:**
```
Main Session (Claude Code)
    │
    ├── AgentExecutor.execute_story()
    │       │
    │       ▼
    │   agent-wrapper.py (background process)
    │       │
    │       ├── Write .agent-status.json [START]
    │       ├── Execute CLI agent (codex/amp/aider)
    │       ├── Capture stdout/stderr → .agent-outputs/story-xxx.log
    │       ├── Monitor exit code
    │       ├── Write .agent-outputs/story-xxx.result.json
    │       ├── Update .agent-status.json [COMPLETE/FAILED]
    │       └── Append progress.txt
    │
    └── AgentMonitor
            │
            ├── check_running_agents() → Poll PIDs, read result files
            ├── get_agent_result() → Read result.json
            └── wait_for_completion() → Block until done
```

**Supported Agents:**
| Agent | Type | Description |
|-------|------|-------------|
| `claude-code` | task-tool | Claude Code Task tool (built-in, always available) |
| `codex` | cli | OpenAI Codex CLI |
| `amp-code` | cli | Amp Code CLI |
| `aider` | cli | Aider AI pair programming |
| `cursor-cli` | cli | Cursor CLI |
| `claude-cli` | cli | Claude CLI (standalone) |

---

## [3.0.0] - 2026-01-28

### Added

- **MCP Server Support** - Full MCP (Model Context Protocol) server for integration with MCP-compatible tools
  - Support for Cursor, Windsurf, Cline, Continue, Zed, Amp Code
  - 18 MCP tools across three layers (Mega Plan, PRD, Execution)
  - 8 MCP resources for reading planning state
  - 2 MCP prompts for common workflows
  - Both stdio and SSE transport support

- **MCP Tools:**
  - **Mega Plan Tools (6):** `mega_generate`, `mega_add_feature`, `mega_validate`, `mega_get_batches`, `mega_update_feature_status`, `mega_get_merge_plan`
  - **PRD Tools (6):** `prd_generate`, `prd_add_story`, `prd_validate`, `prd_get_batches`, `prd_update_story_status`, `prd_detect_dependencies`
  - **Execution Tools (6):** `get_story_context`, `get_execution_status`, `append_findings`, `mark_story_complete`, `get_progress`, `cleanup_locks`

- **MCP Resources:**
  - `plan-cascade://prd` - Current PRD
  - `plan-cascade://mega-plan` - Current mega-plan
  - `plan-cascade://findings` - Development findings
  - `plan-cascade://progress` - Progress timeline
  - `plan-cascade://mega-status` - Mega-plan execution status
  - `plan-cascade://mega-findings` - Project-level findings
  - `plan-cascade://story/{story_id}` - Specific story details
  - `plan-cascade://feature/{feature_id}` - Specific feature details

- **Configuration Examples:**
  - `mcp-configs/` directory with ready-to-use config files
  - Platform-specific examples (Windows, macOS, Linux)
  - Virtual environment configuration example
  - Setup scripts for automated configuration (`setup-mcp.sh`, `setup-mcp.ps1`)

### Changed

- Updated README with MCP server documentation
- Updated version to 3.0.0 (major version bump for MCP support)
- Reorganized project structure to include `mcp_server/` and `mcp-configs/`

### Removed

- Removed outdated documentation for unsupported tools (Antigravity, Codex, Factory, etc.)
- Removed duplicate `.agent/skills/` directory (consolidated to `skills/`)
- Removed duplicate `templates/` directory (use `skills/planning-with-files/templates/`)
- Removed `examples/` directory (outdated for current architecture)
- Cleaned up legacy files: `DISCUSSION_2_CONTENT.md`, `WIKI_HOME_PAGE.md`, `MIGRATION.md`, `CONTRIBUTORS.md`

### Technical Details

**MCP Server Architecture:**
```
mcp_server/
├── server.py           # FastMCP main entry point
├── resources.py        # MCP resources (8 total)
└── tools/
    ├── prd_tools.py    # PRD layer tools (6)
    ├── mega_tools.py   # Mega Plan layer tools (6)
    └── execution_tools.py  # Execution layer tools (6)
```

**Supported IDE Configurations:**
| Tool | Config Location |
|------|-----------------|
| Cursor | `.cursor/mcp.json` |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` |
| Cline | VS Code `settings.json` |
| Continue | `~/.continue/config.json` |
| Zed | `~/.config/zed/settings.json` |
| Claude Code | `claude mcp add` command |

**Cross-Tool Compatibility:**
- Files generated by MCP tools compatible with Claude Code plugin
- Files generated by Claude Code readable by MCP resources
- Seamless switching between tools during development

---

## [2.8.0] - 2026-01-28

### Project Renamed

- **planning-with-files → Plan Cascade**
- New name reflects the three-layer cascaded architecture
- Forked from [OthmanAdi/planning-with-files](https://github.com/OthmanAdi/planning-with-files) v2.7.1

### Added

- **Mega Plan: Project-Level Multi-Feature Orchestration** - A new three-layer architecture for managing complex projects
  - **Level 1: Mega Plan** - Project-level orchestration of multiple features
  - **Level 2: Features** - Each feature runs as a `hybrid:worktree` task
  - **Level 3: Stories** - Internal parallelism within each feature

- **New Commands:**
  - `/mega:plan <description>` - Generate a mega-plan from project description, breaking it into features with dependencies
  - `/mega:edit` - Edit the mega-plan interactively
  - `/mega:approve [--auto-prd]` - Approve mega-plan and start execution; `--auto-prd` skips PRD review
  - `/mega:status` - Show comprehensive project progress with visual progress bars
  - `/mega:complete [target-branch]` - Merge all features in dependency order and clean up

- **Core Modules:**
  - `mega_generator.py` - Generate and validate mega-plans, calculate feature batches
  - `mega_state.py` - Thread-safe state management with file locking
  - `feature_orchestrator.py` - Create worktrees, generate PRDs, manage batch execution
  - `merge_coordinator.py` - Coordinate dependency-ordered merging and cleanup

- **Shared Findings Mechanism:**
  - `mega-findings.md` at project root for cross-feature discoveries
  - Read-only copies automatically placed in each feature worktree
  - Feature-specific findings remain independent in each worktree

- **Dependency-Driven Batch Execution:**
  - Automatic calculation of feature dependencies
  - Features without dependencies execute in parallel
  - Dependent features wait for their dependencies to complete

### Technical Details

**Feature Status Flow:**
```
pending → prd_generated → approved → in_progress → complete
                                           ↓
                                        failed
```

**PRD Approval Modes:**
| Mode | Command | Behavior |
|------|---------|----------|
| Manual | `/mega:approve` | Review and `/approve` in each worktree |
| Auto | `/mega:approve --auto-prd` | PRDs auto-approved, immediate execution |

**File Structure:**
```
project-root/
├── mega-plan.json              # Project-level plan
├── mega-findings.md            # Shared findings
├── .mega-status.json           # Execution status
└── .worktree/
    ├── feature-auth/           # Feature 1 worktree
    │   ├── prd.json
    │   ├── findings.md         # Feature-specific
    │   └── mega-findings.md    # Read-only copy
    └── feature-products/       # Feature 2 worktree
```

---

## [2.7.11] - 2026-01-27

### Added

- **Operating System Detection** - Auto-detect OS and use appropriate shell (bash/PowerShell)
  - Commands now detect Linux/macOS/Windows environments automatically
  - Choose bash for Unix-like systems, PowerShell for Windows
  - Ensures commands use correct syntax for detected shell
  - Applied to both `hybrid-worktree` and `approve` commands

- **Intelligent Settings Merge** - New `scripts/ensure-settings.py` for smart configuration management
  - Checks for existing `.claude/settings.local.json` before creating new one
  - Merges required auto-approval patterns with user's existing configuration
  - Preserves custom user settings while ensuring Hybrid Ralph patterns are present
  - Non-destructive: adds missing patterns without overwriting existing ones
  - Runs automatically during `hybrid-worktree` and `approve` command execution

### Changed

- Updated `hybrid-worktree.md` with OS detection (Step 2) and settings merge (Step 3)
- Updated `approve.md` with OS detection (Step 1) and settings merge (Step 2)
- Settings file now created via Python script for intelligent merging
- All renumbered steps in commands after adding new detection steps

### Technical Details

**OS Detection Pattern:**
```bash
OS_TYPE="$(uname -s 2>/dev/null || echo Windows)"
case "$OS_TYPE" in
    Linux*|Darwin*|MINGW*|MSYS*) SHELL_TYPE="bash" ;;
    *) SHELL_TYPE="powershell" ;;
esac
```

**Settings Merge Pattern:**
- Read existing settings if present
- Compare with required patterns list
- Add only missing patterns
- Preserve all user customizations
- Write back merged configuration

---

## [2.7.10] - 2026-01-27

### Added

- **Command Auto-Approval Configuration** - Added `.claude/settings.local.json` for seamless automated workflow
  - Eliminates manual confirmation prompts for safe development commands
  - Auto-approves: `git *`, `cd *`, `pwd`, `ls *`, `find *`, `grep *`, `cat *`, `head *`, `tail *`, `mkdir *`, `echo *`, `python3 *`, `node *`, `npm *`, `cp *`
  - Windows PowerShell commands also supported: `dir`, `type`, `chdir`, `copy`
  - Users can customize patterns to add their own safe commands
  - Critical for unattended hybrid-worktree and approve command execution

### Changed

- Updated README with "命令自动批准配置" section documenting the new settings file
- Enhanced quick start guide with auto-approval configuration instructions

---

## [2.7.9] - 2026-01-27

### Fixed

- **Worktree Directory Path Issue** - Fixed `hybrid-worktree` mode using relative path instead of absolute path
  - Previously: `WORKTREE_DIR=".worktree/$(basename $TASK_NAME)"` (relative)
  - Now: `WORKTREE_DIR="$ROOT_DIR/.worktree/$(basename $TASK_NAME)"` (absolute)
  - Prevents planning files from being created in project root directory
  - Ensures all planning files are correctly placed in worktree directory
  - Variables re-ordered to define `ROOT_DIR` before `WORKTREE_DIR`

### Added

- **Execution Mode Selection** - Choose between Auto and Manual batch progression in hybrid mode
  - Auto Mode (default): Batches progress automatically when successful, pause only on errors
  - Manual Mode: Require approval before each batch starts for full control
  - User prompted at start: `[1] Auto Mode` or `[2] Manual Mode`
  - Best for: Auto (routine tasks), Manual (critical/complex tasks)

### Changed

- Updated plugin version to 2.7.9
- Updated commands/approve.md with mode selection dialog
- Split execution logic into Auto/Manual paths in Step 7
- Both modes pause on `[ERROR]` or `[FAILED]` markers from agents
- Auto mode:无缝流转 between batches without intervention
- Manual mode: Explicit confirmation before launching each batch

### Technical Details

**Auto Mode Flow:**
```
Batch 1 → Monitor → Success? → Auto-launch Batch 2 → Monitor → Success? → ...
                ↓ Error? → Pause → Fix → Resume
```

**Manual Mode Flow:**
```
Batch 1 → Monitor → Success? → Prompt user → Confirm? → Launch Batch 2
                ↓ Error? → Pause → Fix → Resume
```

---

## [2.7.8] - 2026-01-26

### Fixed

- **Directory-Aware Worktree Completion** - Both `/planning-with-files:complete` and `/planning-with-files:hybrid-complete` now work from any directory
  - Commands previously required being inside the worktree directory
  - Now automatically scan for available worktrees and prompt user to select
  - Intelligently detects planning vs hybrid mode worktrees
  - Auto-navigates to selected worktree before proceeding with completion
  - Prevents accidental data loss from running completion from wrong directory

### Changed

- Updated plugin version to 2.7.8
- Updated hybrid-ralph skill version to 2.7.8 in all IDE locations
- Updated command descriptions to reflect "can be run from any directory" capability

---

## [2.7.7] - 2026-01-26

### Added

- **Claude Code Command Definitions** (NEW)
  - Created 8 new command definition files in `commands/` directory
  - All hybrid-ralph functionality now discoverable in Claude Code
  - Commands: `/planning-with-files:hybrid-auto`, `/planning-with-files:hybrid-manual`, `/planning-with-files:hybrid-worktree`, `/planning-with-files:approve`, `/planning-with-files:edit`, `/planning-with-files:hybrid-status`, `/planning-with-files:show-dependencies`, `/planning-with-files:hybrid-complete`

### Fixed

- **Command Discovery** - Claude Code only discovers commands from `commands/` directory, not from `skills/` subdirectories
  - All hybrid-ralph commands now properly accessible in Claude Code
  - Fixed issue where hybrid functionality was not available after plugin installation

### Changed

- Updated plugin version to 2.7.7
- Updated hybrid-ralph skill version to 2.7.7 in all IDE locations

---

## [2.7.6] - 2026-01-26

### Fixed

- **Plugin Validation Error** (Critical)
  - Removed invalid `skills` field from `.claude-plugin/plugin.json`
  - The `skills` field is not part of Claude Code's plugin manifest format
  - Plugin now installs correctly without validation errors
  - All hybrid-ralph functionality remains fully available

### Changed

- Updated plugin version to 2.7.6
- Updated skill version to 2.7.6 in all SKILL.md files

### Notes

- Hybrid Ralph skill is fully functional and available in all IDE locations
- Commands `/hybrid:auto`, `/hybrid:manual`, `/hybrid:worktree`, `/approve`, `/edit`, `/status`, `/show-dependencies`, `/hybrid:complete` all work as expected

---

## [2.7.5] - 2026-01-26

### Added

- **Hybrid Ralph + Planning-with-Files** (NEW)
  - PRD-based parallel story execution with dependency resolution
  - Auto-generates PRDs from task descriptions using `/hybrid:auto`
  - Manages parallel execution of user stories with Claude Code Task tool
  - Context-filtered agents receive only relevant information per story
  - Complete orchestration system for multi-story development

### New Skills

- **`hybrid-ralph`** - Combines Ralph's PRD format with Planning-with-Files' structured approach
  - `/hybrid:auto <description>` - Generate PRD from task description
  - `/hybrid:manual [path]` - Load existing PRD file
  - `/hybrid:worktree <name> [branch] [desc]` - Create worktree + PRD in one command
  - `/approve` - Approve PRD and begin execution
  - `/edit` - Edit PRD in default editor
  - `/status` - Show execution status of all stories
  - `/show-dependencies` - Display dependency graph and analysis
  - `/hybrid:complete [branch]` - Complete worktree task and merge

### New Core Modules

- `context_filter.py` - Filter findings by tags/dependencies for specific stories
- `state_manager.py` - Thread-safe file operations with platform-specific locking
- `prd_generator.py` - Generate PRDs from descriptions, manage story dependencies
- `orchestrator.py` - Manage parallel execution of stories with batch coordination

### New Scripts

- `prd-validate.py` - Validate PRD structure and display review
- `status.py` - Monitor execution status of all stories
- `show-dependencies.py` - Visualize dependency graph with analysis
- `agent-exec.py` - Helper for agents executing individual stories
- `prd-generate.py` - Generate PRD template from description
- `hybrid-worktree-init.sh` / `.ps1` - Initialize worktree + hybrid mode
- `hybrid-worktree-complete.sh` / `.ps1` - Complete worktree task and merge

### New Templates

- `templates/prd_review.md` - PRD review display format
- `templates/prd.json.example` - Complete PRD structure example

### Key Features

- **Automatic PRD Generation**: Describe your task, get structured user stories
- **Dependency Resolution**: Stories automatically organized into execution batches
- **Parallel Execution**: Independent stories run simultaneously
- **Context Filtering**: Each agent gets only relevant context
- **Progress Tracking**: Real-time status monitoring with `/status`
- **File Locking**: Concurrent access safety across platforms (fcntl/msvcrt)
- **Worktree Integration**: `/hybrid:worktree` combines Git worktree with PRD mode for isolated parallel tasks

### Changed

- Updated plugin version to 2.7.6
- Updated skill version to 2.7.6
- Added "prd", "ralph", "hybrid", "orchestration", "story-execution" keywords
- Updated plugin.json with skills metadata
- Updated README.md with hybrid-ralph documentation

### Architecture

Hybrid Ralph combines three approaches:
- **Ralph**: PRD format (prd.json), progress.txt pattern, small task philosophy
- **Planning-with-Files**: 3-file planning pattern, Git Worktree support
- **Claude Code Native**: Task tool with subagents for parallel execution

---

## [2.7.3] - 2026-01-23

### Changed

- **Worktree mode is now optional** - Clarified that standard mode is the default and recommended approach
- Standard mode uses no extra disk space (planning files in project root)
- Worktree mode should only be used when multi-task parallel development is needed
- Updated documentation to clearly distinguish between standard mode (default) and worktree mode (optional)

---

## [2.7.2] - 2026-01-23

### Added

- **Git Worktree Mode** - True multi-task parallel development using Git worktrees
  - New `/planning-with-files:worktree` command to create isolated worktree directories
  - New `/planning-with-files:complete` command to merge and cleanup worktrees
  - Uses `git worktree add` to create separate working directories per task
  - **Multiple tasks can run simultaneously without conflicts**
  - **Main directory stays on original branch** (no branch switching)
  - Task branch format: `task-YYYY-MM-DD-HHMM` (time included for uniqueness)
  - Auto-detects default branch (main/master) as merge target
  - Creates `.planning-config.json` in each worktree with task metadata
  - Bash and PowerShell scripts for cross-platform support

### Key Features

- **Parallel Development**: Create multiple worktrees for different tasks
  - Example: `.worktree/fix-auth-bug/`, `.worktree/refactor-api/`, `.worktree/update-docs/`
  - Each worktree has its own branch, files, and planning documents
  - Work on multiple tasks in parallel without switching branches
- **Isolated Environments**: Each task has complete file isolation
- **Easy Cleanup**: Complete command removes worktree and merges changes
- **No Main Directory Impact**: Main directory remains untouched on original branch

### New Commands

- `commands/worktree.md` - Start worktree mode with branch creation
- `commands/complete.md` - Complete task and merge branch

### New Scripts

- `scripts/worktree-init.sh` - Bash script for worktree initialization
- `scripts/worktree-init.ps1` - PowerShell script for worktree initialization
- `scripts/worktree-complete.sh` - Bash script for worktree completion
- `scripts/worktree-complete.ps1` - PowerShell script for worktree completion

### Changed

- Updated plugin version to 2.7.2
- Updated skill version to 2.7.2
- Added "worktree" and "git" keywords to plugin.json
- Updated SKILL.md with worktree mode documentation
- Distributed new commands and scripts to all IDE skill directories
