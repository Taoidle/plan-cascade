---
description: "Show execution status dashboard. Usage: /plan-cascade:dashboard [--verbose|-v] [--json]"
---

# Plan Cascade Dashboard

Display execution status for the current Plan Cascade task.

## Step 1: Parse Arguments

```bash
VERBOSE="False"
JSON_OUTPUT="False"

for arg in $ARGUMENTS; do
    case "$arg" in
        --verbose|-v) VERBOSE="True" ;;
        --json) JSON_OUTPUT="True" ;;
    esac
done
```

## Step 2: Detect State Files

**CRITICAL**: Use the Read tool to check for state files first.

Check for existence of:
- `mega-plan.json` - indicates MEGA mode
- `prd.json` - indicates HYBRID mode
- `progress.txt` - execution tracking
- `.iteration-state.json` - iteration state

## Step 3: Run Dashboard Aggregator

```bash
uv run python << 'PYTHON_EOF'
import sys
import json
from pathlib import Path

# Setup import path for plan_cascade module
plugin_root = Path(r"${CLAUDE_PLUGIN_ROOT}") if "${CLAUDE_PLUGIN_ROOT}" else None
if plugin_root and (plugin_root / "src").exists():
    sys.path.insert(0, str(plugin_root / "src"))
else:
    # Try current directory parent (for development)
    if (Path.cwd().parent / "src").exists():
        sys.path.insert(0, str(Path.cwd().parent / "src"))

project_root = Path.cwd()
verbose = ${VERBOSE}
json_output = ${JSON_OUTPUT}

try:
    # Use the dashboard module's public API
    from plan_cascade.core.dashboard import get_dashboard, format_dashboard

    # Get aggregated dashboard state
    state = get_dashboard(
        project_root=project_root,
        legacy_mode=True,  # Use legacy mode for compatibility
    )

    if json_output:
        # JSON output using state.to_dict()
        print(json.dumps(state.to_dict(), indent=2, default=str))
    else:
        # Formatted output using the dashboard formatter
        print(format_dashboard(state, verbose=verbose))

except ImportError as e:
    # Fallback if dashboard module not available
    print(f"Dashboard module not available: {e}")
    print("")
    print("=" * 60)
    print("PLAN CASCADE DASHBOARD")
    print("=" * 60)
    print("")

    # Simple fallback detection
    has_mega = (project_root / "mega-plan.json").exists()
    has_prd = (project_root / "prd.json").exists()

    if has_mega:
        print("Strategy: MEGA (Multi-Feature)")
        print("Status: Run /plan-cascade:mega-status for details")
    elif has_prd:
        print("Strategy: HYBRID (Single Feature)")
        print("Status: Run /plan-cascade:hybrid-status for details")
    else:
        print("Status: NO ACTIVE EXECUTION")
        print("")
        print("Start a new task with:")
        print("  /plan-cascade:hybrid-auto <description>")
        print("  /plan-cascade:mega-plan <description>")

    print("=" * 60)

except Exception as e:
    print(f"Dashboard error: {e}")
    import traceback
    traceback.print_exc()
    print("")
    print("No active execution found. Start with:")
    print("  /plan-cascade:hybrid-auto <description>")
    print("  /plan-cascade:mega-plan <description>")
PYTHON_EOF
```

## Step 4: Explain Output

The dashboard displays:
- **Strategy**: MEGA (multi-feature) or HYBRID (single feature)
- **Status**: NOT_STARTED, IN_PROGRESS, COMPLETED, or FAILED
- **Progress**: Completion percentage and story/feature counts
- **Quality Gates**: DoR, Verification, Review, TDD, DoD pass counts
- **Recommended Actions**: Context-aware next steps

### Options

| Option | Description |
|--------|-------------|
| `--verbose` / `-v` | Show individual story/feature status |
| `--json` | Output in JSON format for programmatic use |

### Examples

```bash
# Basic status
/plan-cascade:dashboard

# Detailed view with story breakdown
/plan-cascade:dashboard --verbose

# JSON output for automation
/plan-cascade:dashboard --json
```
