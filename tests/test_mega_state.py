"""Tests for MegaStateManager module."""

import json
import time
from pathlib import Path

import pytest

from plan_cascade.state.mega_state import MegaStateManager
from plan_cascade.state.path_resolver import PathResolver


class TestMegaStateManagerBasic:
    """Basic tests for MegaStateManager class."""

    def test_init(self, tmp_path: Path):
        """Test MegaStateManager initialization."""
        msm = MegaStateManager(tmp_path)
        assert msm.project_root == tmp_path

    def test_read_mega_plan_not_found(self, tmp_path: Path):
        """Test reading mega-plan when file doesn't exist."""
        msm = MegaStateManager(tmp_path)
        result = msm.read_mega_plan()
        assert result is None

    def test_write_read_mega_plan(self, tmp_path: Path):
        """Test writing and reading mega-plan."""
        msm = MegaStateManager(tmp_path)
        plan = {
            "project_name": "test-project",
            "features": [
                {"id": "feature-001", "name": "feature-one", "status": "pending"}
            ]
        }

        msm.write_mega_plan(plan)
        result = msm.read_mega_plan()

        assert result is not None
        assert result["project_name"] == "test-project"
        assert len(result["features"]) == 1

    def test_read_status_not_found(self, tmp_path: Path):
        """Test reading status when file doesn't exist."""
        msm = MegaStateManager(tmp_path)
        result = msm.read_status()
        assert result is None

    def test_write_read_status(self, tmp_path: Path):
        """Test writing and reading status."""
        msm = MegaStateManager(tmp_path)
        status = {
            "updated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "features": {}
        }

        msm.write_status(status)
        result = msm.read_status()

        assert result is not None
        assert "updated_at" in result

    def test_read_mega_findings_not_found(self, tmp_path: Path):
        """Test reading mega-findings when file doesn't exist."""
        msm = MegaStateManager(tmp_path)
        result = msm.read_mega_findings()
        assert result == ""

    def test_initialize_mega_findings(self, tmp_path: Path):
        """Test initializing mega-findings file."""
        msm = MegaStateManager(tmp_path)
        msm.initialize_mega_findings()

        content = msm.read_mega_findings()
        assert "Mega Plan Findings" in content
        assert "shared findings" in content

    def test_append_mega_findings(self, tmp_path: Path):
        """Test appending to mega-findings."""
        msm = MegaStateManager(tmp_path)

        msm.append_mega_findings("Test finding 1", feature_id="feature-001")
        msm.append_mega_findings("Test finding 2")

        content = msm.read_mega_findings()
        assert "Test finding 1" in content
        assert "Test finding 2" in content
        assert "@feature: feature-001" in content

    def test_worktree_helpers(self, tmp_path: Path):
        """Test worktree helper methods."""
        msm = MegaStateManager(tmp_path)

        # Test get_worktree_path
        path = msm.get_worktree_path("feature-one")
        assert path == msm.worktree_dir / "feature-one"

        # Test worktree_exists
        assert not msm.worktree_exists("feature-one")

        # Create the worktree directory
        path.mkdir(parents=True)
        assert msm.worktree_exists("feature-one")

    def test_cleanup_locks(self, tmp_path: Path):
        """Test cleanup_locks doesn't raise errors."""
        msm = MegaStateManager(tmp_path)
        msm.cleanup_locks()  # Should not raise

    def test_cleanup_all(self, tmp_path: Path):
        """Test cleanup_all removes all files."""
        msm = MegaStateManager(tmp_path)

        # Create files
        msm.write_mega_plan({"features": []})
        msm.write_status({"features": {}})
        msm.initialize_mega_findings()

        # Verify they exist
        assert msm.mega_plan_path.exists()
        assert msm.mega_status_path.exists()
        assert msm.mega_findings_path.exists()

        # Clean up
        msm.cleanup_all()

        # Verify they're gone
        assert not msm.mega_plan_path.exists()
        assert not msm.mega_status_path.exists()
        assert not msm.mega_findings_path.exists()


class TestMegaStateManagerWithPathResolver:
    """Tests for MegaStateManager integration with PathResolver."""

    def test_init_with_default_legacy_mode(self, tmp_path: Path):
        """Test that default initialization uses legacy mode for backward compatibility."""
        msm = MegaStateManager(tmp_path)

        # Default should be legacy mode for backward compatibility
        assert msm.is_legacy_mode() is True
        assert msm.mega_plan_path == tmp_path / "mega-plan.json"
        assert msm.mega_status_path == tmp_path / ".mega-status.json"
        assert msm.mega_findings_path == tmp_path / "mega-findings.md"
        assert msm.worktree_dir == tmp_path / ".worktree"
        assert msm.locks_dir == tmp_path / ".locks"

    def test_init_with_explicit_legacy_mode(self, tmp_path: Path):
        """Test explicit legacy mode initialization."""
        msm = MegaStateManager(tmp_path, legacy_mode=True)

        assert msm.is_legacy_mode() is True
        assert msm.mega_plan_path == tmp_path / "mega-plan.json"
        assert msm.mega_status_path == tmp_path / ".mega-status.json"
        assert msm.mega_findings_path == tmp_path / "mega-findings.md"

    def test_init_with_new_mode(self, tmp_path: Path):
        """Test MegaStateManager with new mode (user directory structure)."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        msm = MegaStateManager(project_root, path_resolver=resolver)

        assert msm.is_legacy_mode() is False
        # Mega plan should be in data directory
        assert data_dir in msm.mega_plan_path.parents or data_dir == msm.mega_plan_path.parent.parent
        # Mega status should be in state directory
        assert ".state" in str(msm.mega_status_path)
        # Mega findings should be in project root
        assert msm.mega_findings_path == project_root / "mega-findings.md"
        # Worktree should be in data directory
        assert data_dir in msm.worktree_dir.parents or data_dir == msm.worktree_dir.parent.parent

    def test_init_with_legacy_mode_false(self, tmp_path: Path):
        """Test initialization with legacy_mode=False creates PathResolver in new mode."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        msm = MegaStateManager(project_root, path_resolver=resolver)

        assert msm.is_legacy_mode() is False
        # Status file should be in the state directory
        assert ".state" in str(msm.mega_status_path)

    def test_path_resolver_property(self, tmp_path: Path):
        """Test that path_resolver property returns the resolver."""
        msm = MegaStateManager(tmp_path)

        assert msm.path_resolver is not None
        assert isinstance(msm.path_resolver, PathResolver)

    def test_injected_path_resolver_is_used(self, tmp_path: Path):
        """Test that injected PathResolver is used."""
        data_dir = tmp_path / "custom_data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        msm = MegaStateManager(project_root, path_resolver=resolver)

        assert msm.path_resolver is resolver
        assert msm.is_legacy_mode() is False

    def test_new_mode_mega_plan_operations(self, tmp_path: Path):
        """Test mega-plan read/write operations in new mode."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        msm = MegaStateManager(project_root, path_resolver=resolver)

        plan = {
            "project_name": "test-project",
            "features": [
                {"id": "feature-001", "name": "feature-one", "status": "pending"}
            ]
        }

        msm.write_mega_plan(plan)
        result = msm.read_mega_plan()

        assert result is not None
        assert result["project_name"] == "test-project"
        # Verify file was written to new location
        assert msm.mega_plan_path.exists()
        assert data_dir in msm.mega_plan_path.parents

    def test_new_mode_status_operations(self, tmp_path: Path):
        """Test status file operations in new mode."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        msm = MegaStateManager(project_root, path_resolver=resolver)

        status = {
            "updated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "features": {}
        }
        msm.write_status(status)
        result = msm.read_status()

        assert result is not None
        # Verify file was written to state directory
        assert msm.mega_status_path.exists()
        assert ".state" in str(msm.mega_status_path)

    def test_new_mode_locks_in_user_directory(self, tmp_path: Path):
        """Test that locks are created in user directory in new mode."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        msm = MegaStateManager(project_root, path_resolver=resolver)

        # Perform an operation that creates a lock
        plan = {"features": []}
        msm.write_mega_plan(plan)

        # Locks directory should be in data directory
        assert ".locks" in str(msm.locks_dir)
        assert data_dir in msm.locks_dir.parents

    def test_mega_findings_in_project_root_new_mode(self, tmp_path: Path):
        """Test that mega-findings.md remains in project root even in new mode."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        msm = MegaStateManager(project_root, path_resolver=resolver)

        # mega-findings should stay in project root
        assert msm.mega_findings_path == project_root / "mega-findings.md"

        # Test operations
        msm.initialize_mega_findings()

        assert msm.mega_findings_path.exists()

    def test_ensure_directories(self, tmp_path: Path):
        """Test ensure_directories creates all necessary directories."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        msm = MegaStateManager(project_root, path_resolver=resolver)
        msm.ensure_directories()

        # Verify directories were created
        assert msm.locks_dir.exists()
        project_dir = resolver.get_project_dir()
        assert project_dir.exists()
        assert resolver.get_state_dir().exists()
        assert resolver.get_worktree_dir().exists()

    def test_backward_compatibility_existing_code(self, tmp_path: Path):
        """Test that existing code using MegaStateManager(project_root) continues to work."""
        # This simulates existing code that only passes project_root
        msm = MegaStateManager(tmp_path)

        # All operations should work in legacy mode
        plan = {"features": [{"id": "f1", "name": "feature1", "status": "pending"}]}
        msm.write_mega_plan(plan)
        result = msm.read_mega_plan()
        assert result["features"][0]["id"] == "f1"

        msm.initialize_mega_findings()
        assert "Mega Plan Findings" in msm.read_mega_findings()

        status = {"features": {}}
        msm.write_status(status)
        assert msm.read_status() is not None

        # Verify files are in project root (legacy behavior)
        assert (tmp_path / "mega-plan.json").exists()
        assert (tmp_path / "mega-findings.md").exists()
        assert (tmp_path / ".mega-status.json").exists()

    def test_cleanup_locks_works_in_both_modes(self, tmp_path: Path):
        """Test cleanup_locks works in both legacy and new modes."""
        # Legacy mode
        msm_legacy = MegaStateManager(tmp_path / "legacy", legacy_mode=True)
        (tmp_path / "legacy").mkdir(parents=True)
        msm_legacy.cleanup_locks()  # Should not raise

        # New mode
        data_dir = tmp_path / "data"
        resolver = PathResolver(
            project_root=tmp_path / "new",
            legacy_mode=False,
            data_dir_override=data_dir,
        )
        (tmp_path / "new").mkdir(parents=True)
        msm_new = MegaStateManager(tmp_path / "new", path_resolver=resolver)
        msm_new.cleanup_locks()  # Should not raise


class TestMegaStateManagerWorktreeOperations:
    """Tests for worktree operations with PathResolver."""

    def test_sync_status_from_worktrees_no_worktrees(self, tmp_path: Path):
        """Test sync_status_from_worktrees when no worktrees exist."""
        msm = MegaStateManager(tmp_path)
        result = msm.sync_status_from_worktrees()
        assert result == {}

    def test_sync_status_from_worktrees_new_mode(self, tmp_path: Path):
        """Test sync_status_from_worktrees works in new mode."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        msm = MegaStateManager(project_root, path_resolver=resolver)

        # Create mega-plan
        plan = {
            "features": [
                {"id": "f1", "name": "feature-one", "status": "in_progress"}
            ]
        }
        msm.write_mega_plan(plan)

        # Create worktree directory with prd.json
        worktree_path = msm.worktree_dir / "feature-one"
        worktree_path.mkdir(parents=True)
        prd = {
            "stories": [
                {"id": "story-001", "status": "complete"},
                {"id": "story-002", "status": "pending"}
            ]
        }
        with open(worktree_path / "prd.json", "w") as f:
            json.dump(prd, f)

        # Sync status
        result = msm.sync_status_from_worktrees()

        assert "feature-one" in result
        assert result["feature-one"]["worktree_exists"] is True
        assert result["feature-one"]["prd_exists"] is True
        assert result["feature-one"]["stories_complete"] is False
        assert result["feature-one"]["stories_status"]["story-001"] == "complete"
        assert result["feature-one"]["stories_status"]["story-002"] == "pending"

    def test_copy_mega_findings_to_worktree_new_mode(self, tmp_path: Path):
        """Test copying mega-findings to worktree in new mode."""
        data_dir = tmp_path / "data"
        project_root = tmp_path / "project"
        project_root.mkdir(parents=True)

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        msm = MegaStateManager(project_root, path_resolver=resolver)

        # Create mega findings
        msm.initialize_mega_findings()
        msm.append_mega_findings("Important finding")

        # Create worktree directory
        worktree_path = msm.worktree_dir / "feature-one"
        worktree_path.mkdir(parents=True)

        # Copy findings
        msm.copy_mega_findings_to_worktree("feature-one")

        # Verify copy exists
        target_path = worktree_path / "mega-findings.md"
        assert target_path.exists()

        content = target_path.read_text()
        assert "READ-ONLY copy" in content
        assert "Important finding" in content

    def test_copy_mega_findings_to_nonexistent_worktree(self, tmp_path: Path):
        """Test copying mega-findings when worktree doesn't exist."""
        msm = MegaStateManager(tmp_path)
        msm.initialize_mega_findings()

        # Should not raise even if worktree doesn't exist
        msm.copy_mega_findings_to_worktree("nonexistent")


class TestPathResolverMegaMethods:
    """Tests for PathResolver mega-related methods."""

    def test_get_mega_status_path_new_mode(self, tmp_path: Path):
        """Test mega status path in new mode goes to state directory."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        status_path = resolver.get_mega_status_path()

        assert status_path.name == ".mega-status.json"
        assert ".state" in str(status_path)

    def test_get_mega_status_path_legacy_mode(self, tmp_path: Path):
        """Test mega status path in legacy mode stays in project root."""
        resolver = PathResolver(tmp_path, legacy_mode=True)

        status_path = resolver.get_mega_status_path()

        assert status_path == tmp_path.resolve() / ".mega-status.json"

    def test_get_mega_findings_path(self, tmp_path: Path):
        """Test mega findings path always goes to project root."""
        data_dir = tmp_path / "data"
        resolver = PathResolver(tmp_path, data_dir_override=data_dir)

        findings_path = resolver.get_mega_findings_path()

        assert findings_path == tmp_path.resolve() / "mega-findings.md"

    def test_get_mega_findings_path_legacy_mode(self, tmp_path: Path):
        """Test mega findings path in legacy mode stays in project root."""
        resolver = PathResolver(tmp_path, legacy_mode=True)

        findings_path = resolver.get_mega_findings_path()

        assert findings_path == tmp_path.resolve() / "mega-findings.md"
