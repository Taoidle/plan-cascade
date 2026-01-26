---
name: edit
description: Edit the current PRD in your default editor
---

# /edit

Open the current PRD file (prd.json) in your default text editor for manual modification.

## Usage

```
/edit
```

## What It Does

1. **Opens prd.json** - Launches your system's default editor
2. **Waits for changes** - Pauses until you save and close
3. **Re-validates** - Checks the PRD after editing
4. **Shows updated review** - Displays the modified PRD

## Common Edits

### Adding a Story

```json
{
  "id": "story-005",
  "title": "New feature title",
  "description": "What this story does",
  "priority": "medium",
  "dependencies": ["story-001"],
  "status": "pending",
  "acceptance_criteria": [
    "First criterion",
    "Second criterion"
  ],
  "context_estimate": "small",
  "tags": ["feature", "api"]
}
```

### Changing Priority

```json
"priority": "high"  // Options: high, medium, low
```

### Adding Dependencies

```json
"dependencies": ["story-001", "story-002"]
```

### Modifying Acceptance Criteria

```json
"acceptance_criteria": [
  "Updated criterion 1",
  "New criterion 2",
  "Additional requirement"
]
```

## Validation After Edit

When you save and close, the PRD is validated for:
- Proper JSON syntax
- Unique story IDs
- Valid dependency references
- Required fields present

If validation fails, you'll see specific errors to fix.

## Re-entering Review Mode

After saving your changes, the PRD review is displayed again with:
- Updated story list
- Re-calculated dependency graph
- Re-generated execution batches

You can then:
- `/approve` - Approve the modified PRD
- `/edit` - Make more changes
- `/show-dependencies` - Review the updated dependency graph

## Editor Configuration

The command uses your system's default editor:
- **Linux**: `$EDITOR` environment variable, or `nano` if not set
- **macOS**: `$EDITOR` environment variable, or `vim` if not set
- **Windows**: Opens with `start` command (uses associated app)

To set a custom editor:

```bash
# Linux/macOS
export EDITOR="code --wait"  # VS Code
export EDITOR="vim"          # Vim
export EDITOR="nano"         # Nano

# Windows PowerShell
$env:EDITOR="code --wait"
```

## Tips

1. **Use a JSON-aware editor** - VS Code, Sublime Text, or similar provide syntax highlighting
2. **Validate after changes** - The tool automatically validates on save
3. **Check dependencies** - Ensure referenced story IDs exist
4. **Keep descriptions clear** - Agents work better with specific descriptions

## See Also

- `/approve` - Approve after editing
- `/hybrid:manual` - Load a different PRD file
- `/show-dependencies` - Verify dependency structure
