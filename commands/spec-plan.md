---
description: "Run a resumable spec interview to produce spec.json/spec.md (planning-time), then optionally compile to prd.json. Usage: /plan-cascade:spec-plan \"<desc>\" [--output-dir <dir>] [--flow <quick|standard|full>] [--first-principles] [--max-questions N] [--feature-slug <slug>] [--compile] [--tdd <off|on|auto>] [--confirm] [--no-confirm]"
---

# Plan Cascade - Spec Plan (Interview)

Start (or resume) a **Specification Interview** and generate:
- `spec.json` (structured spec)
- `spec.md` (human-readable spec)
- `.state/spec-interview.json` (resume state)

Optionally compile `spec.json` → `prd.json`.

## Step 1: Parse Arguments

Parse:
- `<desc>`: required description
- `--output-dir <dir>`: optional (default: current directory)
- `--flow <quick|standard|full>`: optional (default: standard)
- `--first-principles`: optional
- `--max-questions N`: optional (default: 18)
- `--feature-slug <slug>`: optional (useful for Mega features)
- `--compile`: optional (also run compile after interview)
- `--tdd <off|on|auto>`: optional (only used with `--compile`)
- `--confirm` / `--no-confirm`: optional (only used with `--compile`, `--no-confirm` wins)

## Step 2: Run Interview (CLI)

Run:

```bash
uv run plan-cascade spec plan "<desc>" \
  --output-dir "<output-dir>" \
  --flow <flow> \
  --mode on \
  --max-questions <N> \
  --feature-slug "<slug>" \
  $( [ "<first-principles>" = true ] && echo "--first-principles" )
```

## Step 3 (Optional): Compile to PRD

If `--compile`:

```bash
uv run plan-cascade spec compile \
  --output-dir "<output-dir>" \
  --flow <flow> \
  --tdd <tdd> \
  $( [ "<no-confirm>" = true ] && echo "--no-confirm" || true ) \
  $( [ "<confirm>" = true ] && echo "--confirm" || true )
```

## Output Files

- `spec.json`
- `spec.md`
- `.state/spec-interview.json`
- `prd.json` (if `--compile`)

