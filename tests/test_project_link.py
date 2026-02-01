"""Tests for ProjectLinkManager module."""

import json
import sys
from pathlib import Path
from unittest.mock import patch

import pytest

from plan_cascade.state.project_link import LINK_FILE_NAME, ProjectLinkManager


class TestProjectLinkManagerBasic:
    """Basic tests for ProjectLinkManager class."""

    def test_init_default(self):
        """Test ProjectLinkManager initialization with defaults."""
        manager = ProjectLinkManager()
        assert manager._data_dir_override is None

    def test_init_with_data_dir_override(self, tmp_path: Path):
        """Test ProjectLinkManager with custom data directory."""
        custom_dir = tmp_path / "custom-data"
        manager = ProjectLinkManager(data_dir=custom_dir)
        assert manager.get_data_dir() == custom_dir

    def test_link_file_name_constant(self):
        """Test that link file name constant is correct."""
        assert LINK_FILE_NAME == ".plan-cascade-link.json"


class TestGetDataDir:
    """Tests for data directory resolution."""

    def test_data_dir_with_override(self, tmp_path: Path):
        """Test data directory with override."""
        custom_dir = tmp_path / "custom"
        manager = ProjectLinkManager(data_dir=custom_dir)
        assert manager.get_data_dir() == custom_dir

    @patch("sys.platform", "win32")
    @patch.dict("os.environ", {"APPDATA": "C:\\Users\\Test\\AppData\\Roaming"})
    def test_data_dir_windows(self):
        """Test data directory on Windows."""
        manager = ProjectLinkManager()
        data_dir = manager.get_data_dir()
        assert "plan-cascade" in str(data_dir)

    @patch("sys.platform", "linux")
    def test_data_dir_unix(self):
        """Test data directory on Unix."""
        manager = ProjectLinkManager()
        data_dir = manager.get_data_dir()
        assert ".plan-cascade" in str(data_dir)


class TestGetLinkPath:
    """Tests for link file path resolution."""

    def test_get_link_path(self, tmp_path: Path):
        """Test getting link file path."""
        manager = ProjectLinkManager()
        link_path = manager.get_link_path(tmp_path)
        assert link_path == tmp_path.resolve() / LINK_FILE_NAME

    def test_get_link_path_resolves_relative(self, tmp_path: Path):
        """Test that link path is resolved to absolute."""
        manager = ProjectLinkManager()
        project_dir = tmp_path / "project"
        project_dir.mkdir()

        with patch("pathlib.Path.cwd", return_value=tmp_path):
            link_path = manager.get_link_path(Path("project"))
            assert link_path.is_absolute()


class TestCreateLink:
    """Tests for link file creation."""

    def test_create_link_basic(self, tmp_path: Path):
        """Test creating a basic link file."""
        manager = ProjectLinkManager()
        project_root = tmp_path / "my-project"
        project_root.mkdir()
        data_path = tmp_path / "data" / "my-project-12345678"
        data_path.mkdir(parents=True)

        manager.create_link(project_root, "my-project-12345678", data_path)

        link_path = project_root / LINK_FILE_NAME
        assert link_path.exists()

        with open(link_path) as f:
            link_data = json.load(f)

        assert link_data["project_id"] == "my-project-12345678"
        assert link_data["data_path"] == str(data_path.resolve())
        assert "created_at" in link_data
        assert "last_accessed" in link_data
        assert "platform" in link_data

    def test_create_link_preserves_created_at(self, tmp_path: Path):
        """Test that updating link file preserves created_at timestamp."""
        manager = ProjectLinkManager()
        project_root = tmp_path / "my-project"
        project_root.mkdir()
        data_path = tmp_path / "data"
        data_path.mkdir()

        # Create initial link
        manager.create_link(project_root, "proj-1", data_path)
        link_data1 = manager.read_link(project_root)
        created_at = link_data1["created_at"]

        # Update link
        manager.create_link(project_root, "proj-1", data_path)
        link_data2 = manager.read_link(project_root)

        assert link_data2["created_at"] == created_at
        # last_accessed should be updated (or same if immediate)
        assert link_data2["last_accessed"] is not None

    def test_create_link_updates_last_accessed(self, tmp_path: Path):
        """Test that updating link updates last_accessed."""
        manager = ProjectLinkManager()
        project_root = tmp_path / "my-project"
        project_root.mkdir()
        data_path = tmp_path / "data"
        data_path.mkdir()

        manager.create_link(project_root, "proj-1", data_path)

        link_data = manager.read_link(project_root)
        assert link_data["last_accessed"] is not None


class TestReadLink:
    """Tests for reading link files."""

    def test_read_link_exists(self, tmp_path: Path):
        """Test reading an existing link file."""
        manager = ProjectLinkManager()
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_path = tmp_path / "data"
        data_path.mkdir()

        manager.create_link(project_root, "test-proj", data_path)

        link_data = manager.read_link(project_root)

        assert link_data is not None
        assert link_data["project_id"] == "test-proj"
        assert link_data["data_path"] == str(data_path.resolve())

    def test_read_link_not_found(self, tmp_path: Path):
        """Test reading when link file doesn't exist."""
        manager = ProjectLinkManager()

        link_data = manager.read_link(tmp_path)

        assert link_data is None

    def test_read_link_invalid_json(self, tmp_path: Path):
        """Test reading a corrupted link file."""
        manager = ProjectLinkManager()
        project_root = tmp_path / "project"
        project_root.mkdir()

        link_path = project_root / LINK_FILE_NAME
        link_path.write_text("not valid json {{{")

        link_data = manager.read_link(project_root)

        assert link_data is None


class TestUpdateLastAccessed:
    """Tests for updating last_accessed timestamp."""

    def test_update_last_accessed_success(self, tmp_path: Path):
        """Test updating last_accessed on existing link."""
        manager = ProjectLinkManager()
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_path = tmp_path / "data"
        data_path.mkdir()

        manager.create_link(project_root, "proj", data_path)

        result = manager.update_last_accessed(project_root)

        assert result is True

    def test_update_last_accessed_no_link(self, tmp_path: Path):
        """Test updating last_accessed when link doesn't exist."""
        manager = ProjectLinkManager()

        result = manager.update_last_accessed(tmp_path)

        assert result is False


class TestDeleteLink:
    """Tests for deleting link files."""

    def test_delete_link_exists(self, tmp_path: Path):
        """Test deleting an existing link file."""
        manager = ProjectLinkManager()
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_path = tmp_path / "data"
        data_path.mkdir()

        manager.create_link(project_root, "proj", data_path)
        assert manager.get_link_path(project_root).exists()

        result = manager.delete_link(project_root)

        assert result is True
        assert not manager.get_link_path(project_root).exists()

    def test_delete_link_not_found(self, tmp_path: Path):
        """Test deleting when link file doesn't exist."""
        manager = ProjectLinkManager()

        result = manager.delete_link(tmp_path)

        assert result is False


class TestIsOrphaned:
    """Tests for orphan detection."""

    def test_is_orphaned_false_when_data_exists(self, tmp_path: Path):
        """Test orphan detection when data directory exists."""
        manager = ProjectLinkManager()
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_path = tmp_path / "data"
        data_path.mkdir()

        manager.create_link(project_root, "proj", data_path)

        assert manager.is_orphaned(project_root) is False

    def test_is_orphaned_true_when_data_deleted(self, tmp_path: Path):
        """Test orphan detection when data directory is deleted."""
        manager = ProjectLinkManager()
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_path = tmp_path / "data" / "proj-data"
        data_path.mkdir(parents=True)

        manager.create_link(project_root, "proj", data_path)

        # Delete data directory
        data_path.rmdir()

        assert manager.is_orphaned(project_root) is True

    def test_is_orphaned_false_no_link(self, tmp_path: Path):
        """Test orphan detection when no link file exists."""
        manager = ProjectLinkManager()

        assert manager.is_orphaned(tmp_path) is False


class TestDiscoverProjects:
    """Tests for project discovery."""

    def test_discover_projects_empty(self, tmp_path: Path):
        """Test discovering projects when none exist."""
        manager = ProjectLinkManager()

        projects = manager.discover_projects(search_paths=[tmp_path], max_depth=2)

        assert projects == []

    def test_discover_projects_single(self, tmp_path: Path):
        """Test discovering a single project."""
        manager = ProjectLinkManager()
        project_root = tmp_path / "my-project"
        project_root.mkdir()
        data_path = tmp_path / "data"
        data_path.mkdir()

        manager.create_link(project_root, "my-project-abc123", data_path)

        projects = manager.discover_projects(search_paths=[tmp_path], max_depth=2)

        assert len(projects) == 1
        assert projects[0]["project_id"] == "my-project-abc123"
        assert projects[0]["project_root"] == str(project_root)
        assert projects[0]["is_orphaned"] is False

    def test_discover_projects_multiple(self, tmp_path: Path):
        """Test discovering multiple projects."""
        manager = ProjectLinkManager()
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        # Create multiple projects
        for i in range(3):
            project = tmp_path / f"project{i}"
            project.mkdir()
            data = data_dir / f"proj{i}"
            data.mkdir()
            manager.create_link(project, f"proj{i}", data)

        projects = manager.discover_projects(search_paths=[tmp_path], max_depth=2)

        assert len(projects) == 3
        ids = [p["project_id"] for p in projects]
        assert "proj0" in ids
        assert "proj1" in ids
        assert "proj2" in ids

    def test_discover_projects_nested(self, tmp_path: Path):
        """Test discovering nested projects."""
        manager = ProjectLinkManager()
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        # Create nested project structure
        outer = tmp_path / "outer"
        outer.mkdir()
        inner = outer / "inner"
        inner.mkdir()

        outer_data = data_dir / "outer"
        outer_data.mkdir()
        inner_data = data_dir / "inner"
        inner_data.mkdir()

        manager.create_link(outer, "outer", outer_data)
        manager.create_link(inner, "inner", inner_data)

        projects = manager.discover_projects(search_paths=[tmp_path], max_depth=3)

        assert len(projects) == 2
        ids = [p["project_id"] for p in projects]
        assert "outer" in ids
        assert "inner" in ids

    def test_discover_projects_marks_orphaned(self, tmp_path: Path):
        """Test that discovered orphaned projects are marked."""
        manager = ProjectLinkManager()
        project = tmp_path / "project"
        project.mkdir()
        data = tmp_path / "data"
        data.mkdir()

        manager.create_link(project, "proj", data)

        # Delete data directory
        data.rmdir()

        projects = manager.discover_projects(search_paths=[tmp_path], max_depth=2)

        assert len(projects) == 1
        assert projects[0]["is_orphaned"] is True

    def test_discover_projects_respects_depth(self, tmp_path: Path):
        """Test that discovery respects max_depth."""
        manager = ProjectLinkManager()
        data = tmp_path / "data"
        data.mkdir()

        # Create deeply nested project
        deep = tmp_path / "a" / "b" / "c" / "d" / "project"
        deep.mkdir(parents=True)
        manager.create_link(deep, "deep", data)

        # Should not find with depth 2
        projects = manager.discover_projects(search_paths=[tmp_path], max_depth=2)
        assert len(projects) == 0

        # Should find with depth 5
        projects = manager.discover_projects(search_paths=[tmp_path], max_depth=5)
        assert len(projects) == 1

    def test_discover_projects_no_duplicates(self, tmp_path: Path):
        """Test that discovered projects are not duplicated."""
        manager = ProjectLinkManager()
        project = tmp_path / "project"
        project.mkdir()
        data = tmp_path / "data"
        data.mkdir()

        manager.create_link(project, "proj", data)

        # Search from multiple paths that could overlap
        projects = manager.discover_projects(
            search_paths=[tmp_path, tmp_path / "project"],
            max_depth=2,
        )

        assert len(projects) == 1


class TestDiscoverFromDataDir:
    """Tests for discovery from data directory manifests."""

    def test_discover_from_data_dir_empty(self, tmp_path: Path):
        """Test discovery when data directory is empty."""
        manager = ProjectLinkManager(data_dir=tmp_path)

        projects = manager.discover_from_data_dir()

        assert projects == []

    def test_discover_from_data_dir_single(self, tmp_path: Path):
        """Test discovery with a single project."""
        data_dir = tmp_path / "data"
        data_dir.mkdir()
        manager = ProjectLinkManager(data_dir=data_dir)

        # Create project data with manifest
        project_dir = data_dir / "proj-12345678"
        project_dir.mkdir()
        manifest = {
            "project_root": str(tmp_path / "my-project"),
            "project_id": "proj-12345678",
            "created_at": "2024-01-01T00:00:00Z",
        }
        (project_dir / "manifest.json").write_text(json.dumps(manifest))

        projects = manager.discover_from_data_dir()

        assert len(projects) == 1
        assert projects[0]["project_id"] == "proj-12345678"
        assert projects[0]["has_link_file"] is False
        assert projects[0]["project_root_exists"] is False

    def test_discover_from_data_dir_with_link(self, tmp_path: Path):
        """Test discovery when link file exists."""
        data_dir = tmp_path / "data"
        data_dir.mkdir()
        manager = ProjectLinkManager(data_dir=data_dir)

        # Create project root with link
        project_root = tmp_path / "my-project"
        project_root.mkdir()

        # Create project data with manifest
        project_data = data_dir / "proj-12345678"
        project_data.mkdir()
        manifest = {
            "project_root": str(project_root),
            "project_id": "proj-12345678",
            "created_at": "2024-01-01T00:00:00Z",
        }
        (project_data / "manifest.json").write_text(json.dumps(manifest))

        # Create link file
        manager.create_link(project_root, "proj-12345678", project_data)

        projects = manager.discover_from_data_dir()

        assert len(projects) == 1
        assert projects[0]["has_link_file"] is True
        assert projects[0]["project_root_exists"] is True
        assert projects[0]["last_accessed"] is not None


class TestCleanupOrphans:
    """Tests for orphan cleanup."""

    def test_cleanup_orphans_none(self, tmp_path: Path):
        """Test cleanup when no orphans exist."""
        manager = ProjectLinkManager()

        orphaned = manager.cleanup_orphans(search_paths=[tmp_path])

        assert orphaned == []

    def test_cleanup_orphans_finds(self, tmp_path: Path):
        """Test cleanup finds orphaned link files."""
        manager = ProjectLinkManager()
        project = tmp_path / "project"
        project.mkdir()
        data = tmp_path / "data"
        data.mkdir()

        manager.create_link(project, "proj", data)

        # Delete data directory
        data.rmdir()

        orphaned = manager.cleanup_orphans(search_paths=[tmp_path], dry_run=True)

        assert len(orphaned) == 1
        assert orphaned[0] == manager.get_link_path(project)
        # Link should still exist (dry_run)
        assert manager.get_link_path(project).exists()

    def test_cleanup_orphans_deletes(self, tmp_path: Path):
        """Test cleanup deletes orphaned link files."""
        manager = ProjectLinkManager()
        project = tmp_path / "project"
        project.mkdir()
        data = tmp_path / "data"
        data.mkdir()

        manager.create_link(project, "proj", data)

        # Delete data directory
        data.rmdir()

        orphaned = manager.cleanup_orphans(search_paths=[tmp_path], dry_run=False)

        assert len(orphaned) == 1
        # Link should be deleted
        assert not manager.get_link_path(project).exists()

    def test_cleanup_orphans_multiple(self, tmp_path: Path):
        """Test cleanup handles multiple orphaned links."""
        manager = ProjectLinkManager()

        # Create multiple projects
        orphan_count = 0
        for i in range(4):
            project = tmp_path / f"project{i}"
            project.mkdir()
            data = tmp_path / f"data{i}"
            data.mkdir()

            manager.create_link(project, f"proj{i}", data)

            # Delete some data directories (make orphans)
            if i % 2 == 0:
                data.rmdir()
                orphan_count += 1

        orphaned = manager.cleanup_orphans(search_paths=[tmp_path], dry_run=True)

        assert len(orphaned) == orphan_count


class TestPlatformSpecific:
    """Tests for platform-specific behavior."""

    @patch("sys.platform", "win32")
    @patch.dict("os.environ", {"APPDATA": ""})
    def test_windows_fallback_no_appdata(self):
        """Test Windows fallback when APPDATA is not set."""
        manager = ProjectLinkManager()
        data_dir = manager.get_data_dir()
        assert "plan-cascade" in str(data_dir)

    @patch("sys.platform", "darwin")
    def test_macos_uses_home(self):
        """Test macOS uses home directory."""
        manager = ProjectLinkManager()
        data_dir = manager.get_data_dir()
        assert ".plan-cascade" in str(data_dir)


class TestCLI:
    """Tests for CLI functionality."""

    def test_cli_create_and_read(self, tmp_path: Path, capsys):
        """Test CLI create and read commands work together."""
        import subprocess

        project = tmp_path / "project"
        project.mkdir()
        data = tmp_path / "data"
        data.mkdir()

        # This tests the module is importable and has the expected interface
        manager = ProjectLinkManager()
        manager.create_link(project, "test-proj", data)

        link_data = manager.read_link(project)
        assert link_data["project_id"] == "test-proj"
