---
description: "Resume a spec interview from .state/spec-interview.json. Usage: /plan-cascade:spec-resume [--output-dir <dir>] [--flow <quick|standard|full>]"
---

# Plan Cascade - Spec Resume

Resume an in-progress spec interview in the current directory (or `--output-dir`).

## Step 1: Run Resume (CLI)

```bash
uv run --project "${CLAUDE_PLUGIN_ROOT}" plan-cascade spec resume --output-dir "<output-dir>" --flow <flow>
```

