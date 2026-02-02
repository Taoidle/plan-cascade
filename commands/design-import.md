---
description: "Import an external design document (Markdown, JSON, or HTML from Confluence/Notion) and convert it to design_doc.json format for Plan Cascade integration."
---

# Plan Cascade - Import Design Document

Import an external design document and convert it to the unified `design_doc.json` format used by Plan Cascade.

## Path Storage Modes

Design documents are user-visible files and remain in the working directory in both modes:
- `design_doc.json`: Created in current working directory (project root or worktree)

This command does not use PathResolver since it works with user-provided paths and creates user-visible documentation files.

## Supported Formats

| Format | Extensions | Source |
|--------|------------|--------|
| Markdown | `.md`, `.markdown` | Any markdown document |
| JSON | `.json` | Structured JSON documents |
| HTML | `.html`, `.htm` | Confluence/Notion exports |

## Tool Usage Policy (CRITICAL)

**To avoid command confirmation prompts:**

1. **Use Read tool for file reading** - NEVER use `cat` via Bash
   - ✅ `Read("design.md")`, `Read("architecture.html")`
   - ❌ `Bash("cat design.md")`

2. **Use Write tool for file creation**
   - ✅ `Write("design_doc.json", content)`
   - ❌ `Bash("echo '...' > design_doc.json")`

## Step 1: Parse Arguments

Get the input file path from arguments:

```
INPUT_FILE="{{args}}"
```

If no file provided:
```
ERROR: Please provide a path to the design document to import.

Usage:
  /plan-cascade:design-import <path>

Examples:
  /plan-cascade:design-import docs/architecture.md
  /plan-cascade:design-import design-spec.json
  /plan-cascade:design-import confluence-export.html
```

## Step 2: Read and Detect Format

Read the input file and detect its format:

```
content = Read(INPUT_FILE)

# Detect format from extension
if INPUT_FILE ends with .md or .markdown:
    FORMAT = "markdown"
elif INPUT_FILE ends with .json:
    FORMAT = "json"
elif INPUT_FILE ends with .html or .htm:
    FORMAT = "html"
else:
    FORMAT = "markdown"  # default
```

## Step 3: Convert Based on Format

### 3.1: Markdown Conversion

Parse the markdown structure to extract design information:

**Heading Mapping:**
| Markdown Heading | Design Doc Section |
|------------------|-------------------|
| `# Title` | `overview.title` |
| `## Overview/Summary/Introduction` | `overview.summary` |
| `## Goals/Objectives` | `overview.goals` |
| `## Non-Goals/Out of Scope` | `overview.non_goals` |
| `## Architecture/Design` | `architecture` section |
| `## Components/Modules` | `architecture.components` |
| `## Patterns` | `architecture.patterns` |
| `## Decisions/ADR` | `decisions` |
| `## API/Endpoints` | `interfaces.apis` |
| `## Data Models/Schema` | `interfaces.data_models` |

**Example Markdown Input:**
```markdown
# User Authentication System

## Overview
A secure authentication system for web applications.

## Goals
- Secure user registration and login
- JWT-based session management
- Self-service password reset

## Architecture
### Components
- AuthController - HTTP request handling
- AuthService - Business logic
- UserRepository - Data access

### Patterns
- Repository Pattern - For data abstraction
- Service Layer - For business logic isolation

## Decisions
### ADR-001: Use bcrypt for passwords
We chose bcrypt because it's battle-tested and includes salt.

### ADR-002: Use JWT tokens
Stateless authentication for horizontal scaling.

## APIs
- POST /api/auth/register - User registration
- POST /api/auth/login - User login
```

### 3.2: JSON Conversion

If the JSON is already in design_doc format, validate and use directly.

Otherwise, map common field names:

| Input Field | Design Doc Field |
|-------------|-----------------|
| `title`, `name` | `overview.title` |
| `summary`, `description` | `overview.summary` |
| `goals`, `objectives` | `overview.goals` |
| `components` | `architecture.components` |
| `patterns` | `architecture.patterns` |
| `decisions`, `adrs` | `decisions` |
| `apis`, `endpoints` | `interfaces.apis` |
| `models`, `data_models` | `interfaces.data_models` |

### 3.3: HTML Conversion (Confluence/Notion)

Extract content from HTML structure:
- `<title>` or `<h1>` → `overview.title`
- `<h2>` sections → Mapped by heading text
- `<ul>/<ol>` lists → Extracted as array items
- `<p>` paragraphs → Extracted as text content

Strip all HTML tags and decode entities.

## Step 4: Enhance with AI Analysis

After basic conversion, enhance the document:

1. **Fill in missing sections**: If architecture section is sparse, analyze the content to infer components
2. **Generate story mappings**: If prd.json exists, create mappings between stories and design elements
3. **Normalize formatting**: Ensure all fields follow the design_doc schema

## Step 5: Validate Result

Check the converted document:
- Has required sections (metadata, overview, architecture, decisions, story_mappings)
- overview.title is not empty
- All referenced IDs are valid
- JSON is well-formed

## Step 6: Save and Display

Save to `design_doc.json`:
```
Write("design_doc.json", <formatted JSON>)
```

Display conversion summary:
```
=== Design Document Imported ===

Source: <INPUT_FILE>
Format: <FORMAT>

Converted Content:
  Title: <overview.title>
  Summary: <first 100 chars of summary>...

Sections Extracted:
  ✓ Overview: <summary length> chars
  ✓ Goals: X items
  ✓ Components: Y defined
  ✓ Patterns: Z identified
  ✓ Decisions: N ADRs
  ✓ APIs: M endpoints
  ✓ Data Models: P models

Story Mappings: <auto-generated if PRD exists, otherwise empty>

Next steps:
  - Review with: /plan-cascade:design-review
  - Generate mappings manually if needed
  - Proceed to: /plan-cascade:approve
```

## Step 7: Handle PRD Integration

If PRD exists (check both new mode and legacy locations):

```
# Get PRD path from PathResolver
PRD_PATH = uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_prd_path())"

# Check for PRD in local directory first, then PathResolver location
If file exists at "prd.json" or PRD_PATH:
    PRD_FOUND = true
Else:
    PRD_FOUND = false
```

If PRD_FOUND:

1. Read the PRD to get story IDs
2. Attempt to auto-map stories to design elements:
   - Match story descriptions to component names
   - Match story acceptance criteria to API endpoints
   - Match story tags to component categories

3. Populate `story_mappings` section

If PRD doesn't exist:
```
Note: No prd.json found. Story mappings were not generated.
      Run /plan-cascade:hybrid-auto first, then use /plan-cascade:design-generate
      to create a design document with automatic story mappings.
```

## Example Conversion

**Input (architecture.md):**
```markdown
# E-Commerce Platform

## Summary
A scalable e-commerce platform with product catalog, cart, and checkout.

## Components
- ProductService - Product catalog management
- CartService - Shopping cart operations
- OrderService - Order processing

## Decisions
### Use PostgreSQL
Chose PostgreSQL for ACID compliance and JSON support.
```

**Output (design_doc.json):**
```json
{
  "metadata": {
    "created_at": "2024-01-15T10:00:00Z",
    "version": "1.0.0",
    "source": "converted",
    "prd_reference": null,
    "original_file": "architecture.md"
  },
  "overview": {
    "title": "E-Commerce Platform",
    "summary": "A scalable e-commerce platform with product catalog, cart, and checkout.",
    "goals": [],
    "non_goals": []
  },
  "architecture": {
    "components": [
      {"name": "ProductService", "description": "Product catalog management", ...},
      {"name": "CartService", "description": "Shopping cart operations", ...},
      {"name": "OrderService", "description": "Order processing", ...}
    ],
    "data_flow": "",
    "patterns": []
  },
  "decisions": [
    {
      "id": "ADR-001",
      "title": "Use PostgreSQL",
      "decision": "Chose PostgreSQL for ACID compliance and JSON support.",
      ...
    }
  ],
  "story_mappings": {}
}
```

## Notes

- Existing `design_doc.json` will be overwritten
- For best results, use structured markdown with clear headings
- Run `/plan-cascade:design-review` after import to verify and enhance
- AI will fill in gaps where possible, but manual review is recommended
