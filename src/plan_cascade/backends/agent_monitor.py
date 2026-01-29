#!/usr/bin/env python3
"""
Agent Monitor for Plan Cascade Multi-Agent Collaboration

Monitors running agents and updates their status:
- Detects process completion/death
- Reads result files for completion status
- Cleans up stale entries
- Provides polling interface for main session
"""

import json
import os
import subprocess
import sys
import time
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any


class AgentMonitor:
    """
    Monitors running agents and provides status updates.

    The monitor checks:
    1. Process liveness (is PID still running?)
    2. Result files (.agent-outputs/<story-id>.result.json)
    3. Output logs for completion markers
    """

    def __init__(self, project_root: Path):
        """
        Initialize the agent monitor.

        Args:
            project_root: Root directory of the project
        """
        self.project_root = Path(project_root)
        self.agent_status_path = self.project_root / ".agent-status.json"
        self.output_dir = self.project_root / ".agent-outputs"
        self.progress_path = self.project_root / "progress.txt"

    def _timestamp(self) -> str:
        """Get ISO timestamp."""
        return datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")

    def _read_status(self) -> dict:
        """Read .agent-status.json."""
        if not self.agent_status_path.exists():
            return {"running": [], "completed": [], "failed": []}

        try:
            with open(self.agent_status_path, encoding="utf-8") as f:
                return json.load(f)
        except (OSError, json.JSONDecodeError):
            return {"running": [], "completed": [], "failed": []}

    def _write_status(self, status: dict) -> None:
        """Write .agent-status.json."""
        status["updated_at"] = self._timestamp()
        try:
            with open(self.agent_status_path, "w", encoding="utf-8") as f:
                json.dump(status, f, indent=2)
        except OSError as e:
            print(f"[Warning] Could not write status: {e}", file=sys.stderr)

    def _append_progress(self, story_id: str, message: str) -> None:
        """Append to progress.txt."""
        try:
            timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
            with open(self.progress_path, "a", encoding="utf-8") as f:
                f.write(f"[{timestamp}] {story_id}: {message}\n")
        except OSError:
            pass

    def _is_process_alive(self, pid: int) -> bool:
        """Check if a process is still running."""
        try:
            if sys.platform == "win32":
                result = subprocess.run(
                    ["tasklist", "/FI", f"PID eq {pid}", "/NH"],
                    capture_output=True,
                    text=True,
                    timeout=5
                )
                return str(pid) in result.stdout
            else:
                os.kill(pid, 0)
                return True
        except (OSError, subprocess.SubprocessError, subprocess.TimeoutExpired):
            return False

    def _read_result_file(self, story_id: str) -> dict | None:
        """Read result file if it exists."""
        result_file = self.output_dir / f"{story_id}.result.json"
        if not result_file.exists():
            return None

        try:
            with open(result_file, encoding="utf-8") as f:
                return json.load(f)
        except (OSError, json.JSONDecodeError):
            return None

    def _check_output_for_completion(self, story_id: str) -> tuple[bool, str] | None:
        """Check output log for completion markers."""
        output_file = self.output_dir / f"{story_id}.log"
        if not output_file.exists():
            return None

        try:
            with open(output_file, encoding="utf-8") as f:
                content = f.read()

            if "# Exit Code: 0" in content:
                return (True, "Completed successfully")
            elif "# Exit Code:" in content:
                for line in content.split("\n"):
                    if "# Exit Code:" in line:
                        code = line.split(":")[-1].strip()
                        return (False, f"Exit code {code}")
            elif "[TIMEOUT]" in content:
                return (False, "Timeout")

            return None
        except OSError:
            return None

    def check_running_agents(self) -> dict[str, Any]:
        """
        Check all running agents and update status.

        Returns:
            Dict with updated, running, completed, failed agents
        """
        status = self._read_status()
        updated = []
        still_running = []
        newly_completed = []
        newly_failed = []

        for entry in status.get("running", []):
            story_id = entry.get("story_id")
            pid = entry.get("pid")
            agent = entry.get("agent", "unknown")

            # Check result file first
            result = self._read_result_file(story_id)
            if result:
                if result.get("success"):
                    newly_completed.append({
                        **entry,
                        "exit_code": result.get("exit_code", 0),
                        "completed_at": result.get("completed_at", self._timestamp())
                    })
                else:
                    newly_failed.append({
                        **entry,
                        "error": result.get("error", "Unknown error"),
                        "exit_code": result.get("exit_code", -1),
                        "failed_at": result.get("completed_at", self._timestamp())
                    })
                updated.append(story_id)
                continue

            # Check if process is still alive
            if pid and not self._is_process_alive(pid):
                completion = self._check_output_for_completion(story_id)
                if completion:
                    success, message = completion
                    if success:
                        newly_completed.append({**entry, "completed_at": self._timestamp()})
                    else:
                        newly_failed.append({**entry, "error": message, "failed_at": self._timestamp()})
                else:
                    newly_failed.append({
                        **entry,
                        "error": "Process exited unexpectedly",
                        "failed_at": self._timestamp()
                    })
                updated.append(story_id)
                continue

            still_running.append(entry)

        # Update status if anything changed
        if updated:
            status["running"] = still_running

            for entry in newly_completed:
                if "completed" not in status:
                    status["completed"] = []
                clean_entry = {k: v for k, v in entry.items() if k not in ["pid", "timeout"]}
                status["completed"].append(clean_entry)
                self._append_progress(entry["story_id"], f"[COMPLETE] via {entry.get('agent', 'unknown')}")

            for entry in newly_failed:
                if "failed" not in status:
                    status["failed"] = []
                clean_entry = {k: v for k, v in entry.items() if k not in ["pid", "timeout"]}
                status["failed"].append(clean_entry)
                self._append_progress(entry["story_id"], f"[FAILED] via {entry.get('agent', 'unknown')}: {entry.get('error', 'Unknown')}")

            self._write_status(status)

        return {
            "updated": updated,
            "running": still_running,
            "newly_completed": newly_completed,
            "newly_failed": newly_failed,
            "total_running": len(still_running),
            "total_completed": len(status.get("completed", [])),
            "total_failed": len(status.get("failed", []))
        }

    def get_agent_result(self, story_id: str) -> dict | None:
        """Get the result of a completed agent."""
        result = self._read_result_file(story_id)
        if result:
            return result

        status = self._read_status()

        for entry in status.get("completed", []):
            if entry.get("story_id") == story_id:
                return {
                    "story_id": story_id,
                    "success": True,
                    "exit_code": entry.get("exit_code", 0),
                    "completed_at": entry.get("completed_at"),
                    "output_file": entry.get("output_file")
                }

        for entry in status.get("failed", []):
            if entry.get("story_id") == story_id:
                return {
                    "story_id": story_id,
                    "success": False,
                    "error": entry.get("error"),
                    "exit_code": entry.get("exit_code", -1),
                    "failed_at": entry.get("failed_at"),
                    "output_file": entry.get("output_file")
                }

        return None

    def get_agent_output(self, story_id: str, tail_lines: int = 50) -> str | None:
        """Get the output log of an agent."""
        output_file = self.output_dir / f"{story_id}.log"
        if not output_file.exists():
            return None

        try:
            with open(output_file, encoding="utf-8") as f:
                content = f.read()

            if tail_lines > 0:
                lines = content.split("\n")
                return "\n".join(lines[-tail_lines:])

            return content
        except OSError:
            return None

    def wait_for_completion(
        self,
        story_ids: list[str] | None = None,
        timeout: int = 3600,
        poll_interval: int = 5
    ) -> dict[str, Any]:
        """Wait for agents to complete."""
        start_time = time.time()
        completed = []
        failed = []
        result = {"running": []}

        while True:
            result = self.check_running_agents()
            completed.extend(result.get("newly_completed", []))
            failed.extend(result.get("newly_failed", []))

            if story_ids:
                pending = set(story_ids) - set(e["story_id"] for e in completed + failed)
                if not pending:
                    break
            else:
                if result["total_running"] == 0:
                    break

            if time.time() - start_time > timeout:
                break

            time.sleep(poll_interval)

        return {
            "completed": completed,
            "failed": failed,
            "still_running": result.get("running", []),
            "elapsed_seconds": time.time() - start_time
        }

    def cleanup_stale(self, max_age_hours: int = 24) -> dict[str, Any]:
        """Clean up stale entries older than max_age_hours."""
        status = self._read_status()
        cutoff = datetime.utcnow() - timedelta(hours=max_age_hours)
        removed = {"completed": 0, "failed": 0}

        for key in ["completed", "failed"]:
            original = status.get(key, [])
            filtered = []
            for entry in original:
                timestamp_key = "completed_at" if key == "completed" else "failed_at"
                timestamp_str = entry.get(timestamp_key, "")
                try:
                    timestamp = datetime.fromisoformat(timestamp_str.replace("Z", "+00:00"))
                    if timestamp.replace(tzinfo=None) > cutoff:
                        filtered.append(entry)
                    else:
                        removed[key] += 1
                except (ValueError, TypeError):
                    filtered.append(entry)

            status[key] = filtered

        if removed["completed"] > 0 or removed["failed"] > 0:
            self._write_status(status)

        return {
            "removed_completed": removed["completed"],
            "removed_failed": removed["failed"],
            "remaining_completed": len(status.get("completed", [])),
            "remaining_failed": len(status.get("failed", []))
        }
