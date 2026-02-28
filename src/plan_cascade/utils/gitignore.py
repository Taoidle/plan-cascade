#!/usr/bin/env python3
"""
Gitignore Manager for Plan Cascade

Automatically checks and updates .gitignore in user projects to ensure
Plan Cascade temporary files are not committed to version control.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional

# Plan Cascade files that should be ignored in user projects
PLAN_CASCADE_GITIGNORE_ENTRIES = [
    "# Plan Cascade - AI Planning Framework",
    "# https://github.com/anthropics/plan-cascade",
    "",
    "# Runtime directories",
    ".worktree/",
    ".locks/",
    ".state/",
    "",
    "# Planning documents (regenerated, not source code)",
    "spec.json",
    "spec.md",
    "prd.json",
    "prd.md",
    "mega-plan.json",
    "mega-plan.md",
    "design_doc.json",
    "design_doc.md",
    "",
    "# Status and state files",
    ".mega-status.json",
    ".planning-config.json",
    ".agent-status.json",
    ".iteration-state.json",
    ".retry-state.json",
    "",
    "# Progress tracking files",
    "findings.md",
    "mega-findings.md",
    "progress.txt",
    "",
    "# Context recovery files",
    ".hybrid-execution-context.md",
    ".mega-execution-context.md",
    "",
    "# Project link and backup (new mode)",
    ".plan-cascade-link.json",
    ".plan-cascade-backup/",
    "",
    "# Project config",
    ".plan-cascade.json",
    "",
    "# Agent outputs",
    ".agent-outputs/",
]

# Minimal entries for quick check (key files that indicate Plan Cascade is configured)
PLAN_CASCADE_KEY_ENTRIES = [
    ".plan-cascade-link.json",
    ".worktree/",
    "prd.json",
    ".planning-config.json",
]


@dataclass
class GitignoreCheckResult:
    """Result of checking .gitignore for Plan Cascade entries."""

    gitignore_exists: bool
    has_plan_cascade_section: bool
    missing_entries: List[str]
    needs_update: bool
    gitignore_path: Path

    def to_dict(self) -> dict:
        """Convert to dictionary."""
        return {
            "gitignore_exists": self.gitignore_exists,
            "has_plan_cascade_section": self.has_plan_cascade_section,
            "missing_entries": self.missing_entries,
            "needs_update": self.needs_update,
            "gitignore_path": str(self.gitignore_path),
        }


@dataclass
class GitignoreUpdateResult:
    """Result of updating .gitignore."""

    success: bool
    action: str  # "created", "updated", "skipped"
    entries_added: List[str]
    message: str

    def to_dict(self) -> dict:
        """Convert to dictionary."""
        return {
            "success": self.success,
            "action": self.action,
            "entries_added": self.entries_added,
            "message": self.message,
        }


class GitignoreManager:
    """
    Manages .gitignore entries for Plan Cascade in user projects.

    This class provides functionality to:
    - Check if .gitignore exists and contains Plan Cascade entries
    - Add missing Plan Cascade entries to .gitignore
    - Preserve existing .gitignore content
    """

    SECTION_MARKER = "# Plan Cascade - AI Planning Framework"
    SECTION_END_MARKER = "# End Plan Cascade"

    def __init__(self, project_root: Path):
        """
        Initialize GitignoreManager.

        Args:
            project_root: Root directory of the user's project
        """
        self.project_root = Path(project_root).resolve()
        self.gitignore_path = self.project_root / ".gitignore"

    def check(self) -> GitignoreCheckResult:
        """
        Check if .gitignore contains Plan Cascade entries.

        Returns:
            GitignoreCheckResult with details about the current state
        """
        if not self.gitignore_path.exists():
            return GitignoreCheckResult(
                gitignore_exists=False,
                has_plan_cascade_section=False,
                missing_entries=PLAN_CASCADE_KEY_ENTRIES.copy(),
                needs_update=True,
                gitignore_path=self.gitignore_path,
            )

        content = self.gitignore_path.read_text(encoding="utf-8")
        lines = content.splitlines()

        # Check for Plan Cascade section marker
        has_section = self.SECTION_MARKER in content

        # Check for key entries
        missing_entries = []
        for entry in PLAN_CASCADE_KEY_ENTRIES:
            # Check if entry exists (ignoring comments and whitespace)
            entry_found = False
            for line in lines:
                line = line.strip()
                if line == entry or line == entry.rstrip("/"):
                    entry_found = True
                    break
            if not entry_found:
                missing_entries.append(entry)

        return GitignoreCheckResult(
            gitignore_exists=True,
            has_plan_cascade_section=has_section,
            missing_entries=missing_entries,
            needs_update=len(missing_entries) > 0,
            gitignore_path=self.gitignore_path,
        )

    def update(self, dry_run: bool = False) -> GitignoreUpdateResult:
        """
        Update .gitignore with Plan Cascade entries.

        Args:
            dry_run: If True, don't actually modify the file

        Returns:
            GitignoreUpdateResult with details about what was done
        """
        check_result = self.check()

        if not check_result.needs_update:
            return GitignoreUpdateResult(
                success=True,
                action="skipped",
                entries_added=[],
                message="All Plan Cascade entries already present in .gitignore",
            )

        if dry_run:
            return GitignoreUpdateResult(
                success=True,
                action="would_update",
                entries_added=check_result.missing_entries,
                message=f"Would add {len(check_result.missing_entries)} entries to .gitignore",
            )

        try:
            if not check_result.gitignore_exists:
                # Create new .gitignore with Plan Cascade entries
                content = "\n".join(PLAN_CASCADE_GITIGNORE_ENTRIES) + "\n"
                self.gitignore_path.write_text(content, encoding="utf-8")
                return GitignoreUpdateResult(
                    success=True,
                    action="created",
                    entries_added=PLAN_CASCADE_KEY_ENTRIES.copy(),
                    message="Created .gitignore with Plan Cascade entries",
                )

            # Append to existing .gitignore
            existing_content = self.gitignore_path.read_text(encoding="utf-8")

            # Ensure there's a newline at the end
            if existing_content and not existing_content.endswith("\n"):
                existing_content += "\n"

            # Add separator and Plan Cascade entries
            new_entries = "\n" + "\n".join(PLAN_CASCADE_GITIGNORE_ENTRIES) + "\n"

            self.gitignore_path.write_text(
                existing_content + new_entries,
                encoding="utf-8"
            )

            return GitignoreUpdateResult(
                success=True,
                action="updated",
                entries_added=check_result.missing_entries,
                message=f"Added {len(check_result.missing_entries)} Plan Cascade entries to .gitignore",
            )

        except Exception as e:
            return GitignoreUpdateResult(
                success=False,
                action="error",
                entries_added=[],
                message=f"Failed to update .gitignore: {e}",
            )

    def ensure(self, silent: bool = False) -> bool:
        """
        Ensure .gitignore contains Plan Cascade entries.

        This is a convenience method that checks and updates in one call.

        Args:
            silent: If True, don't print any messages

        Returns:
            True if .gitignore is properly configured (was already ok or updated successfully)
        """
        check_result = self.check()

        if not check_result.needs_update:
            return True

        update_result = self.update()

        if not silent and update_result.success:
            if update_result.action == "created":
                print(f"Created .gitignore with Plan Cascade entries")
            elif update_result.action == "updated":
                print(f"Updated .gitignore with Plan Cascade entries ({len(update_result.entries_added)} added)")

        return update_result.success

    @classmethod
    def ensure_for_project(cls, project_root: Path, silent: bool = False) -> bool:
        """
        Class method to ensure .gitignore is configured for a project.

        Args:
            project_root: Root directory of the project
            silent: If True, don't print any messages

        Returns:
            True if .gitignore is properly configured
        """
        manager = cls(project_root)
        return manager.ensure(silent=silent)


def ensure_gitignore(project_root: Path, silent: bool = False) -> bool:
    """
    Convenience function to ensure .gitignore is configured.

    Args:
        project_root: Root directory of the project
        silent: If True, don't print any messages

    Returns:
        True if .gitignore is properly configured
    """
    return GitignoreManager.ensure_for_project(project_root, silent=silent)


# CLI interface for testing
if __name__ == "__main__":
    import sys

    if len(sys.argv) < 2:
        project_path = Path.cwd()
    else:
        project_path = Path(sys.argv[1])

    manager = GitignoreManager(project_path)

    print(f"Checking .gitignore in: {project_path}")
    print()

    check_result = manager.check()
    print(f"Gitignore exists: {check_result.gitignore_exists}")
    print(f"Has Plan Cascade section: {check_result.has_plan_cascade_section}")
    print(f"Missing entries: {check_result.missing_entries}")
    print(f"Needs update: {check_result.needs_update}")
    print()

    if check_result.needs_update:
        if "--dry-run" in sys.argv:
            result = manager.update(dry_run=True)
            print(f"Dry run: {result.message}")
        elif "--update" in sys.argv:
            result = manager.update()
            print(f"Result: {result.message}")
        else:
            print("Run with --update to add missing entries, or --dry-run to preview")
