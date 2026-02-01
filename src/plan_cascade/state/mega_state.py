#!/usr/bin/env python3
"""
Mega State Manager for Plan Cascade

Provides thread-safe file operations for mega-plan state files.
Handles mega-plan.json, .mega-status.json, and mega-findings.md with concurrent access safety.

Now integrated with PathResolver for unified path resolution.
"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path
from typing import TYPE_CHECKING

from .state_manager import FileLock

if TYPE_CHECKING:
    from plan_cascade.state.path_resolver import PathResolver


class MegaStateManager:
    """Manages mega-plan state files with locking.

    Can operate in two modes:
    - New mode (with PathResolver): Files stored in ~/.plan-cascade/<project-id>/
    - Legacy mode: Files stored in project root (backward compatible)
    """

    def __init__(
        self,
        project_root: Path,
        path_resolver: PathResolver | None = None,
        legacy_mode: bool | None = None,
    ):
        """
        Initialize the mega state manager.

        Args:
            project_root: Root directory of the project
            path_resolver: Optional PathResolver instance. If not provided,
                creates a default one based on legacy_mode setting.
            legacy_mode: If True, use project root for all paths (backward compatible).
                If None, defaults to True when path_resolver is not provided for
                backward compatibility. If False, uses new ~/.plan-cascade/<project-id>/
                structure.
        """
        self.project_root = Path(project_root)

        # Determine legacy mode and set up PathResolver
        if path_resolver is not None:
            # Use provided resolver's mode
            self._path_resolver = path_resolver
        else:
            # Create default resolver
            # Default to legacy mode for backward compatibility when no resolver provided
            if legacy_mode is None:
                legacy_mode = True
            from plan_cascade.state.path_resolver import PathResolver
            self._path_resolver = PathResolver(
                project_root=self.project_root,
                legacy_mode=legacy_mode,
            )

        # Initialize paths using PathResolver
        self._init_paths()

    def _init_paths(self) -> None:
        """Initialize all file paths using PathResolver."""
        resolver = self._path_resolver

        # Lock directory
        self.locks_dir = resolver.get_locks_dir()

        # Mega plan file
        self.mega_plan_path = resolver.get_mega_plan_path()

        # Mega status file
        self.mega_status_path = resolver.get_mega_status_path()

        # Mega findings - stays in project root as user-visible file
        self.mega_findings_path = resolver.get_mega_findings_path()

        # Worktree directory
        self.worktree_dir = resolver.get_worktree_dir()

    @property
    def path_resolver(self) -> PathResolver:
        """Get the PathResolver instance."""
        return self._path_resolver

    def is_legacy_mode(self) -> bool:
        """Check if running in legacy mode."""
        return self._path_resolver.is_legacy_mode()

    def ensure_directories(self) -> None:
        """Ensure all necessary directories exist."""
        self._path_resolver.ensure_directories()

    def _get_lock_path(self, file_path: Path) -> Path:
        """Get the lock file path for a given file."""
        return self.locks_dir / f"{file_path.name}.lock"

    # ========== Mega Plan Operations ==========

    def read_mega_plan(self) -> dict | None:
        """
        Read the mega-plan.json file safely.

        Returns:
            Mega-plan dictionary or None if not found
        """
        if not self.mega_plan_path.exists():
            return None

        lock_path = self._get_lock_path(self.mega_plan_path)

        with FileLock(lock_path):
            try:
                with open(self.mega_plan_path, encoding="utf-8") as f:
                    return json.load(f)
            except (OSError, json.JSONDecodeError) as e:
                raise OSError(f"Could not read mega-plan: {e}")

    def write_mega_plan(self, plan: dict) -> None:
        """
        Write the mega-plan.json file safely.

        Args:
            plan: Mega-plan dictionary to write
        """
        lock_path = self._get_lock_path(self.mega_plan_path)

        with FileLock(lock_path):
            try:
                # Ensure parent directory exists
                self.mega_plan_path.parent.mkdir(parents=True, exist_ok=True)
                with open(self.mega_plan_path, "w", encoding="utf-8") as f:
                    json.dump(plan, f, indent=2)
            except OSError as e:
                raise OSError(f"Could not write mega-plan: {e}")

    def update_feature_status(self, feature_id: str, status: str) -> None:
        """
        Update the status of a feature in the mega-plan.

        Args:
            feature_id: Feature ID to update
            status: New status (pending, prd_generated, approved, in_progress, complete, failed)
        """
        plan = self.read_mega_plan()
        if not plan:
            raise ValueError("No mega-plan found")

        for feature in plan.get("features", []):
            if feature.get("id") == feature_id:
                feature["status"] = status
                break

        self.write_mega_plan(plan)
        self._sync_status_file()

    # ========== Status File Operations ==========

    def read_status(self) -> dict | None:
        """
        Read the .mega-status.json file.

        Returns:
            Status dictionary or None if not found
        """
        if not self.mega_status_path.exists():
            return None

        lock_path = self._get_lock_path(self.mega_status_path)

        with FileLock(lock_path):
            try:
                with open(self.mega_status_path, encoding="utf-8") as f:
                    return json.load(f)
            except (OSError, json.JSONDecodeError) as e:
                raise OSError(f"Could not read mega-status: {e}")

    def write_status(self, status: dict) -> None:
        """
        Write the .mega-status.json file.

        Args:
            status: Status dictionary to write
        """
        lock_path = self._get_lock_path(self.mega_status_path)

        with FileLock(lock_path):
            try:
                # Ensure parent directory exists
                self.mega_status_path.parent.mkdir(parents=True, exist_ok=True)
                with open(self.mega_status_path, "w", encoding="utf-8") as f:
                    json.dump(status, f, indent=2)
            except OSError as e:
                raise OSError(f"Could not write mega-status: {e}")

    def _sync_status_file(self) -> None:
        """
        Synchronize .mega-status.json with current state.
        """
        plan = self.read_mega_plan()
        if not plan:
            return

        status = {
            "updated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "execution_mode": plan.get("execution_mode", "auto"),
            "target_branch": plan.get("target_branch", "main"),
            "current_batch": self._get_current_batch_number(plan),
            "features": {}
        }

        for feature in plan.get("features", []):
            fid = feature["id"]
            status["features"][fid] = {
                "name": feature["name"],
                "status": feature["status"],
                "worktree_path": str(self.worktree_dir / feature["name"]) if feature["status"] != "pending" else None
            }

        self.write_status(status)

    def _get_current_batch_number(self, plan: dict) -> int:
        """Determine current batch number based on feature statuses."""
        # Import here to avoid circular imports
        from ..core.mega_generator import MegaPlanGenerator
        mg = MegaPlanGenerator(self.project_root)
        batches = mg.generate_feature_batches(plan)

        for i, batch in enumerate(batches, 1):
            # Check if any feature in this batch is not complete
            for feature in batch:
                status = feature.get("status", "pending")
                if status not in ["complete"]:
                    return i

        return len(batches) + 1  # All complete

    def sync_status_from_worktrees(self) -> dict[str, dict]:
        """
        Sync status from worktree directories.

        Returns:
            Dictionary mapping feature names to their worktree status
        """
        results = {}

        if not self.worktree_dir.exists():
            return results

        plan = self.read_mega_plan()
        if not plan:
            return results

        for feature in plan.get("features", []):
            name = feature["name"]
            worktree_path = self.worktree_dir / name

            if worktree_path.exists():
                prd_path = worktree_path / "prd.json"

                feature_status = {
                    "worktree_exists": True,
                    "prd_exists": prd_path.exists(),
                    "stories_complete": False,
                    "stories_status": {}
                }

                # Check PRD and stories if exists
                if prd_path.exists():
                    try:
                        with open(prd_path, encoding="utf-8") as f:
                            prd = json.load(f)
                            stories = prd.get("stories", [])
                            all_complete = True
                            for story in stories:
                                status = story.get("status", "pending")
                                feature_status["stories_status"][story["id"]] = status
                                if status != "complete":
                                    all_complete = False
                            feature_status["stories_complete"] = all_complete
                    except Exception:
                        pass

                results[name] = feature_status
            else:
                results[name] = {"worktree_exists": False}

        return results

    # ========== Mega Findings Operations ==========

    def read_mega_findings(self) -> str:
        """
        Read the mega-findings.md file.

        Returns:
            Findings content or empty string if not found
        """
        if not self.mega_findings_path.exists():
            return ""

        lock_path = self._get_lock_path(self.mega_findings_path)

        with FileLock(lock_path):
            try:
                with open(self.mega_findings_path, encoding="utf-8") as f:
                    return f.read()
            except OSError as e:
                raise OSError(f"Could not read mega-findings: {e}")

    def append_mega_findings(self, content: str, feature_id: str | None = None) -> None:
        """
        Append content to mega-findings.md.

        Args:
            content: Content to append
            feature_id: Optional feature ID for tagging
        """
        lock_path = self._get_lock_path(self.mega_findings_path)

        with FileLock(lock_path):
            try:
                self.mega_findings_path.parent.mkdir(parents=True, exist_ok=True)

                with open(self.mega_findings_path, "a", encoding="utf-8") as f:
                    if feature_id:
                        f.write(f"\n<!-- @feature: {feature_id} -->\n")
                    timestamp = time.strftime("%Y-%m-%d %H:%M:%S")
                    f.write(f"<!-- Added: {timestamp} -->\n")
                    f.write(content)
                    f.write("\n\n")
            except OSError as e:
                raise OSError(f"Could not append to mega-findings: {e}")

    def initialize_mega_findings(self) -> None:
        """
        Initialize the mega-findings.md file with header.
        """
        if self.mega_findings_path.exists():
            return

        content = """# Mega Plan Findings

This file contains shared findings across all features.
Feature-specific findings should be in their respective worktrees.

---

"""
        lock_path = self._get_lock_path(self.mega_findings_path)

        with FileLock(lock_path):
            try:
                with open(self.mega_findings_path, "w", encoding="utf-8") as f:
                    f.write(content)
            except OSError as e:
                raise OSError(f"Could not initialize mega-findings: {e}")

    def copy_mega_findings_to_worktree(self, feature_name: str) -> None:
        """
        Copy mega-findings.md to a worktree as read-only reference.

        Args:
            feature_name: Name of the feature worktree
        """
        worktree_path = self.worktree_dir / feature_name
        if not worktree_path.exists():
            return

        target_path = worktree_path / "mega-findings.md"
        source_content = self.read_mega_findings()

        if source_content:
            header = """<!--
    This is a READ-ONLY copy of the project-level mega-findings.md.
    Do not edit this file. Make updates to the root mega-findings.md instead.
    This file is automatically synced during execution.
-->

"""
            with open(target_path, "w", encoding="utf-8") as f:
                f.write(header + source_content)

    # ========== Worktree Helpers ==========

    def get_worktree_path(self, feature_name: str) -> Path:
        """
        Get the worktree path for a feature.

        Args:
            feature_name: Name of the feature

        Returns:
            Path to the worktree directory
        """
        return self.worktree_dir / feature_name

    def worktree_exists(self, feature_name: str) -> bool:
        """
        Check if a worktree exists for a feature.

        Args:
            feature_name: Name of the feature

        Returns:
            True if worktree exists
        """
        return self.get_worktree_path(feature_name).exists()

    # ========== Cleanup ==========

    def cleanup_locks(self) -> None:
        """Remove stale lock files."""
        try:
            if self.locks_dir.exists():
                for lock_file in self.locks_dir.glob("*.lock"):
                    if lock_file.stat().st_mtime < time.time() - 3600:
                        lock_file.unlink()
        except Exception:
            pass

    def cleanup_all(self) -> None:
        """
        Clean up all mega-plan related files.
        Called after /mega:complete.
        """
        files_to_remove = [
            self.mega_plan_path,
            self.mega_status_path,
            self.mega_findings_path
        ]

        for file_path in files_to_remove:
            try:
                if file_path.exists():
                    file_path.unlink()
            except Exception as e:
                print(f"Warning: Could not remove {file_path}: {e}")


def main():
    """CLI interface for testing mega state manager."""
    if len(sys.argv) < 2:
        print("Usage: mega_state.py [--legacy] <command> [args]")
        print("Options:")
        print("  --legacy                    - Use legacy mode (files in project root)")
        print("Commands:")
        print("  read-plan                   - Read mega-plan")
        print("  read-status                 - Read mega-status")
        print("  read-findings               - Read mega-findings")
        print("  sync-worktrees              - Sync status from worktrees")
        print("  update-feature <id> <status> - Update feature status")
        print("  cleanup-locks               - Remove stale locks")
        print("  show-paths                  - Show all file paths")
        sys.exit(1)

    # Check for --legacy flag
    legacy_mode = "--legacy" in sys.argv
    args = [a for a in sys.argv[1:] if a != "--legacy"]
    command = args[0] if args else ""

    project_root = Path.cwd()

    sm = MegaStateManager(project_root, legacy_mode=legacy_mode)

    if command == "read-plan":
        plan = sm.read_mega_plan()
        print(json.dumps(plan, indent=2) if plan else "No mega-plan found")

    elif command == "read-status":
        status = sm.read_status()
        print(json.dumps(status, indent=2) if status else "No status found")

    elif command == "read-findings":
        findings = sm.read_mega_findings()
        print(findings if findings else "No findings found")

    elif command == "sync-worktrees":
        results = sm.sync_status_from_worktrees()
        print(json.dumps(results, indent=2))

    elif command == "update-feature" and len(args) >= 3:
        feature_id = args[1]
        status = args[2]
        sm.update_feature_status(feature_id, status)
        print(f"Updated {feature_id} to {status}")

    elif command == "cleanup-locks":
        sm.cleanup_locks()
        print("Locks cleaned up")

    elif command == "show-paths":
        print(f"Mode: {'legacy' if sm.is_legacy_mode() else 'new'}")
        print(f"Project root: {sm.project_root}")
        print(f"Mega plan path: {sm.mega_plan_path}")
        print(f"Mega status path: {sm.mega_status_path}")
        print(f"Mega findings path: {sm.mega_findings_path}")
        print(f"Worktree directory: {sm.worktree_dir}")
        print(f"Locks directory: {sm.locks_dir}")

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
