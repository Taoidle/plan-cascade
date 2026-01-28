# Changelog

All notable changes to this project will be documented in this file.

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
