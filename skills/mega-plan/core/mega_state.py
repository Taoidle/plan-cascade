#!/usr/bin/env python3
"""
Mega State Manager

Provides thread-safe file operations for mega-plan state files.
Handles mega-plan.json, .mega-status.json, and mega-findings.md with concurrent access safety.
"""

import json
import os
import sys
import time
from pathlib import Path
from typing import Optional, Dict, Any, List

# Platform-specific locking imports
try:
    import fcntl
    HAS_FCNTL = True
except ImportError:
    HAS_FCNTL = False

try:
    import msvcrt
    HAS_MSVCRT = True
except ImportError:
    HAS_MSVCRT = False


class FileLock:
    """Platform-independent file locking."""

    def __init__(self, lock_file: Path, timeout: float = 30.0):
        """
        Initialize a file lock.

        Args:
            lock_file: Path to the lock file
            timeout: Maximum time to wait for lock (seconds)
        """
        self.lock_file = lock_file
        self.timeout = timeout
        self.lock_fd = None

    def acquire(self) -> bool:
        """
        Acquire the file lock.

        Returns:
            True if lock acquired, False if timeout
        """
        self.lock_file.parent.mkdir(parents=True, exist_ok=True)
        start_time = time.time()

        while True:
            try:
                self.lock_fd = open(self.lock_file, 'w')

                if HAS_FCNTL:
                    fcntl.flock(self.lock_fd.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                    return True
                elif HAS_MSVCRT:
                    msvcrt.locking(self.lock_fd.fileno(), msvcrt.LK_NBLCK, 1)
                    return True
                else:
                    pid = os.getpid()
                    self.lock_fd.write(str(pid))
                    self.lock_fd.flush()
                    return True

            except (IOError, OSError):
                if self.lock_fd:
                    self.lock_fd.close()
                self.lock_fd = None

                if time.time() - start_time >= self.timeout:
                    return False

                wait_time = min(0.1 * (2 ** int((time.time() - start_time))), 2.0)
                time.sleep(wait_time)

    def release(self):
        """Release the file lock."""
        if self.lock_fd:
            try:
                if HAS_FCNTL:
                    fcntl.flock(self.lock_fd.fileno(), fcntl.LOCK_UN)
                elif HAS_MSVCRT:
                    msvcrt.locking(self.lock_fd.fileno(), msvcrt.LK_UNLCK, 1)
                self.lock_fd.close()
            except Exception:
                pass
            self.lock_fd = None

            try:
                self.lock_file.unlink(missing_ok=True)
            except Exception:
                pass

    def __enter__(self):
        if not self.acquire():
            raise TimeoutError(f"Could not acquire lock on {self.lock_file} within {self.timeout}s")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.release()


class MegaStateManager:
    """Manages mega-plan state files with locking."""

    def __init__(self, project_root: Path):
        """
        Initialize the mega state manager.

        Args:
            project_root: Root directory of the project
        """
        self.project_root = Path(project_root)
        self.locks_dir = self.project_root / ".locks"
        self.mega_plan_path = self.project_root / "mega-plan.json"
        self.mega_status_path = self.project_root / ".mega-status.json"
        self.mega_findings_path = self.project_root / "mega-findings.md"
        self.worktree_dir = self.project_root / ".worktree"

    def _get_lock_path(self, file_path: Path) -> Path:
        """Get the lock file path for a given file."""
        return self.locks_dir / f"{file_path.name}.lock"

    # ========== Mega Plan Operations ==========

    def read_mega_plan(self) -> Optional[Dict]:
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
                with open(self.mega_plan_path, "r", encoding="utf-8") as f:
                    return json.load(f)
            except (json.JSONDecodeError, IOError) as e:
                raise IOError(f"Could not read mega-plan: {e}")

    def write_mega_plan(self, plan: Dict) -> None:
        """
        Write the mega-plan.json file safely.

        Args:
            plan: Mega-plan dictionary to write
        """
        lock_path = self._get_lock_path(self.mega_plan_path)

        with FileLock(lock_path):
            try:
                with open(self.mega_plan_path, "w", encoding="utf-8") as f:
                    json.dump(plan, f, indent=2)
            except IOError as e:
                raise IOError(f"Could not write mega-plan: {e}")

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

    def read_status(self) -> Optional[Dict]:
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
                with open(self.mega_status_path, "r", encoding="utf-8") as f:
                    return json.load(f)
            except (json.JSONDecodeError, IOError) as e:
                raise IOError(f"Could not read mega-status: {e}")

    def write_status(self, status: Dict) -> None:
        """
        Write the .mega-status.json file.

        Args:
            status: Status dictionary to write
        """
        lock_path = self._get_lock_path(self.mega_status_path)

        with FileLock(lock_path):
            try:
                with open(self.mega_status_path, "w", encoding="utf-8") as f:
                    json.dump(status, f, indent=2)
            except IOError as e:
                raise IOError(f"Could not write mega-status: {e}")

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

    def _get_current_batch_number(self, plan: Dict) -> int:
        """Determine current batch number based on feature statuses."""
        from .mega_generator import MegaPlanGenerator
        mg = MegaPlanGenerator(self.project_root)
        batches = mg.generate_feature_batches(plan)

        for i, batch in enumerate(batches, 1):
            # Check if any feature in this batch is not complete
            for feature in batch:
                status = feature.get("status", "pending")
                if status not in ["complete"]:
                    return i

        return len(batches) + 1  # All complete

    def sync_status_from_worktrees(self) -> Dict[str, Dict]:
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
                progress_path = worktree_path / "progress.txt"

                feature_status = {
                    "worktree_exists": True,
                    "prd_exists": prd_path.exists(),
                    "stories_complete": False,
                    "stories_status": {}
                }

                # Check PRD and stories if exists
                if prd_path.exists():
                    try:
                        with open(prd_path, "r", encoding="utf-8") as f:
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
                with open(self.mega_findings_path, "r", encoding="utf-8") as f:
                    return f.read()
            except IOError as e:
                raise IOError(f"Could not read mega-findings: {e}")

    def append_mega_findings(self, content: str, feature_id: Optional[str] = None) -> None:
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
            except IOError as e:
                raise IOError(f"Could not append to mega-findings: {e}")

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
            except IOError as e:
                raise IOError(f"Could not initialize mega-findings: {e}")

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
    import sys

    if len(sys.argv) < 2:
        print("Usage: mega_state.py <command> [args]")
        print("Commands:")
        print("  read-plan                   - Read mega-plan")
        print("  read-status                 - Read mega-status")
        print("  read-findings               - Read mega-findings")
        print("  sync-worktrees              - Sync status from worktrees")
        print("  update-feature <id> <status> - Update feature status")
        print("  cleanup-locks               - Remove stale locks")
        sys.exit(1)

    command = sys.argv[1]
    project_root = Path.cwd()

    sm = MegaStateManager(project_root)

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

    elif command == "update-feature" and len(sys.argv) >= 4:
        feature_id = sys.argv[2]
        status = sys.argv[3]
        sm.update_feature_status(feature_id, status)
        print(f"Updated {feature_id} to {status}")

    elif command == "cleanup-locks":
        sm.cleanup_locks()
        print("Locks cleaned up")

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
