"""
Changed Files Detector for Plan Cascade

Detects changed files using git diff to support incremental quality gate checking.
"""

import hashlib
import subprocess
import sys
from pathlib import Path
from typing import Any


class ChangedFilesDetector:
    """
    Detects changed files using git diff for incremental quality gate checking.

    Uses git as the source of truth for change detection, supporting comparison
    against HEAD, specific commits, or branches.
    """

    def __init__(self, project_root: Path):
        """
        Initialize the changed files detector.

        Args:
            project_root: Root directory of the git repository
        """
        self.project_root = Path(project_root)
        self._cached_tree_hash: str | None = None
        self._cached_changed_files: list[str] | None = None

    def _run_git_command(
        self,
        args: list[str],
        timeout: int = 30,
    ) -> tuple[int, str, str]:
        """
        Run a git command and return (exit_code, stdout, stderr).

        Args:
            args: Git command arguments (without 'git' prefix)
            timeout: Command timeout in seconds

        Returns:
            Tuple of (exit_code, stdout, stderr)
        """
        command = ["git"] + args

        try:
            kwargs: dict[str, Any] = {
                "capture_output": True,
                "text": True,
                "timeout": timeout,
                "cwd": str(self.project_root),
            }

            if sys.platform == "win32":
                kwargs["creationflags"] = subprocess.CREATE_NO_WINDOW

            result = subprocess.run(command, **kwargs)
            return result.returncode, result.stdout, result.stderr

        except subprocess.TimeoutExpired:
            return -1, "", f"Git command timed out after {timeout} seconds"
        except FileNotFoundError:
            return -1, "", "Git not found in PATH"
        except Exception as e:
            return -1, "", f"Error running git command: {e}"

    def is_git_repository(self) -> bool:
        """
        Check if the project root is a git repository.

        Returns:
            True if the directory is a git repository
        """
        exit_code, _, _ = self._run_git_command(["rev-parse", "--git-dir"])
        return exit_code == 0

    def get_changed_files(
        self,
        base_ref: str = "HEAD",
        include_staged: bool = True,
        include_unstaged: bool = True,
        include_untracked: bool = False,
    ) -> list[str]:
        """
        Get list of changed files compared to a base reference.

        Args:
            base_ref: Git reference to compare against (default: HEAD)
            include_staged: Include staged changes
            include_unstaged: Include unstaged changes
            include_untracked: Include untracked files

        Returns:
            List of changed file paths relative to project root
        """
        changed_files: set[str] = set()

        if include_staged:
            # Get staged changes (diff against base_ref)
            exit_code, stdout, _ = self._run_git_command(["diff", "--name-only", "--cached", base_ref])
            if exit_code != 0 and base_ref == "HEAD":
                # Unborn HEAD (e.g., initial commit not created yet) can make
                # `git diff --cached HEAD` fail with "ambiguous argument 'HEAD'".
                # Fall back to comparing index against the empty tree.
                exit_code, stdout, _ = self._run_git_command(["diff", "--name-only", "--cached"])
            if exit_code == 0 and stdout.strip():
                changed_files.update(stdout.strip().split("\n"))

        if include_unstaged:
            # Get unstaged changes (working directory vs index)
            exit_code, stdout, _ = self._run_git_command(
                ["diff", "--name-only"]
            )
            if exit_code == 0 and stdout.strip():
                changed_files.update(stdout.strip().split("\n"))

        if include_untracked:
            # Get untracked files
            exit_code, stdout, _ = self._run_git_command(
                ["ls-files", "--others", "--exclude-standard"]
            )
            if exit_code == 0 and stdout.strip():
                changed_files.update(stdout.strip().split("\n"))

        # Filter out empty strings and normalize paths
        return sorted([f for f in changed_files if f])

    def get_changed_files_by_extension(
        self,
        extensions: list[str],
        base_ref: str = "HEAD",
        include_staged: bool = True,
        include_unstaged: bool = True,
        include_untracked: bool = False,
    ) -> list[str]:
        """
        Get changed files filtered by file extensions.

        Args:
            extensions: List of file extensions to include (e.g., [".py", ".pyi"])
            base_ref: Git reference to compare against
            include_staged: Include staged changes
            include_unstaged: Include unstaged changes
            include_untracked: Include untracked files

        Returns:
            List of changed file paths with matching extensions
        """
        all_changed = self.get_changed_files(
            base_ref=base_ref,
            include_staged=include_staged,
            include_unstaged=include_unstaged,
            include_untracked=include_untracked,
        )

        # Normalize extensions to lowercase with leading dot
        normalized_extensions = set()
        for ext in extensions:
            if not ext.startswith("."):
                ext = "." + ext
            normalized_extensions.add(ext.lower())

        # Filter by extensions
        return [
            f for f in all_changed
            if Path(f).suffix.lower() in normalized_extensions
        ]

    def compute_tree_hash(self) -> str:
        """
        Compute a hash of the current working tree state.

        This can be used for caching to detect when the working tree changes.
        Combines the HEAD commit hash with a hash of current changes.

        Returns:
            Hash string representing current tree state
        """
        # Get HEAD commit hash
        exit_code, head_hash, _ = self._run_git_command(["rev-parse", "HEAD"])
        if exit_code != 0:
            head_hash = "unknown"
        else:
            head_hash = head_hash.strip()

        # Get hash of staged changes
        exit_code, staged_diff, _ = self._run_git_command(
            ["diff", "--cached", "--stat"]
        )
        staged_hash = hashlib.md5(staged_diff.encode()).hexdigest()[:8] if exit_code == 0 else "none"

        # Get hash of unstaged changes
        exit_code, unstaged_diff, _ = self._run_git_command(["diff", "--stat"])
        unstaged_hash = hashlib.md5(unstaged_diff.encode()).hexdigest()[:8] if exit_code == 0 else "none"

        # Combine into a single hash
        combined = f"{head_hash}:{staged_hash}:{unstaged_hash}"
        return hashlib.md5(combined.encode()).hexdigest()

    def get_files_changed_since_commit(
        self,
        commit_hash: str,
    ) -> list[str]:
        """
        Get files changed since a specific commit.

        Args:
            commit_hash: The commit hash to compare against

        Returns:
            List of changed file paths
        """
        exit_code, stdout, _ = self._run_git_command(
            ["diff", "--name-only", commit_hash, "HEAD"]
        )

        if exit_code != 0 or not stdout.strip():
            return []

        return sorted([f for f in stdout.strip().split("\n") if f])

    def get_test_files_for_changes(
        self,
        changed_files: list[str],
        test_patterns: list[str] | None = None,
    ) -> list[str]:
        """
        Infer related test files for a list of changed source files.

        Uses common test file naming conventions:
        - test_<module>.py for tests/<module>.py
        - <module>_test.py
        - tests/test_<module>.py
        - tests/<module>/test_*.py

        Args:
            changed_files: List of changed source files
            test_patterns: Optional custom test file patterns

        Returns:
            List of test file paths that likely test the changed code
        """
        if test_patterns is None:
            test_patterns = ["test_*.py", "*_test.py", "tests/**/test_*.py"]

        test_files: set[str] = set()

        for changed_file in changed_files:
            path = Path(changed_file)

            # Skip if already a test file
            if path.name.startswith("test_") or path.name.endswith("_test.py"):
                test_files.add(changed_file)
                continue

            # Skip non-Python files for Python test inference
            if path.suffix != ".py":
                continue

            stem = path.stem
            parent = path.parent

            # Check common test locations
            possible_tests = [
                # test_<module>.py in same directory
                str(parent / f"test_{stem}.py"),
                # <module>_test.py in same directory
                str(parent / f"{stem}_test.py"),
                # tests/test_<module>.py
                str(Path("tests") / f"test_{stem}.py"),
                # tests/<parent>/test_<module>.py
                str(Path("tests") / parent / f"test_{stem}.py"),
            ]

            for test_path in possible_tests:
                full_path = self.project_root / test_path
                if full_path.exists():
                    test_files.add(test_path)

        return sorted(test_files)

    def filter_existing_files(self, files: list[str]) -> list[str]:
        """
        Filter a list of files to only those that exist.

        Args:
            files: List of file paths relative to project root

        Returns:
            List of existing file paths
        """
        return [
            f for f in files
            if (self.project_root / f).exists()
        ]
