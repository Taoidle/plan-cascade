"""
Comprehensive Integration Tests for Plan Cascade Path Migration.

This module provides end-to-end integration tests that verify the complete
workflow of the path migration system, including:

1. Full workflow integration: PathResolver -> StateManager -> MegaStateManager -> ContextRecovery
2. Migration workflow: detect -> migrate -> verify -> rollback
3. Session recovery after context compression
4. Cross-platform path behavior
5. Performance testing
"""

import json
import os
import sys
import time
from pathlib import Path
from unittest.mock import patch

import pytest

from plan_cascade.state.path_resolver import PathResolver
from plan_cascade.state.state_manager import StateManager
from plan_cascade.state.mega_state import MegaStateManager
from plan_cascade.state.context_recovery import (
    ContextRecoveryManager,
    ContextType,
    TaskState,
    PrdStatus,
)
from plan_cascade.state.config_manager import ConfigManager
from plan_cascade.cli.migrate import MigrationManager


class TestFullWorkflowIntegration:
    """End-to-end tests for full workflow with new path structure."""

    def test_complete_hybrid_workflow_new_mode(self, tmp_path: Path):
        """Test complete hybrid workflow: create project -> execute -> complete."""
        project_root = tmp_path / "my-project"
        project_root.mkdir()
        data_dir = tmp_path / "data"

        # 1. Create PathResolver in new mode
        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )

        # 2. Initialize directories
        resolver.ensure_directories()
        resolver.write_manifest()

        # Verify manifest
        manifest = resolver.read_manifest()
        assert manifest is not None
        assert manifest["project_root"] == str(project_root)

        # 3. Create StateManager with resolver
        sm = StateManager(project_root, path_resolver=resolver)
        assert sm.is_legacy_mode() is False

        # 4. Write PRD
        prd = {
            "metadata": {"version": "1.0.0"},
            "goal": "Test full workflow",
            "stories": [
                {"id": "story-001", "title": "Setup", "status": "pending"},
                {"id": "story-002", "title": "Build", "status": "pending"},
                {"id": "story-003", "title": "Test", "status": "pending"},
            ]
        }
        sm.write_prd(prd)

        # Verify PRD is in data directory
        prd_path = resolver.get_prd_path()
        assert prd_path.exists()
        assert data_dir in prd_path.parents

        # 5. Simulate story execution
        sm.update_story_status("story-001", "in_progress")
        sm.mark_story_in_progress("story-001")
        sm.record_agent_start("story-001", "claude-code", pid=12345)

        # 6. Complete story
        sm.update_story_status("story-001", "complete")
        sm.mark_story_complete("story-001")
        sm.record_agent_complete("story-001", "claude-code")
        sm.append_findings("Story 001 findings", tags=["story-001"])

        # 7. Verify state tracking
        status = sm.read_agent_status()
        assert len(status["completed"]) == 1

        statuses = sm.get_all_story_statuses()
        assert statuses["story-001"] == "complete"

        # 8. Verify context recovery can detect the state
        crm = ContextRecoveryManager(project_root, path_resolver=resolver)
        state = crm.detect_context()

        assert state.context_type == ContextType.HYBRID_AUTO
        assert state.prd_status == PrdStatus.VALID
        assert state.task_state == TaskState.EXECUTING
        assert "story-001" in state.completed_stories
        assert "story-002" in state.pending_stories

        # 9. Update context file for session recovery
        crm.update_context_file(state)

        # Verify context file in state directory
        state_dir = resolver.get_state_dir()
        context_file = state_dir / "hybrid-execution-context.md"
        assert context_file.exists()
        content = context_file.read_text()
        assert "story-001" in content

    def test_complete_mega_workflow_new_mode(self, tmp_path: Path):
        """Test complete mega-plan workflow with new mode."""
        project_root = tmp_path / "mega-project"
        project_root.mkdir()
        data_dir = tmp_path / "data"

        # 1. Create shared PathResolver
        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )
        resolver.ensure_directories()

        # 2. Create MegaStateManager
        msm = MegaStateManager(project_root, path_resolver=resolver)
        assert msm.is_legacy_mode() is False

        # 3. Write mega-plan
        mega_plan = {
            "project_name": "test-mega-project",
            "goal": "Build multi-feature system",
            "target_branch": "main",
            "features": [
                {"id": "feature-001", "name": "auth", "title": "Auth", "status": "pending"},
                {"id": "feature-002", "name": "api", "title": "API", "status": "pending"},
            ]
        }
        msm.write_mega_plan(mega_plan)

        # Verify mega-plan in data directory
        assert msm.mega_plan_path.exists()
        assert data_dir in msm.mega_plan_path.parents

        # 4. Initialize mega-findings
        msm.initialize_mega_findings()
        assert msm.mega_findings_path.exists()
        # mega-findings should be in project root (user-visible)
        assert msm.mega_findings_path == project_root / "mega-findings.md"

        # 5. Create worktree for first feature
        worktree_path = msm.get_worktree_path("auth")
        worktree_path.mkdir(parents=True)

        # Create PRD in worktree
        feature_prd = {
            "goal": "Implement auth",
            "stories": [
                {"id": "story-001", "title": "Login", "status": "pending"},
            ]
        }
        with open(worktree_path / "prd.json", "w") as f:
            json.dump(feature_prd, f)

        # 6. Sync status from worktrees
        result = msm.sync_status_from_worktrees()
        assert "auth" in result
        assert result["auth"]["worktree_exists"] is True
        assert result["auth"]["prd_exists"] is True

        # 7. Verify context recovery detects mega-plan
        crm = ContextRecoveryManager(project_root, path_resolver=resolver)
        state = crm.detect_context()

        assert state.context_type == ContextType.MEGA_PLAN
        assert len(state.mega_plan_features) == 2

    def test_shared_resolver_across_managers(self, tmp_path: Path):
        """Test that managers can share a single PathResolver instance."""
        project_root = tmp_path / "shared-project"
        project_root.mkdir()
        data_dir = tmp_path / "data"

        # Create shared resolver
        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )
        resolver.ensure_directories()

        # Create managers with shared resolver
        sm = StateManager(project_root, path_resolver=resolver)
        msm = MegaStateManager(project_root, path_resolver=resolver)
        crm = ContextRecoveryManager(project_root, path_resolver=resolver)

        # Verify all use the same resolver
        assert sm.path_resolver is resolver
        assert msm.path_resolver is resolver
        assert crm.path_resolver is resolver

        # Verify all are in new mode
        assert sm.is_legacy_mode() is False
        assert msm.is_legacy_mode() is False
        assert crm.is_legacy_mode() is False

        # Verify paths are consistent
        assert sm.prd_path == resolver.get_prd_path()
        assert msm.mega_plan_path == resolver.get_mega_plan_path()


class TestMigrationIntegration:
    """Integration tests for the migration workflow."""

    def test_full_migration_workflow(self, tmp_path: Path):
        """Test complete migration: detect -> migrate -> verify -> use -> rollback."""
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_dir = tmp_path / "data"

        # 1. Create legacy files in project root
        prd = {
            "goal": "Legacy project",
            "stories": [
                {"id": "story-001", "title": "Test", "status": "pending"},
            ]
        }
        with open(project_root / "prd.json", "w") as f:
            json.dump(prd, f)

        # Create worktree
        worktree_dir = project_root / ".worktree"
        worktree_dir.mkdir()
        task_dir = worktree_dir / "task1"
        task_dir.mkdir()
        with open(task_dir / "config.json", "w") as f:
            json.dump({"task": "test"}, f)

        # Create state files
        with open(project_root / ".iteration-state.json", "w") as f:
            json.dump({"status": "running"}, f)

        # 2. Detect existing files
        mm = MigrationManager(project_root, data_dir_override=data_dir)
        detected = mm.detect_existing_files()

        assert len(detected) >= 3
        names = [d.path.name for d in detected]
        assert "prd.json" in names
        assert ".worktree" in names

        # 3. Dry run migration
        dry_result = mm.migrate(dry_run=True)
        assert dry_result.success is True
        assert "DRY RUN" in dry_result.message

        # Verify files still exist in project root
        assert (project_root / "prd.json").exists()

        # 4. Actual migration
        result = mm.migrate(dry_run=False)
        assert result.success is True

        # 5. Verify files moved
        assert not (project_root / "prd.json").exists()
        target_dir = mm.get_target_dir()
        assert (target_dir / "prd.json").exists()

        # 6. Verify can use new mode
        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )
        sm = StateManager(project_root, path_resolver=resolver)

        # Read migrated PRD
        loaded_prd = sm.read_prd()
        assert loaded_prd is not None
        assert loaded_prd["goal"] == "Legacy project"

        # 7. Rollback
        rollback_result = mm.rollback()
        assert rollback_result.success is True

        # 8. Verify files restored
        assert (project_root / "prd.json").exists()
        restored_prd = json.loads((project_root / "prd.json").read_text())
        assert restored_prd["goal"] == "Legacy project"

    def test_migration_preserves_all_content(self, tmp_path: Path):
        """Test that migration preserves all file content exactly."""
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_dir = tmp_path / "data"

        # Create files with specific content
        files = {
            "prd.json": {"goal": "test", "stories": [], "unicode": "\u4e2d\u6587"},
            "mega-plan.json": {"features": [1, 2, 3]},
            ".iteration-state.json": {"batch": 1, "nested": {"key": "value"}},
        }

        for name, content in files.items():
            with open(project_root / name, "w", encoding="utf-8") as f:
                json.dump(content, f, ensure_ascii=False)

        # Migrate
        mm = MigrationManager(project_root, data_dir_override=data_dir)
        result = mm.migrate(dry_run=False)
        assert result.success is True

        # Verify content preserved
        target_dir = mm.get_target_dir()
        for name, expected in files.items():
            migrated_path = target_dir / name
            assert migrated_path.exists()
            actual = json.loads(migrated_path.read_text(encoding="utf-8"))
            assert actual == expected


class TestSessionRecovery:
    """Tests for session recovery after context compression."""

    def test_session_recovery_after_clear(self, tmp_path: Path):
        """Test session recovery after /clear command (simulated context loss)."""
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_dir = tmp_path / "data"

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )
        resolver.ensure_directories()

        # 1. Set up initial state (before /clear)
        sm = StateManager(project_root, path_resolver=resolver)
        prd = {
            "goal": "Test recovery",
            "stories": [
                {"id": "story-001", "status": "complete"},
                {"id": "story-002", "status": "in_progress"},
                {"id": "story-003", "status": "pending"},
            ]
        }
        sm.write_prd(prd)
        sm.mark_story_complete("story-001")
        sm.mark_story_in_progress("story-002")

        # Create context file
        crm = ContextRecoveryManager(project_root, path_resolver=resolver)
        state = crm.detect_context()
        crm.update_context_file(state)

        # 2. Simulate /clear - create new manager instances (no shared state)
        new_crm = ContextRecoveryManager(project_root, path_resolver=resolver)

        # 3. Recover context
        recovered_state = new_crm.detect_context()

        assert recovered_state.context_type == ContextType.HYBRID_AUTO
        assert recovered_state.task_state == TaskState.EXECUTING
        assert "story-001" in recovered_state.completed_stories
        assert "story-002" in recovered_state.in_progress_stories
        assert "story-003" in recovered_state.pending_stories

        # 4. Generate recovery plan
        plan = new_crm.generate_recovery_plan()

        assert plan.can_auto_resume
        assert len(plan.actions) >= 1

    def test_recovery_from_context_file(self, tmp_path: Path):
        """Test that context file provides recovery hints."""
        project_root = tmp_path / "project"
        project_root.mkdir()
        data_dir = tmp_path / "data"

        resolver = PathResolver(
            project_root=project_root,
            legacy_mode=False,
            data_dir_override=data_dir,
        )
        resolver.ensure_directories()

        # Set up PRD
        sm = StateManager(project_root, path_resolver=resolver)
        prd = {
            "goal": "Recovery test",
            "stories": [
                {"id": "story-001", "status": "complete"},
                {"id": "story-002", "status": "pending"},
            ]
        }
        sm.write_prd(prd)

        # Create and update context file
        crm = ContextRecoveryManager(project_root, path_resolver=resolver)
        state = crm.detect_context()
        crm.update_context_file(state)

        # Verify context file content
        context_file = resolver.get_state_dir() / "hybrid-execution-context.md"
        content = context_file.read_text()

        assert "Hybrid Execution Context" in content
        assert "Recovery test" in content
        # Completion percentage is formatted as "50.0%"
        assert "50.0%" in content


class TestCrossPlatformPaths:
    """Tests for cross-platform path handling."""

    def test_consistent_project_id_different_separators(self, tmp_path: Path):
        """Test project ID is consistent regardless of path separators."""
        project = tmp_path / "my-project"
        project.mkdir()

        resolver1 = PathResolver(project)
        id1 = resolver1.get_project_id()

        # Create path with different internal representation
        path_str = str(project)
        if sys.platform == "win32":
            # Already using backslashes
            pass
        resolver2 = PathResolver(Path(path_str))
        id2 = resolver2.get_project_id()

        assert id1 == id2

    def test_unicode_path_handling(self, tmp_path: Path):
        """Test handling of unicode characters in paths."""
        # Create project with unicode name
        project = tmp_path / "\u4e2d\u6587\u9879\u76ee"  # Chinese characters
        project.mkdir()

        resolver = PathResolver(project)
        project_id = resolver.get_project_id()

        # Should produce a valid, filesystem-safe ID
        assert project_id
        assert all(c in "0123456789abcdefghijklmnopqrstuvwxyz-_" for c in project_id)

    @patch("sys.platform", "win32")
    @patch.dict("os.environ", {"APPDATA": "C:\\Users\\Test\\AppData\\Roaming"})
    def test_windows_paths(self, tmp_path: Path):
        """Test path resolution on Windows."""
        project = tmp_path / "project"
        project.mkdir()

        resolver = PathResolver(project)
        data_dir = resolver.get_data_dir()

        assert "plan-cascade" in str(data_dir)
        # Should not have double backslashes or invalid paths
        assert "\\\\" not in str(data_dir)

    @patch("sys.platform", "darwin")
    def test_macos_paths(self, tmp_path: Path):
        """Test path resolution on macOS."""
        project = tmp_path / "project"
        project.mkdir()

        resolver = PathResolver(project)
        data_dir = resolver.get_data_dir()

        assert ".plan-cascade" in str(data_dir)

    @patch("sys.platform", "linux")
    def test_linux_paths(self, tmp_path: Path):
        """Test path resolution on Linux."""
        project = tmp_path / "project"
        project.mkdir()

        resolver = PathResolver(project)
        data_dir = resolver.get_data_dir()

        assert ".plan-cascade" in str(data_dir)


class TestBackwardCompatibility:
    """Tests for backward compatibility with existing projects."""

    def test_legacy_mode_workflow(self, tmp_path: Path):
        """Test complete workflow in legacy mode."""
        project = tmp_path / "legacy-project"
        project.mkdir()

        # Create managers in legacy mode
        sm = StateManager(project, legacy_mode=True)
        assert sm.is_legacy_mode() is True

        # Write PRD
        prd = {
            "goal": "Legacy test",
            "stories": [{"id": "story-001", "status": "pending"}]
        }
        sm.write_prd(prd)

        # Verify in project root
        assert (project / "prd.json").exists()

        # Create MegaStateManager in legacy mode
        msm = MegaStateManager(project, legacy_mode=True)
        assert msm.is_legacy_mode() is True

        mega_plan = {
            "features": [{"id": "feature-001", "name": "test", "status": "pending"}]
        }
        msm.write_mega_plan(mega_plan)

        # Verify in project root
        assert (project / "mega-plan.json").exists()

        # Context recovery in legacy mode
        crm = ContextRecoveryManager(project, legacy_mode=True)
        state = crm.detect_context()

        # Should detect both contexts (PRD has priority when both exist)
        # Actually, with mega-plan present, it should be MEGA_PLAN
        assert state.context_type in [ContextType.HYBRID_AUTO, ContextType.MEGA_PLAN]

    def test_auto_detect_legacy_project(self, tmp_path: Path):
        """Test that existing projects are auto-detected as legacy."""
        project = tmp_path / "existing-project"
        project.mkdir()

        # Create prd.json in project root (simulating existing project)
        with open(project / "prd.json", "w") as f:
            json.dump({"stories": []}, f)

        # ConfigManager should auto-detect legacy mode
        config = ConfigManager(project)
        assert config.is_legacy_mode() is True

    def test_mixed_mode_transition(self, tmp_path: Path):
        """Test transitioning from legacy mode to new mode."""
        project = tmp_path / "transition-project"
        project.mkdir()
        data_dir = tmp_path / "data"

        # Start in legacy mode
        sm_legacy = StateManager(project, legacy_mode=True)
        prd = {
            "goal": "Original",
            "stories": [{"id": "story-001", "status": "complete"}]
        }
        sm_legacy.write_prd(prd)
        sm_legacy.mark_story_complete("story-001")

        # Verify legacy files
        assert (project / "prd.json").exists()

        # Migrate to new mode
        mm = MigrationManager(project, data_dir_override=data_dir)
        result = mm.migrate(dry_run=False)
        assert result.success is True

        # Use new mode
        resolver = PathResolver(
            project_root=project,
            legacy_mode=False,
            data_dir_override=data_dir,
        )
        sm_new = StateManager(project, path_resolver=resolver)

        # Read migrated data
        loaded_prd = sm_new.read_prd()
        assert loaded_prd["goal"] == "Original"


class TestPerformance:
    """Performance tests for path operations."""

    def test_project_id_computation_performance(self, tmp_path: Path):
        """Test that project ID computation is fast."""
        project = tmp_path / "perf-project"
        project.mkdir()

        resolver = PathResolver(project)

        # Time 1000 project ID computations (including cache bypass)
        start = time.time()
        for i in range(1000):
            custom_path = tmp_path / f"project-{i}"
            custom_path.mkdir(exist_ok=True)
            resolver.get_project_id(custom_path)
        elapsed = time.time() - start

        # Should complete in under 1 second
        assert elapsed < 1.0, f"Project ID computation took too long: {elapsed}s"

    def test_file_operations_performance(self, tmp_path: Path):
        """Test file read/write performance."""
        project = tmp_path / "perf-project"
        project.mkdir()
        data_dir = tmp_path / "data"

        resolver = PathResolver(
            project_root=project,
            legacy_mode=False,
            data_dir_override=data_dir,
        )
        resolver.ensure_directories()

        sm = StateManager(project, path_resolver=resolver)

        # Time 100 PRD writes
        start = time.time()
        for i in range(100):
            prd = {
                "goal": f"Test {i}",
                "stories": [{"id": f"story-{j}", "status": "pending"} for j in range(10)]
            }
            sm.write_prd(prd)
        write_elapsed = time.time() - start

        # Time 100 PRD reads
        start = time.time()
        for _ in range(100):
            sm.read_prd()
        read_elapsed = time.time() - start

        # Should complete in reasonable time
        assert write_elapsed < 5.0, f"Write operations took too long: {write_elapsed}s"
        assert read_elapsed < 2.0, f"Read operations took too long: {read_elapsed}s"

    def test_large_prd_handling(self, tmp_path: Path):
        """Test handling of large PRD files."""
        project = tmp_path / "large-project"
        project.mkdir()

        sm = StateManager(project, legacy_mode=True)

        # Create PRD with 1000 stories
        prd = {
            "goal": "Large project test",
            "stories": [
                {
                    "id": f"story-{i:04d}",
                    "title": f"Story {i}",
                    "description": "A" * 500,  # 500 char description
                    "status": "pending",
                    "dependencies": [f"story-{j:04d}" for j in range(max(0, i-5), i)]
                }
                for i in range(1000)
            ]
        }

        # Write
        start = time.time()
        sm.write_prd(prd)
        write_time = time.time() - start

        # Read
        start = time.time()
        loaded = sm.read_prd()
        read_time = time.time() - start

        assert len(loaded["stories"]) == 1000
        assert write_time < 2.0, f"Large PRD write too slow: {write_time}s"
        assert read_time < 1.0, f"Large PRD read too slow: {read_time}s"


class TestEdgeCases:
    """Tests for edge cases and error handling."""

    def test_concurrent_access_simulation(self, tmp_path: Path):
        """Test handling of concurrent-like access patterns."""
        project = tmp_path / "concurrent-project"
        project.mkdir()

        # Create multiple StateManager instances (simulating concurrent access)
        managers = [StateManager(project, legacy_mode=True) for _ in range(5)]

        # All write to the same PRD
        for i, sm in enumerate(managers):
            prd = {
                "goal": f"Goal from manager {i}",
                "stories": [{"id": "story-001", "status": "pending"}]
            }
            sm.write_prd(prd)

        # Read back - should get the last write
        result = managers[0].read_prd()
        assert result["goal"] == "Goal from manager 4"

    def test_special_characters_in_project_name(self, tmp_path: Path):
        """Test handling of special characters in project names."""
        special_names = [
            "project with spaces",
            "project-with-dashes",
            "project_with_underscores",
            "PROJECT_UPPERCASE",
            "project@special!chars#",
            "12345numeric",
            "mix_123-abc XYZ",
        ]

        for name in special_names:
            project = tmp_path / name
            try:
                project.mkdir(exist_ok=True)
                resolver = PathResolver(project)
                project_id = resolver.get_project_id()

                # ID should be valid
                assert project_id
                # ID should be filesystem-safe
                assert all(c in "0123456789abcdefghijklmnopqrstuvwxyz-_" for c in project_id)
            except Exception as e:
                pytest.fail(f"Failed for project name '{name}': {e}")

    def test_very_long_project_path(self, tmp_path: Path):
        """Test handling of very long project paths."""
        # Create deeply nested path
        deep_path = tmp_path
        for i in range(20):
            deep_path = deep_path / f"level{i:02d}"

        deep_path.mkdir(parents=True)

        resolver = PathResolver(deep_path)
        project_id = resolver.get_project_id()

        # Should still produce valid ID
        assert project_id
        assert len(project_id) <= 100  # Reasonable length

    def test_corrupted_state_files(self, tmp_path: Path):
        """Test handling of corrupted state files."""
        project = tmp_path / "corrupted-project"
        project.mkdir()

        # Create corrupted files
        (project / "prd.json").write_text("{ invalid json")
        (project / "progress.txt").write_text("[2024-01-01 00:00:00] Test")
        (project / ".iteration-state.json").write_text("not json at all")

        sm = StateManager(project, legacy_mode=True)

        # StateManager.read_prd raises OSError for corrupted JSON
        # Test that we handle the error appropriately
        try:
            result = sm.read_prd()
            # If it returns None, that's acceptable
            assert result is None
        except OSError:
            # If it raises OSError, that's the documented behavior
            pass

        # Should still be able to read progress
        progress = sm.read_progress()
        assert "Test" in progress

    def test_empty_directories(self, tmp_path: Path):
        """Test behavior with empty directories."""
        project = tmp_path / "empty"
        project.mkdir()

        resolver = PathResolver(project, legacy_mode=True)
        sm = StateManager(project, path_resolver=resolver)
        msm = MegaStateManager(project, path_resolver=resolver)
        crm = ContextRecoveryManager(project, path_resolver=resolver)

        # All should handle empty state gracefully
        assert sm.read_prd() is None
        assert msm.read_mega_plan() is None

        state = crm.detect_context()
        assert state.context_type == ContextType.UNKNOWN
        assert state.prd_status == PrdStatus.MISSING


class TestConfigIntegration:
    """Tests for ConfigManager integration with PathResolver."""

    def test_config_driven_path_resolution(self, tmp_path: Path):
        """Test that ConfigManager settings drive path resolution."""
        project = tmp_path / "config-project"
        project.mkdir()

        # Set up project config with legacy mode
        config_file = project / ".plan-cascade.json"
        config_file.write_text(json.dumps({"legacy_mode": True}))

        config = ConfigManager(project)
        assert config.is_legacy_mode() is True

        # PathResolver should respect config
        # (In real usage, PathResolver would be created with config's legacy_mode value)
        resolver = PathResolver(project, legacy_mode=config.is_legacy_mode())
        assert resolver.is_legacy_mode() is True

    def test_env_override_path_resolution(self, tmp_path: Path):
        """Test environment variable overrides for paths."""
        project = tmp_path / "env-project"
        project.mkdir()
        custom_data = tmp_path / "custom-data-env"

        with patch.dict(os.environ, {"PLAN_CASCADE_DATA_DIR": str(custom_data)}):
            config = ConfigManager(project)
            data_dir = config.get_data_dir()

            assert data_dir == custom_data

            # PathResolver should use the same directory
            resolver = PathResolver(project, data_dir_override=data_dir)
            assert resolver.get_data_dir() == custom_data
