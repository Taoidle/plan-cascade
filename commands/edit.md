---
description: "Edit the current PRD in your default editor. Opens prd.json, validates after saving, and re-displays review. Supports adding/removing stories, changing priorities, and modifying dependencies."
---

# Hybrid Ralph - Edit PRD

You are opening the PRD file for manual editing.

## Step 1: Verify PRD Exists

```bash
if [ ! -f "prd.json" ]; then
    echo "ERROR: No PRD found. Please generate one first with:"
    echo "  /plan-cascade:hybrid-auto <description>"
    echo "  /plan-cascade:hybrid-manual <path>"
    exit 1
fi
```

## Step 2: Open in Editor

Open `prd.json` with the system's default editor:

```bash
# Detect platform and open editor
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" || "$OS" == "Windows_NT" ]]; then
    # Windows
    start prd.json
elif [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    open prd.json
else
    # Linux - use $EDITOR or fallback
    ${EDITOR:-nano} prd.json
fi
```

Tell the user:
```
Opening prd.json in your default editor...

Make your changes, then save and close the editor to continue.
```

## Step 3: Wait for Editor to Close

Wait for the user to finish editing. (The editor command will block until closed.)

## Step 4: Validate Updated PRD

After the editor closes, validate the modified PRD:

```bash
# Check JSON syntax
if ! python3 -m json.tool prd.json > /dev/null 2>&1; then
    echo "ERROR: Invalid JSON in prd.json"
    echo "Please fix the syntax errors and run /plan-cascade:edit again"
    exit 1
fi
```

Validate structure:
- Has `metadata`, `goal`, `objectives`, `stories`
- Each story has required fields
- Story IDs are unique
- Dependency references exist

If validation fails, show specific errors and suggest re-running `/plan-cascade:edit`.

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
  - /plan-cascade:approve - Approve and start execution
  - /plan-cascade:edit - Make more changes
  - /plan-cascade:show-dependencies - View dependency graph
```

## Notes

- The editor used depends on your system default application for .json files
- On Linux, you can set the EDITOR environment variable to control which editor is used
- Changes are validated automatically after saving
- If stories were added/removed, execution batches will be recalculated
