---
name: mega:plan
description: Generate a mega-plan from project description for multi-feature parallel development
arguments:
  - name: description
    description: Project description to generate mega-plan from
    required: true
  - name: prd-agent
    description: Agent to use for PRD generation (e.g., codex, amp-code, claude-code)
    required: false
  - name: story-agent
    description: Default agent for story execution (e.g., codex, amp-code, claude-code)
    required: false
---

# /mega:plan

Generate a project-level mega-plan from a description, breaking it into parallel features.

## Your Task

The user wants to create a mega-plan for: `$ARGUMENTS`

### Step 1: Check for Existing Mega Plan

First, check if a mega-plan already exists:

```bash
ls -la mega-plan.json 2>/dev/null
```

If it exists, ask the user:
- Do they want to overwrite it?
- Or should they use `/mega:edit` instead?

### Step 2: Analyze the Project Description

Analyze the user's project description and break it into 2-6 features. Consider:

1. **Feature Independence**: Each feature should be developable in isolation
2. **Dependencies**: Identify which features depend on others
3. **Granularity**: Features should be significant but not too large
4. **Parallelism**: Maximize features that can run in parallel (no dependencies)

### Step 3: Generate the Mega Plan

Create `mega-plan.json` with this structure:

```json
{
  "metadata": {
    "created_at": "<current ISO timestamp>",
    "version": "1.0.0",
    "prd_agent": "<agent for PRD generation, optional>",
    "default_story_agent": "<default agent for stories, optional>"
  },
  "goal": "<one-sentence project goal>",
  "description": "<original user description>",
  "execution_mode": "auto",
  "target_branch": "main",
  "features": [
    {
      "id": "feature-001",
      "name": "<lowercase-hyphenated-name>",
      "title": "<Human Readable Title>",
      "description": "<detailed description for PRD generation>",
      "priority": "high|medium|low",
      "dependencies": [],
      "status": "pending",
      "prd_agent": "<optional: override agent for this feature's PRD>",
      "story_agent": "<optional: override agent for this feature's stories>"
    }
  ]
}
```

**Agent Configuration:**
- `metadata.prd_agent`: Agent used to generate PRDs for all features
- `metadata.default_story_agent`: Default agent for executing stories
- `feature.prd_agent`: Override PRD generation agent for specific feature
- `feature.story_agent`: Override story execution agent for specific feature

Available agents: `claude-code` (built-in), `codex`, `amp-code`, `aider`, `cursor-cli`, `claude-cli`
```

**Feature Naming Rules:**
- Use lowercase letters, numbers, and hyphens only
- Start with a letter
- Be descriptive but concise (e.g., "feature-auth", "feature-products", "feature-cart")

**Priority Guidelines:**
- `high`: Core functionality, must be done first
- `medium`: Important but can wait for dependencies
- `low`: Nice-to-have or can be deferred

**Dependencies:**
- List feature IDs that must complete before this feature can start
- Keep dependencies minimal to maximize parallelism

### Step 4: Initialize Supporting Files

Create `mega-findings.md`:

```markdown
# Mega Plan Findings

Project: <goal>
Created: <timestamp>

This file contains shared findings across all features.
Feature-specific findings should be in their respective worktrees.

---

## Project-Wide Decisions

<!-- Add project-wide architectural decisions here -->

## Shared Patterns

<!-- Add patterns that apply across features -->

## Integration Notes

<!-- Add notes about how features will integrate -->
```

Create `.mega-status.json`:

```json
{
  "updated_at": "<current ISO timestamp>",
  "execution_mode": "auto",
  "target_branch": "main",
  "current_batch": 0,
  "features": {}
}
```

### Step 5: Validate and Display

Validate the mega-plan using:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/mega-plan/core/mega_generator.py" validate
```

Then display the plan summary:

```
============================================================
MEGA PLAN CREATED
============================================================

Goal: <goal>
Execution Mode: auto
Target Branch: main
Total Features: N

Feature Batches:

Batch 1 (Parallel):
  - feature-001: <title> [high priority]
  - feature-002: <title> [high priority]

Batch 2 (After Batch 1):
  - feature-003: <title> [medium priority] (depends on: feature-001)

Batch 3 (After Batch 2):
  - feature-004: <title> [medium priority] (depends on: feature-003)

============================================================

Files created:
  - mega-plan.json
  - mega-findings.md
  - .mega-status.json

Next steps:
  1. Review the mega-plan: cat mega-plan.json
  2. Edit if needed: /mega:edit
  3. Start execution: /mega:approve
     Or with auto PRD approval: /mega:approve --auto-prd
```

### Step 6: Ask Execution Mode

Ask the user which execution mode they prefer:

**Auto Mode (Recommended for most cases):**
- Feature batches progress automatically
- Story batches within features progress automatically
- Minimal manual intervention

**Manual Mode:**
- Feature batches require confirmation
- Story batches within features require confirmation
- More control but slower

Use AskUserQuestion to let them choose, then update `mega-plan.json` with their choice.

## Example

User: `/mega:plan Build a blog platform with user accounts, post management, comments, and RSS feeds`

Generated mega-plan.json:

```json
{
  "metadata": {
    "created_at": "2026-01-28T10:00:00Z",
    "version": "1.0.0"
  },
  "goal": "Build a blog platform with user accounts, post management, comments, and RSS feeds",
  "description": "Build a blog platform with user accounts, post management, comments, and RSS feeds",
  "execution_mode": "auto",
  "target_branch": "main",
  "features": [
    {
      "id": "feature-001",
      "name": "feature-users",
      "title": "User Account System",
      "description": "Implement user registration, login, profile management, and authentication middleware. Include email verification and password reset functionality.",
      "priority": "high",
      "dependencies": [],
      "status": "pending"
    },
    {
      "id": "feature-002",
      "name": "feature-posts",
      "title": "Post Management",
      "description": "Implement blog post CRUD operations including rich text editor, draft/publish workflow, categories, tags, and image uploads.",
      "priority": "high",
      "dependencies": [],
      "status": "pending"
    },
    {
      "id": "feature-003",
      "name": "feature-comments",
      "title": "Comment System",
      "description": "Implement comment functionality on posts including nested replies, moderation, spam filtering, and email notifications.",
      "priority": "medium",
      "dependencies": ["feature-001", "feature-002"],
      "status": "pending"
    },
    {
      "id": "feature-004",
      "name": "feature-rss",
      "title": "RSS Feed Generation",
      "description": "Implement RSS/Atom feed generation for posts, categories, and author feeds. Include proper caching and standards compliance.",
      "priority": "low",
      "dependencies": ["feature-002"],
      "status": "pending"
    }
  ]
}
```

Batch Analysis:
- **Batch 1**: feature-users, feature-posts (parallel, no dependencies)
- **Batch 2**: feature-comments, feature-rss (both depend on Batch 1 features)
