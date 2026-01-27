#!/usr/bin/env python3
"""
Mega Status Script

Displays brief or detailed status of mega-plan execution.
Used by hooks and commands.
"""

import json
import os
import sys
from pathlib import Path


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

    return cwd


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


def get_feature_progress(project_root: Path, feature: dict) -> dict:
    """Get progress for a single feature."""
    worktree_path = project_root / ".worktree" / feature["name"]

    result = {
        "status": feature.get("status", "pending"),
        "worktree_exists": worktree_path.exists(),
        "stories_total": 0,
        "stories_complete": 0
    }

    if worktree_path.exists():
        prd_path = worktree_path / "prd.json"
        if prd_path.exists():
            try:
                with open(prd_path, "r", encoding="utf-8") as f:
                    prd = json.load(f)
                stories = prd.get("stories", [])
                result["stories_total"] = len(stories)
                result["stories_complete"] = sum(1 for s in stories if s.get("status") == "complete")
            except Exception:
                pass

    return result


def display_brief(project_root: Path, plan: dict):
    """Display brief status (for hooks)."""
    features = plan.get("features", [])
    total = len(features)
    complete = sum(1 for f in features if f.get("status") == "complete")
    in_progress = sum(1 for f in features if f.get("status") in ["prd_generated", "approved", "in_progress"])

    pct = int((complete / total) * 100) if total > 0 else 0

    print(f"Progress: {pct}% ({complete}/{total} features)")
    print(f"Mode: {plan.get('execution_mode', 'auto')} | Target: {plan.get('target_branch', 'main')}")


def display_full(project_root: Path, plan: dict):
    """Display full status."""
    features = plan.get("features", [])
    total = len(features)
    complete = sum(1 for f in features if f.get("status") == "complete")

    pct = int((complete / total) * 100) if total > 0 else 0

    # Progress bar
    bar_width = 20
    filled = int(bar_width * pct / 100)
    bar = "█" * filled + "░" * (bar_width - filled)

    print("=" * 60)
    print("MEGA PLAN STATUS")
    print("=" * 60)
    print()
    print(f"Goal: {plan.get('goal', 'N/A')}")
    print(f"Mode: {plan.get('execution_mode', 'auto')}")
    print(f"Target: {plan.get('target_branch', 'main')}")
    print()
    print(f"Progress: {bar} {pct}% ({complete}/{total} features)")
    print()
    print("Features:")

    # Group by status
    status_symbols = {
        "pending": "[ ]",
        "prd_generated": "[~]",
        "approved": "[~]",
        "in_progress": "[>]",
        "complete": "[X]",
        "failed": "[!]"
    }

    for feature in features:
        fid = feature["id"]
        name = feature["name"]
        title = feature["title"]
        status = feature.get("status", "pending")
        symbol = status_symbols.get(status, "[?]")

        progress = get_feature_progress(project_root, feature)

        story_info = ""
        if progress["stories_total"] > 0:
            story_info = f" ({progress['stories_complete']}/{progress['stories_total']} stories)"

        deps = feature.get("dependencies", [])
        dep_info = f" → {', '.join(deps)}" if deps else ""

        print(f"  {symbol} {fid}: {title}{story_info}{dep_info}")

    print()
    print("=" * 60)


def main():
    """Main entry point."""
    project_root = get_project_root()
    plan = read_mega_plan(project_root)

    if not plan:
        print("No mega-plan.json found")
        sys.exit(1)

    # Check for brief mode
    mode = sys.argv[1] if len(sys.argv) > 1 else "full"

    if mode == "brief":
        display_brief(project_root, plan)
    else:
        display_full(project_root, plan)


if __name__ == "__main__":
    main()
