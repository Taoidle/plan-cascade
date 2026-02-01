"""Tests for incremental quality gate checking."""

import subprocess
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from plan_cascade.core.changed_files import ChangedFilesDetector
from plan_cascade.core.quality_gate import (
    GateConfig,
    GateType,
    LintGate,
    ProjectType,
    QualityGate,
    TestGate,
    TypeCheckGate,
)


class TestChangedFilesDetector:
    """Tests for ChangedFilesDetector class."""

    def test_init(self, tmp_path: Path):
        """Test ChangedFilesDetector initialization."""
        detector = ChangedFilesDetector(tmp_path)
        assert detector.project_root == tmp_path

    def test_is_git_repository_true(self, tmp_path: Path):
        """Test is_git_repository returns True for git repos."""
        # Initialize a git repo
        subprocess.run(["git", "init"], cwd=tmp_path, capture_output=True)

        detector = ChangedFilesDetector(tmp_path)
        assert detector.is_git_repository() is True

    def test_is_git_repository_false(self, tmp_path: Path):
        """Test is_git_repository returns False for non-git directories."""
        detector = ChangedFilesDetector(tmp_path)
        assert detector.is_git_repository() is False

    def test_get_changed_files_empty(self, tmp_path: Path):
        """Test get_changed_files returns empty list when no changes."""
        # Initialize git repo
        subprocess.run(["git", "init"], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "config", "user.email", "test@test.com"],
            cwd=tmp_path,
            capture_output=True,
        )
        subprocess.run(
            ["git", "config", "user.name", "Test"],
            cwd=tmp_path,
            capture_output=True,
        )

        # Create and commit a file
        (tmp_path / "test.py").write_text("print('hello')")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-m", "Initial"],
            cwd=tmp_path,
            capture_output=True,
        )

        detector = ChangedFilesDetector(tmp_path)
        changed = detector.get_changed_files()
        assert changed == []

    def test_get_changed_files_unstaged(self, tmp_path: Path):
        """Test get_changed_files detects unstaged changes."""
        # Initialize git repo
        subprocess.run(["git", "init"], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "config", "user.email", "test@test.com"],
            cwd=tmp_path,
            capture_output=True,
        )
        subprocess.run(
            ["git", "config", "user.name", "Test"],
            cwd=tmp_path,
            capture_output=True,
        )

        # Create and commit a file
        (tmp_path / "test.py").write_text("print('hello')")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-m", "Initial"],
            cwd=tmp_path,
            capture_output=True,
        )

        # Modify the file (unstaged change)
        (tmp_path / "test.py").write_text("print('world')")

        detector = ChangedFilesDetector(tmp_path)
        changed = detector.get_changed_files()
        assert "test.py" in changed

    def test_get_changed_files_by_extension(self, tmp_path: Path):
        """Test get_changed_files_by_extension filters correctly."""
        # Initialize git repo
        subprocess.run(["git", "init"], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "config", "user.email", "test@test.com"],
            cwd=tmp_path,
            capture_output=True,
        )
        subprocess.run(
            ["git", "config", "user.name", "Test"],
            cwd=tmp_path,
            capture_output=True,
        )

        # Create initial commit
        (tmp_path / "README.md").write_text("# Test")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-m", "Initial"],
            cwd=tmp_path,
            capture_output=True,
        )

        # Create multiple file types and stage them (untracked files not included by default)
        (tmp_path / "script.py").write_text("print('hello')")
        (tmp_path / "app.js").write_text("console.log('hello')")
        (tmp_path / "README.md").write_text("# Updated")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)

        detector = ChangedFilesDetector(tmp_path)

        # Filter by .py extension
        py_files = detector.get_changed_files_by_extension([".py"])
        assert "script.py" in py_files
        assert "app.js" not in py_files
        assert "README.md" not in py_files

        # Filter by .js extension
        js_files = detector.get_changed_files_by_extension([".js"])
        assert "app.js" in js_files
        assert "script.py" not in js_files

    def test_compute_tree_hash(self, tmp_path: Path):
        """Test compute_tree_hash returns consistent hashes."""
        # Initialize git repo
        subprocess.run(["git", "init"], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "config", "user.email", "test@test.com"],
            cwd=tmp_path,
            capture_output=True,
        )
        subprocess.run(
            ["git", "config", "user.name", "Test"],
            cwd=tmp_path,
            capture_output=True,
        )

        (tmp_path / "test.py").write_text("print('hello')")
        subprocess.run(["git", "add", "."], cwd=tmp_path, capture_output=True)
        subprocess.run(
            ["git", "commit", "-m", "Initial"],
            cwd=tmp_path,
            capture_output=True,
        )

        detector = ChangedFilesDetector(tmp_path)

        hash1 = detector.compute_tree_hash()
        hash2 = detector.compute_tree_hash()
        assert hash1 == hash2

        # Hash should change when files change
        (tmp_path / "test.py").write_text("print('world')")
        hash3 = detector.compute_tree_hash()
        assert hash1 != hash3

    def test_get_test_files_for_changes(self, tmp_path: Path):
        """Test get_test_files_for_changes infers test files."""
        # Create test file structure
        tests_dir = tmp_path / "tests"
        tests_dir.mkdir()

        (tmp_path / "module.py").write_text("# module")
        (tests_dir / "test_module.py").write_text("# test")

        detector = ChangedFilesDetector(tmp_path)

        test_files = detector.get_test_files_for_changes(["module.py"])
        # Normalize path separators for cross-platform compatibility
        test_files_normalized = [f.replace("\\", "/") for f in test_files]
        assert "tests/test_module.py" in test_files_normalized

    def test_filter_existing_files(self, tmp_path: Path):
        """Test filter_existing_files removes non-existent files."""
        (tmp_path / "exists.py").write_text("# exists")

        detector = ChangedFilesDetector(tmp_path)

        files = ["exists.py", "missing.py"]
        filtered = detector.filter_existing_files(files)

        assert "exists.py" in filtered
        assert "missing.py" not in filtered


class TestGateConfigIncremental:
    """Tests for GateConfig incremental field."""

    def test_gate_config_incremental_default(self):
        """Test GateConfig has incremental=False by default."""
        config = GateConfig(
            name="test",
            type=GateType.TEST,
        )
        assert config.incremental is False

    def test_gate_config_incremental_true(self):
        """Test GateConfig with incremental=True."""
        config = GateConfig(
            name="test",
            type=GateType.TEST,
            incremental=True,
        )
        assert config.incremental is True

    def test_gate_config_to_dict_incremental(self):
        """Test GateConfig.to_dict() includes incremental field."""
        config = GateConfig(
            name="test",
            type=GateType.TEST,
            incremental=True,
        )
        result = config.to_dict()
        assert result["incremental"] is True

    def test_gate_config_from_dict_incremental(self):
        """Test GateConfig.from_dict() reads incremental field."""
        data = {
            "name": "test",
            "type": "test",
            "incremental": True,
        }
        config = GateConfig.from_dict(data)
        assert config.incremental is True

    def test_gate_config_from_dict_incremental_default(self):
        """Test GateConfig.from_dict() defaults incremental to False."""
        data = {
            "name": "test",
            "type": "test",
        }
        config = GateConfig.from_dict(data)
        assert config.incremental is False


class TestTypeCheckGateIncremental:
    """Tests for TypeCheckGate with incremental mode."""

    def test_typecheck_incremental_no_changes(self, tmp_path: Path):
        """Test TypeCheckGate skips when no Python files changed."""
        config = GateConfig(
            name="typecheck",
            type=GateType.TYPECHECK,
            incremental=True,
        )

        # Mock the changed files detector to return empty list
        with patch.object(
            TypeCheckGate, "_get_changed_files_for_gate", return_value=[]
        ):
            gate = TypeCheckGate(config, tmp_path)
            result = gate.execute("story-001", {"project_type": ProjectType.PYTHON})

            assert result.passed is True
            assert result.skipped is True
            assert result.checked_files == []
            assert "skipping" in result.stdout.lower()

    def test_typecheck_incremental_with_changes(self, tmp_path: Path):
        """Test TypeCheckGate runs on changed files."""
        config = GateConfig(
            name="typecheck",
            type=GateType.TYPECHECK,
            incremental=True,
            command="echo",
            args=["typecheck", "passed"],
        )

        # Mock the changed files detector
        with patch.object(
            TypeCheckGate,
            "_get_changed_files_for_gate",
            return_value=["src/module.py", "src/utils.py"],
        ):
            gate = TypeCheckGate(config, tmp_path)
            result = gate.execute("story-001", {"project_type": ProjectType.PYTHON})

            assert result.passed is True
            assert result.skipped is False
            assert result.checked_files == ["src/module.py", "src/utils.py"]

    def test_typecheck_non_incremental(self, tmp_path: Path):
        """Test TypeCheckGate runs normally when incremental=False."""
        config = GateConfig(
            name="typecheck",
            type=GateType.TYPECHECK,
            incremental=False,
            command="echo",
            args=["typecheck", "passed"],
        )

        gate = TypeCheckGate(config, tmp_path)
        result = gate.execute("story-001", {"project_type": ProjectType.PYTHON})

        assert result.passed is True
        assert result.skipped is False
        assert result.checked_files is None


class TestLintGateIncremental:
    """Tests for LintGate with incremental mode."""

    def test_lint_incremental_no_changes(self, tmp_path: Path):
        """Test LintGate skips when no lintable files changed."""
        config = GateConfig(
            name="lint",
            type=GateType.LINT,
            incremental=True,
        )

        with patch.object(LintGate, "_get_changed_files_for_gate", return_value=[]):
            gate = LintGate(config, tmp_path)
            result = gate.execute("story-001", {"project_type": ProjectType.PYTHON})

            assert result.passed is True
            assert result.skipped is True
            assert result.checked_files == []

    def test_lint_incremental_with_changes(self, tmp_path: Path):
        """Test LintGate runs on changed files."""
        config = GateConfig(
            name="lint",
            type=GateType.LINT,
            incremental=True,
            command="echo",
            args=["lint", "passed"],
        )

        with patch.object(
            LintGate,
            "_get_changed_files_for_gate",
            return_value=["src/module.py"],
        ):
            gate = LintGate(config, tmp_path)
            result = gate.execute("story-001", {"project_type": ProjectType.PYTHON})

            assert result.passed is True
            assert result.checked_files == ["src/module.py"]


class TestTestGateIncremental:
    """Tests for TestGate with incremental mode."""

    def test_test_incremental_no_changes(self, tmp_path: Path):
        """Test TestGate skips when no source files changed."""
        config = GateConfig(
            name="tests",
            type=GateType.TEST,
            incremental=True,
        )

        with patch.object(TestGate, "_get_changed_files_for_gate", return_value=[]):
            gate = TestGate(config, tmp_path)
            result = gate.execute("story-001", {"project_type": ProjectType.PYTHON})

            assert result.passed is True
            assert result.skipped is True
            assert result.checked_files == []

    def test_test_incremental_with_changes(self, tmp_path: Path):
        """Test TestGate runs related tests for changed files."""
        config = GateConfig(
            name="tests",
            type=GateType.TEST,
            incremental=True,
            command="echo",
            args=["tests", "passed"],
        )

        # Create test directory structure
        tests_dir = tmp_path / "tests"
        tests_dir.mkdir()
        (tests_dir / "test_module.py").write_text("# test")

        with patch.object(
            TestGate,
            "_get_changed_files_for_gate",
            return_value=["module.py"],
        ):
            gate = TestGate(config, tmp_path)
            result = gate.execute("story-001", {"project_type": ProjectType.PYTHON})

            assert result.passed is True
            assert result.checked_files == ["module.py"]


class TestGateOutputCheckedFiles:
    """Tests for GateOutput checked_files field."""

    def test_gate_output_checked_files_none(self, tmp_path: Path):
        """Test GateOutput.checked_files is None for non-incremental."""
        config = GateConfig(
            name="test",
            type=GateType.TEST,
            incremental=False,
            command="echo",
            args=["test"],
        )

        gate = TestGate(config, tmp_path)
        result = gate.execute("story-001", {"project_type": ProjectType.PYTHON})

        assert result.checked_files is None

    def test_gate_output_checked_files_list(self, tmp_path: Path):
        """Test GateOutput.checked_files contains file list in incremental mode."""
        config = GateConfig(
            name="test",
            type=GateType.TEST,
            incremental=True,
            command="echo",
            args=["test"],
        )

        with patch.object(
            TestGate,
            "_get_changed_files_for_gate",
            return_value=["file1.py", "file2.py"],
        ):
            gate = TestGate(config, tmp_path)
            result = gate.execute("story-001", {"project_type": ProjectType.PYTHON})

            assert result.checked_files == ["file1.py", "file2.py"]


class TestQualityGateIncrementalIntegration:
    """Integration tests for incremental quality gates."""

    def test_execute_all_with_incremental_gates(self, tmp_path: Path):
        """Test execute_all handles mixed incremental/non-incremental gates."""
        gates = [
            GateConfig(
                name="typecheck",
                type=GateType.TYPECHECK,
                incremental=True,
                command="echo",
                args=["typecheck"],
            ),
            GateConfig(
                name="lint",
                type=GateType.LINT,
                incremental=False,
                command="echo",
                args=["lint"],
            ),
        ]

        qg = QualityGate(tmp_path, gates=gates)

        # Mock incremental gate to skip
        with patch.object(
            TypeCheckGate, "_get_changed_files_for_gate", return_value=[]
        ):
            results = qg.execute_all("story-001")

            # Typecheck should be skipped (incremental, no changes)
            assert results["typecheck"].skipped is True
            assert results["typecheck"].passed is True

            # Lint should run normally (not incremental)
            assert results["lint"].skipped is False
            assert results["lint"].passed is True

    def test_create_default_does_not_enable_incremental(self, tmp_path: Path):
        """Test create_default creates gates with incremental=False."""
        (tmp_path / "pyproject.toml").write_text("[project]")

        qg = QualityGate.create_default(tmp_path)

        for gate in qg.gates:
            assert gate.incremental is False
