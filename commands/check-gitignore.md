---
description: "Check and update .gitignore to exclude Plan Cascade temporary files. Ensures planning files won't be accidentally committed to version control."
---

# Plan Cascade - Check and Update .gitignore

This command checks if your project's `.gitignore` is configured to exclude Plan Cascade temporary files, and updates it if needed.

## Why This Matters

Plan Cascade creates various temporary files during planning and execution:
- `prd.json`, `mega-plan.json` - Planning documents
- `.worktree/` - Git worktree directories
- `.plan-cascade-link.json` - Project link file
- State files (`.agent-status.json`, `.iteration-state.json`, etc.)

These files should NOT be committed to version control because:
1. They are regenerated each session
2. They may contain environment-specific paths
3. They can cause merge conflicts
4. They clutter the repository history

## Step 1: Check Current Status

First, check the current state of `.gitignore`:

```bash
uv run python -c "
from plan_cascade.utils.gitignore import GitignoreManager
from pathlib import Path

manager = GitignoreManager(Path.cwd())
result = manager.check()

print('=== .gitignore Status ===')
print()
print(f'File exists: {result.gitignore_exists}')
print(f'Has Plan Cascade section: {result.has_plan_cascade_section}')
print(f'Needs update: {result.needs_update}')
print()

if result.missing_entries:
    print('Missing entries:')
    for entry in result.missing_entries:
        print(f'  - {entry}')
else:
    print('All Plan Cascade entries are present.')
"
```

## Step 2: Update if Needed

If the check shows missing entries, update `.gitignore`:

```bash
uv run python -c "
from plan_cascade.utils.gitignore import GitignoreManager
from pathlib import Path

manager = GitignoreManager(Path.cwd())
result = manager.update()

if result.success:
    if result.action == 'created':
        print('Created .gitignore with Plan Cascade entries')
    elif result.action == 'updated':
        print(f'Updated .gitignore: added {len(result.entries_added)} entries')
    else:
        print('No update needed - all entries already present')
else:
    print(f'Error: {result.message}')
"
```

## Step 3: Show Summary

Display the final status:

```
=== .gitignore Update Complete ===

The following Plan Cascade entries are now in .gitignore:

Runtime directories:
  - .worktree/
  - .locks/
  - .state/

Planning documents:
  - prd.json
  - mega-plan.json
  - design_doc.json

Status files:
  - .mega-status.json
  - .planning-config.json
  - .agent-status.json
  - .iteration-state.json
  - .retry-state.json

Progress tracking:
  - findings.md
  - mega-findings.md
  - progress.txt

Context recovery:
  - .hybrid-execution-context.md
  - .mega-execution-context.md

New mode files:
  - .plan-cascade-link.json
  - .plan-cascade-backup/
  - .plan-cascade.json

Agent outputs:
  - .agent-outputs/

Your planning files will not be committed to version control.
```

## Notes

- This command is automatically run when starting `/plan-cascade:auto`, `/plan-cascade:hybrid-auto`, `/plan-cascade:hybrid-worktree`, or `/plan-cascade:mega-plan`
- If `.gitignore` doesn't exist, it will be created
- Existing `.gitignore` content is preserved - new entries are appended
- The update is idempotent - running it multiple times is safe

## Manual Usage

You can also run this directly in Python:

```python
from plan_cascade.utils.gitignore import ensure_gitignore
from pathlib import Path

# Returns True if gitignore is properly configured
ensure_gitignore(Path("/path/to/project"))
```

Or via CLI:

```bash
python -m plan_cascade.utils.gitignore /path/to/project --update
```
