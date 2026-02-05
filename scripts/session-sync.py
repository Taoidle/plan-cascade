#!/usr/bin/env python3
"""
Session Sync Script (Claude Code)

Incrementally tails Claude Code's local session logs and persists tool inputs/
outputs to a durable journal under `.state/claude-session/` (or user data dir
in new mode if PathResolver is used elsewhere).

This is primarily used by plugin hooks to make executions resilient to
conversation history compaction/truncation.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


def _setup_path() -> None:
    root = Path(__file__).parent.parent
    src = root / "src"
    if src.exists() and str(src) not in sys.path:
        sys.path.insert(0, str(src))


_setup_path()


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Sync Claude session logs into a local journal")
    p.add_argument(
        "--project-root",
        type=Path,
        default=Path.cwd(),
        help="Project/worktree directory to sync (default: cwd)",
    )
    p.add_argument(
        "--journal-dir",
        type=Path,
        default=None,
        help="Override journal directory (default: <project-root>/.state/claude-session)",
    )
    p.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress output (exit code still indicates success/failure)",
    )
    return p.parse_args()


def main() -> int:
    args = parse_args()
    project_root: Path = args.project_root.resolve()
    journal_dir = args.journal_dir or (project_root / ".state" / "claude-session")

    try:
        from plan_cascade.state.claude_journal import ClaudeSessionJournal
    except Exception as e:
        if not args.quiet:
            print(f"[session-sync] Error: cannot import journal: {e}", file=sys.stderr)
        return 2

    journal = ClaudeSessionJournal(project_root=project_root, journal_dir=journal_dir)
    result = journal.sync()

    if not args.quiet:
        if result.ok:
            print(
                f"[session-sync] ok session={result.session_file} "
                f"+uses={result.new_tool_uses} +results={result.new_tool_results} +bytes={result.new_bytes}"
            )
        else:
            print(f"[session-sync] skipped: {result.reason}", file=sys.stderr)

    return 0 if result.ok else 1


if __name__ == "__main__":
    raise SystemExit(main())

