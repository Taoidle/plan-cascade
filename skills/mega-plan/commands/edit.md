---
name: mega:edit
description: Edit the mega-plan in your default editor
---

# /mega:edit

Edit the mega-plan.json file interactively.

## Your Task

### Step 1: Check for Mega Plan

**Use Read tool (NOT Bash) to check if mega-plan.json exists:**

```
Read("mega-plan.json")
```

If the file doesn't exist (Read returns error), inform the user:
```
No mega-plan.json found.
Use /mega:plan <description> to create one first.
```

### Step 2: Read Current Plan

The mega-plan was already read in Step 1 using the Read tool. Parse the JSON content from that result.

### Step 3: Display Current Structure

Show the user the current plan structure:

```
============================================================
CURRENT MEGA PLAN
============================================================

Goal: <goal>
Execution Mode: <mode>
Target Branch: <branch>

Features:
  1. [feature-001] <title>
     - Priority: <priority>
     - Status: <status>
     - Dependencies: <deps or "none">

  2. [feature-002] <title>
     ...

============================================================
```

### Step 4: Ask What to Edit

Use AskUserQuestion to ask what the user wants to edit:

**Options:**
1. **Add a new feature** - Add another feature to the plan
2. **Remove a feature** - Remove an existing feature
3. **Edit a feature** - Modify an existing feature's details
4. **Change execution mode** - Switch between auto/manual
5. **Change target branch** - Modify the merge target
6. **Edit in text editor** - Open in $EDITOR

### Step 5: Perform the Edit

Based on user selection:

#### Add a New Feature

Ask for:
- Feature name (lowercase-hyphenated)
- Title (human readable)
- Description (detailed for PRD generation)
- Priority (high/medium/low)
- Dependencies (existing feature IDs)

Then add to the features array with the next sequential ID.

#### Remove a Feature

Show list of features, let user select one to remove.
Warn if other features depend on it.
Remove the feature and update any dependencies.

#### Edit a Feature

Show list of features, let user select one.
Then ask what to edit:
- Name
- Title
- Description
- Priority
- Dependencies
- Status

#### Change Execution Mode

Toggle between "auto" and "manual".

#### Change Target Branch

Ask for new target branch name.

#### Edit in Text Editor

```bash
${EDITOR:-code} mega-plan.json
```

After editor closes, validate the file.

### Step 6: Validate Changes

After any edit, validate the mega-plan:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/mega-plan/core/mega_generator.py" validate
```

If validation fails, show errors and offer to fix.

### Step 7: Show Updated Plan

Display the updated plan structure and batch analysis:

```
============================================================
UPDATED MEGA PLAN
============================================================

Goal: <goal>
Execution Mode: <mode>
Target Branch: <branch>

Feature Batches:

Batch 1 (Parallel):
  - feature-001: <title>
  ...

Batch 2 (After Batch 1):
  - feature-003: <title> (depends on: feature-001)
  ...

============================================================

Changes saved to mega-plan.json
```

## Quick Edit Examples

### Adding a Feature

User selects "Add a new feature":

```
Feature name: feature-analytics
Title: Analytics Dashboard
Description: Implement analytics tracking and dashboard for monitoring user engagement, page views, and conversion metrics.
Priority: low
Dependencies: feature-users

Added feature-005: Analytics Dashboard
Dependencies: feature-users
```

### Changing Dependencies

User selects "Edit a feature" → "feature-003" → "Dependencies":

```
Current dependencies: feature-001

Available features:
  - feature-001: User Authentication
  - feature-002: Product Catalog

Enter new dependencies (comma-separated IDs, or 'none'):
> feature-001, feature-002

Updated dependencies for feature-003
```

### Removing a Feature

User selects "Remove a feature" → "feature-004":

```
Warning: The following features depend on feature-004:
  - feature-005

Do you want to:
1. Remove feature-004 and update dependents
2. Cancel

> 1

Removed feature-004
Updated feature-005 dependencies
```
