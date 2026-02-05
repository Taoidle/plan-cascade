#!/usr/bin/env python3
"""
Claude Session Journal (compaction-safe tool I/O persistence).

Claude Code stores full conversation + tool inputs/outputs in local session jsonl
logs (usually under ~/.claude/projects/<project>/*.jsonl). During long runs,
conversation history may be compacted/summarized before the agent has written
all important details to user-visible docs (progress/findings/etc.).

This module tails the session logs incrementally and persists:
- Tool uses (name + truncated input)
- Tool results (full output saved to disk, indexed in JSONL)

The resulting journal can be referenced by context reminder files so the agent
can recover the last operations even after history compaction/truncation.
"""

from __future__ import annotations

import json
import time
from collections import deque
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from .claude_session import (
    SessionCursor,
    get_latest_session_file,
    resolve_project_dir,
)
from .state_manager import FileLock


def _truncate_string(value: str, limit: int) -> str:
    if len(value) <= limit:
        return value
    return value[: max(0, limit - 20)] + "\n...[truncated]...\n"


def _truncate_deep(value: Any, *, max_str: int, max_list: int, max_dict: int, max_depth: int) -> Any:
    if max_depth <= 0:
        return "<max-depth>"

    if isinstance(value, str):
        return _truncate_string(value, max_str)

    if isinstance(value, (int, float, bool)) or value is None:
        return value

    if isinstance(value, list):
        trimmed = value[:max_list]
        return [
            _truncate_deep(v, max_str=max_str, max_list=max_list, max_dict=max_dict, max_depth=max_depth - 1)
            for v in trimmed
        ] + (["<truncated-list>"] if len(value) > max_list else [])

    if isinstance(value, dict):
        items = list(value.items())[:max_dict]
        out: dict[str, Any] = {}
        for k, v in items:
            out[str(k)] = _truncate_deep(v, max_str=max_str, max_list=max_list, max_dict=max_dict, max_depth=max_depth - 1)
        if len(value) > max_dict:
            out["<truncated-dict>"] = True
        return out

    return str(value)


def _iter_nested_dicts(obj: Any) -> Iterable[dict[str, Any]]:
    if isinstance(obj, dict):
        yield obj
        for v in obj.values():
            if isinstance(v, (dict, list)):
                yield from _iter_nested_dicts(v)
        return
    if isinstance(obj, list):
        for item in obj:
            if isinstance(item, (dict, list)):
                yield from _iter_nested_dicts(item)


def _extract_tool_uses(entry: dict[str, Any]) -> list[dict[str, Any]]:
    uses: dict[str, dict[str, Any]] = {}
    for d in _iter_nested_dicts(entry):
        if d.get("type") != "tool_use":
            continue
        tool_id = d.get("id")
        name = d.get("name")
        if not tool_id or not name:
            continue
        uses[str(tool_id)] = {
            "tool_use_id": str(tool_id),
            "tool_name": str(name),
            "input": d.get("input", {}),
        }
    return list(uses.values())


def _extract_tool_results(entry: dict[str, Any]) -> list[dict[str, Any]]:
    results: list[dict[str, Any]] = []
    for d in _iter_nested_dicts(entry):
        if d.get("type") != "tool_result":
            continue
        tool_use_id = d.get("tool_use_id")
        content = d.get("content")
        if not tool_use_id or content is None:
            continue
        results.append(
            {
                "tool_use_id": str(tool_use_id),
                "content": str(content),
            }
        )
    return results


@dataclass
class JournalSyncResult:
    ok: bool
    reason: str = ""
    session_file: str | None = None
    new_tool_uses: int = 0
    new_tool_results: int = 0
    new_bytes: int = 0


class ClaudeSessionJournal:
    """
    Persist tool I/O from Claude session logs into a durable on-disk journal.

    Storage layout (under journal_dir):
      - cursor.json
      - index.jsonl
      - recent.json
      - tool-results/<tool_use_id>.txt
    """

    CURSOR_FILE = "cursor.json"
    INDEX_FILE = "index.jsonl"
    RECENT_FILE = "recent.json"
    RESULTS_DIR = "tool-results"
    LOCK_FILE = "sync.lock"

    def __init__(
        self,
        project_root: Path,
        journal_dir: Path,
        *,
        max_tool_result_chars: int = 250_000,
        max_tool_input_chars: int = 10_000,
        recent_events: int = 30,
    ):
        self.project_root = Path(project_root).resolve()
        self.journal_dir = Path(journal_dir)
        self.cursor_path = self.journal_dir / self.CURSOR_FILE
        self.index_path = self.journal_dir / self.INDEX_FILE
        self.recent_path = self.journal_dir / self.RECENT_FILE
        self.results_dir = self.journal_dir / self.RESULTS_DIR
        self.lock_path = self.journal_dir / self.LOCK_FILE

        self.max_tool_result_chars = max_tool_result_chars
        self.max_tool_input_chars = max_tool_input_chars
        self.recent_events = recent_events

    def _ensure_dirs(self) -> None:
        self.results_dir.mkdir(parents=True, exist_ok=True)

    def sync(self) -> JournalSyncResult:
        """
        Incrementally sync from the latest Claude session jsonl to the journal.

        Returns:
            JournalSyncResult with counts; ok=False if no Claude logs found.
        """
        projects_dir = resolve_project_dir(self.project_root)
        if projects_dir is None:
            return JournalSyncResult(ok=False, reason="claude_project_dir_not_found")

        session_file = get_latest_session_file(projects_dir)
        if session_file is None:
            return JournalSyncResult(ok=False, reason="no_session_file_found")

        self._ensure_dirs()

        # Use a single lock to keep cursor/index consistent across parallel agents.
        with FileLock(self.lock_path):
            cursor = SessionCursor.load(self.cursor_path)
            if cursor.session_file != session_file.name:
                cursor.session_file = session_file.name
                cursor.offset = 0

            try:
                file_size = session_file.stat().st_size
            except OSError:
                return JournalSyncResult(ok=False, reason="cannot_stat_session_file", session_file=session_file.name)

            if cursor.offset < 0 or cursor.offset > file_size:
                cursor.offset = 0

            new_events: list[dict[str, Any]] = []
            tool_uses_count = 0
            tool_results_count = 0
            new_bytes = 0

            try:
                with open(session_file, "rb") as f:
                    f.seek(cursor.offset)
                    while True:
                        line = f.readline()
                        if not line:
                            break
                        new_bytes += len(line)
                        cursor.offset = f.tell()

                        try:
                            text_line = line.decode("utf-8", errors="replace").strip()
                            if not text_line:
                                continue
                            entry = json.loads(text_line)
                        except Exception:
                            continue

                        ts = entry.get("timestamp") or ""
                        session_id = entry.get("sessionId") or ""
                        uuid = entry.get("uuid") or ""

                        # Tool uses
                        for tool in _extract_tool_uses(entry):
                            tool_uses_count += 1
                            ev = {
                                "ts": ts,
                                "session_id": session_id,
                                "uuid": uuid,
                                "kind": "tool_use",
                                "tool_name": tool.get("tool_name"),
                                "tool_use_id": tool.get("tool_use_id"),
                                "input": _truncate_deep(
                                    tool.get("input", {}),
                                    max_str=self.max_tool_input_chars,
                                    max_list=50,
                                    max_dict=80,
                                    max_depth=6,
                                ),
                            }
                            new_events.append(ev)

                        # Tool results (full content persisted to file)
                        for result in _extract_tool_results(entry):
                            tool_results_count += 1
                            tool_use_id = result.get("tool_use_id", "unknown")
                            content = result.get("content", "")
                            truncated = len(content) > self.max_tool_result_chars
                            content_to_write = _truncate_string(content, self.max_tool_result_chars)

                            result_path = self.results_dir / f"{tool_use_id}.txt"
                            if not result_path.exists():
                                try:
                                    with open(result_path, "w", encoding="utf-8") as out:
                                        out.write(content_to_write)
                                except OSError:
                                    # If we can't persist, still index a preview to avoid losing everything.
                                    result_path = None

                            tool_meta = entry.get("toolUseResult") if isinstance(entry.get("toolUseResult"), dict) else None
                            ev = {
                                "ts": ts,
                                "session_id": session_id,
                                "uuid": uuid,
                                "kind": "tool_result",
                                "tool_use_id": tool_use_id,
                                "content_file": str(result_path) if result_path else None,
                                "preview": _truncate_string(content, 800),
                                "truncated": truncated,
                                "meta": _truncate_deep(tool_meta or {}, max_str=2000, max_list=50, max_dict=80, max_depth=4),
                            }
                            new_events.append(ev)

            except OSError:
                return JournalSyncResult(ok=False, reason="cannot_read_session_file", session_file=session_file.name)

            # Append new events to index.jsonl
            if new_events:
                try:
                    with open(self.index_path, "a", encoding="utf-8") as idx:
                        for ev in new_events:
                            idx.write(json.dumps(ev, ensure_ascii=False) + "\n")
                except OSError:
                    # Non-fatal; cursor still advances, but recent may be stale.
                    pass

                # Update recent.json for fast tail reads
                recent = self.read_recent(max_events=self.recent_events)
                recent.extend(new_events)
                recent = recent[-self.recent_events :]
                try:
                    with open(self.recent_path, "w", encoding="utf-8") as f:
                        json.dump(recent, f, ensure_ascii=False, indent=2)
                except OSError:
                    pass

            # Save cursor last (after index write)
            try:
                cursor.save(self.cursor_path)
            except OSError:
                pass

            return JournalSyncResult(
                ok=True,
                session_file=session_file.name,
                new_tool_uses=tool_uses_count,
                new_tool_results=tool_results_count,
                new_bytes=new_bytes,
            )

    def read_recent(self, *, max_events: int = 20) -> list[dict[str, Any]]:
        """Read up to max_events recent events (fast path)."""
        if self.recent_path.exists():
            try:
                with open(self.recent_path, encoding="utf-8") as f:
                    data = json.load(f)
                if isinstance(data, list):
                    return data[-max_events:]
            except Exception:
                pass

        # Fallback: read tail from index.jsonl (may be slower for large files)
        if not self.index_path.exists():
            return []

        try:
            dq: deque[dict[str, Any]] = deque(maxlen=max_events)
            with open(self.index_path, encoding="utf-8") as f:
                for line in f:
                    try:
                        dq.append(json.loads(line))
                    except Exception:
                        continue
            return list(dq)
        except OSError:
            return []

