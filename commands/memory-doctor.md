---
description: "Diagnose decision conflicts, duplicates, and superseded entries across all design documents. Helps maintain decision hygiene as the project evolves. Usage: /plan-cascade:memory-doctor"
---

# Memory Doctor — 决策健康诊断

You are running a full diagnosis on all Architecture Decision Records (ADRs) across the project's design documents.

## Step 1: Collect and Diagnose All Decisions

**CRITICAL**: Use Bash to run the memory doctor script in full diagnosis mode:

```bash
uv run python "${CLAUDE_PLUGIN_ROOT}/skills/hybrid-ralph/scripts/memory-doctor.py" \
  --mode full \
  --project-root "$(pwd)"
```

This script collects all decisions from every `design_doc.json` in the project (root, worktrees, feature directories) and uses LLM to detect conflicts, superseded entries, and semantic duplicates.

Exit code handling:
- **Exit 0**: No issues found, or no decisions to check — display "No issues found" and stop here
- **Exit 1**: Diagnosis issues found — proceed to Step 2
- **Exit 2** (or script crash/traceback): Infrastructure error. Common causes:
  - No API key configured — tell the user to set `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, or `DEEPSEEK_API_KEY`
  - No `design_doc.json` files found in the project
  - Display the error message and stop here

## Step 2: Display Diagnosis Report

Display the full diagnosis report from Step 1 output. The report groups findings by type:
- 🔴 **CONFLICT**: Contradictory decisions on the same concern
- 🟠 **SUPERSEDED**: A newer decision covers the scope of an older one
- 🟡 **DUPLICATE**: Semantically identical decisions with different wording

## Step 3: Interactive Resolution

**CRITICAL**: For each diagnosis finding, use `AskUserQuestion` to let the user choose an action:

**For CONFLICT findings:**
- **Deprecate** — Mark the older decision as `deprecated` (recommended)
- **Skip** — Keep both decisions as-is

**For SUPERSEDED findings:**
- **Deprecate** — Mark the superseded decision as `deprecated` (recommended)
- **Skip** — Keep both decisions as-is

**For DUPLICATE findings:**
- **Merge** — Keep one decision, remove the duplicate (recommended)
- **Skip** — Keep both decisions as-is

Present each finding with its explanation and suggestion from the diagnosis report. Example question:

> **ADR-F003 vs ADR-F012**: Two decisions conflict on API response format.
> Old: "API uses custom JSON structure" (feature-auth/design_doc.json)
> New: "API uses JSON:API spec" (feature-order/design_doc.json)
> Suggestion: Deprecate ADR-F003
>
> How would you like to resolve this?

## Step 4: Apply Changes

**CRITICAL**: Construct a JSON array of the user's choices and invoke the script to apply them.

Save the user's choices to a temporary file `_doctor_actions.json`:
```json
[
  {
    "action": "deprecate",
    "diagnosis": {
      "type": "conflict",
      "decision_a": {"id": "ADR-F003", "_source": "path/to/design_doc.json"},
      "decision_b": {"id": "ADR-F012"},
      "source_a": "path/to/design_doc.json",
      "source_b": "other/design_doc.json"
    }
  }
]
```

Then run:
```bash
uv run python "${CLAUDE_PLUGIN_ROOT}/skills/hybrid-ralph/scripts/memory-doctor.py" \
  --apply _doctor_actions.json \
  --project-root "$(pwd)"
```

Clean up the temporary file after execution:
```bash
rm -f _doctor_actions.json
```

## Step 5: Summary

Display a summary of all actions taken:

```
Memory Doctor — 处理结果
━━━━━━━━━━━━━━━━━━━━━━━━
✓ Deprecated: N decisions
✓ Merged: N decision pairs
○ Skipped: N findings
```

If any design_doc.json files were modified, list them so the user knows which files changed.

## Step 6: Next Steps

After the diagnosis is complete:
- Review changed files with `git diff` to verify the modifications
- Commit the changes if satisfied
- Note: Decision conflict checks also run automatically during `/plan-cascade:hybrid-auto` and `/plan-cascade:mega-plan` when new design documents are generated
