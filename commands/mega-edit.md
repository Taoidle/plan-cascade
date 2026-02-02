---
description: "Edit the mega-plan interactively. Add, remove, or modify features. Usage: /plan-cascade:mega-edit"
---

# Edit Mega Plan

Edit the mega-plan.json file interactively to add, remove, or modify features.

## Path Storage Modes

Mega-plan file location depends on the storage mode:
- **New Mode**: `~/.plan-cascade/<project-id>/mega-plan.json`
- **Legacy Mode**: `mega-plan.json` in project root

The command uses PathResolver to find the correct file location.

## Step 1: Verify Mega Plan Exists

```bash
# Get mega-plan path from PathResolver
MEGA_PLAN_PATH=$(uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_mega_plan_path())" 2>/dev/null || echo "mega-plan.json")

if [ ! -f "$MEGA_PLAN_PATH" ]; then
    echo "No mega-plan.json found at: $MEGA_PLAN_PATH"
    echo "Use /plan-cascade:mega-plan <description> to create one first."
    exit 1
fi
```

## Step 2: Read Current Plan

Read and parse mega-plan.json:

```bash
cat "$MEGA_PLAN_PATH"
```

## Step 3: Display Current Structure

```
============================================================
CURRENT MEGA PLAN
============================================================

Goal: <goal>
Execution Mode: <mode>
Target Branch: <branch>

Features:
  1. [feature-001] <title>
     - Name: <name>
     - Priority: <priority>
     - Status: <status>
     - Dependencies: <deps or "none">

  2. [feature-002] <title>
     ...

============================================================
```

## Step 4: Ask What to Edit

Use AskUserQuestion to present options:

**What would you like to edit?**

Options:
1. **Add a new feature** - Add another feature to the plan
2. **Remove a feature** - Remove an existing feature
3. **Edit a feature** - Modify an existing feature's details
4. **Change execution mode** - Switch between auto/manual
5. **Change target branch** - Modify the merge target

## Step 5: Perform the Edit

### Option 1: Add a New Feature

Ask for:
- Feature name (lowercase-hyphenated, e.g., "feature-analytics")
- Title (human readable)
- Description (detailed for PRD generation)
- Priority (high/medium/low)
- Dependencies (comma-separated feature IDs, or "none")

Generate the next sequential ID and add to features array.

### Option 2: Remove a Feature

Show list of features, let user select one.
- Warn if other features depend on it
- Ask for confirmation
- Remove and update dependent features' dependencies

### Option 3: Edit a Feature

Show list of features, let user select one.
Then ask what to edit:
- Name
- Title
- Description
- Priority
- Dependencies

### Option 4: Change Execution Mode

Toggle between "auto" and "manual":
- Auto: Batches progress automatically
- Manual: Confirm before each batch

### Option 5: Change Target Branch

Ask for new branch name and validate it exists.

## Step 6: Validate Changes

After any edit, validate the mega-plan:
- Check all required fields
- Verify dependencies exist
- Check for circular dependencies
- Validate feature names format

## Step 7: Save and Display

Save the updated mega-plan.json to `$MEGA_PLAN_PATH` and show:

```
============================================================
MEGA PLAN UPDATED
============================================================

Changes saved to mega-plan.json

Updated Feature Batches:

Batch 1 (Parallel):
  - feature-001: <title>
  ...

Batch 2 (After Batch 1):
  - feature-003: <title> (depends on: feature-001)
  ...

============================================================

Next steps:
  - Review changes: cat mega-plan.json
  - Start execution: /plan-cascade:mega-approve
```

## Validation Rules

- Feature names: `^[a-z0-9][a-z0-9-]*$`
- Feature IDs: Must be unique
- Dependencies: Must reference existing feature IDs
- No circular dependencies allowed
- At least one feature required
