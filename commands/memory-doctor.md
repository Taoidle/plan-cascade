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

This script:
- Collects all decisions from every `design_doc.json` in the project (root, worktrees, feature directories)
- Uses LLM to detect conflicts, superseded entries, and semantic duplicates
- Outputs a formatted diagnosis report to stdout
- Outputs structured JSON findings to stderr

If the script reports "No LLM Available", inform the user they need to set an API key environment variable (ANTHROPIC_API_KEY, OPENAI_API_KEY, or DEEPSEEK_API_KEY).

If the script reports "No Decisions Found", inform the user that no design_doc.json files with decisions were found.

## Step 2: Display Diagnosis Report

Display the full diagnosis report from Step 1 output. The report groups findings by type:
- 🔴 **CONFLICT**: Contradictory decisions on the same concern
- 🟠 **SUPERSEDED**: A newer decision covers the scope of an older one
- 🟡 **DUPLICATE**: Semantically identical decisions with different wording

If no issues are found, display the "all healthy" message and stop here.

## Step 3: Interactive Resolution

For each diagnosis finding, use `AskUserQuestion` to let the user choose an action:

**For CONFLICT findings:**
- **Deprecate** — Mark the older decision as `deprecated` (recommended)
- **Skip** — Keep both decisions as-is

**For SUPERSEDED findings:**
- **Deprecate** — Mark the superseded decision as `deprecated` (recommended)
- **Skip** — Keep both decisions as-is

**For DUPLICATE findings:**
- **Merge** — Keep one decision, remove the duplicate
- **Skip** — Keep both decisions as-is

Present each finding with its explanation and suggestion from the diagnosis report.

## Step 4: Apply Changes

For each finding where the user chose an action (not "Skip"):

**Deprecate action:**
Read the target `design_doc.json` file, find the decision by ID, and update it:
```json
{
  "status": "deprecated",
  "deprecated_by": "<other-decision-id>",
  "deprecated_at": "<ISO-8601 timestamp>"
}
```

**Merge action:**
1. Read both source `design_doc.json` files
2. In the kept decision, append to `rationale`: `"\n[Merged from <removed-id> on <timestamp>]"`
3. Remove the duplicate decision from its source file
4. Write both files back

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
