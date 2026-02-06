---
description: "Edit the current PRD in your default editor. Opens prd.json, validates after saving, and re-displays review. Supports adding/removing stories, changing priorities, and modifying dependencies."
---

# Hybrid Ralph - Edit PRD

You are opening the PRD file for manual editing.

## Path Storage Modes

PRD file location depends on the storage mode:
- **New Mode**: `~/.plan-cascade/<project-id>/prd.json` or in worktree directory
- **Legacy Mode**: `prd.json` in project root or worktree

The command uses PathResolver to find the correct file location.

## Step 1: Verify PRD Exists

```bash
# Get PRD path from PathResolver
PRD_PATH=$(uv run python -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; print(PathResolver(Path.cwd()).get_prd_path())" 2>/dev/null || echo "prd.json")

# Also check local prd.json (in worktree)
if [ -f "prd.json" ]; then
    PRD_PATH="prd.json"
elif [ ! -f "$PRD_PATH" ]; then
    echo "ERROR: No PRD found at: $PRD_PATH"
    echo "Please generate one first with:"
    echo "  /hybrid:auto <description>"
    echo "  /hybrid:manual <path>"
    exit 1
fi
```

## Step 2: Open in Editor

Open the PRD file with the system's default editor:

```bash
# Detect platform and open editor
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" || "$OS" == "Windows_NT" ]]; then
    start "$PRD_PATH"
elif [[ "$OSTYPE" == "darwin"* ]]; then
    open "$PRD_PATH"
else
    ${EDITOR:-nano} "$PRD_PATH"
fi
```

Tell the user:
```
Opening PRD at: {PRD_PATH} in your default editor...

Make your changes, then save and close the editor to continue.
```

## Step 3: Wait for Editor to Close

Wait for the user to finish editing. (The editor command will block until closed.)

## Step 4: Validate Updated PRD

After the editor closes, validate the modified PRD:

```bash
if ! uv run python -m json.tool "$PRD_PATH" > /dev/null 2>&1; then
    echo "ERROR: Invalid JSON in PRD at: $PRD_PATH"
    echo "Please fix the syntax errors and run /edit again"
    exit 1
fi
```

Validate structure:
- Has `metadata`, `goal`, `objectives`, `stories`
- Each story has required fields
- Story IDs are unique
- Dependency references exist

If validation fails, show specific errors and suggest re-running `/edit`.

## Step 5: Display Updated PRD Review

Show the updated PRD review with:
- Goal and objectives
- All stories with current status
- Re-calculated dependency graph
- Re-generated execution batches

## Step 6: Show Next Steps

```
PRD updated successfully!

Next steps:
  - /approve - Approve and start execution
  - /edit - Make more changes
  - /show-dependencies - View dependency graph
```

## Notes

- The editor used depends on your system default application for .json files
- On Linux, you can set the EDITOR environment variable to control which editor is used
- Changes are validated automatically after saving
- If stories were added/removed, execution batches will be recalculated
