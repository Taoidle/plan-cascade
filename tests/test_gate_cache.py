"""Tests for GateCache module."""

import json
import time
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from plan_cascade.core.error_parser import ErrorInfo
from plan_cascade.core.gate_cache import CacheEntry, GateCache
from plan_cascade.core.quality_gate import (
    GateConfig,
    GateOutput,
    GateType,
    QualityGate,
)


class TestCacheEntry:
    """Tests for CacheEntry class."""

    def test_init(self):
        """Test CacheEntry initialization."""
        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="success",
            stderr="",
            duration_seconds=0.5,
            command="pytest",
        )

        entry = CacheEntry(
            cache_key="abc123",
            gate_name="test",
            output=output,
            created_at="2024-01-01T00:00:00Z",
            expires_at=None,
        )

        assert entry.cache_key == "abc123"
        assert entry.gate_name == "test"
        assert entry.output.passed is True
        assert entry.expires_at is None

    def test_is_expired_no_expiry(self):
        """Test is_expired returns False when no expiry set."""
        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="pytest",
        )

        entry = CacheEntry(
            cache_key="abc123",
            gate_name="test",
            output=output,
            created_at="2024-01-01T00:00:00Z",
            expires_at=None,
        )

        assert entry.is_expired() is False

    def test_is_expired_future_expiry(self):
        """Test is_expired returns False when expiry is in the future."""
        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="pytest",
        )

        # Set expiry to 1 hour from now
        future_time = time.strftime(
            "%Y-%m-%dT%H:%M:%SZ", time.gmtime(time.time() + 3600)
        )

        entry = CacheEntry(
            cache_key="abc123",
            gate_name="test",
            output=output,
            created_at="2024-01-01T00:00:00Z",
            expires_at=future_time,
        )

        assert entry.is_expired() is False

    def test_is_expired_past_expiry(self):
        """Test is_expired returns True when expiry is in the past."""
        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="pytest",
        )

        # Set expiry to 1 hour ago
        past_time = time.strftime(
            "%Y-%m-%dT%H:%M:%SZ", time.gmtime(time.time() - 3600)
        )

        entry = CacheEntry(
            cache_key="abc123",
            gate_name="test",
            output=output,
            created_at="2024-01-01T00:00:00Z",
            expires_at=past_time,
        )

        assert entry.is_expired() is True

    def test_to_dict(self):
        """Test CacheEntry to_dict serialization."""
        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="output",
            stderr="",
            duration_seconds=1.5,
            command="pytest -v",
            error_summary=None,
            skipped=False,
            checked_files=["file1.py", "file2.py"],
        )

        entry = CacheEntry(
            cache_key="abc123",
            gate_name="test",
            output=output,
            created_at="2024-01-01T00:00:00Z",
            expires_at="2024-01-02T00:00:00Z",
        )

        result = entry.to_dict()

        assert result["cache_key"] == "abc123"
        assert result["gate_name"] == "test"
        assert result["created_at"] == "2024-01-01T00:00:00Z"
        assert result["expires_at"] == "2024-01-02T00:00:00Z"
        assert result["output"]["passed"] is True
        assert result["output"]["gate_type"] == "test"
        assert result["output"]["checked_files"] == ["file1.py", "file2.py"]

    def test_to_dict_with_structured_errors(self):
        """Test CacheEntry to_dict with structured errors."""
        errors = [
            ErrorInfo(
                file="test.py",
                line=10,
                column=5,
                code="E001",
                message="Test error",
                severity="error",
            )
        ]

        output = GateOutput(
            gate_name="lint",
            gate_type=GateType.LINT,
            passed=False,
            exit_code=1,
            stdout="",
            stderr="error",
            duration_seconds=0.5,
            command="ruff check .",
            structured_errors=errors,
        )

        entry = CacheEntry(
            cache_key="abc123",
            gate_name="lint",
            output=output,
            created_at="2024-01-01T00:00:00Z",
        )

        result = entry.to_dict()

        assert len(result["output"]["structured_errors"]) == 1
        assert result["output"]["structured_errors"][0]["file"] == "test.py"
        assert result["output"]["structured_errors"][0]["line"] == 10
        assert result["output"]["structured_errors"][0]["code"] == "E001"

    def test_from_dict(self):
        """Test CacheEntry from_dict deserialization."""
        data = {
            "cache_key": "abc123",
            "gate_name": "test",
            "created_at": "2024-01-01T00:00:00Z",
            "expires_at": None,
            "output": {
                "gate_name": "test",
                "gate_type": "test",
                "passed": True,
                "exit_code": 0,
                "stdout": "success",
                "stderr": "",
                "duration_seconds": 1.0,
                "command": "pytest",
                "error_summary": None,
                "structured_errors": [],
                "skipped": False,
                "checked_files": None,
            },
        }

        entry = CacheEntry.from_dict(data)

        assert entry.cache_key == "abc123"
        assert entry.gate_name == "test"
        assert entry.output.passed is True
        assert entry.output.gate_type == GateType.TEST

    def test_from_dict_with_structured_errors(self):
        """Test CacheEntry from_dict with structured errors."""
        data = {
            "cache_key": "abc123",
            "gate_name": "lint",
            "created_at": "2024-01-01T00:00:00Z",
            "output": {
                "gate_name": "lint",
                "gate_type": "lint",
                "passed": False,
                "exit_code": 1,
                "stdout": "",
                "stderr": "error",
                "duration_seconds": 0.5,
                "command": "ruff check .",
                "structured_errors": [
                    {
                        "file": "test.py",
                        "line": 10,
                        "column": 5,
                        "code": "E001",
                        "message": "Test error",
                        "severity": "error",
                    }
                ],
            },
        }

        entry = CacheEntry.from_dict(data)

        assert len(entry.output.structured_errors) == 1
        assert entry.output.structured_errors[0].file == "test.py"
        assert entry.output.structured_errors[0].line == 10


class TestGateCache:
    """Tests for GateCache class."""

    def test_init_default_cache_dir(self, tmp_path: Path):
        """Test GateCache initialization with default cache directory."""
        cache = GateCache(tmp_path)

        assert cache.project_root == tmp_path
        assert cache._cache_dir == tmp_path / ".state"

    def test_init_custom_cache_dir(self, tmp_path: Path):
        """Test GateCache initialization with custom cache directory."""
        custom_dir = tmp_path / "custom_cache"
        cache = GateCache(tmp_path, cache_dir=custom_dir)

        assert cache._cache_dir == custom_dir

    def test_compute_cache_key(self, tmp_path: Path):
        """Test cache key computation."""
        # Initialize git repo
        import subprocess

        subprocess.run(["git", "init"], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "config", "user.email", "test@test.com"],
            cwd=tmp_path,
            capture_output=True,
        )
        subprocess.run(
            ["git", "config", "user.name", "Test User"],
            cwd=tmp_path,
            capture_output=True,
        )

        # Create a file and commit
        (tmp_path / "test.py").write_text("print('hello')")
        subprocess.run(["git", "add", "test.py"], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-m", "initial"],
            cwd=tmp_path,
            capture_output=True,
        )

        cache = GateCache(tmp_path)
        key = cache._compute_cache_key()

        # Key should be a hash string
        assert isinstance(key, str)
        assert len(key) == 32  # MD5 hash length

    def test_cache_key_changes_with_file_modification(self, tmp_path: Path):
        """Test that cache key changes when files are modified."""
        import subprocess

        # Initialize git repo
        subprocess.run(["git", "init"], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "config", "user.email", "test@test.com"],
            cwd=tmp_path,
            capture_output=True,
        )
        subprocess.run(
            ["git", "config", "user.name", "Test User"],
            cwd=tmp_path,
            capture_output=True,
        )

        # Create a file and commit
        (tmp_path / "test.py").write_text("print('hello')")
        subprocess.run(["git", "add", "test.py"], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-m", "initial"],
            cwd=tmp_path,
            capture_output=True,
        )

        cache = GateCache(tmp_path)
        key1 = cache._compute_cache_key()

        # Modify file (creates unstaged change)
        (tmp_path / "test.py").write_text("print('modified')")
        cache.invalidate_cache_key()
        key2 = cache._compute_cache_key()

        assert key1 != key2

    def test_set_and_get(self, tmp_path: Path):
        """Test setting and getting cached results."""
        cache = GateCache(tmp_path)

        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="success",
            stderr="",
            duration_seconds=0.5,
            command="pytest",
        )

        # Set cache
        cache.set("test", output)

        # Get cache
        cached = cache.get("test")

        assert cached is not None
        assert cached.passed is True
        assert cached.gate_name == "test"
        assert cached.from_cache is True

    def test_get_nonexistent(self, tmp_path: Path):
        """Test getting a nonexistent cache entry."""
        cache = GateCache(tmp_path)

        result = cache.get("nonexistent")

        assert result is None

    def test_get_expired_entry(self, tmp_path: Path):
        """Test that expired entries return None."""
        cache = GateCache(tmp_path)

        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="pytest",
        )

        # Set with very short TTL (already expired)
        cache.set("test", output, ttl=-1)

        result = cache.get("test")

        assert result is None

    def test_invalidate_specific_gate(self, tmp_path: Path):
        """Test invalidating a specific gate's cache."""
        cache = GateCache(tmp_path)

        output1 = GateOutput(
            gate_name="test1",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="pytest",
        )

        output2 = GateOutput(
            gate_name="test2",
            gate_type=GateType.LINT,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="ruff",
        )

        cache.set("test1", output1)
        cache.set("test2", output2)

        # Invalidate only test1
        cache.invalidate("test1")

        assert cache.get("test1") is None
        assert cache.get("test2") is not None

    def test_invalidate_all(self, tmp_path: Path):
        """Test invalidating all cache entries."""
        cache = GateCache(tmp_path)

        output1 = GateOutput(
            gate_name="test1",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="pytest",
        )

        output2 = GateOutput(
            gate_name="test2",
            gate_type=GateType.LINT,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="ruff",
        )

        cache.set("test1", output1)
        cache.set("test2", output2)

        # Invalidate all
        cache.invalidate()

        assert cache.get("test1") is None
        assert cache.get("test2") is None

    def test_is_valid(self, tmp_path: Path):
        """Test is_valid method."""
        cache = GateCache(tmp_path)

        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="pytest",
        )

        cache.set("test", output)

        assert cache.is_valid("test") is True
        assert cache.is_valid("nonexistent") is False

    def test_is_valid_stale_cache(self, tmp_path: Path):
        """Test is_valid returns False when cache key changed."""
        cache = GateCache(tmp_path)

        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="pytest",
        )

        cache.set("test", output)

        # Simulate cache key change by modifying internal state
        cache._current_cache_key = "different_key"

        assert cache.is_valid("test") is False

    def test_cache_persistence(self, tmp_path: Path):
        """Test that cache persists to disk and can be reloaded."""
        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="success",
            stderr="",
            duration_seconds=0.5,
            command="pytest",
        )

        # Create cache and set value
        cache1 = GateCache(tmp_path)
        cache1.set("test", output)

        # Create new cache instance (should load from disk)
        cache2 = GateCache(tmp_path)

        # The entry should exist (though may not be valid due to cache key)
        assert "test" in cache2._entries

    def test_clear(self, tmp_path: Path):
        """Test clearing all cache and removing file."""
        cache = GateCache(tmp_path)

        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="pytest",
        )

        cache.set("test", output)
        cache_file = cache._cache_file

        # Verify cache file exists
        assert cache_file.exists()

        # Clear cache
        cache.clear()

        assert len(cache._entries) == 0
        assert not cache_file.exists()

    def test_get_cache_stats(self, tmp_path: Path):
        """Test get_cache_stats method."""
        cache = GateCache(tmp_path)

        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="pytest",
        )

        cache.set("test", output)

        stats = cache.get_cache_stats()

        assert "total_entries" in stats
        assert "valid_entries" in stats
        assert "expired_entries" in stats
        assert "stale_entries" in stats
        assert "current_cache_key" in stats
        assert "cache_file" in stats


class TestQualityGateCacheIntegration:
    """Tests for QualityGate with cache integration."""

    def test_use_cache_default_false(self, tmp_path: Path):
        """Test that use_cache defaults to False."""
        qg = QualityGate(tmp_path)
        assert qg.use_cache is False

    def test_use_cache_can_be_enabled(self, tmp_path: Path):
        """Test that use_cache can be set to True."""
        qg = QualityGate(tmp_path, use_cache=True)
        assert qg.use_cache is True

    def test_cache_property_lazy_initialization(self, tmp_path: Path):
        """Test that cache is lazily initialized."""
        qg = QualityGate(tmp_path, use_cache=True)

        # Cache should not be initialized yet
        assert qg._cache is None

        # Access cache property
        cache = qg.cache

        # Now it should be initialized
        assert qg._cache is not None
        assert isinstance(cache, GateCache)

    def test_invalidate_cache_method(self, tmp_path: Path):
        """Test invalidate_cache method."""
        qg = QualityGate(tmp_path, use_cache=True)

        # Force cache initialization by accessing property
        _ = qg.cache

        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="pytest",
        )

        qg.cache.set("test", output)

        # Verify it's cached
        assert qg.cache.is_valid("test")

        # Invalidate
        qg.invalidate_cache("test")

        # Verify it's invalidated
        assert not qg.cache.is_valid("test")

    def test_execute_all_with_cache_stores_results(self, tmp_path: Path):
        """Test that execute_all stores results in cache when use_cache=True."""
        gates = [
            GateConfig(
                name="test",
                type=GateType.TEST,
                command="echo",
                args=["pass"],
            )
        ]
        qg = QualityGate(tmp_path, gates=gates, use_cache=True)

        result = qg.execute_all("story-001")

        assert "test" in result
        assert result["test"].passed is True

        # Result should be cached
        cached = qg.cache.get("test")
        assert cached is not None
        assert cached.from_cache is True

    def test_execute_all_with_cache_returns_cached_results(self, tmp_path: Path):
        """Test that execute_all returns cached results when cache is valid."""
        gates = [
            GateConfig(
                name="test",
                type=GateType.TEST,
                command="echo",
                args=["pass"],
            )
        ]
        qg = QualityGate(tmp_path, gates=gates, use_cache=True)

        # First execution
        result1 = qg.execute_all("story-001")
        assert result1["test"].from_cache is False

        # Second execution should return cached result
        result2 = qg.execute_all("story-001")
        assert result2["test"].from_cache is True

    def test_execute_all_without_cache_does_not_store(self, tmp_path: Path):
        """Test that execute_all does not store results when use_cache=False."""
        gates = [
            GateConfig(
                name="test",
                type=GateType.TEST,
                command="echo",
                args=["pass"],
            )
        ]
        qg = QualityGate(tmp_path, gates=gates, use_cache=False)

        result = qg.execute_all("story-001")

        assert "test" in result

        # Cache should not be initialized
        assert qg._cache is None

    def test_cache_ttl(self, tmp_path: Path):
        """Test cache with TTL."""
        gates = [
            GateConfig(
                name="test",
                type=GateType.TEST,
                command="echo",
                args=["pass"],
            )
        ]
        qg = QualityGate(tmp_path, gates=gates, use_cache=True, cache_ttl=3600)

        qg.execute_all("story-001")

        # Result should be cached
        cached = qg.cache.get("test")
        assert cached is not None

    def test_from_prd_reads_cache_settings(self, tmp_path: Path):
        """Test that from_prd reads cache settings."""
        prd = {
            "quality_gates": {
                "enabled": True,
                "use_cache": True,
                "cache_ttl": 7200,
                "gates": [{"name": "test", "type": "test", "required": True}],
            }
        }

        qg = QualityGate.from_prd(tmp_path, prd)

        assert qg.use_cache is True
        assert qg.cache_ttl == 7200

    def test_to_dict_includes_cache_settings(self, tmp_path: Path):
        """Test that to_dict includes cache settings."""
        qg = QualityGate(tmp_path, gates=[], use_cache=True, cache_ttl=3600)

        result = qg.to_dict()

        assert result["use_cache"] is True
        assert result["cache_ttl"] == 3600

    def test_to_dict_omits_cache_settings_when_disabled(self, tmp_path: Path):
        """Test that to_dict omits cache settings when disabled."""
        qg = QualityGate(tmp_path, gates=[], use_cache=False)

        result = qg.to_dict()

        assert "use_cache" not in result
        assert "cache_ttl" not in result

    def test_create_default_with_cache(self, tmp_path: Path):
        """Test create_default with cache settings."""
        (tmp_path / "pyproject.toml").write_text("[project]")

        qg = QualityGate.create_default(tmp_path, use_cache=True, cache_ttl=1800)

        assert qg.use_cache is True
        assert qg.cache_ttl == 1800


class TestQualityGateCacheAsyncIntegration:
    """Tests for QualityGate async execution with cache."""

    @pytest.mark.asyncio
    async def test_execute_all_async_with_cache_stores_results(self, tmp_path: Path):
        """Test that execute_all_async stores results in cache."""
        gates = [
            GateConfig(
                name="test",
                type=GateType.TEST,
                command="echo",
                args=["pass"],
            )
        ]
        qg = QualityGate(tmp_path, gates=gates, use_cache=True)

        result = await qg.execute_all_async("story-001")

        assert "test" in result
        assert result["test"].passed is True

        # Result should be cached
        cached = qg.cache.get("test")
        assert cached is not None

    @pytest.mark.asyncio
    async def test_execute_all_async_returns_cached_results(self, tmp_path: Path):
        """Test that execute_all_async returns cached results."""
        gates = [
            GateConfig(
                name="test",
                type=GateType.TEST,
                command="echo",
                args=["pass"],
            )
        ]
        qg = QualityGate(tmp_path, gates=gates, use_cache=True)

        # First execution
        result1 = await qg.execute_all_async("story-001")
        assert result1["test"].from_cache is False

        # Second execution should return cached result
        result2 = await qg.execute_all_async("story-001")
        assert result2["test"].from_cache is True

    @pytest.mark.asyncio
    async def test_execute_all_async_partial_cache_hit(self, tmp_path: Path):
        """Test execute_all_async with partial cache hit."""
        gates = [
            GateConfig(
                name="test1",
                type=GateType.TEST,
                command="echo",
                args=["pass1"],
            ),
            GateConfig(
                name="test2",
                type=GateType.LINT,
                command="echo",
                args=["pass2"],
            ),
        ]
        qg = QualityGate(tmp_path, gates=gates, use_cache=True)

        # First execution
        result1 = await qg.execute_all_async("story-001")
        assert result1["test1"].from_cache is False
        assert result1["test2"].from_cache is False

        # Invalidate one gate
        qg.cache.invalidate("test1")

        # Second execution - test1 should be re-executed, test2 from cache
        result2 = await qg.execute_all_async("story-001")
        assert result2["test1"].from_cache is False
        assert result2["test2"].from_cache is True


class TestGateOutputFromCache:
    """Tests for GateOutput.from_cache field."""

    def test_gate_output_from_cache_default_false(self):
        """Test that from_cache defaults to False."""
        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="pytest",
        )
        assert output.from_cache is False

    def test_gate_output_from_cache_can_be_set(self):
        """Test that from_cache can be set to True."""
        output = GateOutput(
            gate_name="test",
            gate_type=GateType.TEST,
            passed=True,
            exit_code=0,
            stdout="",
            stderr="",
            duration_seconds=0.1,
            command="pytest",
            from_cache=True,
        )
        assert output.from_cache is True


class TestCacheWithFailFast:
    """Tests for cache interaction with fail_fast."""

    def test_cached_failure_triggers_fail_fast(self, tmp_path: Path):
        """Test that cached failure triggers fail_fast."""
        import sys

        if sys.platform == "win32":
            fail_cmd = "cmd"
            fail_args = ["/c", "exit", "1"]
        else:
            fail_cmd = "false"
            fail_args = []

        gates = [
            GateConfig(
                name="failing",
                type=GateType.TYPECHECK,
                command=fail_cmd,
                args=fail_args,
                required=True,
            ),
            GateConfig(
                name="passing",
                type=GateType.TEST,
                command="echo",
                args=["pass"],
                required=True,
            ),
        ]
        qg = QualityGate(tmp_path, gates=gates, use_cache=True, fail_fast=True)

        # First execution - failing gate should cache failure
        result1 = qg.execute_all("story-001")
        assert result1["failing"].passed is False
        assert result1["passing"].skipped is True

        # Second execution - should use cached failure and skip passing gate
        result2 = qg.execute_all("story-002")
        assert result2["failing"].from_cache is True
        assert result2["failing"].passed is False
        assert result2["passing"].skipped is True
