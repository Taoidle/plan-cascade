#!/usr/bin/env python3
"""
Shared State Manager for Plan Cascade

Provides thread-safe file operations with platform-specific locking.
Handles prd.json, findings.md, progress.txt, and .agent-status.json
with concurrent access safety.

Extended for multi-agent collaboration support.
Now integrated with PathResolver for unified path resolution.
"""

from __future__ import annotations

import json
import os
import sys
import time
from pathlib import Path
from typing import Any, TYPE_CHECKING

if TYPE_CHECKING:
    from plan_cascade.state.path_resolver import PathResolver

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
        # Create lock directory if needed
        self.lock_file.parent.mkdir(parents=True, exist_ok=True)

        start_time = time.time()

        while True:
            try:
                # Open file for exclusive access
                self.lock_fd = open(self.lock_file, 'w')

                if HAS_FCNTL:
                    # Unix/Linux/Mac - use fcntl
                    fcntl.flock(self.lock_fd.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                    return True
                elif HAS_MSVCRT:
                    # Windows - use msvcrt
                    # Try to lock the file (mode 0 is exclusive lock)
                    msvcrt.locking(self.lock_fd.fileno(), msvcrt.LK_NBLCK, 1)
                    return True
                else:
                    # Fallback: no locking available
                    # Create a PID-based lock file for basic coordination
                    pid = os.getpid()
                    self.lock_fd.write(str(pid))
                    self.lock_fd.flush()
                    return True

            except OSError:
                # Lock failed - file is locked
                if self.lock_fd:
                    self.lock_fd.close()
                self.lock_fd = None

                # Check timeout
                if time.time() - start_time >= self.timeout:
                    return False

                # Exponential backoff
                wait_time = min(0.1 * (2 ** int(time.time() - start_time)), 2.0)
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

            # Try to remove lock file
            try:
                self.lock_file.unlink(missing_ok=True)
            except Exception:
                pass

    def __enter__(self):
        """Context manager entry."""
        if not self.acquire():
            raise TimeoutError(f"Could not acquire lock on {self.lock_file} within {self.timeout}s")
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit."""
        self.release()


class StateManager:
    """Manages shared state files with locking.

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
        Initialize the state manager.

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

        # Determine legacy mode
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

        # PRD file - use resolver's method
        self.prd_path = resolver.get_prd_path()

        # State files - use state directory from resolver
        state_dir = resolver.get_state_dir()

        # For legacy mode, keep files at project root
        # For new mode, use state subdirectory
        if resolver.is_legacy_mode():
            # Legacy: files directly in project root
            self.findings_path = self.project_root / "findings.md"
            self.progress_path = self.project_root / "progress.txt"
            self.agent_status_path = self.project_root / ".agent-status.json"
            self.iteration_state_path = self.project_root / ".iteration-state.json"
            self.retry_state_path = self.project_root / ".retry-state.json"
        else:
            # New mode: organized in state directory
            # Note: findings.md and progress.txt remain in project root
            # as they are user-visible documentation files
            self.findings_path = self.project_root / "findings.md"
            self.progress_path = self.project_root / "progress.txt"
            # Internal state files go to state directory
            self.agent_status_path = resolver.get_state_file_path("agent-status.json")
            self.iteration_state_path = resolver.get_state_file_path("iteration-state.json")
            self.retry_state_path = resolver.get_state_file_path("retry-state.json")

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

    # ========== PRD Operations ==========

    def read_prd(self) -> dict | None:
        """
        Read the PRD file safely.

        Returns:
            PRD dictionary or None if not found
        """
        if not self.prd_path.exists():
            return None

        lock_path = self._get_lock_path(self.prd_path)

        with FileLock(lock_path):
            try:
                with open(self.prd_path, encoding="utf-8") as f:
                    return json.load(f)
            except (OSError, json.JSONDecodeError) as e:
                raise OSError(f"Could not read PRD: {e}")

    def write_prd(self, prd: dict) -> None:
        """
        Write the PRD file safely.

        Args:
            prd: PRD dictionary to write
        """
        lock_path = self._get_lock_path(self.prd_path)

        with FileLock(lock_path):
            try:
                # Ensure parent directory exists
                self.prd_path.parent.mkdir(parents=True, exist_ok=True)
                with open(self.prd_path, "w", encoding="utf-8") as f:
                    json.dump(prd, f, indent=2)
            except OSError as e:
                raise OSError(f"Could not write PRD: {e}")

    def update_story_status(self, story_id: str, status: str) -> None:
        """
        Update the status of a story in the PRD.

        Args:
            story_id: Story ID to update
            status: New status (pending, in_progress, complete)
        """
        prd = self.read_prd()
        if not prd:
            raise ValueError("No PRD found")

        for story in prd.get("stories", []):
            if story.get("id") == story_id:
                story["status"] = status
                break

        self.write_prd(prd)

    # ========== Findings Operations ==========

    def append_findings(self, content: str, tags: list[str] | None = None) -> None:
        """
        Append content to findings.md with optional tags.

        Args:
            content: Content to append
            tags: Optional list of story tags
        """
        lock_path = self._get_lock_path(self.findings_path)

        with FileLock(lock_path):
            try:
                # Create file if it doesn't exist
                self.findings_path.parent.mkdir(parents=True, exist_ok=True)

                with open(self.findings_path, "a", encoding="utf-8") as f:
                    # Write tags if provided
                    if tags:
                        tags_str = ",".join(tags)
                        f.write(f"\n<!-- @tags: {tags_str} -->\n")

                    f.write(content)
                    f.write("\n\n")
            except OSError as e:
                raise OSError(f"Could not append to findings: {e}")

    def read_findings(self) -> str:
        """
        Read the findings file safely.

        Returns:
            Findings content or empty string if not found
        """
        if not self.findings_path.exists():
            return ""

        lock_path = self._get_lock_path(self.findings_path)

        with FileLock(lock_path):
            try:
                with open(self.findings_path, encoding="utf-8") as f:
                    return f.read()
            except OSError as e:
                raise OSError(f"Could not read findings: {e}")

    # ========== Progress Operations ==========

    def append_progress(self, content: str, story_id: str | None = None) -> None:
        """
        Append content to progress.txt.

        Args:
            content: Content to append
            story_id: Optional story ID for tracking
        """
        lock_path = self._get_lock_path(self.progress_path)

        with FileLock(lock_path):
            try:
                # Create file if it doesn't exist
                self.progress_path.parent.mkdir(parents=True, exist_ok=True)

                with open(self.progress_path, "a", encoding="utf-8") as f:
                    if story_id:
                        timestamp = time.strftime("%Y-%m-%d %H:%M:%S")
                        f.write(f"[{timestamp}] {story_id}: {content}\n")
                    else:
                        f.write(content + "\n")
            except OSError as e:
                raise OSError(f"Could not append to progress: {e}")

    def mark_story_complete(self, story_id: str) -> None:
        """
        Mark a story as complete in progress.txt.

        Args:
            story_id: Story ID to mark complete
        """
        self.append_progress(f"[COMPLETE] {story_id}", story_id=story_id)

    def mark_story_in_progress(self, story_id: str) -> None:
        """
        Mark a story as in progress in progress.txt.

        Args:
            story_id: Story ID to mark in progress
        """
        self.append_progress(f"[IN_PROGRESS] {story_id}", story_id=story_id)

    def read_progress(self) -> str:
        """
        Read the progress file safely.

        Returns:
            Progress content or empty string if not found
        """
        if not self.progress_path.exists():
            return ""

        lock_path = self._get_lock_path(self.progress_path)

        with FileLock(lock_path):
            try:
                with open(self.progress_path, encoding="utf-8") as f:
                    return f.read()
            except OSError as e:
                raise OSError(f"Could not read progress: {e}")

    # ========== Utility Methods ==========

    def get_all_story_statuses(self) -> dict[str, str]:
        """
        Get the status of all stories from progress.txt.

        Returns:
            Dictionary mapping story_id to status
        """
        content = self.read_progress()
        if not content:
            return {}

        statuses: dict[str, str] = {}

        for line in content.split("\n"):
            line = line.strip()
            if "[COMPLETE]" in line:
                # Extract story ID
                for word in line.split():
                    if word.startswith("story-"):
                        # Remove trailing colon if present
                        story_id = word.rstrip(":")
                        statuses[story_id] = "complete"
                        break
            elif "[IN_PROGRESS]" in line:
                for word in line.split():
                    if word.startswith("story-"):
                        # Remove trailing colon if present
                        story_id = word.rstrip(":")
                        statuses[story_id] = "in_progress"
                        break

        return statuses

    def cleanup_locks(self):
        """Remove stale lock files."""
        try:
            if self.locks_dir.exists():
                for lock_file in self.locks_dir.glob("*.lock"):
                    # Check if lock is stale (older than 1 hour)
                    if lock_file.stat().st_mtime < time.time() - 3600:
                        lock_file.unlink()
        except Exception:
            pass

    # ========== Agent Status Operations ==========

    def read_agent_status(self) -> dict:
        """
        Read .agent-status.json file safely.

        Returns:
            Agent status dictionary with running, completed, failed lists
        """
        if not self.agent_status_path.exists():
            return {
                "running": [],
                "completed": [],
                "failed": [],
                "updated_at": None
            }

        lock_path = self._get_lock_path(self.agent_status_path)

        with FileLock(lock_path):
            try:
                with open(self.agent_status_path, encoding="utf-8") as f:
                    return json.load(f)
            except (OSError, json.JSONDecodeError):
                return {
                    "running": [],
                    "completed": [],
                    "failed": [],
                    "updated_at": None
                }

    def write_agent_status(self, status: dict) -> None:
        """
        Write .agent-status.json file safely.

        Args:
            status: Agent status dictionary
        """
        lock_path = self._get_lock_path(self.agent_status_path)
        status["updated_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ")

        with FileLock(lock_path):
            try:
                # Ensure parent directory exists
                self.agent_status_path.parent.mkdir(parents=True, exist_ok=True)
                with open(self.agent_status_path, "w", encoding="utf-8") as f:
                    json.dump(status, f, indent=2)
            except OSError as e:
                raise OSError(f"Could not write agent status: {e}")

    def record_agent_start(
        self,
        story_id: str,
        agent_name: str,
        pid: int | None = None,
        output_file: str | None = None
    ) -> None:
        """
        Record agent start in .agent-status.json.

        Args:
            story_id: Story ID being executed
            agent_name: Name of the agent
            pid: Process ID (for CLI agents)
            output_file: Path to output log file
        """
        status = self.read_agent_status()

        # Remove any existing entry for this story
        status["running"] = [
            r for r in status.get("running", [])
            if r.get("story_id") != story_id
        ]

        # Add new running entry
        entry = {
            "story_id": story_id,
            "agent": agent_name,
            "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ")
        }
        if pid is not None:
            entry["pid"] = pid
        if output_file:
            entry["output_file"] = output_file

        status["running"].append(entry)
        self.write_agent_status(status)

        # Also log to progress.txt with agent info
        self.append_progress(
            f"[START] via {agent_name}" + (f" (pid:{pid})" if pid else ""),
            story_id=story_id
        )

    def record_agent_complete(self, story_id: str, agent_name: str) -> None:
        """
        Record agent completion in .agent-status.json.

        Args:
            story_id: Story ID that completed
            agent_name: Name of the agent
        """
        status = self.read_agent_status()

        # Find entry in running
        running_entry = None
        for entry in status.get("running", []):
            if entry.get("story_id") == story_id:
                running_entry = entry
                break

        # Remove from running
        status["running"] = [
            r for r in status.get("running", [])
            if r.get("story_id") != story_id
        ]

        # Add to completed
        entry = {
            "story_id": story_id,
            "agent": agent_name,
            "completed_at": time.strftime("%Y-%m-%dT%H:%M:%SZ")
        }
        if running_entry:
            entry["started_at"] = running_entry.get("started_at")

        if "completed" not in status:
            status["completed"] = []
        status["completed"].append(entry)

        self.write_agent_status(status)

        # Log to progress.txt
        self.append_progress(f"[COMPLETE] via {agent_name}", story_id=story_id)

    def record_agent_failure(
        self,
        story_id: str,
        agent_name: str,
        error: str
    ) -> None:
        """
        Record agent failure in .agent-status.json.

        Args:
            story_id: Story ID that failed
            agent_name: Name of the agent
            error: Error message
        """
        status = self.read_agent_status()

        # Find entry in running
        running_entry = None
        for entry in status.get("running", []):
            if entry.get("story_id") == story_id:
                running_entry = entry
                break

        # Remove from running
        status["running"] = [
            r for r in status.get("running", [])
            if r.get("story_id") != story_id
        ]

        # Add to failed
        entry = {
            "story_id": story_id,
            "agent": agent_name,
            "failed_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "error": error
        }
        if running_entry:
            entry["started_at"] = running_entry.get("started_at")

        if "failed" not in status:
            status["failed"] = []
        status["failed"].append(entry)

        self.write_agent_status(status)

        # Log to progress.txt
        self.append_progress(f"[FAILED] via {agent_name}: {error}", story_id=story_id)

    def get_running_agents(self) -> list[dict]:
        """
        Get list of currently running agents.

        Returns:
            List of running agent entries
        """
        status = self.read_agent_status()
        return status.get("running", [])

    def get_agent_for_story(self, story_id: str) -> dict | None:
        """
        Get agent info for a specific story.

        Args:
            story_id: Story ID to look up

        Returns:
            Agent entry dict or None
        """
        status = self.read_agent_status()

        # Check running
        for entry in status.get("running", []):
            if entry.get("story_id") == story_id:
                entry["status"] = "running"
                return entry

        # Check completed
        for entry in status.get("completed", []):
            if entry.get("story_id") == story_id:
                entry["status"] = "completed"
                return entry

        # Check failed
        for entry in status.get("failed", []):
            if entry.get("story_id") == story_id:
                entry["status"] = "failed"
                return entry

        return None

    def get_agent_summary(self) -> dict[str, int]:
        """
        Get summary counts of agent statuses.

        Returns:
            Dict with running, completed, failed counts
        """
        status = self.read_agent_status()
        return {
            "running": len(status.get("running", [])),
            "completed": len(status.get("completed", [])),
            "failed": len(status.get("failed", []))
        }

    # ========== Iteration State Operations ==========

    def read_iteration_state(self) -> dict | None:
        """
        Read .iteration-state.json file safely.

        Returns:
            Iteration state dictionary or None if not found
        """
        if not self.iteration_state_path.exists():
            return None

        lock_path = self._get_lock_path(self.iteration_state_path)

        with FileLock(lock_path):
            try:
                with open(self.iteration_state_path, encoding="utf-8") as f:
                    return json.load(f)
            except (OSError, json.JSONDecodeError):
                return None

    def write_iteration_state(self, state: dict) -> None:
        """
        Write .iteration-state.json file safely.

        Args:
            state: Iteration state dictionary
        """
        lock_path = self._get_lock_path(self.iteration_state_path)
        state["updated_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ")

        with FileLock(lock_path):
            try:
                # Ensure parent directory exists
                self.iteration_state_path.parent.mkdir(parents=True, exist_ok=True)
                with open(self.iteration_state_path, "w", encoding="utf-8") as f:
                    json.dump(state, f, indent=2)
            except OSError as e:
                raise OSError(f"Could not write iteration state: {e}")

    def clear_iteration_state(self) -> None:
        """Clear the iteration state file."""
        lock_path = self._get_lock_path(self.iteration_state_path)

        with FileLock(lock_path):
            try:
                if self.iteration_state_path.exists():
                    self.iteration_state_path.unlink()
            except OSError:
                pass

    def get_iteration_progress(self) -> dict[str, Any] | None:
        """
        Get iteration progress summary.

        Returns:
            Progress summary dict or None if no iteration state
        """
        state = self.read_iteration_state()
        if not state:
            return None

        return {
            "status": state.get("status", "unknown"),
            "current_batch": state.get("current_batch", 0),
            "total_batches": state.get("total_batches", 0),
            "completed_stories": state.get("completed_stories", 0),
            "failed_stories": state.get("failed_stories", 0),
            "total_stories": state.get("total_stories", 0),
            "current_iteration": state.get("current_iteration", 0),
        }

    # ========== Retry State Operations ==========

    def read_retry_state(self) -> dict | None:
        """
        Read .retry-state.json file safely.

        Returns:
            Retry state dictionary or None if not found
        """
        if not self.retry_state_path.exists():
            return None

        lock_path = self._get_lock_path(self.retry_state_path)

        with FileLock(lock_path):
            try:
                with open(self.retry_state_path, encoding="utf-8") as f:
                    return json.load(f)
            except (OSError, json.JSONDecodeError):
                return None

    def write_retry_state(self, state: dict) -> None:
        """
        Write .retry-state.json file safely.

        Args:
            state: Retry state dictionary
        """
        lock_path = self._get_lock_path(self.retry_state_path)
        state["updated_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ")

        with FileLock(lock_path):
            try:
                # Ensure parent directory exists
                self.retry_state_path.parent.mkdir(parents=True, exist_ok=True)
                with open(self.retry_state_path, "w", encoding="utf-8") as f:
                    json.dump(state, f, indent=2)
            except OSError as e:
                raise OSError(f"Could not write retry state: {e}")

    def clear_retry_state(self) -> None:
        """Clear the retry state file."""
        lock_path = self._get_lock_path(self.retry_state_path)

        with FileLock(lock_path):
            try:
                if self.retry_state_path.exists():
                    self.retry_state_path.unlink()
            except OSError:
                pass

    def get_retry_summary(self, story_id: str | None = None) -> dict[str, Any]:
        """
        Get retry summary for all or a specific story.

        Args:
            story_id: Optional story ID to get summary for

        Returns:
            Retry summary dict
        """
        state = self.read_retry_state()
        if not state:
            return {"total_retries": 0, "stories": {}}

        stories = state.get("stories", {})

        if story_id:
            story_state = stories.get(story_id, {})
            return {
                "story_id": story_id,
                "current_attempt": story_state.get("current_attempt", 0),
                "exhausted": story_state.get("exhausted", False),
                "failures": len(story_state.get("failures", [])),
            }

        # Summary of all stories
        total_retries = sum(s.get("current_attempt", 0) for s in stories.values())
        exhausted_count = sum(1 for s in stories.values() if s.get("exhausted", False))

        return {
            "total_retries": total_retries,
            "exhausted_stories": exhausted_count,
            "stories_with_retries": len(stories),
        }

    def record_retry_attempt(
        self,
        story_id: str,
        agent: str,
        error_type: str,
        error_message: str
    ) -> None:
        """
        Record a retry attempt for a story.

        Args:
            story_id: Story ID
            agent: Agent that failed
            error_type: Type of error
            error_message: Error message
        """
        state = self.read_retry_state() or {"version": "1.0.0", "stories": {}}

        if story_id not in state["stories"]:
            state["stories"][story_id] = {
                "story_id": story_id,
                "current_attempt": 0,
                "failures": [],
                "exhausted": False,
            }

        story_state = state["stories"][story_id]
        story_state["current_attempt"] += 1
        story_state["last_agent"] = agent

        story_state["failures"].append({
            "attempt": story_state["current_attempt"],
            "agent": agent,
            "error_type": error_type,
            "error_message": error_message,
            "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
        })

        self.write_retry_state(state)


def main():
    """CLI interface for testing state manager."""
    if len(sys.argv) < 2:
        print("Usage: state_manager.py [--legacy] <command> [args]")
        print("Options:")
        print("  --legacy                    - Use legacy mode (files in project root)")
        print("Commands:")
        print("  read-prd                    - Read PRD file")
        print("  write-prd <json>            - Write PRD file")
        print("  append-findings <content>   - Append to findings")
        print("  mark-complete <story_id>    - Mark story complete")
        print("  get-statuses                - Get all story statuses")
        print("  cleanup-locks               - Remove stale locks")
        print("  show-paths                  - Show all file paths")
        sys.exit(1)

    # Check for --legacy flag
    legacy_mode = "--legacy" in sys.argv
    args = [a for a in sys.argv[1:] if a != "--legacy"]
    command = args[0] if args else ""

    project_root = Path.cwd()

    sm = StateManager(project_root, legacy_mode=legacy_mode)

    if command == "read-prd":
        prd = sm.read_prd()
        print(json.dumps(prd, indent=2) if prd else "No PRD found")

    elif command == "write-prd" and len(sys.argv) >= 3:
        prd = json.loads(sys.argv[2])
        sm.write_prd(prd)
        print("PRD written successfully")

    elif command == "append-findings" and len(sys.argv) >= 3:
        content = sys.argv[2]
        tags = sys.argv[3].split(",") if len(sys.argv) >= 4 else None
        sm.append_findings(content, tags)
        print("Findings appended successfully")

    elif command == "mark-complete" and len(sys.argv) >= 3:
        story_id = sys.argv[2]
        sm.mark_story_complete(story_id)
        print(f"Marked {story_id} as complete")

    elif command == "get-statuses":
        statuses = sm.get_all_story_statuses()
        print(json.dumps(statuses, indent=2))

    elif command == "cleanup-locks":
        sm.cleanup_locks()
        print("Locks cleaned up")

    elif command == "show-paths":
        print(f"Mode: {'legacy' if sm.is_legacy_mode() else 'new'}")
        print(f"Project root: {sm.project_root}")
        print(f"PRD path: {sm.prd_path}")
        print(f"Findings path: {sm.findings_path}")
        print(f"Progress path: {sm.progress_path}")
        print(f"Locks directory: {sm.locks_dir}")
        print(f"Agent status path: {sm.agent_status_path}")
        print(f"Iteration state path: {sm.iteration_state_path}")
        print(f"Retry state path: {sm.retry_state_path}")

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
