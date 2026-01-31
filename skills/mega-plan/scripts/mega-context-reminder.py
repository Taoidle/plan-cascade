#!/usr/bin/env python3
"""
Mega Context Reminder Script

Generates and displays .mega-execution-context.md to help AI recover
execution context after context compression/truncation.

This script:
1. Reads current mega-plan state
2. Generates a persistent context reminder file
3. Displays critical parallel execution reminders
"""

import json
import os
import sys
import time
from pathlib import Path
from datetime import datetime


def get_project_root():
    """Find project root by looking for mega-plan.json."""
    cwd = Path.cwd()

    # Check current directory
    if (cwd / "mega-plan.json").exists():
        return cwd

    # Check parent directories
    for parent in cwd.parents:
        if (parent / "mega-plan.json").exists():
            return parent

    return None


def read_mega_plan(project_root: Path):
    """Read mega-plan.json."""
    plan_path = project_root / "mega-plan.json"
    if not plan_path.exists():
        return None

    try:
        with open(plan_path, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return None


def read_mega_status(project_root: Path):
    """Read .mega-status.json."""
    status_path = project_root / ".mega-status.json"
    if not status_path.exists():
        return None

    try:
        with open(status_path, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return None


def get_feature_batches(plan: dict) -> list:
    """Calculate feature batches based on dependencies."""
    features = plan.get("features", [])
    if not features:
        return []

    # Build dependency graph
    feature_map = {f["id"]: f for f in features}
    completed = set()
    batches = []

    remaining = list(features)
    while remaining:
        # Find features whose dependencies are all complete
        batch = []
        for feature in remaining:
            deps = feature.get("dependencies", [])
            if all(dep in completed for dep in deps):
                batch.append(feature)

        if not batch:
            # Circular dependency or missing deps - add remaining as one batch
            batch = remaining
            remaining = []
        else:
            for f in batch:
                remaining.remove(f)

        batches.append(batch)

        # Mark this batch as complete for dependency resolution
        for f in batch:
            completed.add(f["id"])

    return batches


def get_current_batch_info(plan: dict, batches: list) -> tuple:
    """Get current batch number and features."""
    for i, batch in enumerate(batches, 1):
        # Check if any feature in this batch is not complete
        for feature in batch:
            status = feature.get("status", "pending")
            if status not in ["complete", "merged"]:
                return i, batch

    return len(batches) + 1, []  # All complete


def get_active_worktrees(project_root: Path, current_batch: list) -> list:
    """Get list of active worktrees for current batch."""
    worktrees = []
    worktree_dir = project_root / ".worktree"

    for feature in current_batch:
        name = feature["name"]
        path = worktree_dir / name
        status = feature.get("status", "pending")

        if status in ["prd_generated", "approved", "in_progress"]:
            worktrees.append({
                "name": name,
                "id": feature["id"],
                "title": feature["title"],
                "path": str(path),
                "status": status,
                "exists": path.exists()
            })

    return worktrees


def generate_context_file(project_root: Path, plan: dict, batches: list,
                          current_batch_num: int, current_batch: list,
                          active_worktrees: list) -> str:
    """Generate the .mega-execution-context.md content."""
    timestamp = datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")
    total_batches = len(batches)

    # Determine execution status
    if not current_batch:
        exec_status = "COMPLETE"
    elif any(f.get("status") in ["in_progress", "approved"] for f in current_batch):
        exec_status = "IN_PROGRESS"
    else:
        exec_status = "PENDING"

    content = f"""<!-- AUTO-GENERATED - DO NOT EDIT MANUALLY -->
<!-- Last Updated: {timestamp} -->

# MEGA-PLAN PARALLEL EXECUTION ACTIVE

## Current State
- **Mode**: MEGA_PLAN
- **Batch**: {current_batch_num} of {total_batches}
- **Execution**: {exec_status}
- **Goal**: {plan.get('goal', 'N/A')}
- **Target Branch**: {plan.get('target_branch', 'main')}

## Active Worktrees (PARALLEL EXECUTION REQUIRED)

| Feature | Worktree Path | Status |
|---------|---------------|--------|
"""

    if active_worktrees:
        for wt in active_worktrees:
            exists_marker = "exists" if wt["exists"] else "NOT CREATED"
            content += f"| {wt['name']} | `{wt['path']}/` | {wt['status']} ({exists_marker}) |\n"
    else:
        content += "| (no active worktrees) | - | - |\n"

    content += f"""
## CRITICAL RULES

1. **DO NOT** work in main/master branch for feature code
2. **ALL** feature work MUST happen in respective worktrees listed above
3. Use **Task agents** to execute features **IN PARALLEL**
4. Each Task agent works in its own worktree directory
5. Features in the same batch are **independent** and can run simultaneously

## Pending Features in Current Batch

"""

    for feature in current_batch:
        status = feature.get("status", "pending")
        if status != "complete":
            content += f"- **{feature['id']}**: {feature['title']} (status: {status})\n"

    content += f"""
## Recovery Command

If context was compressed/truncated or you're unsure of the current state:

```
/plan-cascade:resume
```

Or use the specific command:
```
/plan-cascade:mega-resume --auto-prd
```

This will:
- Auto-detect current state from files
- Skip already-completed work
- Resume parallel execution from where it left off

## Quick Status

Run `/plan-cascade:mega-status` to see detailed progress.
"""

    return content


def display_brief_reminder(current_batch_num: int, total_batches: int,
                           active_worktrees: list, plan: dict):
    """Display brief reminder to stdout (for hooks)."""
    print()
    print("+" + "=" * 62 + "+")
    print("|  MEGA-PLAN ACTIVE - PARALLEL EXECUTION REQUIRED" + " " * 14 + "|")
    print("+" + "=" * 62 + "+")
    print(f"|  Batch: {current_batch_num}/{total_batches}  |  Mode: {plan.get('execution_mode', 'auto')}" + " " * 30 + "|"[:64])

    if active_worktrees:
        print("|" + "-" * 62 + "|")
        print("|  Active Worktrees (use these for feature work):" + " " * 14 + "|")
        for wt in active_worktrees[:3]:  # Show max 3
            line = f"|    - {wt['path']}/"
            print(line + " " * max(0, 63 - len(line)) + "|")
        if len(active_worktrees) > 3:
            print(f"|    ... and {len(active_worktrees) - 3} more" + " " * 44 + "|")

    print("|" + "-" * 62 + "|")
    print("|  DO NOT execute in main branch! Use parallel Task agents!   |")
    print("|  If context lost: /plan-cascade:resume (auto-detects mode)  |")
    print("+" + "=" * 62 + "+")
    print()


def main():
    """Main entry point."""
    project_root = get_project_root()

    if not project_root:
        # No mega-plan found, exit silently
        sys.exit(0)

    plan = read_mega_plan(project_root)
    if not plan:
        sys.exit(0)

    # Calculate batches and current state
    batches = get_feature_batches(plan)
    current_batch_num, current_batch = get_current_batch_info(plan, batches)
    active_worktrees = get_active_worktrees(project_root, current_batch)

    # Check mode: "update" to write file, "display" to show reminder, default is both
    mode = sys.argv[1] if len(sys.argv) > 1 else "both"

    if mode in ["update", "both"]:
        # Generate and write context file
        content = generate_context_file(
            project_root, plan, batches,
            current_batch_num, current_batch, active_worktrees
        )
        context_file = project_root / ".mega-execution-context.md"
        try:
            with open(context_file, "w", encoding="utf-8") as f:
                f.write(content)
        except Exception as e:
            print(f"Warning: Could not write context file: {e}", file=sys.stderr)

    if mode in ["display", "both"]:
        # Display brief reminder
        display_brief_reminder(current_batch_num, len(batches), active_worktrees, plan)


if __name__ == "__main__":
    main()
