---
description: "Review and interactively edit the current design_doc.json. Displays the design document in a readable format and allows modifications to components, patterns, decisions, and story mappings."
---

# Plan Cascade - Review Design Document

Review the current `design_doc.json` and make interactive edits to components, patterns, architectural decisions, and story mappings.

## Tool Usage Policy (CRITICAL)

**To avoid command confirmation prompts:**

1. **Use Read tool for file reading** - NEVER use `cat` via Bash
   - ✅ `Read("design_doc.json")`, `Read("prd.json")`
   - ❌ `Bash("cat design_doc.json")`

2. **Use Write tool for file updates**
   - ✅ `Write("design_doc.json", content)`
   - ❌ `Bash("echo '...' > design_doc.json")`

## Step 1: Load Design Document

Read `design_doc.json`:

```
If design_doc.json does not exist:
    ERROR: No design document found.

    Generate one with:
      /plan-cascade:design-generate    (from PRD)
      /plan-cascade:design-import <file>  (from external doc)
    EXIT
```

## Step 2: Validate Document

Check for structural issues:
- Missing required sections
- Empty overview title
- Invalid ADR references
- Orphaned story mappings

Report any validation errors before proceeding.

## Step 3: Display Design Document

Format and display the design document:

```
════════════════════════════════════════════════════════════════
                    DESIGN DOCUMENT REVIEW
════════════════════════════════════════════════════════════════

📋 OVERVIEW
────────────────────────────────────────────────────────────────
Title:   <overview.title>
Summary: <overview.summary>

Goals:
  • <goal 1>
  • <goal 2>
  ...

Non-Goals:
  • <non-goal 1>
  ...

🏗️ ARCHITECTURE
────────────────────────────────────────────────────────────────
Components (N total):

  [1] ComponentName
      Description: <description>
      Responsibilities:
        • <responsibility 1>
        • <responsibility 2>
      Dependencies: <dep1>, <dep2>
      Files: <file1>, <file2>

  [2] AnotherComponent
      ...

Data Flow:
  <data_flow description>

Patterns (M total):

  [1] PatternName
      Description: <description>
      Rationale: <rationale>

  [2] AnotherPattern
      ...

📝 ARCHITECTURAL DECISIONS
────────────────────────────────────────────────────────────────
  [ADR-001] <title>
            Status: <status>
            Context: <context>
            Decision: <decision>
            Rationale: <rationale>
            Alternatives: <alt1>, <alt2>

  [ADR-002] <title>
            ...

🔌 INTERFACES
────────────────────────────────────────────────────────────────
APIs (P endpoints):

  [API-001] <METHOD> <path>
            Description: <description>
            Request: <request_body summary>
            Response: <response summary>

  [API-002] ...

Data Models (Q models):

  [1] ModelName
      Description: <description>
      Fields:
        • field_name: <type>
        • field_name: <type>

  [2] AnotherModel
      ...

🔗 STORY MAPPINGS
────────────────────────────────────────────────────────────────
  story-001: Components=[A, B], Decisions=[ADR-001], Interfaces=[API-001]
  story-002: Components=[C], Decisions=[], Interfaces=[Model1]
  ...

════════════════════════════════════════════════════════════════
```

## Step 4: Interactive Editing Menu

Present editing options:

```
What would you like to do?

  [1] Edit Overview
      - Modify title, summary, goals, non-goals

  [2] Edit Components
      - Add, modify, or remove components
      - Update responsibilities and dependencies

  [3] Edit Patterns
      - Add, modify, or remove architectural patterns

  [4] Edit Decisions (ADRs)
      - Add, modify, or remove architectural decisions
      - Update decision status

  [5] Edit Interfaces
      - Add, modify, or remove APIs
      - Add, modify, or remove data models

  [6] Edit Story Mappings
      - Map stories to components, decisions, interfaces
      - Auto-generate mappings from PRD

  [7] Validate Document
      - Check for errors and inconsistencies

  [8] Save and Exit
      - Save changes and exit review

  [9] Exit Without Saving
      - Discard changes and exit

Enter choice [1-9]:
```

Use the AskUserQuestion tool to get user choice.

## Step 5: Handle Edit Actions

### 5.1: Edit Overview

```
Current Overview:
  Title: <title>
  Summary: <summary>
  Goals: <goals>
  Non-Goals: <non-goals>

What would you like to change?
  [1] Title
  [2] Summary
  [3] Add/Remove Goals
  [4] Add/Remove Non-Goals
  [5] Back to main menu
```

### 5.2: Edit Components

```
Current Components:
  [1] ComponentA - <description>
  [2] ComponentB - <description>

Options:
  [A] Add new component
  [M] Modify component (enter number)
  [D] Delete component (enter number)
  [B] Back to main menu

Enter choice:
```

For adding/modifying, prompt for:
- Component name
- Description
- Responsibilities (list)
- Dependencies (list of other components)
- Files (list of file paths)

### 5.3: Edit Patterns

```
Current Patterns:
  [1] PatternA - <description>
  [2] PatternB - <description>

Options:
  [A] Add new pattern
  [M] Modify pattern (enter number)
  [D] Delete pattern (enter number)
  [B] Back to main menu
```

### 5.4: Edit Decisions

```
Current Decisions:
  [ADR-001] <title> (status: accepted)
  [ADR-002] <title> (status: proposed)

Options:
  [A] Add new decision
  [M] Modify decision (enter ADR ID)
  [D] Delete decision (enter ADR ID)
  [S] Change status (enter ADR ID)
  [B] Back to main menu
```

### 5.5: Edit Interfaces

```
=== APIs ===
  [API-001] POST /api/endpoint
  [API-002] GET /api/resource

=== Data Models ===
  [1] User
  [2] Order

Options:
  [AA] Add API
  [AM] Modify API
  [AD] Delete API
  [MA] Add Model
  [MM] Modify Model
  [MD] Delete Model
  [B] Back to main menu
```

### 5.6: Edit Story Mappings

```
Current Mappings:
  story-001: [ComponentA, ComponentB] | [ADR-001] | [API-001]
  story-002: [ComponentC] | [] | [User]
  story-003: <not mapped>

Options:
  [M] Manually map a story
  [A] Auto-generate all mappings from PRD
  [C] Clear all mappings
  [B] Back to main menu
```

For manual mapping:
```
Enter story ID to map: story-003

Select components (comma-separated, or 'none'):
  Available: ComponentA, ComponentB, ComponentC
  Enter: ComponentA, ComponentC

Select decisions (comma-separated ADR IDs, or 'none'):
  Available: ADR-001, ADR-002
  Enter: ADR-002

Select interfaces (comma-separated API IDs or model names, or 'none'):
  Available: API-001, API-002, User, Order
  Enter: API-002, Order

Mapping updated:
  story-003: [ComponentA, ComponentC] | [ADR-002] | [API-002, Order]
```

## Step 6: Auto-Generate Mappings

If user selects auto-generate:

1. Read `prd.json` to get story details
2. For each story:
   - Match story title/description keywords to component names
   - Match acceptance criteria to API paths
   - Match story tags to component categories
3. Create/update story_mappings

```
Auto-generating story mappings from PRD...

  story-001: "Design database schema"
    → Matched: UserRepository (keyword: database)
    → Matched: User model (keyword: schema)

  story-002: "Implement user registration"
    → Matched: AuthController, AuthService (keyword: implement)
    → Matched: API-001 /api/auth/register (path match)
    → Matched: ADR-001 bcrypt (keyword: password)

  story-003: "Implement user login"
    → Matched: AuthController, AuthService
    → Matched: API-002 /api/auth/login

Generated mappings for 3 stories.
Review the mappings and adjust as needed.
```

## Step 7: Validate and Save

After each edit or before saving:

```
Validating design document...

✓ Overview complete
✓ 4 components defined
✓ 3 patterns documented
✓ 2 architectural decisions
✓ 4 API endpoints
✓ 2 data models
✓ All story mappings valid

Document is valid and ready to save.
```

If errors found:
```
Validation found issues:

  ⚠ story-005 mapped but not in PRD
  ⚠ ADR-003 referenced but not defined
  ⚠ Component "UnknownService" referenced in mapping but not defined

Fix these issues before saving? [Y/n]
```

## Step 8: Save Changes

Write updated document:
```
Write("design_doc.json", <formatted JSON>)
```

```
Design document saved successfully!

Changes made:
  - Added 1 component
  - Modified 1 decision
  - Updated 3 story mappings

Next steps:
  - Proceed to approval: /plan-cascade:approve
  - Re-review if needed: /plan-cascade:design-review
```

## Notes

- Changes are only saved when you select "Save and Exit"
- Use "Exit Without Saving" to discard all changes
- Run `/plan-cascade:design-generate` to regenerate from scratch
- Story mappings help execution agents receive relevant context
- Keep mappings up-to-date as stories evolve
