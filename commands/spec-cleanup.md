---
description: "Cleanup spec interview state (and optionally spec artifacts). Usage: /plan-cascade:spec-cleanup [--output-dir <dir>] [--all]"
---

# Plan Cascade - Spec Cleanup

Remove spec interview state files.

## Run Cleanup (CLI)

```bash
uv run --project "${CLAUDE_PLUGIN_ROOT}" plan-cascade spec cleanup --output-dir "<output-dir>" [--all]
```

`--all` also deletes `spec.json` and `spec.md`.

