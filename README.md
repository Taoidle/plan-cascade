# Planning with Files

> **Work like Manus** — the AI agent company Meta just acquired for **$2 billion**.

A Claude Code plugin containing an [Agent Skill](https://code.claude.com/docs/en/skills) that transforms your workflow to use persistent markdown files for planning, progress tracking, and knowledge storage — the exact pattern that made Manus worth billions.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Claude Code Plugin](https://img.shields.io/badge/Claude%20Code-Plugin-blue)](https://code.claude.com/docs/en/plugins)
[![Claude Code Skill](https://img.shields.io/badge/Claude%20Code-Skill-green)](https://code.claude.com/docs/en/skills)

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=OthmanAdi/planning-with-files&type=Date)](https://star-history.com/#OthmanAdi/planning-with-files&Date)

---

## Why This Skill?

On December 29, 2025, [Meta acquired Manus for $2 billion](https://techcrunch.com/2025/12/29/meta-just-bought-manus-an-ai-startup-everyone-has-been-talking-about/). In just 8 months, Manus went from launch to $100M+ revenue. Their secret? **Context engineering**.

This skill implements Manus's core workflow pattern:

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
notes.md          → Store research and findings
[deliverable].md  → Final output
```

### The Loop

```
1. Create task_plan.md with goal and phases
2. Research → save to notes.md → update task_plan.md
3. Read notes.md → create deliverable → update task_plan.md
4. Deliver final output
```

**Key insight:** By reading `task_plan.md` before each decision, goals stay in the attention window. This is how Manus handles ~50 tool calls without losing track.

## Installation

### As a Claude Code Plugin (Recommended)

Install directly using the Claude Code CLI:

```
/plugin marketplace add OthmanAdi/planning-with-files
/plugin install planning-with-files@planning-with-files
```

### Manual Installation

Clone or copy this repository into your project's `.claude/plugins/` directory:

```bash
# Option 1: Clone into plugins directory
mkdir -p .claude/plugins
git clone https://github.com/OthmanAdi/planning-with-files.git .claude/plugins/planning-with-files

# Option 2: Add as git submodule
git submodule add https://github.com/OthmanAdi/planning-with-files.git .claude/plugins/planning-with-files

# Option 3: Use --plugin-dir flag
git clone https://github.com/OthmanAdi/planning-with-files.git
claude --plugin-dir ./planning-with-files
```

### Legacy Installation (Skills Only)

Copy the `skills/` directory contents into your `.claude/skills/` folder:

```bash
git clone https://github.com/OthmanAdi/planning-with-files.git
cp -r planning-with-files/skills/* ~/.claude/skills/
```

### Verify Installation

Once installed, the skill will automatically activate when you:
- Start complex tasks
- Mention "planning", "organize", or "track progress"
- Ask for structured work

## Usage

Once installed, Claude will automatically:

1. **Create `task_plan.md`** before starting complex tasks
2. **Update progress** with checkboxes after each phase
3. **Store findings** in `notes.md` instead of stuffing context
4. **Log errors** for future reference
5. **Re-read plan** before major decisions

### Example

**You:** "Research the benefits of TypeScript and write a summary"

**Claude creates:**

```markdown
# Task Plan: TypeScript Benefits Research

## Goal
Create a research summary on TypeScript benefits.

## Phases
- [x] Phase 1: Create plan ✓
- [ ] Phase 2: Research and gather sources (CURRENT)
- [ ] Phase 3: Synthesize findings
- [ ] Phase 4: Deliver summary

## Status
**Currently in Phase 2** - Searching for sources
```

Then continues through each phase, updating the file as it goes.

## The Manus Principles

This skill implements these key context engineering principles:

| Principle | Implementation |
|-----------|----------------|
| Filesystem as memory | Store in files, not context |
| Attention manipulation | Re-read plan before decisions |
| Error persistence | Log failures in plan file |
| Goal tracking | Checkboxes show progress |
| Append-only context | Never modify history |

## File Structure

```
planning-with-files/
├── .claude-plugin/
│   └── plugin.json         # Plugin manifest (required for plugin distribution)
├── skills/
│   └── planning-with-files/
│       ├── SKILL.md        # Agent Skill definition (what Claude reads)
│       ├── reference.md    # Manus principles deep dive
│       └── examples.md     # Real usage examples
├── LICENSE
└── README.md               # This file
```

> **Note:** This repository is a **Plugin** that contains an **Agent Skill**. The plugin format (`.claude-plugin/`) enables easy distribution and installation, while the skill (`skills/planning-with-files/SKILL.md`) defines the actual behavior that Claude uses.

## When to Use

**Use this pattern for:**
- Multi-step tasks (3+ steps)
- Research tasks
- Building/creating projects
- Tasks spanning many tool calls
- Anything requiring organization

**Skip for:**
- Simple questions
- Single-file edits
- Quick lookups

## Acknowledgments

- **Manus AI** — For pioneering context engineering patterns that made this possible
- **Anthropic** — For Claude Code, the [Agent Skills](https://code.claude.com/docs/en/skills) framework and the [Plugin system](https://code.claude.com/docs/en/plugins)
- Based on [Context Engineering for AI Agents](https://manus.im/de/blog/Context-Engineering-for-AI-Agents-Lessons-from-Building-Manus)

## Contributing

Contributions welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Submit a pull request

## License

MIT License — feel free to use, modify, and distribute.

## Thank You

To everyone who starred, forked, and shared this skill — thank you. This project blew up in less than 24 hours, and the support from the community has been incredible.

If this skill helps you work smarter, that's all I wanted.

---

**Author:** [Ahmad Othman Ammar Adi](https://github.com/OthmanAdi)
