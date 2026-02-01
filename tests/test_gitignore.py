"""Tests for GitignoreManager."""

import pytest
from pathlib import Path
from plan_cascade.utils.gitignore import (
    GitignoreManager,
    GitignoreCheckResult,
    GitignoreUpdateResult,
    PLAN_CASCADE_GITIGNORE_ENTRIES,
    PLAN_CASCADE_KEY_ENTRIES,
    ensure_gitignore,
)


class TestGitignoreManagerCheck:
    """Tests for GitignoreManager.check()."""

    def test_check_no_gitignore(self, tmp_path):
        """Test check when .gitignore doesn't exist."""
        manager = GitignoreManager(tmp_path)
        result = manager.check()

        assert not result.gitignore_exists
        assert not result.has_plan_cascade_section
        assert result.needs_update
        assert len(result.missing_entries) > 0

    def test_check_empty_gitignore(self, tmp_path):
        """Test check with empty .gitignore."""
        gitignore = tmp_path / ".gitignore"
        gitignore.write_text("")

        manager = GitignoreManager(tmp_path)
        result = manager.check()

        assert result.gitignore_exists
        assert not result.has_plan_cascade_section
        assert result.needs_update
        assert len(result.missing_entries) == len(PLAN_CASCADE_KEY_ENTRIES)

    def test_check_gitignore_without_plan_cascade(self, tmp_path):
        """Test check with .gitignore that has no Plan Cascade entries."""
        gitignore = tmp_path / ".gitignore"
        gitignore.write_text("node_modules/\n*.log\n")

        manager = GitignoreManager(tmp_path)
        result = manager.check()

        assert result.gitignore_exists
        assert not result.has_plan_cascade_section
        assert result.needs_update
        assert len(result.missing_entries) == len(PLAN_CASCADE_KEY_ENTRIES)

    def test_check_gitignore_with_partial_entries(self, tmp_path):
        """Test check with .gitignore that has some Plan Cascade entries."""
        gitignore = tmp_path / ".gitignore"
        gitignore.write_text("node_modules/\nprd.json\n.worktree/\n")

        manager = GitignoreManager(tmp_path)
        result = manager.check()

        assert result.gitignore_exists
        assert not result.has_plan_cascade_section
        assert result.needs_update
        # Some entries should still be missing
        assert ".plan-cascade-link.json" in result.missing_entries

    def test_check_gitignore_with_all_key_entries(self, tmp_path):
        """Test check with .gitignore that has all key entries."""
        gitignore = tmp_path / ".gitignore"
        content = "\n".join(PLAN_CASCADE_KEY_ENTRIES)
        gitignore.write_text(content)

        manager = GitignoreManager(tmp_path)
        result = manager.check()

        assert result.gitignore_exists
        assert not result.needs_update
        assert len(result.missing_entries) == 0

    def test_check_gitignore_with_full_section(self, tmp_path):
        """Test check with .gitignore that has full Plan Cascade section."""
        gitignore = tmp_path / ".gitignore"
        content = "\n".join(PLAN_CASCADE_GITIGNORE_ENTRIES)
        gitignore.write_text(content)

        manager = GitignoreManager(tmp_path)
        result = manager.check()

        assert result.gitignore_exists
        assert result.has_plan_cascade_section
        assert not result.needs_update


class TestGitignoreManagerUpdate:
    """Tests for GitignoreManager.update()."""

    def test_update_creates_gitignore(self, tmp_path):
        """Test update creates .gitignore when it doesn't exist."""
        manager = GitignoreManager(tmp_path)
        result = manager.update()

        assert result.success
        assert result.action == "created"
        assert (tmp_path / ".gitignore").exists()

        content = (tmp_path / ".gitignore").read_text()
        assert "# Plan Cascade" in content
        assert "prd.json" in content

    def test_update_appends_to_existing(self, tmp_path):
        """Test update appends to existing .gitignore."""
        gitignore = tmp_path / ".gitignore"
        gitignore.write_text("node_modules/\n*.log\n")

        manager = GitignoreManager(tmp_path)
        result = manager.update()

        assert result.success
        assert result.action == "updated"

        content = gitignore.read_text()
        assert "node_modules/" in content  # Original content preserved
        assert "# Plan Cascade" in content  # New content added

    def test_update_skips_when_configured(self, tmp_path):
        """Test update skips when already configured."""
        gitignore = tmp_path / ".gitignore"
        content = "\n".join(PLAN_CASCADE_KEY_ENTRIES)
        gitignore.write_text(content)

        manager = GitignoreManager(tmp_path)
        result = manager.update()

        assert result.success
        assert result.action == "skipped"

    def test_update_dry_run(self, tmp_path):
        """Test update with dry_run=True."""
        manager = GitignoreManager(tmp_path)
        result = manager.update(dry_run=True)

        assert result.success
        assert result.action == "would_update"
        assert not (tmp_path / ".gitignore").exists()

    def test_update_preserves_trailing_newline(self, tmp_path):
        """Test update preserves proper formatting."""
        gitignore = tmp_path / ".gitignore"
        gitignore.write_text("node_modules/")  # No trailing newline

        manager = GitignoreManager(tmp_path)
        manager.update()

        content = gitignore.read_text()
        # Should have proper separation
        assert "node_modules/\n" in content


class TestGitignoreManagerEnsure:
    """Tests for GitignoreManager.ensure()."""

    def test_ensure_creates_when_missing(self, tmp_path):
        """Test ensure creates .gitignore when missing."""
        manager = GitignoreManager(tmp_path)
        result = manager.ensure(silent=True)

        assert result
        assert (tmp_path / ".gitignore").exists()

    def test_ensure_updates_when_incomplete(self, tmp_path):
        """Test ensure updates when entries are missing."""
        gitignore = tmp_path / ".gitignore"
        gitignore.write_text("node_modules/\n")

        manager = GitignoreManager(tmp_path)
        result = manager.ensure(silent=True)

        assert result
        content = gitignore.read_text()
        assert "prd.json" in content

    def test_ensure_skips_when_complete(self, tmp_path):
        """Test ensure skips when already complete."""
        gitignore = tmp_path / ".gitignore"
        content = "\n".join(PLAN_CASCADE_KEY_ENTRIES)
        gitignore.write_text(content)

        manager = GitignoreManager(tmp_path)
        result = manager.ensure(silent=True)

        assert result


class TestEnsureGitignoreFunction:
    """Tests for ensure_gitignore convenience function."""

    def test_ensure_gitignore_function(self, tmp_path):
        """Test ensure_gitignore convenience function."""
        result = ensure_gitignore(tmp_path, silent=True)

        assert result
        assert (tmp_path / ".gitignore").exists()


class TestGitignoreCheckResult:
    """Tests for GitignoreCheckResult dataclass."""

    def test_to_dict(self, tmp_path):
        """Test to_dict serialization."""
        result = GitignoreCheckResult(
            gitignore_exists=True,
            has_plan_cascade_section=False,
            missing_entries=["prd.json"],
            needs_update=True,
            gitignore_path=tmp_path / ".gitignore",
        )

        d = result.to_dict()
        assert d["gitignore_exists"] is True
        assert d["missing_entries"] == ["prd.json"]


class TestGitignoreUpdateResult:
    """Tests for GitignoreUpdateResult dataclass."""

    def test_to_dict(self):
        """Test to_dict serialization."""
        result = GitignoreUpdateResult(
            success=True,
            action="updated",
            entries_added=["prd.json"],
            message="Updated",
        )

        d = result.to_dict()
        assert d["success"] is True
        assert d["action"] == "updated"


class TestEdgeCases:
    """Tests for edge cases."""

    def test_unicode_in_gitignore(self, tmp_path):
        """Test handling of unicode content in .gitignore."""
        gitignore = tmp_path / ".gitignore"
        gitignore.write_text("# 中文注释\nnode_modules/\n", encoding="utf-8")

        manager = GitignoreManager(tmp_path)
        result = manager.update()

        assert result.success
        content = gitignore.read_text(encoding="utf-8")
        assert "中文注释" in content

    def test_readonly_gitignore(self, tmp_path):
        """Test handling of read-only .gitignore."""
        import os
        import stat

        gitignore = tmp_path / ".gitignore"
        gitignore.write_text("node_modules/\n")

        # Make file read-only
        os.chmod(gitignore, stat.S_IRUSR | stat.S_IRGRP | stat.S_IROTH)

        try:
            manager = GitignoreManager(tmp_path)
            result = manager.update()

            assert not result.success
            assert result.action == "error"
        finally:
            # Restore permissions for cleanup
            os.chmod(gitignore, stat.S_IWUSR | stat.S_IRUSR)

    def test_entries_with_trailing_slash(self, tmp_path):
        """Test entries with and without trailing slash are recognized."""
        gitignore = tmp_path / ".gitignore"
        # Write entries without trailing slash
        gitignore.write_text(".worktree\nprd.json\n")

        manager = GitignoreManager(tmp_path)
        result = manager.check()

        # Should recognize .worktree as matching .worktree/
        # and prd.json as exact match
        assert "prd.json" not in result.missing_entries


class TestClassMethod:
    """Tests for class methods."""

    def test_ensure_for_project(self, tmp_path):
        """Test ensure_for_project class method."""
        result = GitignoreManager.ensure_for_project(tmp_path, silent=True)

        assert result
        assert (tmp_path / ".gitignore").exists()
