#!/usr/bin/env python3
"""
Mega Sync Script

Synchronizes mega-plan status from worktree states.
Updates .mega-status.json with current execution state.
"""

import json
import os
import sys
import time
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


def read_json(path: Path):
    """Read a JSON file."""
    if not path.exists():
        return None
    try:
        with open(path, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return None


def write_json(path: Path, data: dict):
    """Write a JSON file."""
    try:
        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2)
        return True
    except Exception:
        return False


def determine_feature_status(project_root: Path, feature: dict) -> str:
    """
    Determine actual feature status by checking worktree.

    Returns one of: pending, prd_generated, approved, in_progress, complete, failed
    """
    name = feature["name"]
    current_status = feature.get("status", "pending")
    worktree_path = project_root / ".worktree" / name

    # If no worktree, it's pending
    if not worktree_path.exists():
        return "pending"

    prd_path = worktree_path / "prd.json"
    progress_path = worktree_path / "progress.txt"

    # Check if PRD exists
    if not prd_path.exists():
        return "pending"

    # Read PRD
    prd = read_json(prd_path)
    if not prd:
        return "prd_generated"

    stories = prd.get("stories", [])
    if not stories:
        return "prd_generated"

    # Check story statuses
    total = len(stories)
    complete = sum(1 for s in stories if s.get("status") == "complete")
    in_progress_count = sum(1 for s in stories if s.get("status") == "in_progress")
    failed = sum(1 for s in stories if s.get("status") == "failed")

    # Determine status
    if failed > 0:
        return "failed"
    if complete == total:
        return "complete"
    if in_progress_count > 0 or complete > 0:
        return "in_progress"
    if current_status in ["approved", "in_progress"]:
        return current_status

    return "prd_generated"


def get_current_batch(features: list, batches: list) -> int:
    """Determine current batch number."""
    for i, batch in enumerate(batches, 1):
        batch_ids = [f["id"] for f in batch]
        for feature in features:
            if feature["id"] in batch_ids:
                if feature.get("status") not in ["complete"]:
                    return i
    return len(batches) + 1  # All complete


def generate_batches(features: list) -> list:
    """Generate execution batches from features."""
    if not features:
        return []

    feature_map = {f["id"]: f for f in features}
    completed = set()
    batches = []

    while len(completed) < len(features):
        ready = []

        for feature in features:
            fid = feature["id"]
            if fid in completed:
                continue

            deps = feature.get("dependencies", [])
            if all(dep in completed for dep in deps):
                ready.append(feature)

        if not ready:
            remaining = [f for f in features if f["id"] not in completed]
            ready = remaining

        batches.append(ready)
        completed.update(f["id"] for f in ready)

    return batches


def sync_mega_plan(project_root: Path):
    """Sync mega-plan status from worktrees."""
    plan_path = project_root / "mega-plan.json"
    status_path = project_root / ".mega-status.json"

    plan = read_json(plan_path)
    if not plan:
        print("No mega-plan.json found")
        return False

    features = plan.get("features", [])
    updated = False

    # Update each feature status
    for feature in features:
        new_status = determine_feature_status(project_root, feature)
        if new_status != feature.get("status"):
            feature["status"] = new_status
            updated = True

    # Write updated plan if changed
    if updated:
        write_json(plan_path, plan)

    # Generate batches
    batches = generate_batches(features)

    # Create status file
    status = {
        "updated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "execution_mode": plan.get("execution_mode", "auto"),
        "target_branch": plan.get("target_branch", "main"),
        "current_batch": get_current_batch(features, batches),
        "total_batches": len(batches),
        "features": {}
    }

    for feature in features:
        fid = feature["id"]
        name = feature["name"]
        worktree_path = project_root / ".worktree" / name

        status["features"][fid] = {
            "name": name,
            "title": feature.get("title", ""),
            "status": feature.get("status", "pending"),
            "worktree_path": str(worktree_path) if worktree_path.exists() else None,
            "stories_total": 0,
            "stories_complete": 0
        }

        # Get story counts if worktree exists
        if worktree_path.exists():
            prd_path = worktree_path / "prd.json"
            prd = read_json(prd_path)
            if prd:
                stories = prd.get("stories", [])
                status["features"][fid]["stories_total"] = len(stories)
                status["features"][fid]["stories_complete"] = sum(
                    1 for s in stories if s.get("status") == "complete"
                )

    write_json(status_path, status)

    return True


def main():
    """Main entry point."""
    project_root = get_project_root()

    if sync_mega_plan(project_root):
        if len(sys.argv) > 1 and sys.argv[1] == "-v":
            print("Mega-plan status synced successfully")
    else:
        sys.exit(1)


if __name__ == "__main__":
    main()
