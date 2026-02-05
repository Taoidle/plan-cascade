#!/usr/bin/env python3
"""
Claude Code session helpers.

This module provides best-effort discovery of Claude Code's on-disk project/session
logs (usually under ~/.claude/projects/) and utilities to locate the project
directory that corresponds to a given working directory.

Why this exists:
- Claude Code can compact/summarize conversation history.
- The raw tool inputs/outputs remain in the local session jsonl logs.
- Plugins can persist a durable "journal" by tailing those logs.

This file intentionally avoids importing any Claude/Anthropic-specific SDKs and
only relies on filesystem conventions observed in Claude Code.
"""

from __future__ import annotations

import json
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any


def get_claude_dir() -> Path:
    """Return Claude Code's home directory (usually ~/.claude)."""
    override = os.environ.get("CLAUDE_HOME") or os.environ.get("CLAUDE_DIR")
    if override:
        return Path(override).expanduser().resolve()
    return Path.home() / ".claude"


def get_projects_dir() -> Path:
    """Return ~/.claude/projects."""
    return get_claude_dir() / "projects"


def normalize_path_for_compare(path: str | Path) -> str:
    """Normalize paths for comparison across platforms."""
    p = Path(path).expanduser()
    try:
        p = p.resolve()
    except Exception:
        # If path doesn't exist, fall back to absolute-ish string.
        p = Path(os.path.abspath(str(p)))

    s = str(p).replace("\\", "/").rstrip("/")
    # Windows paths are case-insensitive in practice.
    if os.name == "nt":
        s = s.lower()
    return s


def _sanitize_like_claude(project_path: str) -> str:
    """
    Approximate Claude Code's project directory naming.

    Observed on Windows:
      D:\\Repo\\proj -> D--Repo-proj
    (":" and path separators become "-"; "_" becomes "-")
    """
    s = project_path
    s = s.replace(":", "-")
    s = s.replace("\\", "-")
    s = s.replace("/", "-")
    s = s.replace("_", "-")
    return s


def candidate_project_dir_names(project_root: Path) -> list[str]:
    """
    Generate candidate ~/.claude/projects directory names for a project root.

    Claude Code's naming has changed across versions/platforms, so we try a few.
    """
    raw = str(project_root)
    raw_resolved = str(project_root.expanduser().resolve())

    cands: list[str] = []
    for basis in [raw_resolved, raw]:
        v2 = _sanitize_like_claude(basis)
        cands.extend(
            [
                v2,
                v2.lstrip("-"),
                "-" + v2.lstrip("-"),
            ]
        )

        # Legacy heuristic used by older scripts (kept for backward compat)
        v1 = basis.replace("/", "-")
        if not v1.startswith("-"):
            v1 = "-" + v1
        v1 = v1.replace("_", "-")
        cands.append(v1)

        v1w = basis.replace("\\", "-").replace(":", "-")
        if not v1w.startswith("-"):
            v1w = "-" + v1w
        v1w = v1w.replace("_", "-")
        cands.append(v1w)

    # Deduplicate while preserving order
    seen: set[str] = set()
    result: list[str] = []
    for name in cands:
        if not name or name in seen:
            continue
        seen.add(name)
        result.append(name)
    return result


def get_latest_session_file(project_dir: Path) -> Path | None:
    """Return the newest non-agent *.jsonl session file under a Claude project dir."""
    try:
        candidates = [
            p
            for p in project_dir.glob("*.jsonl")
            if p.is_file() and not p.name.startswith("agent-")
        ]
    except OSError:
        return None

    if not candidates:
        return None

    try:
        return max(candidates, key=lambda p: p.stat().st_mtime)
    except OSError:
        return None


def _session_file_mentions_cwd(session_file: Path, target_cwd_norm: str) -> bool:
    """Cheap check: scan first N lines of a session file for matching cwd."""
    try:
        with open(session_file, "r", encoding="utf-8") as f:
            for i, line in enumerate(f):
                if i > 80:
                    break
                try:
                    data = json.loads(line)
                except Exception:
                    continue
                cwd = data.get("cwd")
                if not cwd:
                    continue
                if normalize_path_for_compare(cwd) == target_cwd_norm:
                    return True
    except OSError:
        return False
    return False


def resolve_project_dir(project_root: Path, projects_dir: Path | None = None) -> Path | None:
    """
    Resolve ~/.claude/projects/<project>/ directory for the given project root.

    Tries known sanitization patterns first, then falls back to scanning for a
    session file whose `cwd` matches the given project_root.
    """
    projects_dir = projects_dir or get_projects_dir()
    if not projects_dir.exists():
        return None

    # Fast path: try common sanitized directory names.
    for name in candidate_project_dir_names(project_root):
        candidate = projects_dir / name
        if candidate.exists() and candidate.is_dir():
            return candidate

    # Slow path: scan directories and pick the one whose session logs mention cwd.
    target_norm = normalize_path_for_compare(project_root)
    try:
        for child in projects_dir.iterdir():
            if not child.is_dir():
                continue
            session = get_latest_session_file(child)
            if not session:
                continue
            if _session_file_mentions_cwd(session, target_norm):
                return child
    except OSError:
        return None

    return None


@dataclass
class SessionCursor:
    """Cursor for incremental tailing of a session jsonl file."""

    session_file: str | None = None
    offset: int = 0

    @classmethod
    def load(cls, path: Path) -> "SessionCursor":
        if not path.exists():
            return cls()
        try:
            with open(path, encoding="utf-8") as f:
                data: dict[str, Any] = json.load(f)
            return cls(
                session_file=data.get("session_file"),
                offset=int(data.get("offset", 0) or 0),
            )
        except Exception:
            return cls()

    def save(self, path: Path) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        payload = {"session_file": self.session_file, "offset": self.offset}
        with open(path, "w", encoding="utf-8") as f:
            json.dump(payload, f, indent=2)

