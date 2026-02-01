"""
Gate Cache for Plan Cascade

Provides caching of quality gate results based on project state (git commit hash + working tree hash).
When project state hasn't changed, returns cached results instead of re-executing gates.
"""

import json
import time
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import TYPE_CHECKING, Any

from .changed_files import ChangedFilesDetector
from .quality_gate import GateOutput, GateType

if TYPE_CHECKING:
    from plan_cascade.state.path_resolver import PathResolver


@dataclass
class CacheEntry:
    """
    Represents a cached gate result.

    Attributes:
        cache_key: Cache key (git hash + tree hash)
        gate_name: Name of the gate
        output: Gate execution result
        created_at: Creation timestamp (ISO format)
        expires_at: Expiration timestamp (ISO format) or None for no expiration
    """

    cache_key: str
    gate_name: str
    output: GateOutput
    created_at: str
    expires_at: str | None = None

    def is_expired(self) -> bool:
        """Check if this cache entry has expired."""
        if self.expires_at is None:
            return False
        try:
            expires = datetime.fromisoformat(self.expires_at.replace("Z", "+00:00"))
            now = datetime.now(expires.tzinfo) if expires.tzinfo else datetime.now()
            return now > expires
        except (ValueError, TypeError):
            return True

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        # Convert structured_errors to serializable format
        structured_errors_list: list[dict[str, Any]] = []
        if self.output.structured_errors:
            structured_errors_list = [
                {
                    "file": e.file,
                    "line": e.line,
                    "column": e.column,
                    "code": e.code,
                    "message": e.message,
                    "severity": e.severity,
                }
                for e in self.output.structured_errors
            ]

        output_dict: dict[str, Any] = {
            "gate_name": self.output.gate_name,
            "gate_type": self.output.gate_type.value,
            "passed": self.output.passed,
            "exit_code": self.output.exit_code,
            "stdout": self.output.stdout,
            "stderr": self.output.stderr,
            "duration_seconds": self.output.duration_seconds,
            "command": self.output.command,
            "error_summary": self.output.error_summary,
            "skipped": self.output.skipped,
            "checked_files": self.output.checked_files,
            "structured_errors": structured_errors_list,
        }

        return {
            "cache_key": self.cache_key,
            "gate_name": self.gate_name,
            "output": output_dict,
            "created_at": self.created_at,
            "expires_at": self.expires_at,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "CacheEntry":
        """Create from dictionary."""
        from .error_parser import ErrorInfo

        output_data = data["output"]

        # Reconstruct structured_errors
        structured_errors = []
        for e in output_data.get("structured_errors", []):
            structured_errors.append(
                ErrorInfo(
                    file=e.get("file", ""),
                    line=e.get("line"),
                    column=e.get("column"),
                    code=e.get("code"),
                    message=e.get("message", ""),
                    severity=e.get("severity", "error"),
                )
            )

        output = GateOutput(
            gate_name=output_data["gate_name"],
            gate_type=GateType(output_data["gate_type"]),
            passed=output_data["passed"],
            exit_code=output_data["exit_code"],
            stdout=output_data["stdout"],
            stderr=output_data["stderr"],
            duration_seconds=output_data["duration_seconds"],
            command=output_data["command"],
            error_summary=output_data.get("error_summary"),
            structured_errors=structured_errors,
            skipped=output_data.get("skipped", False),
            checked_files=output_data.get("checked_files"),
        )

        return cls(
            cache_key=data["cache_key"],
            gate_name=data["gate_name"],
            output=output,
            created_at=data["created_at"],
            expires_at=data.get("expires_at"),
        )


class GateCache:
    """
    Caches quality gate results based on project state.

    The cache key is computed from:
    - Git HEAD commit hash
    - Staged changes hash
    - Unstaged changes hash

    This ensures cached results are only used when the project state is identical.
    """

    CACHE_FILE = "gate-cache.json"

    def __init__(
        self,
        project_root: Path,
        cache_dir: Path | None = None,
        path_resolver: "PathResolver | None" = None,
    ):
        """
        Initialize the gate cache.

        Args:
            project_root: Root directory of the project
            cache_dir: Directory for cache storage (defaults to .state/)
            path_resolver: Optional PathResolver for determining cache location
        """
        self.project_root = Path(project_root)
        self._detector = ChangedFilesDetector(self.project_root)
        self._entries: dict[str, CacheEntry] = {}
        self._current_cache_key: str | None = None

        # Determine cache directory
        if cache_dir is not None:
            self._cache_dir = Path(cache_dir)
        elif path_resolver is not None:
            self._cache_dir = path_resolver.get_state_dir()
        else:
            # Default to .state/ in project root
            self._cache_dir = self.project_root / ".state"

        self._cache_file = self._cache_dir / self.CACHE_FILE
        self._load_cache()

    def _load_cache(self) -> None:
        """Load cache from disk."""
        if not self._cache_file.exists():
            return

        try:
            with open(self._cache_file, encoding="utf-8") as f:
                data = json.load(f)

            for entry_data in data.get("entries", []):
                try:
                    entry = CacheEntry.from_dict(entry_data)
                    self._entries[entry.gate_name] = entry
                except (KeyError, ValueError):
                    # Skip invalid entries
                    continue
        except (OSError, json.JSONDecodeError):
            # Start with empty cache on error
            self._entries = {}

    def _save_cache(self) -> None:
        """Save cache to disk."""
        self._cache_dir.mkdir(parents=True, exist_ok=True)

        data = {
            "version": "1.0.0",
            "updated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "entries": [entry.to_dict() for entry in self._entries.values()],
        }

        try:
            with open(self._cache_file, "w", encoding="utf-8") as f:
                json.dump(data, f, indent=2)
        except OSError:
            # Ignore write errors
            pass

    def _compute_cache_key(self) -> str:
        """
        Compute the current cache key based on project state.

        Uses ChangedFilesDetector.compute_tree_hash() to get a hash
        representing the current state of the working tree.

        Returns:
            Cache key string
        """
        if self._current_cache_key is None:
            self._current_cache_key = self._detector.compute_tree_hash()
        return self._current_cache_key

    def invalidate_cache_key(self) -> None:
        """
        Invalidate the cached cache key.

        Call this when you know the project state has changed
        and want to force recalculation of the cache key.
        """
        self._current_cache_key = None

    def get(self, gate_name: str) -> GateOutput | None:
        """
        Get cached result for a gate.

        Returns None if:
        - No cached result exists
        - Cache entry has expired
        - Project state (cache key) has changed

        Args:
            gate_name: Name of the gate

        Returns:
            Cached GateOutput with from_cache=True, or None if not cached/invalid
        """
        entry = self._entries.get(gate_name)
        if entry is None:
            return None

        # Check expiration
        if entry.is_expired():
            del self._entries[gate_name]
            self._save_cache()
            return None

        # Check cache key matches current state
        current_key = self._compute_cache_key()
        if entry.cache_key != current_key:
            return None

        # Return cached output with from_cache marker
        # Create a new GateOutput with from_cache=True
        cached_output = GateOutput(
            gate_name=entry.output.gate_name,
            gate_type=entry.output.gate_type,
            passed=entry.output.passed,
            exit_code=entry.output.exit_code,
            stdout=entry.output.stdout,
            stderr=entry.output.stderr,
            duration_seconds=entry.output.duration_seconds,
            command=entry.output.command,
            error_summary=entry.output.error_summary,
            structured_errors=entry.output.structured_errors,
            skipped=entry.output.skipped,
            checked_files=entry.output.checked_files,
            from_cache=True,
        )
        return cached_output

    def set(
        self,
        gate_name: str,
        output: GateOutput,
        ttl: int | None = None,
    ) -> None:
        """
        Cache a gate result.

        Args:
            gate_name: Name of the gate
            output: Gate execution result
            ttl: Time-to-live in seconds (None for no expiration)
        """
        cache_key = self._compute_cache_key()
        created_at = time.strftime("%Y-%m-%dT%H:%M:%SZ")

        expires_at = None
        if ttl is not None:
            expires_at = time.strftime(
                "%Y-%m-%dT%H:%M:%SZ", time.gmtime(time.time() + ttl)
            )

        entry = CacheEntry(
            cache_key=cache_key,
            gate_name=gate_name,
            output=output,
            created_at=created_at,
            expires_at=expires_at,
        )

        self._entries[gate_name] = entry
        self._save_cache()

    def invalidate(self, gate_name: str | None = None) -> None:
        """
        Invalidate cached results.

        Args:
            gate_name: Specific gate to invalidate, or None to clear all
        """
        if gate_name is None:
            self._entries.clear()
        elif gate_name in self._entries:
            del self._entries[gate_name]

        self._save_cache()

    def is_valid(self, gate_name: str) -> bool:
        """
        Check if a cache entry is valid for the current project state.

        Args:
            gate_name: Name of the gate

        Returns:
            True if cache entry exists and is valid for current state
        """
        entry = self._entries.get(gate_name)
        if entry is None:
            return False

        # Check expiration
        if entry.is_expired():
            return False

        # Check cache key matches
        current_key = self._compute_cache_key()
        return entry.cache_key == current_key

    def get_cache_stats(self) -> dict[str, Any]:
        """
        Get statistics about the cache.

        Returns:
            Dictionary with cache statistics
        """
        current_key = self._compute_cache_key()
        valid_count = 0
        expired_count = 0
        stale_count = 0

        for entry in self._entries.values():
            if entry.is_expired():
                expired_count += 1
            elif entry.cache_key != current_key:
                stale_count += 1
            else:
                valid_count += 1

        return {
            "total_entries": len(self._entries),
            "valid_entries": valid_count,
            "expired_entries": expired_count,
            "stale_entries": stale_count,
            "current_cache_key": current_key,
            "cache_file": str(self._cache_file),
        }

    def clear(self) -> None:
        """Clear all cache entries and remove cache file."""
        self._entries.clear()
        self._current_cache_key = None

        try:
            if self._cache_file.exists():
                self._cache_file.unlink()
        except OSError:
            pass
