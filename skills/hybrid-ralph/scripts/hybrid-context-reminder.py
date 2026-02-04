#!/usr/bin/env python3
"""
Hybrid Worktree Context Reminder Script

Generates and displays .hybrid-execution-context.md to help AI recover
execution context after context compression/truncation.

This script:
1. Reads current hybrid-worktree state
2. Generates a persistent context reminder file
3. Displays critical execution reminders
"""

import hashlib
import json
import os
import re
import sys
import time
from pathlib import Path
from datetime import datetime, timezone


# Try to use PathResolver for path consistency
_PATH_RESOLVER_AVAILABLE = False
_PathResolver = None

def _setup_import_path():
    """Setup path to import plan_cascade modules."""
    global _PATH_RESOLVER_AVAILABLE, _PathResolver

    # Find the plugin root
    candidates = [
        Path(__file__).parent.parent.parent.parent / "src",  # Standard location
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


def get_data_dir() -> Path:
    """Get the platform-specific data directory for Plan Cascade."""
    if _PATH_RESOLVER_AVAILABLE and _PathResolver:
        # Use PathResolver's implementation if available
        try:
            return _PathResolver(Path.cwd(), legacy_mode=False).get_data_dir()
        except Exception:
            pass

    # Fallback to inline implementation
    if sys.platform == "win32":
        appdata = os.environ.get("APPDATA")
        if appdata:
            return Path(appdata) / "plan-cascade"
        return Path.home() / "AppData" / "Roaming" / "plan-cascade"
    else:
        return Path.home() / ".plan-cascade"


def get_project_id(project_root: Path) -> str:
    """Compute a unique, filesystem-safe project ID from the project root path.

    Uses PathResolver if available, otherwise falls back to inline implementation.
    """
    if _PATH_RESOLVER_AVAILABLE and _PathResolver:
        resolver = _PathResolver(project_root, legacy_mode=True)
        return resolver.get_project_id()

    # Fallback implementation (must match PathResolver exactly)
    project_root = project_root.resolve()

    name = project_root.name
    # Sanitize name - must match PathResolver._sanitize_name() exactly
    sanitized = name.replace(" ", "-")
    sanitized = re.sub(r"[^a-zA-Z0-9\-_]", "", sanitized)
    sanitized = sanitized.lower()  # Convert to lowercase
    sanitized = re.sub(r"-+", "-", sanitized)  # Remove consecutive hyphens
    sanitized = sanitized.strip("-")  # Remove leading/trailing hyphens
    if not sanitized:
        sanitized = "project"
    sanitized = sanitized[:50]  # Match PathResolver limit (was 32, now 50)

    # Compute hash
    path_str = str(project_root)
    normalized_path = path_str.replace("\\", "/").lower()
    path_hash = hashlib.sha256(normalized_path.encode("utf-8")).hexdigest()[:8]

    return f"{sanitized}-{path_hash}"


def get_new_mode_prd_path(project_root: Path) -> Path:
    """Get the prd.json path in new mode (user data directory)."""
    if _PATH_RESOLVER_AVAILABLE and _PathResolver:
        resolver = _PathResolver(project_root, legacy_mode=False)
        return resolver.get_prd_path()

    # Fallback implementation
    data_dir = get_data_dir()
    project_id = get_project_id(project_root)
    return data_dir / project_id / "prd.json"


def get_worktree_root():
    """Find worktree root by looking for .planning-config.json or prd.json.

    Checks both legacy (project root) and new mode (user data directory) locations.
    """
    cwd = Path.cwd()

    # Check current directory for hybrid worktree indicators
    if (cwd / ".planning-config.json").exists():
        return cwd, "worktree"

    if (cwd / "prd.json").exists():
        return cwd, "regular"

    # Check new mode location for prd.json
    new_mode_prd = get_new_mode_prd_path(cwd)
    if new_mode_prd.exists():
        return cwd, "regular"

    # Check parent directories
    for parent in cwd.parents:
        if (parent / ".planning-config.json").exists():
            return parent, "worktree"
        if (parent / "prd.json").exists():
            return parent, "regular"
        # Also check new mode location for each parent
        new_mode_prd = get_new_mode_prd_path(parent)
        if new_mode_prd.exists():
            return parent, "regular"

    return None, None


def read_planning_config(root: Path):
    """Read .planning-config.json."""
    config_path = root / ".planning-config.json"
    if not config_path.exists():
        return None

    try:
        # Use utf-8-sig to tolerate BOM (common on Windows when files are written via PowerShell)
        with open(config_path, "r", encoding="utf-8-sig") as f:
            return json.load(f)
    except Exception:
        return None


def read_prd(root: Path):
    """Read prd.json from legacy or new mode location."""
    # Try legacy location first (project root)
    prd_path = root / "prd.json"
    if prd_path.exists():
        try:
            # Use utf-8-sig to tolerate BOM (common on Windows when files are written via PowerShell)
            with open(prd_path, "r", encoding="utf-8-sig") as f:
                return json.load(f)
        except Exception:
            pass

    # Try new mode location (user data directory)
    new_mode_prd = get_new_mode_prd_path(root)
    if new_mode_prd.exists():
        try:
            # Use utf-8-sig to tolerate BOM (common on Windows when files are written via PowerShell)
            with open(new_mode_prd, "r", encoding="utf-8-sig") as f:
                return json.load(f)
        except Exception:
            pass

    return None


def read_progress(root: Path) -> dict:
    """Read progress.txt and extract completion info."""
    progress_path = root / "progress.txt"
    if not progress_path.exists():
        return {"complete": [], "failed": [], "in_progress": []}

    try:
        with open(progress_path, "r", encoding="utf-8") as f:
            content = f.read()
    except Exception:
        return {"complete": [], "failed": [], "in_progress": []}

    complete = []
    failed = []
    in_progress = []

    for line in content.split("\n"):
        line = line.strip()
        # New-style markers
        if "[STORY_COMPLETE]" in line or "[COMPLETE]" in line:
            # Extract story ID
            for word in line.split():
                if word.startswith("story-"):
                    complete.append(word)
                    break
        elif "[STORY_FAILED]" in line or "[FAILED]" in line or "[ERROR]" in line:
            for word in line.split():
                if word.startswith("story-"):
                    failed.append(word)
                    break
        elif "[IN_PROGRESS]" in line:
            for word in line.split():
                if word.startswith("story-"):
                    in_progress.append(word)
                    break

    return {
        "complete": list(set(complete)),
        "failed": list(set(failed)),
        "in_progress": list(set(in_progress))
    }


def get_story_batches(prd: dict) -> list:
    """Calculate story batches based on dependencies."""
    stories = prd.get("stories", [])
    if not stories:
        return []

    story_map = {s["id"]: s for s in stories}
    completed = set()
    batches = []

    remaining = list(stories)
    while remaining:
        batch = []
        for story in remaining:
            deps = story.get("dependencies", [])
            if all(dep in completed for dep in deps):
                batch.append(story)

        if not batch:
            batch = remaining
            remaining = []
        else:
            for s in batch:
                remaining.remove(s)

        batches.append(batch)
        for s in batch:
            completed.add(s["id"])

    return batches


def get_current_batch_info(prd: dict, batches: list, progress: dict) -> tuple:
    """Get current batch number and pending stories."""
    complete_set = set(progress["complete"])

    for i, batch in enumerate(batches, 1):
        pending_in_batch = [s for s in batch if s["id"] not in complete_set]
        if pending_in_batch:
            return i, batch, pending_in_batch

    return len(batches) + 1, [], []


def generate_context_file(root: Path, context_type: str, config: dict,
                          prd: dict, batches: list, current_batch_num: int,
                          current_batch: list, pending_stories: list,
                          progress: dict) -> str:
    """Generate the .hybrid-execution-context.md content."""
    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    total_batches = len(batches)
    total_stories = len(prd.get("stories", []))
    complete_count = len(progress["complete"])
    failed_count = len(progress["failed"])

    # Determine execution status
    if complete_count >= total_stories:
        exec_status = "COMPLETE"
    elif progress["in_progress"] or pending_stories:
        exec_status = "IN_PROGRESS"
    else:
        exec_status = "PENDING"

    # Get task info
    if context_type == "worktree" and config:
        task_name = config.get("task_name", "unknown")
        target_branch = config.get("target_branch", "main")
        worktree_dir = config.get("worktree_dir", str(root))
    else:
        task_name = prd.get("metadata", {}).get("description", "Hybrid Task")[:50]
        target_branch = "N/A"
        worktree_dir = str(root)

    content = f"""<!-- AUTO-GENERATED - DO NOT EDIT MANUALLY -->
<!-- Last Updated: {timestamp} -->

# HYBRID-WORKTREE EXECUTION ACTIVE

## Current State
- **Mode**: {"HYBRID_WORKTREE" if context_type == "worktree" else "HYBRID_AUTO"}
- **Task**: {task_name}
- **Batch**: {current_batch_num} of {total_batches}
- **Execution**: {exec_status}
- **Goal**: {prd.get('goal', 'N/A')}
"""

    if context_type == "worktree":
        content += f"""- **Target Branch**: {target_branch}
- **Worktree Path**: {worktree_dir}
"""

    content += f"""
## Progress Summary

| Metric | Count |
|--------|-------|
| Total Stories | {total_stories} |
| Completed | {complete_count} |
| Failed | {failed_count} |
| Pending | {total_stories - complete_count - failed_count} |

## Current Batch ({current_batch_num})

Stories to execute in parallel:

| Story ID | Title | Status |
|----------|-------|--------|
"""

    for story in current_batch:
        sid = story["id"]
        title = story.get("title", "Untitled")[:40]
        if sid in progress["complete"]:
            status = "complete"
        elif sid in progress["failed"]:
            status = "failed"
        elif sid in progress["in_progress"]:
            status = "in_progress"
        else:
            status = "pending"
        content += f"| {sid} | {title} | {status} |\n"

    content += f"""
## CRITICAL RULES

1. Stories in the same batch are **independent** and can run in **parallel**
2. Use **Task agents** with `run_in_background: true` for parallel execution
3. Wait for batch completion before starting next batch
4. Update `progress.txt` with `[STORY_COMPLETE] story-xxx` markers
5. Update `findings.md` with discoveries (tag with `<!-- @tags: story-xxx -->`)

## Recovery Command

If context was compressed/truncated or you're unsure of the current state:

```
/plan-cascade:resume
```

Or use the specific command:
```
/plan-cascade:hybrid-resume --auto
```

This will:
- Auto-detect current state from files
- Skip already-completed stories
- Resume execution from where it left off

## Quick Commands

- View status: `/plan-cascade:hybrid-status`
- Edit PRD: `/plan-cascade:edit`
- Show dependencies: `/plan-cascade:show-dependencies`
"""

    if context_type == "worktree":
        content += f"""- Complete and merge: `/plan-cascade:hybrid-complete`
"""

    return content


def display_brief_reminder(context_type: str, config: dict, prd: dict,
                           current_batch_num: int, total_batches: int,
                           total_stories: int, complete_count: int,
                           pending_stories: list):
    """Display brief reminder to stdout (for hooks)."""
    mode_name = "HYBRID-WORKTREE" if context_type == "worktree" else "HYBRID-AUTO"

    if config:
        task_name = config.get("task_name", "task")
    else:
        task_name = "task"

    print()
    print("+" + "=" * 58 + "+")
    print(f"|  {mode_name} EXECUTION ACTIVE" + " " * (58 - len(mode_name) - 19) + "|")
    print("+" + "=" * 58 + "+")
    print(f"|  Task: {task_name[:40]}" + " " * max(0, 49 - len(task_name[:40])) + "|")
    print(f"|  Progress: {complete_count}/{total_stories} stories | Batch: {current_batch_num}/{total_batches}" + " " * 20 + "|"[:60])
    print("|" + "-" * 58 + "|")

    if pending_stories:
        print("|  Pending in current batch:" + " " * 31 + "|")
        for story in pending_stories[:3]:
            sid = story["id"]
            print(f"|    - {sid}" + " " * max(0, 51 - len(sid)) + "|")
        if len(pending_stories) > 3:
            print(f"|    ... and {len(pending_stories) - 3} more" + " " * 40 + "|")

    print("|" + "-" * 58 + "|")
    print("|  If context lost: /plan-cascade:resume (auto-detects)   |")
    print("+" + "=" * 58 + "+")
    print()


def main():
    """Main entry point."""
    root, context_type = get_worktree_root()

    if not root:
        # No hybrid task found, exit silently
        sys.exit(0)

    config = read_planning_config(root) if context_type == "worktree" else None
    prd = read_prd(root)

    if not prd:
        sys.exit(0)

    progress = read_progress(root)
    batches = get_story_batches(prd)
    current_batch_num, current_batch, pending_stories = get_current_batch_info(prd, batches, progress)

    total_stories = len(prd.get("stories", []))
    complete_count = len(progress["complete"])

    # Check mode
    mode = sys.argv[1] if len(sys.argv) > 1 else "both"

    if mode in ["update", "both"]:
        content = generate_context_file(
            root, context_type, config, prd, batches,
            current_batch_num, current_batch, pending_stories, progress
        )
        context_file = root / ".hybrid-execution-context.md"
        try:
            with open(context_file, "w", encoding="utf-8") as f:
                f.write(content)
        except Exception as e:
            print(f"Warning: Could not write context file: {e}", file=sys.stderr)

    if mode in ["display", "both"]:
        display_brief_reminder(
            context_type, config, prd,
            current_batch_num, len(batches),
            total_stories, complete_count, pending_stories
        )


if __name__ == "__main__":
    main()
