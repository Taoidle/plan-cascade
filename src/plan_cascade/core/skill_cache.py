#!/usr/bin/env python3
"""
Skill Cache for Plan Cascade

Implements caching mechanism for remote URL skills:
- Caches downloaded skills to ~/.plan-cascade/cache/skills/
- Default TTL of 7 days
- Graceful degradation using expired cache on network errors
- Cache metadata with download time, expiry, and content hash

Design Decision ADR-F003:
- Cache location: ~/.plan-cascade/cache/skills/
- Default TTL: 7 days
- Support manual refresh via CLI
"""

import hashlib
import json
import logging
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Optional

logger = logging.getLogger(__name__)


@dataclass
class SkillCacheEntry:
    """Represents a cached skill entry with metadata.

    Attributes:
        url: Original URL the skill was downloaded from
        cached_at: ISO-8601 timestamp when the skill was cached
        expires_at: ISO-8601 timestamp when the cache expires
        content_hash: SHA-256 hash of the content for change detection
        local_path: Local filesystem path where the skill is cached
        size_bytes: Size of the cached content in bytes
    """

    url: str
    cached_at: str
    expires_at: str
    content_hash: str
    local_path: str
    size_bytes: int = 0

    def is_expired(self) -> bool:
        """Check if this cache entry has expired.

        Returns:
            True if the cache entry has expired
        """
        try:
            expires = datetime.fromisoformat(self.expires_at.replace("Z", "+00:00"))
            now = datetime.now(timezone.utc)
            return now >= expires
        except (ValueError, TypeError):
            # Invalid timestamp, consider expired
            return True

    def to_dict(self) -> dict:
        """Convert to dictionary for JSON serialization.

        Returns:
            Dict representation of the cache entry
        """
        return {
            "url": self.url,
            "cached_at": self.cached_at,
            "expires_at": self.expires_at,
            "content_hash": self.content_hash,
            "local_path": self.local_path,
            "size_bytes": self.size_bytes,
        }

    @classmethod
    def from_dict(cls, data: dict) -> "SkillCacheEntry":
        """Create SkillCacheEntry from dictionary.

        Args:
            data: Dict with cache entry fields

        Returns:
            SkillCacheEntry instance
        """
        return cls(
            url=data.get("url", ""),
            cached_at=data.get("cached_at", ""),
            expires_at=data.get("expires_at", ""),
            content_hash=data.get("content_hash", ""),
            local_path=data.get("local_path", ""),
            size_bytes=data.get("size_bytes", 0),
        )


class SkillCache:
    """Manages caching of remote skills for offline access and performance.

    Provides:
    - Local caching of downloaded skills
    - Configurable TTL (default 7 days)
    - Graceful degradation with expired cache on network failures
    - Cache statistics and management commands

    Cache structure:
        ~/.plan-cascade/cache/skills/
        +-- <url-hash-1>/
        |   +-- SKILL.md          # Cached content
        |   +-- metadata.json     # Cache metadata
        +-- <url-hash-2>/
            +-- SKILL.md
            +-- metadata.json
    """

    DEFAULT_CACHE_DIR = Path.home() / ".plan-cascade" / "cache" / "skills"
    DEFAULT_TTL_DAYS = 7
    SKILL_FILENAME = "SKILL.md"
    METADATA_FILENAME = "metadata.json"

    def __init__(
        self,
        cache_dir: Path | None = None,
        ttl_days: int = DEFAULT_TTL_DAYS,
        verbose: bool = False,
    ):
        """Initialize the skill cache.

        Args:
            cache_dir: Directory to store cached skills (default: ~/.plan-cascade/cache/skills/)
            ttl_days: Time-to-live for cache entries in days (default: 7)
            verbose: Enable verbose logging
        """
        self.cache_dir = Path(cache_dir) if cache_dir else self.DEFAULT_CACHE_DIR
        self.ttl_days = ttl_days
        self._verbose = verbose

        # Ensure cache directory exists
        self.cache_dir.mkdir(parents=True, exist_ok=True)

    def _log(self, message: str, level: str = "info") -> None:
        """Log a message with appropriate formatting.

        Args:
            message: Message to log
            level: Log level ("info", "warning", "debug", "error")
        """
        prefix = "[SkillCache]"
        if level == "debug" and not self._verbose:
            return

        if level == "warning":
            logger.warning(f"{prefix} {message}")
        elif level == "error":
            logger.error(f"{prefix} {message}")
        elif level == "debug":
            logger.debug(f"{prefix} {message}")
        else:
            logger.info(f"{prefix} {message}")

        # Also print for CLI visibility
        if self._verbose or level in ("warning", "error"):
            print(f"{prefix} {message}")

    def _url_to_hash(self, url: str) -> str:
        """Generate a unique hash for a URL.

        Args:
            url: The URL to hash

        Returns:
            SHA-256 hash prefix (16 chars) of the URL
        """
        return hashlib.sha256(url.encode("utf-8")).hexdigest()[:16]

    def _content_hash(self, content: str) -> str:
        """Generate a hash for content.

        Args:
            content: The content to hash

        Returns:
            SHA-256 hash of the content with sha256: prefix
        """
        hash_value = hashlib.sha256(content.encode("utf-8")).hexdigest()
        return f"sha256:{hash_value}"

    def get_cache_path(self, url: str) -> Path:
        """Get the cache directory path for a URL.

        Args:
            url: The URL to get cache path for

        Returns:
            Path to the cache directory for this URL
        """
        url_hash = self._url_to_hash(url)
        return self.cache_dir / url_hash

    def get_skill_path(self, url: str) -> Path:
        """Get the path to the cached SKILL.md file.

        Args:
            url: The URL to get skill path for

        Returns:
            Path to the cached SKILL.md file
        """
        return self.get_cache_path(url) / self.SKILL_FILENAME

    def get_metadata_path(self, url: str) -> Path:
        """Get the path to the cache metadata file.

        Args:
            url: The URL to get metadata path for

        Returns:
            Path to the metadata.json file
        """
        return self.get_cache_path(url) / self.METADATA_FILENAME

    def _load_metadata(self, url: str) -> SkillCacheEntry | None:
        """Load cache metadata for a URL.

        Args:
            url: The URL to load metadata for

        Returns:
            SkillCacheEntry or None if metadata doesn't exist or is invalid
        """
        metadata_path = self.get_metadata_path(url)
        if not metadata_path.exists():
            return None

        try:
            data = json.loads(metadata_path.read_text(encoding="utf-8"))
            return SkillCacheEntry.from_dict(data)
        except (json.JSONDecodeError, OSError) as e:
            self._log(f"Error loading metadata for {url}: {e}", "warning")
            return None

    def _save_metadata(self, entry: SkillCacheEntry) -> bool:
        """Save cache metadata.

        Args:
            entry: The SkillCacheEntry to save

        Returns:
            True if save succeeded
        """
        metadata_path = self.get_metadata_path(entry.url)
        try:
            metadata_path.parent.mkdir(parents=True, exist_ok=True)
            metadata_path.write_text(
                json.dumps(entry.to_dict(), indent=2, ensure_ascii=False),
                encoding="utf-8",
            )
            return True
        except OSError as e:
            self._log(f"Error saving metadata: {e}", "error")
            return False

    def is_cached(self, url: str) -> bool:
        """Check if a URL is cached (regardless of expiration).

        Args:
            url: The URL to check

        Returns:
            True if the URL has a cache entry
        """
        skill_path = self.get_skill_path(url)
        metadata_path = self.get_metadata_path(url)
        return skill_path.exists() and metadata_path.exists()

    def is_expired(self, url: str) -> bool:
        """Check if a cached URL has expired.

        Args:
            url: The URL to check

        Returns:
            True if the cache has expired or doesn't exist
        """
        entry = self._load_metadata(url)
        if entry is None:
            return True
        return entry.is_expired()

    def get_cached_content(self, url: str) -> str | None:
        """Get cached content if valid (not expired).

        Args:
            url: The URL to get content for

        Returns:
            Cached content string or None if not cached or expired
        """
        if not self.is_cached(url) or self.is_expired(url):
            return None

        skill_path = self.get_skill_path(url)
        try:
            return skill_path.read_text(encoding="utf-8")
        except OSError as e:
            self._log(f"Error reading cached content for {url}: {e}", "warning")
            return None

    def get_expired_content(self, url: str) -> str | None:
        """Get cached content even if expired (for fallback).

        Args:
            url: The URL to get content for

        Returns:
            Cached content string or None if not cached at all
        """
        if not self.is_cached(url):
            return None

        skill_path = self.get_skill_path(url)
        try:
            return skill_path.read_text(encoding="utf-8")
        except OSError as e:
            self._log(f"Error reading expired content for {url}: {e}", "warning")
            return None

    def cache_content(self, url: str, content: str) -> bool:
        """Cache content for a URL.

        Args:
            url: The URL to cache
            content: The content to cache

        Returns:
            True if caching succeeded
        """
        cache_path = self.get_cache_path(url)
        skill_path = self.get_skill_path(url)

        try:
            # Ensure cache directory exists
            cache_path.mkdir(parents=True, exist_ok=True)

            # Write content
            skill_path.write_text(content, encoding="utf-8")

            # Create and save metadata
            now = datetime.now(timezone.utc)
            expires = now + timedelta(days=self.ttl_days)

            entry = SkillCacheEntry(
                url=url,
                cached_at=now.strftime("%Y-%m-%dT%H:%M:%SZ"),
                expires_at=expires.strftime("%Y-%m-%dT%H:%M:%SZ"),
                content_hash=self._content_hash(content),
                local_path=str(skill_path),
                size_bytes=len(content.encode("utf-8")),
            )

            if not self._save_metadata(entry):
                return False

            self._log(f"Cached skill from {url}", "debug")
            return True

        except OSError as e:
            self._log(f"Error caching content for {url}: {e}", "error")
            return False

    def refresh(self, url: str | None = None) -> dict:
        """Force re-download of cached skills.

        Args:
            url: Specific URL to refresh, or None to refresh all

        Returns:
            Dict with refresh results: {"refreshed": [], "failed": [], "skipped": []}
        """
        results = {"refreshed": [], "failed": [], "skipped": []}

        if url:
            # Refresh specific URL
            urls_to_refresh = [url]
        else:
            # Refresh all cached URLs
            urls_to_refresh = [entry.url for entry in self.list_cached()]

        for target_url in urls_to_refresh:
            try:
                content = self._download_url(target_url)
                if content:
                    if self.cache_content(target_url, content):
                        results["refreshed"].append(target_url)
                        self._log(f"Refreshed: {target_url}")
                    else:
                        results["failed"].append(target_url)
                else:
                    results["failed"].append(target_url)
            except Exception as e:
                self._log(f"Failed to refresh {target_url}: {e}", "warning")
                results["failed"].append(target_url)

        return results

    def _download_url(self, url: str, timeout: int = 30) -> str | None:
        """Download content from a URL.

        Args:
            url: URL to download
            timeout: Request timeout in seconds

        Returns:
            Downloaded content or None on failure
        """
        try:
            with urllib.request.urlopen(url, timeout=timeout) as response:
                return response.read().decode("utf-8")
        except urllib.error.URLError as e:
            self._log(f"Network error downloading {url}: {e}", "warning")
            return None
        except Exception as e:
            self._log(f"Error downloading {url}: {e}", "warning")
            return None

    def clear(self, url: str | None = None) -> dict:
        """Delete cached skills.

        Args:
            url: Specific URL to clear, or None to clear all

        Returns:
            Dict with clear results: {"cleared": [], "failed": []}
        """
        results = {"cleared": [], "failed": []}

        if url:
            # Clear specific URL
            cache_path = self.get_cache_path(url)
            if cache_path.exists():
                try:
                    import shutil

                    shutil.rmtree(cache_path)
                    results["cleared"].append(url)
                    self._log(f"Cleared cache for: {url}")
                except OSError as e:
                    self._log(f"Error clearing cache for {url}: {e}", "error")
                    results["failed"].append(url)
        else:
            # Clear all caches
            for entry in self.list_cached():
                cache_path = self.get_cache_path(entry.url)
                try:
                    import shutil

                    shutil.rmtree(cache_path)
                    results["cleared"].append(entry.url)
                except OSError as e:
                    self._log(f"Error clearing cache for {entry.url}: {e}", "error")
                    results["failed"].append(entry.url)

            if results["cleared"]:
                self._log(f"Cleared {len(results['cleared'])} cached skills")

        return results

    def list_cached(self) -> list[SkillCacheEntry]:
        """List all cached skills.

        Returns:
            List of SkillCacheEntry objects for all cached skills
        """
        entries = []

        if not self.cache_dir.exists():
            return entries

        for cache_subdir in self.cache_dir.iterdir():
            if not cache_subdir.is_dir():
                continue

            metadata_path = cache_subdir / self.METADATA_FILENAME
            if metadata_path.exists():
                try:
                    data = json.loads(metadata_path.read_text(encoding="utf-8"))
                    entry = SkillCacheEntry.from_dict(data)
                    entries.append(entry)
                except (json.JSONDecodeError, OSError):
                    # Skip invalid entries
                    continue

        # Sort by cached_at timestamp (newest first)
        entries.sort(key=lambda e: e.cached_at, reverse=True)
        return entries

    def get_cache_stats(self) -> dict:
        """Get cache statistics.

        Returns:
            Dict with cache statistics:
            - total_cached: Number of cached skills
            - total_size_mb: Total cache size in MB
            - expired_count: Number of expired entries
            - valid_count: Number of valid (non-expired) entries
            - oldest_entry: ISO timestamp of oldest entry
            - newest_entry: ISO timestamp of newest entry
        """
        entries = self.list_cached()

        total_size = sum(e.size_bytes for e in entries)
        expired_count = sum(1 for e in entries if e.is_expired())
        valid_count = len(entries) - expired_count

        oldest_entry = None
        newest_entry = None

        if entries:
            sorted_by_time = sorted(entries, key=lambda e: e.cached_at)
            oldest_entry = sorted_by_time[0].cached_at if sorted_by_time else None
            newest_entry = sorted_by_time[-1].cached_at if sorted_by_time else None

        return {
            "total_cached": len(entries),
            "total_size_mb": round(total_size / (1024 * 1024), 2),
            "expired_count": expired_count,
            "valid_count": valid_count,
            "oldest_entry": oldest_entry,
            "newest_entry": newest_entry,
            "cache_dir": str(self.cache_dir),
            "ttl_days": self.ttl_days,
        }

    def get_or_download(self, url: str) -> str | None:
        """Get skill content with graceful degradation.

        Implements the following fallback pattern:
        1. Return valid cache if available
        2. Try to download fresh content
        3. Fall back to expired cache with warning
        4. Return None if all fails

        Args:
            url: URL to get content for

        Returns:
            Skill content or None if unavailable
        """
        # 1. Check valid cache
        if self.is_cached(url) and not self.is_expired(url):
            self._log(f"Using cached skill: {url}", "debug")
            return self.get_cached_content(url)

        # 2. Try to download fresh
        self._log(f"Downloading skill: {url}", "debug")
        content = self._download_url(url)

        if content:
            self.cache_content(url, content)
            return content

        # 3. Fall back to expired cache
        expired = self.get_expired_content(url)
        if expired:
            self._log(f"Using expired cache for {url} (network error)", "warning")
            return expired

        # 4. Complete failure
        self._log(f"Cannot load skill from {url}: no cache available", "error")
        return None
