# Changelog

All notable changes to this project will be documented in this file.

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

---

## [2.7.1] - 2026-01-22

### Fixed

- **Dynamic Python Command Detection** (Issue #41 by @wqh17101)
  - Replaced hardcoded `python3` with dynamic detection: `$(command -v python3 || command -v python)`
  - Added Windows PowerShell commands using `python` directly
  - Fixed in all 5 IDE-specific SKILL.md files (Claude Code, Codex, Cursor, Kilocode, OpenCode)
  - Resolves compatibility issues on Windows/Anaconda where only `python` exists

### Thanks

- @wqh17101 for reporting and suggesting the fix (Issue #41)

---

## [2.7.0] - 2026-01-22

### Added

- **Gemini CLI Support** (Issue #52)
  - Native Agent Skills support for Google Gemini CLI v0.23+
  - Created `.gemini/skills/planning-with-files/` directory structure
  - SKILL.md formatted for Gemini CLI compatibility
  - Full templates, scripts, and references included
  - Added `docs/gemini.md` installation guide
  - Added Gemini CLI badge to README

### Documentation

- Updated README with Gemini CLI in supported IDEs table
- Updated file structure diagram
- Added Gemini CLI to documentation table

### Thanks

- @airclear for requesting Gemini CLI support (Issue #52)

---

## [2.6.0] - 2026-01-22

### Added

- **Start Command** (PR #51 by @Guozihong)
  - New `/planning-with-files:start` command for easier activation
  - No longer requires copying skills to `~/.claude/skills/` folder
  - Works directly after plugin installation
  - Added `commands/start.md` file

### Fixed

- **Stop Hook Path Resolution** (PR #49 by @fahmyelraie)
  - Fixed "No such file or directory" error when `CLAUDE_PLUGIN_ROOT` is not set
  - Added fallback path: `$HOME/.claude/plugins/planning-with-files/scripts`
  - Made `check-complete.sh` executable (chmod +x)
  - Applied fix to all IDE-specific SKILL.md files (Codex, Cursor, Kilocode, OpenCode)

### Thanks

- @fahmyelraie for the path resolution fix (PR #49)
- @Guozihong for the start command feature (PR #51)

---

## [2.4.0] - 2026-01-20

### Fixed

- **CRITICAL: Fixed SKILL.md frontmatter to comply with official Agent Skills spec** (Issue #39)
  - Removed invalid `hooks:` field from SKILL.md frontmatter (not supported by spec)
  - Removed invalid top-level `version:` field (moved to `metadata.version`)
  - Removed `user-invocable:` field (not in official spec)
  - Changed `allowed-tools:` from YAML list to space-delimited string per spec
  - This fixes `/planning-with-files` slash command not appearing for users

### Changed

- SKILL.md frontmatter now follows [Agent Skills Specification](https://agentskills.io/specification)
- Version now stored in `metadata.version` field
- Removed `${CLAUDE_PLUGIN_ROOT}` variable references from SKILL.md (use relative paths)
- Updated plugin.json to v2.4.0

### Technical Details

The previous SKILL.md used non-standard frontmatter fields:
```yaml
# OLD (broken)
version: "2.3.0"           # NOT supported at top level
user-invocable: true       # NOT in official spec
hooks:                     # NOT supported in SKILL.md
  PreToolUse: ...
```

Now uses spec-compliant format:
```yaml
# NEW (fixed)
name: planning-with-files
description: ...
license: MIT
metadata:
  version: "2.4.0"
  author: OthmanAdi
allowed-tools: Read Write Edit Bash Glob Grep WebFetch WebSearch
```

### Thanks

- @wqh17101 for identifying the issue in #39
- @dalisoft and @zoffyzhang for reporting the problem

## [2.3.0] - 2026-01-17

### Added

- **Codex IDE Support**
  - Created `.codex/INSTALL.md` with installation instructions
  - Skills install to `~/.codex/skills/planning-with-files/`
  - Works with obra/superpowers or standalone
  - Added `docs/codex.md` for user documentation
  - Based on analysis of obra/superpowers Codex implementation

- **OpenCode IDE Support** (Issue #27)
  - Created `.opencode/INSTALL.md` with installation instructions
  - Global installation: `~/.config/opencode/skills/planning-with-files/`
  - Project installation: `.opencode/skills/planning-with-files/`
  - Works with obra/superpowers plugin or standalone
  - oh-my-opencode compatibility documented
  - Added `docs/opencode.md` for user documentation
  - Based on analysis of obra/superpowers OpenCode plugin

### Changed

- Updated README.md with Supported IDEs table
- Updated README.md file structure diagram
- Updated docs/installation.md with Codex and OpenCode sections
- Version bump to 2.3.0

### Documentation

- Added Codex and OpenCode to IDE support table in README
- Created comprehensive installation guides for both IDEs
- Documented skill priority system for OpenCode
- Documented integration with superpowers ecosystem

### Research

This implementation is based on real analysis of:
- [obra/superpowers](https://github.com/obra/superpowers) repository
- Codex skill system and CLI architecture
- OpenCode plugin system and skill resolution
- Skill priority and override mechanisms

### Thanks

- @Realtyxxx for feedback on Issue #27 about OpenCode support
- obra for the superpowers reference implementation

---

## [2.2.2] - 2026-01-17

### Fixed

- **Restored Skill Activation Language** (PR #34)
  - Restored the activation trigger in SKILL.md description
  - Description now includes: "Use when starting complex multi-step tasks, research projects, or any task requiring >5 tool calls"
  - This language was accidentally removed during the v2.2.1 merge
  - Helps Claude auto-activate the skill when detecting appropriate tasks

### Changed

- Updated version to 2.2.2 in all SKILL.md files and plugin.json

### Thanks

- Community members for catching this issue

---

## [2.2.1] - 2026-01-17

### Added

- **Session Recovery Feature** (PR #33 by @lasmarois)
  - Automatically detect and recover unsynced work from previous sessions after `/clear`
  - New `scripts/session-catchup.py` analyzes previous session JSONL files
  - Finds last planning file update and extracts conversation that happened after
  - Recovery triggered automatically when invoking `/planning-with-files`
  - Pure Python stdlib implementation, no external dependencies

- **PreToolUse Hook Enhancement**
  - Now triggers on Read/Glob/Grep in addition to Write/Edit/Bash
  - Keeps task_plan.md in attention during research/exploration phases
  - Better context management throughout workflow

### Changed

- SKILL.md restructured with session recovery as first instruction
- Description updated to mention session recovery feature
- README updated with session recovery workflow and instructions

### Documentation

- Added "Session Recovery" section to README
- Documented optimal workflow for context window management
- Instructions for disabling auto-compact in Claude Code settings

### Thanks

Special thanks to:
- @lasmarois for session recovery implementation (PR #33)
- Community members for testing and feedback

---

## [2.2.0] - 2026-01-17

### Added

- **Kilo Code Support** (PR #30 by @aimasteracc)
  - Added Kilo Code IDE compatibility for the planning-with-files skill
  - Created `.kilocode/rules/planning-with-files.md` with IDE-specific rules
  - Added `docs/kilocode.md` comprehensive documentation for Kilo Code users
  - Enables seamless integration with Kilo Code's planning workflow

- **Windows PowerShell Support** (Fixes #32, #25)
  - Created `check-complete.ps1` - PowerShell equivalent of bash script
  - Created `init-session.ps1` - PowerShell session initialization
  - Scripts available in all three locations (root, plugin, skills)
  - OS-aware hook execution with automatic fallback
  - Improves Windows user experience with native PowerShell support

- **CONTRIBUTORS.md**
  - Recognizes all community contributors
  - Lists code contributors with their impact
  - Acknowledges issue reporters and testers
  - Documents community forks

### Fixed

- **Stop Hook Windows Compatibility** (Fixes #32)
  - Hook now detects Windows environment automatically
  - Uses PowerShell scripts on Windows, bash on Unix/Linux/Mac
  - Graceful fallback if PowerShell not available
  - Tested on Windows 11 PowerShell and Git Bash

- **Script Path Resolution** (Fixes #25)
  - Improved `${CLAUDE_PLUGIN_ROOT}` handling across platforms
  - Scripts now work regardless of installation method
  - Added error handling for missing scripts

### Changed

- **SKILL.md Hook Configuration**
  - Stop hook now uses multi-line command with OS detection
  - Supports pwsh (PowerShell Core), powershell (Windows PowerShell), and bash
  - Automatic fallback chain for maximum compatibility

- **Documentation Updates**
  - Updated to support both Claude Code and Kilo Code environments
  - Enhanced template compatibility across different AI coding assistants
  - Updated `.gitignore` to include `findings.md` and `progress.md`

### Files Added

- `.kilocode/rules/planning-with-files.md` - Kilo Code IDE rules
- `docs/kilocode.md` - Kilo Code-specific documentation
- `scripts/check-complete.ps1` - PowerShell completion check (root level)
- `scripts/init-session.ps1` - PowerShell session init (root level)
- `planning-with-files/scripts/check-complete.ps1` - PowerShell (plugin level)
- `planning-with-files/scripts/init-session.ps1` - PowerShell (plugin level)
- `skills/planning-with-files/scripts/check-complete.ps1` - PowerShell (skills level)
- `skills/planning-with-files/scripts/init-session.ps1` - PowerShell (skills level)
- `CONTRIBUTORS.md` - Community contributor recognition
- `COMPREHENSIVE_ISSUE_ANALYSIS.md` - Detailed issue research and solutions

### Documentation

- Added Windows troubleshooting guidance
- Recognized community contributors in CONTRIBUTORS.md
- Updated README to reflect Windows and Kilo Code support

### Thanks

Special thanks to:
- @aimasteracc for Kilo Code support and PowerShell script contribution (PR #30)
- @mtuwei for reporting Windows compatibility issues (#32)
- All community members who tested and provided feedback

  - Root cause: `${CLAUDE_PLUGIN_ROOT}` resolves to repo root, but templates were only in subfolders
  - Added `templates/` and `scripts/` directories at repo root level
  - Now templates are accessible regardless of how `CLAUDE_PLUGIN_ROOT` resolves
  - Works for both plugin installs and manual installs

### Structure

After this fix, templates exist in THREE locations for maximum compatibility:
- `templates/` - At repo root (for `${CLAUDE_PLUGIN_ROOT}/templates/`)
- `planning-with-files/templates/` - For plugin marketplace installs
- `skills/planning-with-files/templates/` - For legacy `~/.claude/skills/` installs

### Workaround for Existing Users

If you still experience issues after updating:
1. Uninstall: `/plugin uninstall planning-with-files@planning-with-files`
2. Reinstall: `/plugin marketplace add OthmanAdi/planning-with-files`
3. Install: `/plugin install planning-with-files@planning-with-files`

---

## [2.1.1] - 2026-01-10

### Fixed

- **Plugin Template Path Issue** (Fixes #15)
  - Templates weren't found when installed via plugin marketplace
  - Plugin cache expected `planning-with-files/templates/` at repo root
  - Added `planning-with-files/` folder at root level for plugin installs
  - Kept `skills/planning-with-files/` for legacy `~/.claude/skills/` installs

### Structure

- `planning-with-files/` - For plugin marketplace installs
- `skills/planning-with-files/` - For manual `~/.claude/skills/` installs

---

## [2.1.0] - 2026-01-10

### Added

- **Claude Code v2.1 Compatibility**
  - Updated skill to leverage all new Claude Code v2.1 features
  - Requires Claude Code v2.1.0 or later

- **`user-invocable: true` Frontmatter**
  - Skill now appears in slash command menu
  - Users can manually invoke with `/planning-with-files`
  - Auto-detection still works as before

- **`SessionStart` Hook**
  - Notifies user when skill is loaded and ready
  - Displays message at session start confirming skill availability

- **`PostToolUse` Hook**
  - Runs after every Write/Edit operation
  - Reminds Claude to update `task_plan.md` if a phase was completed
  - Helps prevent forgotten status updates

- **YAML List Format for `allowed-tools`**
  - Migrated from comma-separated string to YAML list syntax
  - Cleaner, more maintainable frontmatter
  - Follows Claude Code v2.1 best practices

### Changed

- Version bumped to 2.1.0 in SKILL.md, plugin.json, and README.md
- README.md updated with v2.1.0 features section
- Versions table updated to reflect new release

### Compatibility

- **Minimum Claude Code Version:** v2.1.0
- **Backward Compatible:** Yes (works with older Claude Code, but new hooks may not fire)

## [2.0.1] - 2026-01-09

### Fixed

- Planning files now correctly created in project directory, not skill installation folder
- Added "Important: Where Files Go" section to SKILL.md
- Added Troubleshooting section to README.md

### Thanks

- @wqh17101 for reporting and confirming the fix

## [2.0.0] - 2026-01-08

### Added

- **Hooks Integration** (Claude Code 2.1.0+)
  - `PreToolUse` hook: Automatically reads `task_plan.md` before Write/Edit/Bash operations
  - `Stop` hook: Verifies all phases are complete before stopping
  - Implements Manus "attention manipulation" principle automatically

- **Templates Directory**
  - `templates/task_plan.md` - Structured phase tracking template
  - `templates/findings.md` - Research and discovery storage template
  - `templates/progress.md` - Session logging with test results template

- **Scripts Directory**
  - `scripts/init-session.sh` - Initialize all planning files at once
  - `scripts/check-complete.sh` - Verify all phases are complete

- **New Documentation**
  - `CHANGELOG.md` - This file

- **Enhanced SKILL.md**
  - The 2-Action Rule (save findings after every 2 view/browser operations)
  - The 3-Strike Error Protocol (structured error recovery)
  - Read vs Write Decision Matrix
  - The 5-Question Reboot Test

- **Expanded reference.md**
  - The 3 Context Engineering Strategies (Reduction, Isolation, Offloading)
  - The 7-Step Agent Loop diagram
  - Critical constraints section
  - Updated Manus statistics

### Changed

- SKILL.md restructured for progressive disclosure (<500 lines)
- Version bumped to 2.0.0 in all manifests
- README.md reorganized (Thank You section moved to top)
- Description updated to mention >5 tool calls threshold

### Preserved

- All v1.0.0 content available in `legacy` branch
- Original examples.md retained (proven patterns)
- Core 3-file pattern unchanged
- MIT License unchanged

## [1.0.0] - 2026-01-07

### Added

- Initial release
- SKILL.md with core workflow
- reference.md with 6 Manus principles
- examples.md with 4 real-world examples
- Plugin structure for Claude Code marketplace
- README.md with installation instructions

---

## Versioning

This project follows [Semantic Versioning](https://semver.org/):
- MAJOR: Breaking changes to skill behavior
- MINOR: New features, backward compatible
- PATCH: Bug fixes, documentation updates
