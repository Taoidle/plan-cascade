# Planning with Files

> **Work like Manus** — the AI agent company Meta acquired for **$2 billion**.

## Thank You

To everyone who starred, forked, and shared this skill — thank you. This project blew up in less than 24 hours, and the support from the community has been incredible.

If this skill helps you work smarter, that's all I wanted.

---

A Claude Code plugin that transforms your workflow to use persistent markdown files for planning, progress tracking, and knowledge storage — the exact pattern that made Manus worth billions.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Claude Code Plugin](https://img.shields.io/badge/Claude%20Code-Plugin-blue)](https://code.claude.com/docs/en/plugins)
[![Claude Code Skill](https://img.shields.io/badge/Claude%20Code-Skill-green)](https://code.claude.com/docs/en/skills)
[![Cursor Skills](https://img.shields.io/badge/Cursor-Skills-purple)](https://docs.cursor.com/context/skills)
[![Kilocode Skills](https://img.shields.io/badge/Kilocode-Skills-orange)](https://kilo.ai/docs/agent-behavior/skills)
[![Gemini CLI](https://img.shields.io/badge/Gemini%20CLI-Skills-4285F4)](https://geminicli.com/docs/cli/skills/)
[![Version](https://img.shields.io/badge/version-2.7.5-brightgreen)](https://github.com/OthmanAdi/planning-with-files/releases)
[![SkillCheck Validated](https://img.shields.io/badge/SkillCheck-Validated-4c1)](https://getskillcheck.com)

## Quick Install

```bash
# Install the plugin
claude plugins install OthmanAdi/planning-with-files
```

That's it! Now use `/planning-with-files:start` in Claude Code.

**Alternative:** If you want `/planning-with-files` (without `:start`), copy skills to your local folder:

```bash
# Optional: Copy skills for /planning-with-files command
cp -r ~/.claude/plugins/cache/planning-with-files/planning-with-files/*/skills/planning-with-files ~/.claude/skills/
```

**Windows (PowerShell):**
```powershell
# Install the plugin
claude plugins install OthmanAdi/planning-with-files

# Optional: Copy skills for /planning-with-files command
Copy-Item -Recurse -Path "$env:USERPROFILE\.claude\plugins\cache\planning-with-files\planning-with-files\*\skills\planning-with-files" -Destination "$env:USERPROFILE\.claude\skills\"
```

See [docs/installation.md](docs/installation.md) for all installation methods.

## Supported IDEs

| IDE | Status | Installation Guide | Format |
|-----|--------|-------------------|--------|
| Claude Code | ✅ Full Support | [Installation](docs/installation.md) | Plugin + SKILL.md |
| Gemini CLI | ✅ Full Support | [Gemini Setup](docs/gemini.md) | Agent Skills |
| Cursor | ✅ Full Support | [Cursor Setup](docs/cursor.md) | Skills |
| Kilocode | ✅ Full Support | [Kilocode Setup](docs/kilocode.md) | Skills |
| OpenCode | ✅ Full Support | [OpenCode Setup](docs/opencode.md) | Personal/Project Skill |
| Codex | ✅ Full Support | [Codex Setup](docs/codex.md) | Personal Skill |
| FactoryAI Droid | ✅ Full Support | [Factory Setup](docs/factory.md) | Workspace/Personal Skill |
| Antigravity | ✅ Full Support | [Antigravity Setup](docs/antigravity.md) | Workspace/Personal Skill |

> **Note:** If your IDE uses the legacy Rules system instead of Skills, see the [`legacy-rules-support`](https://github.com/OthmanAdi/planning-with-files/tree/legacy-rules-support) branch.

## Documentation

| Document | Description |
|----------|-------------|
| [Installation Guide](docs/installation.md) | All installation methods (plugin, manual, Cursor, Windows) |
| [Quick Start](docs/quickstart.md) | 5-step guide to using the pattern |
| [Workflow Diagram](docs/workflow.md) | Visual diagram of how files and hooks interact |
| [Troubleshooting](docs/troubleshooting.md) | Common issues and solutions |
| [Gemini CLI Setup](docs/gemini.md) | Google Gemini CLI integration guide |
| [Cursor Setup](docs/cursor.md) | Cursor IDE-specific instructions |
| [Windows Setup](docs/windows.md) | Windows-specific notes |
| [Kilo Code Support](docs/kilocode.md) | Kilo Code integration guide |
| [Codex Setup](docs/codex.md) | Codex IDE installation and usage |
| [OpenCode Setup](docs/opencode.md) | OpenCode IDE installation, oh-my-opencode config |
| [FactoryAI Droid Setup](docs/factory.md) | FactoryAI Droid integration guide |
| [Antigravity Setup](docs/antigravity.md) | Antigravity IDE integration guide |

## Versions

| Version | Features | Install |
|---------|----------|---------|
| **v2.7.5** (current) | Hybrid Ralph: PRD-based parallel story execution | `claude plugins install OthmanAdi/planning-with-files` |
| **v2.7.2** | Worktree mode support | `claude plugins install OthmanAdi/planning-with-files` |
| **v2.7.1** | Dynamic Python detection fix | `claude plugins install OthmanAdi/planning-with-files` |
| **v2.7.0** | Gemini CLI support | See [releases](https://github.com/OthmanAdi/planning-with-files/releases) |
| **v2.6.0** | Start command (`/planning-with-files:start`), path resolution fix | See [releases](https://github.com/OthmanAdi/planning-with-files/releases) |
| **v2.5.0** | Fixed autocomplete - SKILL.md matches Anthropic format | See [releases](https://github.com/OthmanAdi/planning-with-files/releases) |
| **v2.3.0** | Codex & OpenCode IDE support | See [releases](https://github.com/OthmanAdi/planning-with-files/releases) |
| **v2.2.2** | Restored skill activation language | See [releases](https://github.com/OthmanAdi/planning-with-files/releases) |
| **v2.2.1** | Session recovery after /clear, enhanced PreToolUse hook | See [releases](https://github.com/OthmanAdi/planning-with-files/releases) |
| **v2.2.0** | Kilo Code IDE support, Windows PowerShell support, OS-aware hooks | See [releases](https://github.com/OthmanAdi/planning-with-files/releases) |
| **v2.1.2** | Fix template cache issue (Issue #18) | See [releases](https://github.com/OthmanAdi/planning-with-files/releases) |
| **v2.1.0** | Claude Code v2.1 compatible, PostToolUse hook, user-invocable | See [releases](https://github.com/OthmanAdi/planning-with-files/releases) |
| **v2.0.x** | Hooks, templates, scripts | See [releases](https://github.com/OthmanAdi/planning-with-files/releases) |
| **v1.0.0** (legacy) | Core 3-file pattern | `git clone -b legacy` |

See [CHANGELOG.md](CHANGELOG.md) for details.

## Why This Skill?

On December 29, 2025, [Meta acquired Manus for $2 billion](https://techcrunch.com/2025/12/29/meta-just-bought-manus-an-ai-startup-everyone-has-been-talking-about/). In just 8 months, Manus went from launch to $100M+ revenue. Their secret? **Context engineering**.

> "Markdown is my 'working memory' on disk. Since I process information iteratively and my active context has limits, Markdown files serve as scratch pads for notes, checkpoints for progress, building blocks for final deliverables."
> — Manus AI

## The Problem

Claude Code (and most AI agents) suffer from:

- **Volatile memory** — TodoWrite tool disappears on context reset
- **Goal drift** — After 50+ tool calls, original goals get forgotten
- **Hidden errors** — Failures aren't tracked, so the same mistakes repeat
- **Context stuffing** — Everything crammed into context instead of stored

## The Solution: 3-File Pattern

For every complex task, create THREE files:

```
task_plan.md      → Track phases and progress
findings.md       → Store research and findings
progress.md       → Session log and test results
```

### The Core Principle

```
Context Window = RAM (volatile, limited)
Filesystem = Disk (persistent, unlimited)

→ Anything important gets written to disk.
```

## Usage

Once installed, Claude will automatically:

1. **Create `task_plan.md`** before starting complex tasks
2. **Re-read plan** before major decisions (via PreToolUse hook)
3. **Remind you** to update status after file writes (via PostToolUse hook)
4. **Store findings** in `findings.md` instead of stuffing context
5. **Log errors** for future reference
6. **Verify completion** before stopping (via Stop hook)

Or invoke manually with `/planning-with-files:start` (or `/planning-with-files` if you copied skills).

See [docs/quickstart.md](docs/quickstart.md) for the full 5-step guide.

## Hybrid Ralph: PRD-Based Parallel Story Execution (NEW in v2.7.5)

A new skill that combines Ralph's PRD format with Planning-with-Files' structured approach. Auto-generates PRDs from task descriptions and manages parallel story execution with dependency resolution.

### Quick Start

Generate a PRD from your task description:

```
/hybrid:auto Implement a user authentication system with login, registration, and password reset
```

This will:
1. Analyze your task and generate a structured PRD
2. Break it into user stories with priorities and dependencies
3. Show the PRD for your review
4. Wait for approval before executing

### Key Features

- **Automatic PRD Generation**: Describe your task, get structured user stories
- **Dependency Resolution**: Stories automatically organized into execution batches
- **Parallel Execution**: Independent stories run simultaneously using background Task agents
- **Context Filtering**: Each agent receives only relevant context for their story
- **Progress Tracking**: Real-time status monitoring with `/status`
- **Visual Dependency Graph**: See story relationships with `/show-dependencies`

### Commands

| Command | Description |
|---------|-------------|
| `/hybrid:auto <description>` | Generate PRD from task description |
| `/hybrid:manual [path]` | Load existing PRD file |
| `/hybrid:worktree <name> [branch] [desc]` | Create worktree + PRD (isolated parallel tasks) |
| `/approve` | Approve PRD and begin execution |
| `/edit` | Edit PRD in your default editor |
| `/status` | Show execution status of all stories |
| `/show-dependencies` | Display dependency graph |
| `/hybrid:complete [branch]` | Complete worktree task and merge |

### How It Works

1. **PRD Creation**: Either auto-generate from description or load existing PRD
2. **Review**: Review the generated stories, dependencies, and execution plan
3. **Approval**: Approve to start parallel execution
4. **Batch Execution**: Stories execute in parallel batches based on dependencies
5. **Monitoring**: Track progress with `/status`

### Worktree + Hybrid Mode (Isolated Parallel Tasks)

For multi-task parallel development with complete isolation:

```
# Create worktree + PRD in one command
/hybrid:worktree feature-auth main "Implement user authentication"

# This automatically:
# 1. Creates .worktree/feature-auth/ directory
# 2. Creates task branch (task-YYYYMMDD-HHMM)
# 3. Generates PRD from description
# 4. Initializes planning files

# Work in the isolated environment
cd .worktree/feature-auth
/approve

# Complete and merge when done
/hybrid:complete main
```

**Advantages of Worktree + Hybrid:**
- Complete branch isolation
- Main directory untouched
- Multiple parallel tasks possible
- Automatic merge on completion
- Easy to discard if needed

### Example Workflow

**Standard Mode (Single Task):**
```
# 1. Generate PRD
/hybrid:auto Build a REST API with user CRUD operations

# 2. Review the output (shows stories, dependencies, execution plan)

# 3. Approve to start
/approve

# 4. Monitor progress
/status

# 5. Check dependencies
/show-dependencies
```

**Worktree Mode (Multiple Parallel Tasks):**
```
# Terminal 1: Start authentication feature
/hybrid:worktree feature-auth main "Implement user auth"
cd .worktree/feature-auth
/approve

# Terminal 2: Start API refactoring (parallel!)
/hybrid:worktree refactor-api main "Refactor API endpoints"
cd .worktree/refactor-api
/approve

# Each worktree has its own branch, PRD, and execution context
# Complete when done:
cd .worktree/feature-auth
/hybrid:complete main
```

### File Structure

```
project-root/
├── prd.json              # Product Requirements Document
├── findings.md           # Research findings (tagged by story)
├── progress.txt          # Progress tracking
└── .agent-outputs/       # Individual agent logs
```

### PRD Format

The `prd.json` contains:
- **Goal**: One-sentence project objective
- **Objectives**: List of specific objectives
- **Stories**: User stories with:
  - ID, title, description
  - Priority (high/medium/low)
  - Dependencies on other stories
  - Acceptance criteria
  - Context size estimate
  - Tags for categorization

### Parallel Execution Model

Stories are organized into batches:
- **Batch 1**: Stories with no dependencies (run in parallel)
- **Batch 2+**: Stories whose dependencies are complete

Example:
```
Batch 1 (Parallel):
  - story-001: Design database schema
  - story-002: Design API endpoints

Batch 2 (After story-001 complete):
  - story-003: Implement database schema

Batch 3 (After story-002, story-003 complete):
  - story-004: Implement API endpoints
```

### When to Use Hybrid Ralph

**Use for:**
- Multi-story projects with clear dependencies
- Tasks that can be broken into independent units
- Projects requiring parallel execution
- Complex features with multiple components

**Use Worktree + Hybrid for:**
- Multiple parallel tasks requiring isolation
- Features that need separate branches
- Team collaboration on different features
- Experimental work you might discard

**Stick to standard planning-with-files for:**
- Single-phase tasks
- Simple implementations
- Quick fixes and tweaks

### Integration with Standard Mode

Hybrid Ralph integrates seamlessly with planning-with-files:
- Can be used alongside `task_plan.md`, `findings.md`, `progress.md`
- Compatible with worktree mode for isolated parallel tasks
- Uses the same session recovery mechanism

### See Also

- [CHANGELOG.md](CHANGELOG.md#275---2026-01-26) - Detailed v2.7.5 release notes
- `skills/hybrid-ralph/SKILL.md` - Complete skill documentation

---

## Session Recovery (NEW in v2.2.0)

When your context window fills up and you run `/clear`, this skill automatically recovers unsynced work from your previous session.

### Optimal Workflow

For the best experience, we recommend:

1. **Disable auto-compact** in Claude Code settings (use full context window)
2. **Start a fresh session** in your project
3. **Run `/planning-with-files`** when ready to work on a complex task
4. **Work until context fills up** (Claude will warn you)
5. **Run `/clear`** to start fresh
6. **Run `/planning-with-files`** again — it will automatically recover where you left off

### How Recovery Works

When you invoke `/planning-with-files`, the skill:

1. Checks for previous session data (stored in `~/.claude/projects/`)
2. Finds the last time planning files were updated
3. Extracts conversation that happened after (potentially lost context)
4. Shows a catchup report so you can sync planning files

This means even if context filled up before you could update your planning files, the skill will recover that context in your next session.

### Disabling Auto-Compact

To use the full context window without automatic compaction:

```bash
# In your Claude Code settings or .claude/settings.json
{
  "autoCompact": false
}
```

This lets you maximize context usage before manually clearing with `/clear`.

## Key Rules

1. **Create Plan First** — Never start without `task_plan.md`
2. **The 2-Action Rule** — Save findings after every 2 view/browser operations
3. **Log ALL Errors** — They help avoid repetition
4. **Never Repeat Failures** — Track attempts, mutate approach

## File Structure

```
planning-with-files/
├── commands/                # Plugin commands
│   └── start.md             # /planning-with-files:start command
├── templates/               # Root-level templates (for CLAUDE_PLUGIN_ROOT)
├── scripts/                 # Root-level scripts (for CLAUDE_PLUGIN_ROOT)
├── docs/                    # Documentation
│   ├── installation.md
│   ├── quickstart.md
│   ├── workflow.md
│   ├── troubleshooting.md
│   ├── gemini.md            # Gemini CLI setup
│   ├── cursor.md
│   ├── windows.md
│   ├── kilocode.md
│   ├── codex.md
│   └── opencode.md
├── skills/                  # Skill folders
│   ├── planning-with-files/ # Main planning skill
│   │   ├── SKILL.md
│   │   ├── examples.md
│   │   ├── reference.md
│   │   ├── templates/
│   │   └── scripts/
│   │       ├── init-session.sh
│   │       ├── check-complete.sh
│   │       ├── init-session.ps1   # Windows PowerShell
│   │       └── check-complete.ps1 # Windows PowerShell
│   └── hybrid-ralph/        # NEW: Hybrid Ralph skill (v2.7.5)
│       ├── SKILL.md
│       ├── commands/        # Skill commands
│       │   ├── auto.md
│       │   ├── manual.md
│       │   ├── worktree.md
│       │   ├── complete.md
│       │   ├── approve.md
│       │   ├── edit.md
│       │   ├── status.md
│       │   └── show-dependencies.md
│       ├── core/            # Python modules
│       │   ├── context_filter.py
│       │   ├── state_manager.py
│       │   ├── prd_generator.py
│       │   └── orchestrator.py
│       ├── scripts/         # Helper scripts
│       │   ├── prd-validate.py
│       │   ├── status.py
│       │   ├── show-dependencies.py
│       │   ├── agent-exec.py
│       │   ├── prd-generate.py
│       │   ├── hybrid-worktree-init.sh
│       │   ├── hybrid-worktree-init.ps1
│       │   ├── hybrid-worktree-complete.sh
│       │   └── hybrid-worktree-complete.ps1
│       └── templates/       # PRD templates
│           ├── prd_review.md
│           └── prd.json.example
├── .gemini/                 # Gemini CLI skills
│   └── skills/
│       └── planning-with-files/
├── .codex/                  # Codex IDE skills
│   └── skills/
├── .opencode/               # OpenCode IDE skills
│   └── skills/
├── .claude-plugin/          # Plugin manifest
├── .cursor/                 # Cursor skills
│   └── skills/
├── .kilocode/               # Kilo Code skills
│   └── skills/
├── CHANGELOG.md
├── LICENSE
└── README.md
```

## The Manus Principles

| Principle | Implementation |
|-----------|----------------|
| Filesystem as memory | Store in files, not context |
| Attention manipulation | Re-read plan before decisions (hooks) |
| Error persistence | Log failures in plan file |
| Goal tracking | Checkboxes show progress |
| Completion verification | Stop hook checks all phases |

## When to Use

**Use this pattern for:**
- Multi-step tasks (3+ steps)
- Research tasks
- Building/creating projects
- Tasks spanning many tool calls

**Skip for:**
- Simple questions
- Single-file edits
- Quick lookups

## Kilo Code Support

This skill also supports Kilo Code AI through the `.kilocode/rules/` directory.

The [`.kilocode/rules/planning-with-files.md`](.kilocode/rules/planning-with-files.md) file contains all the planning guidelines formatted for Kilo Code's rules system, providing the same Manus-style planning workflow for Kilo Code users.

**Windows users:** The skill now includes PowerShell scripts ([`init-session.ps1`](skills/planning-with-files/scripts/init-session.ps1) and [`check-complete.ps1`](skills/planning-with-files/scripts/check-complete.ps1)) for native Windows support.

See [docs/kilocode.md](docs/kilocode.md) for detailed Kilo Code integration guide.

## Community Forks

| Fork | Author | Features |
|------|--------|----------|
| [devis](https://github.com/st01cs/devis) | [@st01cs](https://github.com/st01cs) | Interview-first workflow, `/devis:intv` and `/devis:impl` commands, guaranteed activation |
| [multi-manus-planning](https://github.com/kmichels/multi-manus-planning) | [@kmichels](https://github.com/kmichels) | Multi-project support, SessionStart git sync |

*Built something? Open an issue to get listed!*

## Acknowledgments

- **Manus AI** — For pioneering context engineering patterns
- **Anthropic** — For Claude Code, Agent Skills, and the Plugin system
- **Lance Martin** — For the detailed Manus architecture analysis
- Based on [Context Engineering for AI Agents](https://manus.im/blog/Context-Engineering-for-AI-Agents-Lessons-from-Building-Manus)

## Contributing

Contributions welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Submit a pull request

## License

MIT License — feel free to use, modify, and distribute.

---

**Author:** [Ahmad Othman Ammar Adi](https://github.com/OthmanAdi)

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=OthmanAdi/planning-with-files&type=Date)](https://star-history.com/#OthmanAdi/planning-with-files&Date)
