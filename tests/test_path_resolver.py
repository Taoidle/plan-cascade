"""Tests for PathResolver module."""

import json
import sys
from pathlib import Path
from unittest.mock import patch

import pytest

from plan_cascade.state.path_resolver import (
    LINK_FILE_NAME,
    PathResolver,
    detect_project_mode,
)


class TestPathResolverBasic:
    """Basic tests for PathResolver class."""

    def test_init(self, tmp_path: Path):
        """Test PathResolver initialization."""
        resolver = PathResolver(tmp_path)
        assert resolver.project_root == tmp_path.resolve()
        assert not resolver.is_legacy_mode()

    def test_init_legacy_mode(self, tmp_path: Path):
        """Test PathResolver initialization in legacy mode."""
        resolver = PathResolver(tmp_path, legacy_mode=True)
        assert resolver.is_legacy_mode()

    def test_init_with_data_dir_override(self, tmp_path: Path):
        """Test PathResolver with custom data directory."""
        custom_data_dir = tmp_path / "custom-data"
        resolver = PathResolver(tmp_path, data_dir_override=custom_data_dir)
        assert resolver.get_data_dir() == custom_data_dir


class TestProjectId:
    """Tests for project ID computation."""

    def test_project_id_format(self, tmp_path: Path):
        """Test project ID has correct format: <name>-<hash>."""
        resolver = PathResolver(tmp_path)
        project_id = resolver.get_project_id()

        # Should match pattern: name-8charhash
        parts = project_id.rsplit("-", 1)
        assert len(parts) == 2
        assert len(parts[1]) == 8  # 8 character hash
        assert all(c in "0123456789abcdef" for c in parts[1])  # Hex characters

    def test_project_id_filesystem_safe(self, tmp_path: Path):
        """Test project ID is filesystem safe."""
        # Create a path with special characters
        project_path = tmp_path / "My Project! @#$%"
        project_path.mkdir()

        resolver = PathResolver(project_path)
        project_id = resolver.get_project_id()

        # Should only contain alphanumeric, hyphen, underscore
        import re
        assert re.match(r"^[a-z0-9\-_]+-[a-f0-9]{8}$", project_id)

    def test_project_id_unique_for_different_paths(self, tmp_path: Path):
        """Test that different paths produce different project IDs."""
        project1 = tmp_path / "project1"
        project2 = tmp_path / "project2"
        project1.mkdir()
        project2.mkdir()

        resolver1 = PathResolver(project1)
        resolver2 = PathResolver(project2)

        assert resolver1.get_project_id() != resolver2.get_project_id()

    def test_project_id_same_for_same_path(self, tmp_path: Path):
        """Test that same path always produces same project ID."""
        resolver1 = PathResolver(tmp_path)
        resolver2 = PathResolver(tmp_path)

        assert resolver1.get_project_id() == resolver2.get_project_id()

    def test_project_id_with_custom_path(self, tmp_path: Path):
        """Test computing project ID for a custom path."""
        resolver = PathResolver(tmp_path)

        custom_path = tmp_path / "custom-project"
        custom_path.mkdir()

        custom_id = resolver.get_project_id(custom_path)
        default_id = resolver.get_project_id()

        assert custom_id != default_id
        assert "custom-project" in custom_id

    def test_project_id_caching(self, tmp_path: Path):
        """Test that project ID is cached for the instance's project root."""
        resolver = PathResolver(tmp_path)

        # First call computes
        id1 = resolver.get_project_id()
        # Second call should use cache
        id2 = resolver.get_project_id()

        assert id1 == id2


class TestSanitizeName:
    """Tests for name sanitization."""

    def test_sanitize_spaces(self, tmp_path: Path):
        """Test that spaces are replaced with hyphens."""
        resolver = PathResolver(tmp_path)
        result = resolver._sanitize_name("my project name")
        assert " " not in result
        assert "my-project-name" == result

    def test_sanitize_special_chars(self, tmp_path: Path):
        """Test that special characters are removed."""
        resolver = PathResolver(tmp_path)
        result = resolver._sanitize_name("project!@#$%^&*()")
        assert result == "project"

    def test_sanitize_uppercase(self, tmp_path: Path):
        """Test that uppercase is converted to lowercase."""
        resolver = PathResolver(tmp_path)
        result = resolver._sanitize_name("MyProject")
        assert result == "myproject"

    def test_sanitize_consecutive_hyphens(self, tmp_path: Path):
        """Test that consecutive hyphens are collapsed."""
        resolver = PathResolver(tmp_path)
        result = resolver._sanitize_name("my---project")
        assert result == "my-project"

    def test_sanitize_empty_result(self, tmp_path: Path):
        """Test that empty result defaults to 'project'."""
        resolver = PathResolver(tmp_path)
        result = resolver._sanitize_name("!@#$%^")
        assert result == "project"

    def test_sanitize_long_name(self, tmp_path: Path):
        """Test that long names are truncated to 50 chars."""
        resolver = PathResolver(tmp_path)
        long_name = "a" * 100
        result = resolver._sanitize_name(long_name)
        assert len(result) == 50


class TestDataDir:
    """Tests for data directory resolution."""

    def test_data_dir_with_override(self, tmp_path: Path):
        """Test data directory with override."""
        custom_dir = tmp_path / "custom"
        resolver = PathResolver(tmp_path, data_dir_override=custom_dir)
        assert resolver.get_data_dir() == custom_dir

    @patch("sys.platform", "win32")
    @patch.dict("os.environ", {"APPDATA": "C:\\Users\\Test\\AppData\\Roaming"})
    def test_data_dir_windows(self, tmp_path: Path):
        """Test data directory on Windows."""
        resolver = PathResolver(tmp_path)
        data_dir = resolver.get_data_dir()
        assert "plan-cascade" in str(data_dir)

    @patch("sys.platform", "linux")
    def test_data_dir_unix(self, tmp_path: Path):
        """Test data directory on Unix."""
        resolver = PathResolver(tmp_path)
        data_dir = resolver.get_data_dir()
        assert ".plan-cascade" in str(data_dir)


class TestProjectDir:
    """Tests for project directory paths."""

    def test_project_dir_normal_mode(self, tmp_path: Path):
        """Test project directory in normal mode."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        project_dir = resolver.get_project_dir()

        assert project_dir.parent == data_dir
        assert resolver.get_project_id() in str(project_dir)

    def test_project_dir_legacy_mode(self, tmp_path: Path):
        """Test project directory in legacy mode."""
        resolver = PathResolver(tmp_path, legacy_mode=True)

        project_dir = resolver.get_project_dir()

        assert project_dir == tmp_path.resolve()


class TestFilePaths:
    """Tests for specific file path methods."""

    def test_prd_path(self, tmp_path: Path):
        """Test PRD path resolution."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        prd_path = resolver.get_prd_path()

        assert prd_path.name == "prd.json"
        assert prd_path.parent == resolver.get_project_dir()

    def test_mega_plan_path(self, tmp_path: Path):
        """Test mega-plan path resolution."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        mega_plan_path = resolver.get_mega_plan_path()

        assert mega_plan_path.name == "mega-plan.json"
        assert mega_plan_path.parent == resolver.get_project_dir()

    def test_worktree_dir(self, tmp_path: Path):
        """Test worktree directory resolution."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        worktree_dir = resolver.get_worktree_dir()

        assert worktree_dir.name == ".worktree"
        assert worktree_dir.parent == resolver.get_project_dir()

    def test_locks_dir(self, tmp_path: Path):
        """Test locks directory resolution."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        locks_dir = resolver.get_locks_dir()

        assert locks_dir.name == ".locks"
        assert locks_dir.parent == resolver.get_project_dir()

    def test_state_dir(self, tmp_path: Path):
        """Test state directory resolution."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        state_dir = resolver.get_state_dir()

        assert state_dir.name == ".state"
        assert state_dir.parent == resolver.get_project_dir()

    def test_state_file_path(self, tmp_path: Path):
        """Test state file path resolution."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        state_file = resolver.get_state_file_path("iteration-state.json")

        assert state_file.name == "iteration-state.json"
        assert state_file.parent == resolver.get_state_dir()

    def test_manifest_path(self, tmp_path: Path):
        """Test manifest path resolution."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        manifest_path = resolver.get_manifest_path()

        assert manifest_path.name == "manifest.json"
        assert manifest_path.parent == resolver.get_project_dir()


class TestLegacyMode:
    """Tests for legacy mode behavior."""

    def test_legacy_mode_prd_path(self, tmp_path: Path):
        """Test PRD path in legacy mode uses project root."""
        resolver = PathResolver(tmp_path, legacy_mode=True)

        prd_path = resolver.get_prd_path()

        assert prd_path == tmp_path.resolve() / "prd.json"

    def test_legacy_mode_worktree_dir(self, tmp_path: Path):
        """Test worktree dir in legacy mode uses project root."""
        resolver = PathResolver(tmp_path, legacy_mode=True)

        worktree_dir = resolver.get_worktree_dir()

        assert worktree_dir == tmp_path.resolve() / ".worktree"

    def test_legacy_mode_locks_dir(self, tmp_path: Path):
        """Test locks dir in legacy mode uses project root."""
        resolver = PathResolver(tmp_path, legacy_mode=True)

        locks_dir = resolver.get_locks_dir()

        assert locks_dir == tmp_path.resolve() / ".locks"


class TestDirectoryManagement:
    """Tests for directory creation and management."""

    def test_ensure_directories(self, tmp_path: Path):
        """Test ensure_directories creates all required directories."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        resolver.ensure_directories()

        assert resolver.get_project_dir().exists()
        assert resolver.get_worktree_dir().exists()
        assert resolver.get_locks_dir().exists()
        assert resolver.get_state_dir().exists()

    def test_ensure_directories_idempotent(self, tmp_path: Path):
        """Test ensure_directories can be called multiple times."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        resolver.ensure_directories()
        resolver.ensure_directories()  # Should not raise

        assert resolver.get_project_dir().exists()


class TestManifest:
    """Tests for manifest file operations."""

    def test_write_manifest(self, tmp_path: Path):
        """Test writing manifest file."""
        data_dir = tmp_path / "data"
        project_dir = tmp_path / "my-project"
        project_dir.mkdir()

        resolver = PathResolver(project_dir, data_dir_override=data_dir)
        resolver.write_manifest()

        manifest_path = resolver.get_manifest_path()
        assert manifest_path.exists()

        with open(manifest_path) as f:
            manifest = json.load(f)

        assert manifest["project_root"] == str(project_dir.resolve())
        assert manifest["project_id"] == resolver.get_project_id()
        assert "created_at" in manifest
        assert "platform" in manifest

    def test_write_manifest_legacy_mode(self, tmp_path: Path):
        """Test that write_manifest does nothing in legacy mode."""
        resolver = PathResolver(tmp_path, legacy_mode=True)
        resolver.write_manifest()

        # No manifest should be written in legacy mode
        manifest_path = resolver.get_manifest_path()
        assert not manifest_path.exists()

    def test_read_manifest(self, tmp_path: Path):
        """Test reading manifest file."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)
        resolver.write_manifest()

        manifest = resolver.read_manifest()

        assert manifest is not None
        assert manifest["project_root"] == str(tmp_path.resolve())

    def test_read_manifest_not_found(self, tmp_path: Path):
        """Test reading manifest when file doesn't exist."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        manifest = resolver.read_manifest()

        assert manifest is None


class TestProjectLookup:
    """Tests for project lookup functionality."""

    def test_find_project_by_id(self, tmp_path: Path):
        """Test finding project root by ID."""
        data_dir = tmp_path / "data"
        project_dir = tmp_path / "my-project"
        project_dir.mkdir()

        resolver = PathResolver(project_dir, data_dir_override=data_dir)
        resolver.write_manifest()

        project_id = resolver.get_project_id()
        found_root = PathResolver.find_project_by_id(project_id, data_dir=data_dir)

        assert found_root == project_dir.resolve()

    def test_find_project_by_id_not_found(self, tmp_path: Path):
        """Test finding project by ID when not found."""
        data_dir = tmp_path / "data"

        found_root = PathResolver.find_project_by_id(
            "nonexistent-12345678", data_dir=data_dir
        )

        assert found_root is None

    def test_list_projects_empty(self, tmp_path: Path):
        """Test listing projects when none exist."""
        data_dir = tmp_path / "data"

        projects = PathResolver.list_projects(data_dir=data_dir)

        assert projects == []

    def test_list_projects(self, tmp_path: Path):
        """Test listing all projects."""
        data_dir = tmp_path / "data"

        # Create two projects
        project1 = tmp_path / "project1"
        project2 = tmp_path / "project2"
        project1.mkdir()
        project2.mkdir()

        resolver1 = PathResolver(project1, data_dir_override=data_dir)
        resolver2 = PathResolver(project2, data_dir_override=data_dir)
        resolver1.write_manifest()
        resolver2.write_manifest()

        projects = PathResolver.list_projects(data_dir=data_dir)

        assert len(projects) == 2
        project_ids = [p["project_id"] for p in projects]
        assert resolver1.get_project_id() in project_ids
        assert resolver2.get_project_id() in project_ids


class TestCleanup:
    """Tests for cleanup functionality."""

    def test_cleanup_project_data(self, tmp_path: Path):
        """Test cleaning up project data."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        resolver.ensure_directories()
        resolver.write_manifest()

        # Verify directories exist
        assert resolver.get_project_dir().exists()

        resolver.cleanup_project_data()

        # Verify project directory is removed
        assert not resolver.get_project_dir().exists()

    def test_cleanup_project_data_legacy_mode(self, tmp_path: Path):
        """Test that cleanup does nothing in legacy mode."""
        resolver = PathResolver(tmp_path, legacy_mode=True)

        resolver.cleanup_project_data()

        # Project root should still exist
        assert tmp_path.exists()


class TestPlatformSpecific:
    """Tests for platform-specific behavior."""

    @patch("sys.platform", "win32")
    @patch.dict("os.environ", {"APPDATA": ""})
    def test_windows_fallback_no_appdata(self, tmp_path: Path):
        """Test Windows fallback when APPDATA is not set."""
        resolver = PathResolver(tmp_path)
        data_dir = resolver.get_data_dir()
        # Should fall back to user home
        assert "plan-cascade" in str(data_dir)

    @patch("sys.platform", "darwin")
    def test_macos_uses_home(self, tmp_path: Path):
        """Test macOS uses home directory."""
        resolver = PathResolver(tmp_path)
        data_dir = resolver.get_data_dir()
        assert ".plan-cascade" in str(data_dir)

    def test_path_normalization_for_hash(self, tmp_path: Path):
        """Test that path separators are normalized for consistent hashing."""
        # Create paths that would differ on Windows vs Unix
        resolver1 = PathResolver(tmp_path)
        project_id1 = resolver1.get_project_id()

        # The hash should be consistent regardless of platform
        assert len(project_id1.split("-")[-1]) == 8


class TestCorruptedManifest:
    """Tests for handling corrupted manifest files."""

    def test_read_corrupted_manifest(self, tmp_path: Path):
        """Test reading a corrupted manifest file."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)
        resolver.ensure_directories()

        # Write corrupted manifest
        manifest_path = resolver.get_manifest_path()
        manifest_path.write_text("{ invalid json content")

        # Should return None, not crash
        manifest = resolver.read_manifest()
        assert manifest is None

    def test_list_projects_with_corrupted_manifests(self, tmp_path: Path):
        """Test listing projects when some manifests are corrupted."""
        data_dir = tmp_path / "data"

        # Create valid project
        project1 = tmp_path / "project1"
        project1.mkdir()
        resolver1 = PathResolver(project1, data_dir_override=data_dir)
        resolver1.write_manifest()

        # Create project with corrupted manifest
        project2 = tmp_path / "project2"
        project2.mkdir()
        resolver2 = PathResolver(project2, data_dir_override=data_dir)
        resolver2.ensure_directories()
        corrupted_manifest = resolver2.get_manifest_path()
        corrupted_manifest.write_text("not valid json {")

        # List projects - should include both
        projects = PathResolver.list_projects(data_dir=data_dir)

        assert len(projects) == 2
        # Valid project should have project_root
        valid_projects = [p for p in projects if p["project_root"] is not None]
        assert len(valid_projects) == 1
        # Corrupted project should have None for project_root
        corrupted_projects = [p for p in projects if p["project_root"] is None]
        assert len(corrupted_projects) == 1


class TestEdgeCasePaths:
    """Tests for edge case path scenarios."""

    def test_very_long_project_name(self, tmp_path: Path):
        """Test handling of very long project names."""
        long_name = "a" * 200
        project = tmp_path / long_name
        project.mkdir()

        resolver = PathResolver(project)
        project_id = resolver.get_project_id()

        # Name part should be truncated to 50 chars
        name_part = project_id.rsplit("-", 1)[0]
        assert len(name_part) <= 50

    def test_unicode_project_name(self, tmp_path: Path):
        """Test handling of unicode project names."""
        # Chinese characters
        project = tmp_path / "\u4e2d\u6587\u9879\u76ee"
        project.mkdir()

        resolver = PathResolver(project)
        project_id = resolver.get_project_id()

        # Should produce valid filesystem-safe ID
        assert project_id
        # ID should only contain safe characters
        import re
        assert re.match(r"^[a-z0-9\-_]+-[a-f0-9]{8}$", project_id)

    def test_project_with_only_special_chars(self, tmp_path: Path):
        """Test project name with only special characters in sanitization."""
        # Instead of creating a directory with special chars (which Windows doesn't allow),
        # we test the sanitization logic directly
        resolver = PathResolver(tmp_path)

        # Test sanitize with a name that would become empty
        result = resolver._sanitize_name("!@#$%^&*()")
        # Should fallback to "project"
        assert result == "project"

    def test_empty_project_name(self, tmp_path: Path):
        """Test sanitizing empty-ish project names."""
        resolver = PathResolver(tmp_path)

        # Test sanitize with names that become empty
        result = resolver._sanitize_name("---")
        assert result == "project"

        result = resolver._sanitize_name("")
        assert result == "project"

    def test_project_id_hash_uniqueness(self, tmp_path: Path):
        """Test that similar paths produce different hashes."""
        # Create paths that differ only slightly
        # Note: On case-insensitive filesystems (Windows, macOS default),
        # paths with only case differences may resolve to same path
        project1 = tmp_path / "project"
        project2 = tmp_path / "project2"
        project3 = tmp_path / "project3"

        for p in [project1, project2, project3]:
            p.mkdir(exist_ok=True)

        resolver = PathResolver(project1)
        ids = set()

        for p in [project1, project2, project3]:
            pid = resolver.get_project_id(p)
            ids.add(pid)

        # All IDs should be unique
        assert len(ids) == 3


class TestStateFileOperations:
    """Tests for state file path operations."""

    def test_multiple_state_files(self, tmp_path: Path):
        """Test getting paths for multiple state files."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        state_files = [
            "iteration-state.json",
            "agent-status.json",
            "retry-state.json",
            "custom-state.json",
        ]

        for name in state_files:
            path = resolver.get_state_file_path(name)
            assert path.name == name
            assert path.parent == resolver.get_state_dir()

    def test_state_dir_in_legacy_mode(self, tmp_path: Path):
        """Test state directory in legacy mode."""
        resolver = PathResolver(tmp_path, legacy_mode=True)

        # In legacy mode, state dir should be .state under project root
        state_dir = resolver.get_state_dir()
        assert state_dir == tmp_path.resolve() / ".state"


class TestCleanupOperations:
    """Tests for cleanup operations."""

    def test_cleanup_removes_all_project_data(self, tmp_path: Path):
        """Test that cleanup removes all project directories."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        # Create directories and files
        resolver.ensure_directories()
        resolver.write_manifest()

        # Create some state files
        (resolver.get_state_dir() / "test.json").write_text("{}")
        (resolver.get_worktree_dir() / "task1").mkdir()

        # Verify exists
        assert resolver.get_project_dir().exists()

        # Cleanup
        resolver.cleanup_project_data()

        # Verify removed
        assert not resolver.get_project_dir().exists()

    def test_cleanup_multiple_times(self, tmp_path: Path):
        """Test that cleanup can be called multiple times safely."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        resolver.ensure_directories()
        resolver.cleanup_project_data()
        # Second cleanup should not raise
        resolver.cleanup_project_data()

    def test_cleanup_nonexistent_project(self, tmp_path: Path):
        """Test cleanup when project directory doesn't exist."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        # Cleanup without ever creating directories
        resolver.cleanup_project_data()  # Should not raise


class TestFindProjectById:
    """Tests for finding projects by ID."""

    def test_find_project_with_special_characters(self, tmp_path: Path):
        """Test finding project that had special characters in name."""
        data_dir = tmp_path / "data"
        project = tmp_path / "my special project"
        project.mkdir()

        resolver = PathResolver(project, data_dir_override=data_dir)
        resolver.write_manifest()

        project_id = resolver.get_project_id()
        found = PathResolver.find_project_by_id(project_id, data_dir=data_dir)

        assert found == project.resolve()

    def test_find_project_empty_data_dir(self, tmp_path: Path):
        """Test finding project when data directory is empty."""
        data_dir = tmp_path / "empty-data"
        # Don't create the directory

        result = PathResolver.find_project_by_id("nonexistent-12345678", data_dir=data_dir)
        assert result is None


class TestDetectProjectMode:
    """Tests for detect_project_mode function."""

    def test_no_link_file_returns_legacy(self, tmp_path: Path):
        """Test that missing link file returns legacy mode."""
        result = detect_project_mode(tmp_path)
        assert result == "legacy"

    def test_valid_link_file_returns_migrated(self, tmp_path: Path):
        """Test that valid link file returns migrated mode."""
        # Create data directory
        data_dir = tmp_path / "data" / "my-project-12345678"
        data_dir.mkdir(parents=True)

        # Create valid link file
        link_data = {
            "project_id": "my-project-12345678",
            "data_path": str(data_dir),
            "created_at": "2024-01-01T00:00:00Z",
            "last_accessed": "2024-01-01T00:00:00Z",
        }
        link_path = tmp_path / LINK_FILE_NAME
        link_path.write_text(json.dumps(link_data))

        result = detect_project_mode(tmp_path)
        assert result == "migrated"

    def test_link_file_with_data_dir_field(self, tmp_path: Path):
        """Test link file using data_dir field (alternative format)."""
        # Create data directory
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        # Create link file with data_dir instead of data_path
        link_data = {
            "version": "1.0",
            "project_id": "test-proj",
            "data_dir": str(data_dir),
            "migrated_at": "2024-01-01T00:00:00Z",
        }
        link_path = tmp_path / LINK_FILE_NAME
        link_path.write_text(json.dumps(link_data))

        result = detect_project_mode(tmp_path)
        assert result == "migrated"

    def test_missing_project_id_returns_legacy(self, tmp_path: Path):
        """Test that link file without project_id returns legacy mode."""
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        # Create link file without project_id
        link_data = {
            "data_path": str(data_dir),
        }
        link_path = tmp_path / LINK_FILE_NAME
        link_path.write_text(json.dumps(link_data))

        result = detect_project_mode(tmp_path)
        assert result == "legacy"

    def test_missing_data_path_returns_legacy(self, tmp_path: Path):
        """Test that link file without data_path or data_dir returns legacy mode."""
        # Create link file without data path
        link_data = {
            "project_id": "test-proj",
        }
        link_path = tmp_path / LINK_FILE_NAME
        link_path.write_text(json.dumps(link_data))

        result = detect_project_mode(tmp_path)
        assert result == "legacy"

    def test_nonexistent_data_dir_returns_legacy(self, tmp_path: Path):
        """Test that link file pointing to nonexistent data dir returns legacy mode."""
        # Create link file with nonexistent data path
        link_data = {
            "project_id": "test-proj",
            "data_path": str(tmp_path / "nonexistent" / "data"),
        }
        link_path = tmp_path / LINK_FILE_NAME
        link_path.write_text(json.dumps(link_data))

        result = detect_project_mode(tmp_path)
        assert result == "legacy"

    def test_invalid_json_returns_legacy(self, tmp_path: Path):
        """Test that invalid JSON link file returns legacy mode."""
        link_path = tmp_path / LINK_FILE_NAME
        link_path.write_text("not valid json {{{")

        result = detect_project_mode(tmp_path)
        assert result == "legacy"

    def test_empty_json_returns_legacy(self, tmp_path: Path):
        """Test that empty JSON object returns legacy mode."""
        link_path = tmp_path / LINK_FILE_NAME
        link_path.write_text("{}")

        result = detect_project_mode(tmp_path)
        assert result == "legacy"

    def test_resolves_relative_path(self, tmp_path: Path):
        """Test that function resolves relative paths."""
        # Create data directory and link file
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        link_data = {
            "project_id": "test-proj",
            "data_path": str(data_dir),
        }
        link_path = tmp_path / LINK_FILE_NAME
        link_path.write_text(json.dumps(link_data))

        # Should work even with Path object
        result = detect_project_mode(Path(tmp_path))
        assert result == "migrated"

    def test_link_file_name_constant(self):
        """Test that the link file name constant is correct."""
        assert LINK_FILE_NAME == ".plan-cascade-link.json"
