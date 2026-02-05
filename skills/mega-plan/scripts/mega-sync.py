#!/usr/bin/env python3
"""
Mega Sync Script

Synchronizes mega-plan status from worktree states.
Updates .mega-status.json with current execution state.
"""

from __future__ import annotations

import json
import os
import re
import sys
import time
from pathlib import Path


# Try to use PathResolver for path consistency (new/legacy mode)
_PATH_RESOLVER_AVAILABLE = False
_PathResolver = None


def _setup_import_path() -> bool:
    """Setup path to import plan_cascade modules (when running from plugin)."""
    global _PATH_RESOLVER_AVAILABLE, _PathResolver

    candidates = [
        Path(__file__).parent.parent.parent.parent / "src",  # Plugin root/src
        Path.cwd() / "src",  # Development mode
    ]
    for src_path in candidates:
        if src_path.exists():
            sys.path.insert(0, str(src_path))
            try:
                from plan_cascade.state.path_resolver import PathResolver

                _PathResolver = PathResolver
                _PATH_RESOLVER_AVAILABLE = True
                return True
            except ImportError:
                pass
    return False


_setup_import_path()


def _get_data_dir() -> Path:
    if _PATH_RESOLVER_AVAILABLE and _PathResolver:
        try:
            return _PathResolver(Path.cwd(), legacy_mode=False).get_data_dir()
        except Exception:
            pass

    if sys.platform == "win32":
        appdata = os.environ.get("APPDATA")
        if appdata:
            return Path(appdata) / "plan-cascade"
        return Path.home() / "AppData" / "Roaming" / "plan-cascade"
    return Path.home() / ".plan-cascade"


def _get_project_id(project_root: Path) -> str:
    if _PATH_RESOLVER_AVAILABLE and _PathResolver:
        try:
            return _PathResolver(project_root, legacy_mode=True).get_project_id()
        except Exception:
            pass

    # Fallback: stable-ish ID based on directory name + path hash (match PathResolver closely)
    import hashlib

    root = project_root.resolve()
    name = root.name.lower()
    name = name.replace(" ", "-")
    name = re.sub(r"[^a-z0-9\\-_]", "", name)
    name = re.sub(r"-+", "-", name).strip("-")[:50] or "project"

    normalized = str(root).replace("\\", "/").lower()
    h = hashlib.sha256(normalized.encode("utf-8")).hexdigest()[:8]
    return f"{name}-{h}"


def _new_mode_path(project_root: Path, filename: str) -> Path:
    if _PATH_RESOLVER_AVAILABLE and _PathResolver:
        resolver = _PathResolver(project_root, legacy_mode=False)
        if filename == "mega-plan.json":
            return resolver.get_mega_plan_path()
        if filename == ".mega-status.json":
            return resolver.get_mega_status_path()
        if filename == ".worktree":
            return resolver.get_worktree_dir()
        return resolver.get_state_file_path(filename)

    data_dir = _get_data_dir()
    project_id = _get_project_id(project_root)
    base = data_dir / project_id
    if filename == "mega-plan.json":
        return base / "mega-plan.json"
    if filename == ".worktree":
        return base / ".worktree"
    if filename == ".mega-status.json":
        return base / ".state" / ".mega-status.json"

    # Default: state file path
    return base / ".state" / filename


def get_project_root():
    """Find project root by looking for mega-plan.json (legacy or new mode)."""
    cwd = Path.cwd()

    # Check current directory
    if (cwd / "mega-plan.json").exists():
        return cwd

    # Check new mode location for current directory
    if _new_mode_path(cwd, "mega-plan.json").exists():
        return cwd

    # Check parent directories
    for parent in cwd.parents:
        if (parent / "mega-plan.json").exists():
            return parent
        if _new_mode_path(parent, "mega-plan.json").exists():
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
        path.parent.mkdir(parents=True, exist_ok=True)
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

    # Worktree location depends on legacy/new mode
    legacy_worktree_dir = project_root / ".worktree"
    worktree_dir = legacy_worktree_dir if legacy_worktree_dir.exists() else _new_mode_path(project_root, ".worktree")
    worktree_path = worktree_dir / name

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

    total = len(stories)

    # Prefer progress.txt markers (more reliable than PRD story status fields)
    complete_ids: set[str] = set()
    failed_ids: set[str] = set()
    in_progress_ids: set[str] = set()

    if progress_path.exists():
        try:
            with open(progress_path, encoding="utf-8") as f:
                for line in f:
                    if "[COMPLETE]" in line:
                        m = re.search(r"(story-[\w-]+)", line)
                        if m:
                            complete_ids.add(m.group(1))
                    if "[FAILED]" in line or "[ERROR]" in line:
                        m = re.search(r"(story-[\w-]+)", line)
                        if m:
                            failed_ids.add(m.group(1))
                    if "[IN_PROGRESS]" in line:
                        m = re.search(r"(story-[\w-]+)", line)
                        if m:
                            in_progress_ids.add(m.group(1))
        except OSError:
            pass

    # Fall back to PRD statuses for any missing signals
    if not complete_ids and not failed_ids and not in_progress_ids:
        complete_ids = {s.get("id") for s in stories if s.get("status") == "complete" and s.get("id")}
        failed_ids = {s.get("id") for s in stories if s.get("status") == "failed" and s.get("id")}
        in_progress_ids = {s.get("id") for s in stories if s.get("status") == "in_progress" and s.get("id")}

    complete = len(complete_ids)
    in_progress_count = len(in_progress_ids)
    failed = len(failed_ids)

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
    # Resolve plan/status paths for legacy or new mode
    legacy_plan = project_root / "mega-plan.json"
    plan_path = legacy_plan if legacy_plan.exists() else _new_mode_path(project_root, "mega-plan.json")

    legacy_status = project_root / ".mega-status.json"
    status_path = legacy_status if legacy_plan.exists() else _new_mode_path(project_root, ".mega-status.json")

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

        legacy_worktree_dir = project_root / ".worktree"
        worktree_dir = legacy_worktree_dir if legacy_worktree_dir.exists() else _new_mode_path(project_root, ".worktree")
        worktree_path = worktree_dir / name

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

                # Prefer progress markers
                progress_path = worktree_path / "progress.txt"
                complete_ids: set[str] = set()
                if progress_path.exists():
                    try:
                        with open(progress_path, encoding="utf-8") as f:
                            for line in f:
                                if "[COMPLETE]" in line:
                                    m = re.search(r"(story-[\w-]+)", line)
                                    if m:
                                        complete_ids.add(m.group(1))
                    except OSError:
                        pass

                if complete_ids:
                    status["features"][fid]["stories_complete"] = len(complete_ids)
                else:
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
