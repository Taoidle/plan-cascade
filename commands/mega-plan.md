---
description: "Generate a mega-plan for project-level multi-feature orchestration. Breaks a complex project into parallel features with dependencies. Usage: /plan-cascade:mega-plan <project description> [design-doc-path]"
---

# Mega Plan - Project-Level Feature Orchestration

You are creating a **Mega Plan** - a project-level plan that orchestrates multiple features in parallel.

## Architecture

```
Level 1: Mega Plan (This level - Project)
    └── Level 2: Features (hybrid:worktree tasks)
              └── Level 3: Stories (hybrid-ralph internal parallelism)
```

## Step 1: Parse Arguments

Parse user arguments:
- **Project description**: First argument (required)
- **Design doc path**: Second argument (optional) - external design document to convert

```bash
PROJECT_DESC="{{args|arg 1}}"
DESIGN_DOC_ARG="{{args|arg 2 or empty}}"
```

If no description provided, ask the user:
```
Please provide a project description. What do you want to build?
Example: "Build an e-commerce platform with user authentication, product catalog, shopping cart, and order processing"

Optional: You can also provide an external design document path as second argument:
/plan-cascade:mega-plan "Build e-commerce platform..." ./architecture.md
```

## Step 2: Check for Existing Mega Plan

```bash
if [ -f "mega-plan.json" ]; then
    echo "A mega-plan.json already exists in this directory."
fi
```

If exists, ask user:
- Overwrite the existing plan?
- Or use `/plan-cascade:mega-edit` to modify it?

## Step 3: Analyze the Project

Analyze the project description and codebase to understand:
1. What features are needed
2. Dependencies between features
3. Priority of each feature
4. Appropriate breakdown granularity

Use the Explore agent or read relevant files to understand the existing codebase structure.

## Step 4: Generate the Mega Plan

Create `mega-plan.json` with 2-6 features:

```json
{
  "metadata": {
    "created_at": "<current ISO timestamp>",
    "version": "1.0.0"
  },
  "goal": "<one-sentence project goal>",
  "description": "<original user description>",
  "execution_mode": "auto",
  "target_branch": "main",
  "features": [
    {
      "id": "feature-001",
      "name": "feature-<name>",
      "title": "<Human Readable Title>",
      "description": "<detailed description for PRD generation>",
      "priority": "high",
      "dependencies": [],
      "status": "pending"
    }
  ]
}
```

**Feature Guidelines:**
- `name`: lowercase letters, numbers, hyphens only (e.g., "feature-auth", "feature-products")
- `description`: Detailed enough to generate a complete PRD with 3-7 stories
- `dependencies`: List feature IDs that must complete first
- `priority`: "high" (core), "medium" (important), "low" (nice-to-have)

## Step 5: Generate Project-Level Design Document

After generating mega-plan.json, automatically generate `design_doc.json` (project-level):

### 5.1: Check for User-Provided Design Document

```
If DESIGN_DOC_ARG is not empty and file exists:
    Read the external document at DESIGN_DOC_ARG
    Detect format:
      - .md files: Parse Markdown structure (headers → sections)
      - .json files: Validate/map to our schema
      - .html files: Parse HTML structure
    Convert to our format:
      - Extract overview, architecture, patterns, decisions
      - Map to feature_mappings based on mega-plan features
    Save as design_doc.json
    DESIGN_SOURCE="Converted from: $DESIGN_DOC_ARG"
Else:
    Auto-generate based on mega-plan analysis
    DESIGN_SOURCE="Auto-generated from mega-plan"
```

### 5.2: Auto-Generate Project Design Document

Based on the features in mega-plan.json, generate `design_doc.json`:

```json
{
  "metadata": {
    "created_at": "<timestamp>",
    "version": "1.0.0",
    "source": "ai-generated",
    "level": "project",
    "mega_plan_reference": "mega-plan.json"
  },
  "overview": {
    "title": "<from mega-plan goal>",
    "summary": "<project summary>",
    "goals": ["<extracted goals>"],
    "non_goals": ["<identified non-goals>"]
  },
  "architecture": {
    "system_overview": "<high-level architecture based on features>",
    "components": [
      {
        "name": "ComponentName",
        "description": "Description",
        "responsibilities": ["resp1"],
        "dependencies": [],
        "features": ["feature-001", "feature-002"]
      }
    ],
    "data_flow": "<how data flows between features>",
    "patterns": [
      {
        "name": "PatternName",
        "description": "What it does",
        "rationale": "Why use it",
        "applies_to": ["feature-001", "all"]
      }
    ],
    "infrastructure": {}
  },
  "interfaces": {
    "api_standards": {
      "style": "RESTful",
      "versioning": "URL-based",
      "authentication": "<method>"
    },
    "shared_data_models": []
  },
  "decisions": [
    {
      "id": "ADR-001",
      "title": "Decision title",
      "context": "Background",
      "decision": "What we decided",
      "rationale": "Why",
      "alternatives_considered": [],
      "status": "accepted",
      "applies_to": ["all"]
    }
  ],
  "feature_mappings": {
    "feature-001": {
      "components": [],
      "patterns": [],
      "decisions": [],
      "description": "<from mega-plan>"
    }
  }
}
```

**Design Document Generation Guidelines:**
- Analyze feature descriptions to identify shared components
- Identify cross-cutting patterns (e.g., Repository, Service Layer)
- Create feature_mappings linking each feature to relevant patterns/decisions
- Mark patterns/decisions with `applies_to: ["all"]` if they apply universally

## Step 6: Create Supporting Files

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
  "updated_at": "<timestamp>",
  "execution_mode": "auto",
  "target_branch": "main",
  "current_batch": 0,
  "features": {}
}
```

## Step 6: Calculate Execution Batches

Group features by dependencies:
- **Batch 1**: Features with no dependencies (run in parallel)
- **Batch 2**: Features depending only on Batch 1 (run in parallel after Batch 1)
- And so on...

**IMPORTANT**: Batches are executed sequentially:
1. Batch 1 features run in parallel
2. When ALL Batch 1 features complete, they are **merged to target_branch**
3. Batch 2 worktrees are created from the **updated** target_branch
4. This ensures Batch 2 features have access to Batch 1 code

## Step 7: Ask Execution Mode

Use AskUserQuestion to ask:

**How should feature batches progress?**

Options:
1. **Auto Mode (Recommended)** - Batches progress automatically when ready
2. **Manual Mode** - Confirm before starting each batch

Update `mega-plan.json` with the chosen mode.

## Step 8: Display the Plan

```
============================================================
MEGA PLAN CREATED
============================================================

Goal: <goal>
Execution Mode: <auto|manual>
Target Branch: <target_branch>
Total Features: <count>

Feature Batches:

Batch 1 (Parallel - No Dependencies):
  - feature-001: <title> [high]
  - feature-002: <title> [high]

Batch 2 (After Batch 1):
  - feature-003: <title> [medium] (depends on: feature-001, feature-002)

Batch 3 (After Batch 2):
  - feature-004: <title> [medium] (depends on: feature-003)

============================================================

Files created:
  - mega-plan.json       (project plan)
  - design_doc.json      (project-level technical design)
  - mega-findings.md     (shared findings)
  - .mega-status.json    (execution status)

Next steps:
  1. Review the plan: Read mega-plan.json
  2. Review the design: Read design_doc.json
  3. Edit if needed: /plan-cascade:mega-edit
  4. Start execution: /plan-cascade:mega-approve
     Or with auto PRD approval: /plan-cascade:mega-approve --auto-prd

============================================================
```

## Example

User: `/plan-cascade:mega-plan Build a blog platform with user accounts, post management, comments, and RSS feeds`

Generated features:
1. **feature-users** (high, no deps) - User accounts with registration, login, profiles
2. **feature-posts** (high, no deps) - Blog post CRUD with categories and tags
3. **feature-comments** (medium, depends on users+posts) - Comment system with moderation
4. **feature-rss** (low, depends on posts) - RSS/Atom feed generation

Batches:
- Batch 1: feature-users, feature-posts (parallel)
- Batch 2: feature-comments, feature-rss (after Batch 1)
