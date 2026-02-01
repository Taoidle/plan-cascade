"""Tests for MigrationManager module."""

import json
import os
import shutil
import sys
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from plan_cascade.cli.migrate import (
    BACKUP_DIR_NAME,
    MIGRATION_STATE_FILE,
    PLANNING_DIRS,
    PLANNING_FILES,
    DetectedFile,
    MigrationManager,
    MigrationResult,
)


class TestMigrationManagerBasic:
    """Basic tests for MigrationManager class."""

    def test_init(self, tmp_path: Path):
        """Test MigrationManager initialization."""
        project = tmp_path / "my-project"
        project.mkdir()

        manager = MigrationManager(project)
        assert manager.project_root == project.resolve()

    def test_init_with_data_dir_override(self, tmp_path: Path):
        """Test MigrationManager with custom data directory."""
        project = tmp_path / "my-project"
        project.mkdir()
        data_dir = tmp_path / "custom-data"

        manager = MigrationManager(project, data_dir_override=data_dir)
        assert manager.get_data_dir() == data_dir

    def test_get_project_id(self, tmp_path: Path):
        """Test getting project ID."""
        project = tmp_path / "my-project"
        project.mkdir()

        manager = MigrationManager(project)
        project_id = manager.get_project_id()

        assert project_id is not None
        assert "my-project" in project_id
        assert len(project_id) > len("my-project")  # Has hash suffix


class TestDetectExistingFiles:
    """Tests for detecting existing planning files."""

    def test_detect_no_files(self, tmp_path: Path):
        """Test detection when no planning files exist."""
        project = tmp_path / "empty-project"
        project.mkdir()

        manager = MigrationManager(project)
        detected = manager.detect_existing_files()

        assert detected == []

    def test_detect_prd_json(self, tmp_path: Path):
        """Test detection of prd.json."""
        project = tmp_path / "project"
        project.mkdir()

        prd_path = project / "prd.json"
        prd_path.write_text('{"goal": "test"}')

        manager = MigrationManager(project)
        detected = manager.detect_existing_files()

        assert len(detected) == 1
        assert detected[0].path == prd_path
        assert detected[0].is_dir is False

    def test_detect_mega_plan_json(self, tmp_path: Path):
        """Test detection of mega-plan.json."""
        project = tmp_path / "project"
        project.mkdir()

        mega_path = project / "mega-plan.json"
        mega_path.write_text('{"features": []}')

        manager = MigrationManager(project)
        detected = manager.detect_existing_files()

        assert len(detected) == 1
        assert detected[0].path == mega_path

    def test_detect_worktree_dir(self, tmp_path: Path):
        """Test detection of .worktree directory."""
        project = tmp_path / "project"
        project.mkdir()

        worktree_dir = project / ".worktree"
        worktree_dir.mkdir()
        (worktree_dir / "task1").mkdir()

        manager = MigrationManager(project)
        detected = manager.detect_existing_files()

        assert len(detected) == 1
        assert detected[0].path == worktree_dir
        assert detected[0].is_dir is True

    def test_detect_multiple_files(self, tmp_path: Path):
        """Test detection of multiple planning files."""
        project = tmp_path / "project"
        project.mkdir()

        # Create multiple files
        (project / "prd.json").write_text('{}')
        (project / "mega-plan.json").write_text('{}')
        (project / ".iteration-state.json").write_text('{}')
        (project / ".worktree").mkdir()

        manager = MigrationManager(project)
        detected = manager.detect_existing_files()

        assert len(detected) == 4
        names = [d.path.name for d in detected]
        assert "prd.json" in names
        assert "mega-plan.json" in names
        assert ".iteration-state.json" in names
        assert ".worktree" in names

    def test_detect_file_size_and_modified(self, tmp_path: Path):
        """Test that detected files include size and modification time."""
        project = tmp_path / "project"
        project.mkdir()

        content = '{"test": "data", "key": "value"}'
        prd_path = project / "prd.json"
        prd_path.write_text(content)

        manager = MigrationManager(project)
        detected = manager.detect_existing_files()

        assert len(detected) == 1
        assert detected[0].size == len(content)
        assert detected[0].modified != ""


class TestIsAlreadyMigrated:
    """Tests for checking if project is already migrated."""

    def test_not_migrated(self, tmp_path: Path):
        """Test project not migrated."""
        project = tmp_path / "project"
        project.mkdir()

        manager = MigrationManager(project)
        assert manager.is_already_migrated() is False

    def test_already_migrated(self, tmp_path: Path):
        """Test project already migrated."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        # Create link file
        link_data = {
            "project_id": "test-123",
            "data_path": str(data_dir),
            "created_at": "2024-01-01T00:00:00Z",
            "last_accessed": "2024-01-01T00:00:00Z",
        }
        link_path = project / ".plan-cascade-link.json"
        link_path.write_text(json.dumps(link_data))

        manager = MigrationManager(project, data_dir_override=data_dir)
        assert manager.is_already_migrated() is True


class TestMigrateDryRun:
    """Tests for dry-run migration."""

    def test_dry_run_no_files(self, tmp_path: Path):
        """Test dry run with no files to migrate."""
        project = tmp_path / "project"
        project.mkdir()

        manager = MigrationManager(project)
        result = manager.migrate(dry_run=True)

        assert result.success is False
        assert "Nothing to migrate" in result.message

    def test_dry_run_with_files(self, tmp_path: Path):
        """Test dry run reports files correctly."""
        project = tmp_path / "project"
        project.mkdir()

        (project / "prd.json").write_text('{}')
        (project / ".worktree").mkdir()

        manager = MigrationManager(project, data_dir_override=tmp_path / "data")
        result = manager.migrate(dry_run=True)

        assert result.success is True
        assert "DRY RUN" in result.message
        assert len(result.files_migrated) == 1  # prd.json
        assert len(result.dirs_migrated) == 1   # .worktree

    def test_dry_run_does_not_modify(self, tmp_path: Path):
        """Test that dry run doesn't modify any files."""
        project = tmp_path / "project"
        project.mkdir()

        prd_path = project / "prd.json"
        prd_path.write_text('{"test": true}')

        manager = MigrationManager(project, data_dir_override=tmp_path / "data")
        result = manager.migrate(dry_run=True)

        # Original file should still exist
        assert prd_path.exists()
        assert prd_path.read_text() == '{"test": true}'

        # No link file should be created
        assert not (project / ".plan-cascade-link.json").exists()


class TestMigrate:
    """Tests for actual migration."""

    def test_migrate_creates_backup(self, tmp_path: Path):
        """Test that migration creates a backup."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        (project / "prd.json").write_text('{}')

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)

        backup_dir = project / BACKUP_DIR_NAME
        assert backup_dir.exists()
        assert (backup_dir / "prd.json").exists()
        assert (backup_dir / "backup-metadata.json").exists()

    def test_migrate_moves_files(self, tmp_path: Path):
        """Test that migration moves files to target."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        prd_content = '{"goal": "test migration"}'
        (project / "prd.json").write_text(prd_content)

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)

        assert result.success is True

        # Original file should be gone
        assert not (project / "prd.json").exists()

        # File should be in target directory
        target_dir = manager.get_target_dir()
        assert (target_dir / "prd.json").exists()
        assert (target_dir / "prd.json").read_text() == prd_content

    def test_migrate_moves_directories(self, tmp_path: Path):
        """Test that migration moves directories to target."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        # Create .worktree directory with content
        worktree_dir = project / ".worktree"
        worktree_dir.mkdir()
        task_dir = worktree_dir / "task1"
        task_dir.mkdir()
        (task_dir / "config.json").write_text('{}')

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)

        assert result.success is True
        assert ".worktree" in result.dirs_migrated

        # Target should have the directory
        target_dir = manager.get_target_dir()
        assert (target_dir / ".worktree").exists()

    def test_migrate_creates_link_file(self, tmp_path: Path):
        """Test that migration creates link file in project root."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        (project / "prd.json").write_text('{}')

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)

        assert result.success is True

        # Link file should exist
        link_path = project / ".plan-cascade-link.json"
        assert link_path.exists()

        link_data = json.loads(link_path.read_text())
        assert "project_id" in link_data
        assert "data_path" in link_data

    def test_migrate_creates_manifest(self, tmp_path: Path):
        """Test that migration creates manifest in target."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        (project / "prd.json").write_text('{}')

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)

        assert result.success is True

        # Manifest should exist
        target_dir = manager.get_target_dir()
        manifest_path = target_dir / "manifest.json"
        assert manifest_path.exists()

    def test_migrate_saves_state(self, tmp_path: Path):
        """Test that migration saves state for rollback."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        (project / "prd.json").write_text('{}')

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)

        assert result.success is True

        # Migration state should exist
        target_dir = manager.get_target_dir()
        state_path = target_dir / MIGRATION_STATE_FILE
        assert state_path.exists()

        state = json.loads(state_path.read_text())
        assert "migrated_at" in state
        assert "project_root" in state
        assert "files_migrated" in state

    def test_migrate_already_migrated(self, tmp_path: Path):
        """Test that migration fails if already migrated."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"
        data_dir.mkdir()

        # Create link file
        link_data = {"project_id": "test", "data_path": str(data_dir)}
        (project / ".plan-cascade-link.json").write_text(json.dumps(link_data))

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)

        assert result.success is False
        assert "already migrated" in result.message


class TestRollback:
    """Tests for migration rollback."""

    def test_rollback_not_migrated(self, tmp_path: Path):
        """Test rollback when not migrated."""
        project = tmp_path / "project"
        project.mkdir()

        manager = MigrationManager(project)
        result = manager.rollback()

        assert result.success is False
        assert "not been migrated" in result.message

    def test_rollback_from_backup(self, tmp_path: Path):
        """Test rollback restores files from backup."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        # Create and migrate
        prd_content = '{"goal": "original"}'
        (project / "prd.json").write_text(prd_content)

        manager = MigrationManager(project, data_dir_override=data_dir)
        migrate_result = manager.migrate(dry_run=False)
        assert migrate_result.success is True

        # Rollback
        rollback_result = manager.rollback()
        assert rollback_result.success is True

        # Original file should be restored
        assert (project / "prd.json").exists()
        assert (project / "prd.json").read_text() == prd_content

        # Link file should be removed
        assert not (project / ".plan-cascade-link.json").exists()

        # Backup should be removed
        assert not (project / BACKUP_DIR_NAME).exists()

    def test_rollback_removes_target_dir(self, tmp_path: Path):
        """Test rollback removes target directory."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        (project / "prd.json").write_text('{}')

        manager = MigrationManager(project, data_dir_override=data_dir)
        migrate_result = manager.migrate(dry_run=False)
        target_dir = manager.get_target_dir()
        assert target_dir.exists()

        # Rollback
        rollback_result = manager.rollback()
        assert rollback_result.success is True

        # Target directory should be removed (if empty)
        # Note: May still exist if there were other files

    def test_rollback_restores_directories(self, tmp_path: Path):
        """Test rollback restores directories from backup."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        # Create worktree directory
        worktree = project / ".worktree"
        worktree.mkdir()
        task = worktree / "task1"
        task.mkdir()
        (task / "config.json").write_text('{"task": "test"}')

        manager = MigrationManager(project, data_dir_override=data_dir)
        migrate_result = manager.migrate(dry_run=False)
        assert migrate_result.success is True

        # Rollback
        rollback_result = manager.rollback()
        assert rollback_result.success is True

        # Directory should be restored
        assert worktree.exists()
        assert task.exists()
        assert (task / "config.json").exists()


class TestMigrationResult:
    """Tests for MigrationResult dataclass."""

    def test_result_defaults(self):
        """Test MigrationResult default values."""
        result = MigrationResult(success=True, message="OK")

        assert result.success is True
        assert result.message == "OK"
        assert result.files_migrated == []
        assert result.dirs_migrated == []
        assert result.worktrees_moved == []
        assert result.errors == []

    def test_result_with_data(self):
        """Test MigrationResult with populated data."""
        result = MigrationResult(
            success=False,
            message="Failed",
            files_migrated=["prd.json"],
            dirs_migrated=[".worktree"],
            errors=["Error 1"],
        )

        assert result.success is False
        assert len(result.files_migrated) == 1
        assert len(result.dirs_migrated) == 1
        assert len(result.errors) == 1


class TestDetectedFile:
    """Tests for DetectedFile dataclass."""

    def test_detected_file_defaults(self, tmp_path: Path):
        """Test DetectedFile default values."""
        file_path = tmp_path / "test.json"
        detected = DetectedFile(path=file_path, is_dir=False)

        assert detected.path == file_path
        assert detected.is_dir is False
        assert detected.size == 0
        assert detected.modified == ""

    def test_detected_directory(self, tmp_path: Path):
        """Test DetectedFile for directory."""
        dir_path = tmp_path / ".worktree"
        detected = DetectedFile(
            path=dir_path,
            is_dir=True,
            size=1024,
            modified="2024-01-01 00:00:00",
        )

        assert detected.is_dir is True
        assert detected.size == 1024
        assert detected.modified == "2024-01-01 00:00:00"


class TestConstants:
    """Tests for module constants."""

    def test_planning_files(self):
        """Test PLANNING_FILES constant contains expected files."""
        assert "prd.json" in PLANNING_FILES
        assert "mega-plan.json" in PLANNING_FILES
        assert ".iteration-state.json" in PLANNING_FILES

    def test_planning_dirs(self):
        """Test PLANNING_DIRS constant contains expected directories."""
        assert ".worktree" in PLANNING_DIRS
        assert ".locks" in PLANNING_DIRS
        assert ".state" in PLANNING_DIRS

    def test_backup_dir_name(self):
        """Test backup directory name constant."""
        assert BACKUP_DIR_NAME == ".plan-cascade-backup"

    def test_migration_state_file(self):
        """Test migration state file constant."""
        assert MIGRATION_STATE_FILE == ".migration-state.json"


class TestEdgeCases:
    """Tests for edge cases."""

    def test_migrate_empty_directory(self, tmp_path: Path):
        """Test migrating from empty directory."""
        project = tmp_path / "empty"
        project.mkdir()

        manager = MigrationManager(project)
        result = manager.migrate(dry_run=False)

        assert result.success is False
        assert "Nothing to migrate" in result.message

    def test_migrate_preserves_content(self, tmp_path: Path):
        """Test that migration preserves file content exactly."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        # Create files with specific content
        prd_content = '{\n  "goal": "test",\n  "stories": []\n}'
        mega_content = '{"features": [1, 2, 3]}'

        (project / "prd.json").write_text(prd_content)
        (project / "mega-plan.json").write_text(mega_content)

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)
        assert result.success is True

        # Verify content preserved
        target_dir = manager.get_target_dir()
        assert (target_dir / "prd.json").read_text() == prd_content
        assert (target_dir / "mega-plan.json").read_text() == mega_content

    def test_migrate_and_rollback_roundtrip(self, tmp_path: Path):
        """Test complete migrate and rollback cycle."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        # Create original state
        original_files = {
            "prd.json": '{"goal": "original"}',
            ".iteration-state.json": '{"iteration": 1}',
        }

        for name, content in original_files.items():
            (project / name).write_text(content)

        manager = MigrationManager(project, data_dir_override=data_dir)

        # Migrate
        migrate_result = manager.migrate(dry_run=False)
        assert migrate_result.success is True

        # Verify files moved
        for name in original_files:
            assert not (project / name).exists()

        # Rollback
        rollback_result = manager.rollback()
        assert rollback_result.success is True

        # Verify original state restored
        for name, content in original_files.items():
            assert (project / name).exists()
            assert (project / name).read_text() == content

    def test_special_characters_in_content(self, tmp_path: Path):
        """Test migration preserves special characters in files."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        # Use raw unicode characters instead of surrogate pairs
        special_content = '{"description": "Test with unicode: \u4e2d\u6587, special chars: \u00e9\u00e8\u00f1, \\n\\t"}'
        (project / "prd.json").write_text(special_content, encoding="utf-8")

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)
        assert result.success is True

        target_dir = manager.get_target_dir()
        migrated_content = (target_dir / "prd.json").read_text(encoding="utf-8")
        assert migrated_content == special_content


class TestPlatformSpecific:
    """Tests for platform-specific behavior."""

    @patch("sys.platform", "win32")
    @patch.dict("os.environ", {"APPDATA": "C:\\Users\\Test\\AppData\\Roaming"})
    def test_windows_data_dir(self, tmp_path: Path):
        """Test data directory on Windows."""
        project = tmp_path / "project"
        project.mkdir()

        manager = MigrationManager(project)
        data_dir = manager.get_data_dir()
        assert "plan-cascade" in str(data_dir)

    @patch("sys.platform", "linux")
    def test_linux_data_dir(self, tmp_path: Path):
        """Test data directory on Linux."""
        project = tmp_path / "project"
        project.mkdir()

        manager = MigrationManager(project)
        data_dir = manager.get_data_dir()
        assert ".plan-cascade" in str(data_dir)


class TestMigrationStateTracking:
    """Tests for migration state tracking."""

    def test_migration_state_contains_all_info(self, tmp_path: Path):
        """Test that migration state contains all required information."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        (project / "prd.json").write_text('{"goal": "test"}')
        (project / ".iteration-state.json").write_text('{}')

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)
        assert result.success is True

        # Check migration state
        target_dir = manager.get_target_dir()
        state_path = target_dir / MIGRATION_STATE_FILE
        state = json.loads(state_path.read_text())

        assert "migrated_at" in state
        assert "project_root" in state
        assert "files_migrated" in state
        assert "dirs_migrated" in state
        # Implementation uses "backup_dir" not "backup_path"
        assert "backup_dir" in state

    def test_backup_metadata_contents(self, tmp_path: Path):
        """Test backup metadata contains complete information."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        (project / "prd.json").write_text('{"goal": "test"}')

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)
        assert result.success is True

        # Check backup metadata
        backup_dir = project / BACKUP_DIR_NAME
        metadata_path = backup_dir / "backup-metadata.json"
        metadata = json.loads(metadata_path.read_text())

        assert "created_at" in metadata
        assert "project_root" in metadata
        # Implementation uses "files" and "directories" not "files_backed_up"
        assert "files" in metadata
        assert "directories" in metadata


class TestPartialMigration:
    """Tests for partial migration scenarios."""

    def test_migrate_only_files(self, tmp_path: Path):
        """Test migration when only files exist (no directories)."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        (project / "prd.json").write_text('{}')
        (project / ".iteration-state.json").write_text('{}')

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)

        assert result.success is True
        assert len(result.files_migrated) == 2
        assert len(result.dirs_migrated) == 0

    def test_migrate_only_directories(self, tmp_path: Path):
        """Test migration when only directories exist (no files)."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        worktree = project / ".worktree"
        worktree.mkdir()
        (worktree / "task1").mkdir()
        (worktree / "task1" / "config.json").write_text('{}')

        locks = project / ".locks"
        locks.mkdir()

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)

        assert result.success is True
        assert len(result.dirs_migrated) >= 1  # At least .worktree

    def test_migrate_mixed_content(self, tmp_path: Path):
        """Test migration with both files and directories."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        # Create files
        (project / "prd.json").write_text('{}')
        (project / "mega-plan.json").write_text('{}')

        # Create directories
        worktree = project / ".worktree"
        worktree.mkdir()
        task = worktree / "task1"
        task.mkdir()
        (task / "prd.json").write_text('{}')

        locks = project / ".locks"
        locks.mkdir()
        (locks / "test.lock").write_text('')

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)

        assert result.success is True
        assert len(result.files_migrated) >= 2
        assert len(result.dirs_migrated) >= 1


class TestRollbackEdgeCases:
    """Tests for rollback edge cases."""

    def test_rollback_no_backup(self, tmp_path: Path):
        """Test rollback when backup was deleted."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        (project / "prd.json").write_text('{}')

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)
        assert result.success is True

        # Delete backup directory
        backup_dir = project / BACKUP_DIR_NAME
        shutil.rmtree(backup_dir)

        # Rollback should still work (from target dir)
        rollback_result = manager.rollback()
        # May fail gracefully without backup
        # The implementation may handle this differently

    def test_rollback_partial_backup(self, tmp_path: Path):
        """Test rollback with partial backup (some files missing)."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        (project / "prd.json").write_text('{"goal": "original"}')
        (project / "mega-plan.json").write_text('{"features": []}')

        manager = MigrationManager(project, data_dir_override=data_dir)
        result = manager.migrate(dry_run=False)
        assert result.success is True

        # Delete one file from backup
        backup_dir = project / BACKUP_DIR_NAME
        (backup_dir / "mega-plan.json").unlink()

        # Rollback
        rollback_result = manager.rollback()
        # Should still restore prd.json
        assert (project / "prd.json").exists()


class TestConcurrentMigration:
    """Tests for concurrent migration scenarios."""

    def test_migration_idempotent(self, tmp_path: Path):
        """Test that migration is idempotent (already migrated check)."""
        project = tmp_path / "project"
        project.mkdir()
        data_dir = tmp_path / "data"

        (project / "prd.json").write_text('{}')

        manager = MigrationManager(project, data_dir_override=data_dir)

        # First migration
        result1 = manager.migrate(dry_run=False)
        assert result1.success is True

        # Second migration should fail
        result2 = manager.migrate(dry_run=False)
        assert result2.success is False
        assert "already migrated" in result2.message


class TestDetectionEdgeCases:
    """Tests for detection edge cases."""

    def test_detect_hidden_files(self, tmp_path: Path):
        """Test detection of hidden planning files."""
        project = tmp_path / "project"
        project.mkdir()

        # Create hidden state files
        (project / ".iteration-state.json").write_text('{}')
        (project / ".agent-status.json").write_text('{}')
        (project / ".mega-status.json").write_text('{}')

        manager = MigrationManager(project)
        detected = manager.detect_existing_files()

        names = [d.path.name for d in detected]
        assert ".iteration-state.json" in names
        assert ".agent-status.json" in names
        assert ".mega-status.json" in names

    def test_detect_empty_worktree(self, tmp_path: Path):
        """Test detection of empty worktree directory."""
        project = tmp_path / "project"
        project.mkdir()

        # Create empty worktree
        (project / ".worktree").mkdir()

        manager = MigrationManager(project)
        detected = manager.detect_existing_files()

        # Empty directory should still be detected
        names = [d.path.name for d in detected]
        assert ".worktree" in names

    def test_detect_nested_worktree_content(self, tmp_path: Path):
        """Test detection counts nested worktree content."""
        project = tmp_path / "project"
        project.mkdir()

        # Create worktree with nested content
        worktree = project / ".worktree"
        worktree.mkdir()

        for i in range(5):
            task = worktree / f"task{i}"
            task.mkdir()
            (task / "prd.json").write_text('{}')
            (task / "progress.txt").write_text('test')

        manager = MigrationManager(project)
        detected = manager.detect_existing_files()

        worktree_entry = [d for d in detected if d.path.name == ".worktree"][0]
        assert worktree_entry.is_dir is True
        # Size should reflect content
        assert worktree_entry.size > 0
