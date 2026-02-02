---
description: "Generate a technical design document. Auto-detects level: project-level from mega-plan.json, or feature-level from prd.json. Provides architectural context for story execution."
---

# Plan Cascade - Generate Design Document

Generate a structured technical design document (`design_doc.json`). Auto-detects the appropriate level based on available files.

## Path Storage Modes

This command works with both new and legacy path storage modes:

### New Mode (Default)
- `mega-plan.json`: In `~/.plan-cascade/<project-id>/`
- `prd.json`: In worktree directory or `~/.plan-cascade/<project-id>/`
- `design_doc.json`: Always created in project root (user-visible file)

### Legacy Mode
- All files in project root or worktree directory

Note: design_doc.json is a user-visible documentation file and always stays in the working directory.

## Two-Level Design Document System

```
┌─────────────────────────────────────────────────────────────┐
│ Level 1: Project Design (from mega-plan.json)               │
│ ─────────────────────────────────────────────────────────── │
│ • Global architecture and system overview                   │
│ • Cross-feature components and patterns                     │
│ • Project-wide ADRs (architectural decisions)               │
│ • Feature mappings (which patterns/decisions apply where)   │
│ • Shared data models and API standards                      │
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
│ • Inherits patterns/decisions from project level            │
└─────────────────────────────────────────────────────────────┘
```

## Tool Usage Policy (CRITICAL)

**To avoid command confirmation prompts:**

1. **Use Read tool for file reading** - NEVER use `cat` via Bash
2. **Use Glob tool for file finding** - NEVER use `ls` or `find` via Bash
3. **Use Write tool for file creation**

## Step 1: Detect Level

Auto-detect based on available files (check both new mode and legacy locations):

```
# Get paths from PathResolver
MEGA_PLAN_PATH = uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_mega_plan_path())"
PRD_PATH = uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_prd_path())"

# Check for mega-plan.json (new mode path first, then legacy)
If file exists at MEGA_PLAN_PATH or "mega-plan.json":
    LEVEL = "project"
    echo "Detected project-level context (mega-plan.json)"
Elif file exists at PRD_PATH or "prd.json":
    LEVEL = "feature"
    echo "Detected feature-level context (prd.json)"
Else:
    ERROR: Neither mega-plan.json nor prd.json found.
    Checked: {MEGA_PLAN_PATH}, mega-plan.json, {PRD_PATH}, prd.json
    Generate one first:
      /plan-cascade:mega-plan <description>  (for project)
      /plan-cascade:hybrid-auto <description>  (for feature)
    EXIT
```

## Step 2A: Generate Project-Level Design Document

If LEVEL == "project":

### 2A.1: Read mega-plan.json

Extract:
- Project goal
- Features list with descriptions and dependencies
- Target branch

### 2A.2: Analyze Project Architecture

Based on features, identify:
- System-wide components (e.g., API Gateway, shared services)
- Cross-cutting patterns (e.g., Repository Pattern, Event-Driven)
- Global infrastructure decisions
- Shared data models

### 2A.3: Generate Project Design Document

```json
{
  "metadata": {
    "created_at": "<ISO-8601>",
    "version": "1.0.0",
    "source": "ai-generated",
    "level": "project",
    "mega_plan_reference": "mega-plan.json"
  },
  "overview": {
    "title": "<project goal>",
    "summary": "<brief description>",
    "goals": ["<goal1>", "<goal2>"],
    "non_goals": ["<non-goal1>"]
  },
  "architecture": {
    "system_overview": "<high-level architecture description>",
    "components": [
      {
        "name": "ComponentName",
        "description": "Description",
        "responsibilities": ["resp1"],
        "dependencies": ["OtherComponent"],
        "features": ["feature-001", "feature-002"]
      }
    ],
    "data_flow": "<how data flows through the system>",
    "patterns": [
      {
        "name": "PatternName",
        "description": "What this pattern does",
        "rationale": "Why we use it",
        "applies_to": ["feature-001", "all"]
      }
    ],
    "infrastructure": {
      "deployment": "<deployment strategy>",
      "database": "<database choices>",
      "cache": "<caching strategy>",
      "message_queue": "<if applicable>"
    }
  },
  "interfaces": {
    "api_standards": {
      "style": "RESTful",
      "versioning": "URL-based (/api/v1/...)",
      "authentication": "JWT Bearer tokens",
      "error_format": {"code": "string", "message": "string"}
    },
    "shared_data_models": [
      {
        "name": "ModelName",
        "description": "Shared across features",
        "fields": {"field": "type"},
        "used_by": ["feature-001", "feature-002"]
      }
    ]
  },
  "decisions": [
    {
      "id": "ADR-001",
      "title": "Decision title",
      "context": "Background",
      "decision": "What we decided",
      "rationale": "Why",
      "alternatives_considered": ["alt1"],
      "status": "accepted",
      "applies_to": ["feature-001", "all"]
    }
  ],
  "feature_mappings": {
    "feature-001": {
      "components": ["ComponentA", "ComponentB"],
      "patterns": ["PatternName"],
      "decisions": ["ADR-001", "ADR-002"],
      "description": "Feature description from mega-plan"
    }
  }
}
```

## Step 2B: Generate Feature-Level Design Document

If LEVEL == "feature":

### 2B.1: Check for Parent Design Document

```
If in worktree (has .planning-config.json):
    Check parent directory for project-level design_doc.json
    If found:
        HAS_PARENT = true
        Read inherited context from parent
```

### 2B.2: Read prd.json

Extract:
- Goal and objectives
- Stories with acceptance criteria
- Dependencies between stories

### 2B.3: Explore Codebase (Optional)

Use Glob to find existing patterns:
```
Glob("src/**/*.py")
Glob("**/*controller*")
Glob("**/*service*")
```

### 2B.4: Generate Feature Design Document

```json
{
  "metadata": {
    "created_at": "<ISO-8601>",
    "version": "1.0.0",
    "source": "ai-generated",
    "level": "feature",
    "prd_reference": "prd.json",
    "parent_design_doc": "../design_doc.json",
    "feature_id": "<from .planning-config.json if in worktree>"
  },
  "overview": {
    "title": "<feature title>",
    "summary": "<from PRD goal>",
    "goals": ["<from PRD objectives>"],
    "non_goals": ["<identified non-goals>"]
  },
  "inherited_context": {
    "description": "Context inherited from project-level design document",
    "patterns": ["PatternName"],
    "decisions": ["ADR-001"],
    "shared_models": ["SharedModel"]
  },
  "architecture": {
    "components": [
      {
        "name": "FeatureComponent",
        "description": "Feature-specific component",
        "responsibilities": ["resp1"],
        "dependencies": ["OtherComponent"],
        "files": ["src/path/to/file.py"]
      }
    ],
    "data_flow": "<feature-specific data flow>",
    "patterns": [
      {
        "name": "FeaturePattern",
        "description": "Feature-specific pattern",
        "rationale": "Why"
      }
    ]
  },
  "interfaces": {
    "apis": [
      {
        "id": "API-001",
        "method": "POST",
        "path": "/api/v1/endpoint",
        "description": "What it does",
        "request_body": {"field": "type"},
        "response": {"success": {}, "error_codes": []}
      }
    ],
    "data_models": [
      {
        "name": "FeatureModel",
        "description": "Feature-specific model",
        "fields": {"field": "type"}
      }
    ]
  },
  "decisions": [
    {
      "id": "ADR-F001",
      "title": "Feature-specific decision",
      "context": "Background",
      "decision": "What we decided",
      "rationale": "Why",
      "alternatives_considered": ["alt1"],
      "status": "accepted"
    }
  ],
  "story_mappings": {
    "story-001": {
      "components": ["ComponentA"],
      "decisions": ["ADR-F001"],
      "interfaces": ["API-001", "ModelName"]
    }
  }
}
```

## Step 3: Validate and Save

Validate the generated document:
- All required sections present
- No duplicate ADR IDs
- Valid references in mappings

Save to `design_doc.json`:
```
Write("design_doc.json", <formatted JSON>)
```

## Step 4: Display Summary

### For Project-Level:
```
=== Project Design Document Generated ===

Level: PROJECT
Overview: <title>

Components: X system-wide components
Patterns: Y architectural patterns
Decisions: Z project-level ADRs
Shared Models: N shared data models

Feature Mappings:
  - feature-001: 2 components, 1 pattern, 2 ADRs
  - feature-002: 3 components, 2 patterns, 1 ADR

Next steps:
  - Review with: /plan-cascade:design-review
  - Create feature worktrees: /plan-cascade:mega-approve
  - Each worktree can have its own feature-level design_doc.json
```

### For Feature-Level:
```
=== Feature Design Document Generated ===

Level: FEATURE
Overview: <title>
Parent: {../design_doc.json if inherited, else "None (standalone)"}

Inherited from project:
  - Patterns: PatternA, PatternB
  - Decisions: ADR-001, ADR-002

Feature-specific:
  - Components: X components
  - APIs: Y endpoints
  - Data Models: Z models
  - Decisions: N feature ADRs (ADR-F###)

Story Mappings:
  - story-001: 2 components, 1 decision, 1 API
  - story-002: 3 components, 0 decisions, 2 APIs

Next steps:
  - Review with: /plan-cascade:design-review
  - Proceed to: /plan-cascade:approve
```

## Guidelines

### Project-Level Design Documents
- Focus on cross-cutting concerns
- Define patterns that apply to multiple features
- Use `applies_to: ["all"]` for universal patterns/decisions
- Create feature_mappings for each feature in mega-plan

### Feature-Level Design Documents
- Reference inherited patterns/decisions, don't duplicate
- Use ADR-F### prefix for feature-specific decisions
- Map each story to relevant components/decisions/interfaces
- Keep story_mappings complete for optimal agent guidance

## Notes

- Design documents are optional but highly recommended for complex projects
- Project-level doc should be created before mega-approve
- Feature-level docs can inherit from project-level in worktrees
- Agents receive filtered context based on story_mappings
