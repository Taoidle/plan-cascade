#!/usr/bin/env python3
"""
Agent Monitor for Plan Cascade Multi-Agent Collaboration

Monitors running agents and updates their status:
- Detects process completion/death
- Reads result files for completion status
- Cleans up stale entries
- Provides polling interface for main session

This module can be used:
1. As a library: AgentMonitor class
2. As a CLI: python agent_monitor.py check|watch|cleanup
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple


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
        from datetime import datetime
        return datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")

    def _read_status(self) -> Dict:
        """Read .agent-status.json."""
        if not self.agent_status_path.exists():
            return {"running": [], "completed": [], "failed": []}

        try:
            with open(self.agent_status_path, "r", encoding="utf-8") as f:
                return json.load(f)
        except (json.JSONDecodeError, IOError):
            return {"running": [], "completed": [], "failed": []}

    def _write_status(self, status: Dict) -> None:
        """Write .agent-status.json."""
        status["updated_at"] = self._timestamp()
        try:
            with open(self.agent_status_path, "w", encoding="utf-8") as f:
                json.dump(status, f, indent=2)
        except IOError as e:
            print(f"[Warning] Could not write status: {e}", file=sys.stderr)

    def _append_progress(self, story_id: str, message: str) -> None:
        """Append to progress.txt."""
        try:
            from datetime import datetime
            timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
            with open(self.progress_path, "a", encoding="utf-8") as f:
                f.write(f"[{timestamp}] {story_id}: {message}\n")
        except IOError:
            pass

    def _is_process_alive(self, pid: int) -> bool:
        """Check if a process is still running."""
        try:
            if sys.platform == "win32":
                # Windows: use tasklist
                result = subprocess.run(
                    ["tasklist", "/FI", f"PID eq {pid}", "/NH"],
                    capture_output=True,
                    text=True,
                    timeout=5
                )
                return str(pid) in result.stdout
            else:
                # Unix: send signal 0
                os.kill(pid, 0)
                return True
        except (OSError, subprocess.SubprocessError, subprocess.TimeoutExpired):
            return False

    def _read_result_file(self, story_id: str) -> Optional[Dict]:
        """Read result file if it exists."""
        result_file = self.output_dir / f"{story_id}.result.json"
        if not result_file.exists():
            return None

        try:
            with open(result_file, "r", encoding="utf-8") as f:
                return json.load(f)
        except (json.JSONDecodeError, IOError):
            return None

    def _check_output_for_completion(self, story_id: str) -> Optional[Tuple[bool, str]]:
        """
        Check output log for completion markers.

        Returns:
            Tuple of (success, message) if completed, None if not
        """
        output_file = self.output_dir / f"{story_id}.log"
        if not output_file.exists():
            return None

        try:
            with open(output_file, "r", encoding="utf-8") as f:
                content = f.read()

            # Look for completion markers
            if "# Exit Code: 0" in content:
                return (True, "Completed successfully")
            elif "# Exit Code:" in content:
                # Extract exit code
                for line in content.split("\n"):
                    if "# Exit Code:" in line:
                        code = line.split(":")[-1].strip()
                        return (False, f"Exit code {code}")
            elif "[TIMEOUT]" in content:
                return (False, "Timeout")

            return None
        except IOError:
            return None

    def check_running_agents(self) -> Dict[str, Any]:
        """
        Check all running agents and update status.

        This is the main polling method for the main session.

        Returns:
            Dict with:
            - updated: List of agents that changed status
            - running: List of still-running agents
            - completed: List of newly completed agents
            - failed: List of newly failed agents
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

            # Check result file first (most reliable)
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
                # Process died - check output for completion status
                completion = self._check_output_for_completion(story_id)
                if completion:
                    success, message = completion
                    if success:
                        newly_completed.append({
                            **entry,
                            "completed_at": self._timestamp()
                        })
                    else:
                        newly_failed.append({
                            **entry,
                            "error": message,
                            "failed_at": self._timestamp()
                        })
                else:
                    # Process died without completion marker
                    newly_failed.append({
                        **entry,
                        "error": "Process exited unexpectedly",
                        "failed_at": self._timestamp()
                    })
                updated.append(story_id)
                continue

            # Check timeout
            started_at = entry.get("started_at")
            timeout = entry.get("timeout", 600)
            if started_at:
                try:
                    from datetime import datetime
                    start_time = datetime.fromisoformat(started_at.replace("Z", "+00:00"))
                    now = datetime.now(start_time.tzinfo)
                    elapsed = (now - start_time).total_seconds()
                    if elapsed > timeout:
                        # Timeout - kill process
                        if pid:
                            try:
                                if sys.platform == "win32":
                                    subprocess.run(["taskkill", "/F", "/PID", str(pid)], check=False)
                                else:
                                    os.kill(pid, 9)
                            except (OSError, subprocess.SubprocessError):
                                pass

                        newly_failed.append({
                            **entry,
                            "error": f"Timeout after {timeout}s",
                            "failed_at": self._timestamp()
                        })
                        updated.append(story_id)
                        continue
                except (ValueError, TypeError):
                    pass

            # Still running
            still_running.append(entry)

        # Update status if anything changed
        if updated:
            status["running"] = still_running

            for entry in newly_completed:
                if "completed" not in status:
                    status["completed"] = []
                # Clean up running-specific fields
                clean_entry = {k: v for k, v in entry.items() if k not in ["pid", "timeout"]}
                status["completed"].append(clean_entry)
                self._append_progress(entry["story_id"], f"[COMPLETE] via {entry.get('agent', 'unknown')}")

            for entry in newly_failed:
                if "failed" not in status:
                    status["failed"] = []
                # Clean up running-specific fields
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

    def get_agent_result(self, story_id: str) -> Optional[Dict]:
        """
        Get the result of a completed agent.

        Args:
            story_id: Story ID to get result for

        Returns:
            Result dict or None if not available
        """
        # Check result file
        result = self._read_result_file(story_id)
        if result:
            return result

        # Check status file
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

    def get_agent_output(self, story_id: str, tail_lines: int = 50) -> Optional[str]:
        """
        Get the output log of an agent.

        Args:
            story_id: Story ID
            tail_lines: Number of lines from end (0 = all)

        Returns:
            Output content or None
        """
        output_file = self.output_dir / f"{story_id}.log"
        if not output_file.exists():
            return None

        try:
            with open(output_file, "r", encoding="utf-8") as f:
                content = f.read()

            if tail_lines > 0:
                lines = content.split("\n")
                return "\n".join(lines[-tail_lines:])

            return content
        except IOError:
            return None

    def wait_for_completion(
        self,
        story_ids: Optional[List[str]] = None,
        timeout: int = 3600,
        poll_interval: int = 5
    ) -> Dict[str, Any]:
        """
        Wait for agents to complete.

        Args:
            story_ids: Specific story IDs to wait for (None = all running)
            timeout: Maximum wait time in seconds
            poll_interval: Seconds between polls

        Returns:
            Dict with completed and failed agents
        """
        start_time = time.time()
        completed = []
        failed = []

        while True:
            # Check running agents
            result = self.check_running_agents()

            # Track completions
            completed.extend(result.get("newly_completed", []))
            failed.extend(result.get("newly_failed", []))

            # Check if we're done
            if story_ids:
                # Waiting for specific stories
                pending = set(story_ids) - set(e["story_id"] for e in completed + failed)
                if not pending:
                    break
            else:
                # Waiting for all running
                if result["total_running"] == 0:
                    break

            # Check timeout
            if time.time() - start_time > timeout:
                break

            time.sleep(poll_interval)

        return {
            "completed": completed,
            "failed": failed,
            "still_running": result.get("running", []),
            "elapsed_seconds": time.time() - start_time
        }

    def cleanup_stale(self, max_age_hours: int = 24) -> Dict[str, Any]:
        """
        Clean up stale entries older than max_age_hours.

        Args:
            max_age_hours: Maximum age in hours

        Returns:
            Cleanup result
        """
        status = self._read_status()
        from datetime import datetime, timedelta

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
                    filtered.append(entry)  # Keep if can't parse

            status[key] = filtered

        if removed["completed"] > 0 or removed["failed"] > 0:
            self._write_status(status)

        return {
            "removed_completed": removed["completed"],
            "removed_failed": removed["failed"],
            "remaining_completed": len(status.get("completed", [])),
            "remaining_failed": len(status.get("failed", []))
        }


def main():
    """CLI interface for agent monitor."""
    import argparse

    parser = argparse.ArgumentParser(description="Agent Monitor CLI")
    parser.add_argument("command", choices=["check", "watch", "result", "output", "wait", "cleanup"],
                        help="Command to run")
    parser.add_argument("--project-root", default=".", help="Project root directory")
    parser.add_argument("--story-id", help="Story ID (for result/output commands)")
    parser.add_argument("--poll-interval", type=int, default=5, help="Poll interval for watch/wait")
    parser.add_argument("--timeout", type=int, default=3600, help="Timeout for wait command")
    parser.add_argument("--tail", type=int, default=50, help="Tail lines for output command")
    parser.add_argument("--max-age", type=int, default=24, help="Max age hours for cleanup")
    parser.add_argument("--json", action="store_true", help="Output as JSON")

    args = parser.parse_args()
    monitor = AgentMonitor(Path(args.project_root).resolve())

    if args.command == "check":
        result = monitor.check_running_agents()
        if args.json:
            print(json.dumps(result, indent=2))
        else:
            print(f"Running: {result['total_running']}")
            print(f"Completed: {result['total_completed']}")
            print(f"Failed: {result['total_failed']}")
            if result["updated"]:
                print(f"Updated: {', '.join(result['updated'])}")

    elif args.command == "watch":
        print("Watching agents... (Ctrl+C to stop)")
        try:
            while True:
                result = monitor.check_running_agents()
                if result["updated"]:
                    print(f"[{monitor._timestamp()}] Updated: {result['updated']}")
                if result["total_running"] == 0:
                    print("No agents running.")
                    break
                time.sleep(args.poll_interval)
        except KeyboardInterrupt:
            print("\nStopped.")

    elif args.command == "result":
        if not args.story_id:
            print("Error: --story-id required", file=sys.stderr)
            sys.exit(1)
        result = monitor.get_agent_result(args.story_id)
        if result:
            if args.json:
                print(json.dumps(result, indent=2))
            else:
                status = "SUCCESS" if result.get("success") else "FAILED"
                print(f"Story: {args.story_id}")
                print(f"Status: {status}")
                if result.get("error"):
                    print(f"Error: {result['error']}")
                print(f"Output: {result.get('output_file', 'N/A')}")
        else:
            print(f"No result found for {args.story_id}")
            sys.exit(1)

    elif args.command == "output":
        if not args.story_id:
            print("Error: --story-id required", file=sys.stderr)
            sys.exit(1)
        output = monitor.get_agent_output(args.story_id, args.tail)
        if output:
            print(output)
        else:
            print(f"No output found for {args.story_id}")
            sys.exit(1)

    elif args.command == "wait":
        story_ids = [args.story_id] if args.story_id else None
        print(f"Waiting for {'story ' + args.story_id if args.story_id else 'all agents'}...")
        result = monitor.wait_for_completion(
            story_ids=story_ids,
            timeout=args.timeout,
            poll_interval=args.poll_interval
        )
        if args.json:
            print(json.dumps(result, indent=2, default=str))
        else:
            print(f"Completed: {len(result['completed'])}")
            print(f"Failed: {len(result['failed'])}")
            print(f"Still running: {len(result['still_running'])}")
            print(f"Elapsed: {result['elapsed_seconds']:.1f}s")

    elif args.command == "cleanup":
        result = monitor.cleanup_stale(args.max_age)
        if args.json:
            print(json.dumps(result, indent=2))
        else:
            print(f"Removed {result['removed_completed']} completed, {result['removed_failed']} failed")


if __name__ == "__main__":
    main()
