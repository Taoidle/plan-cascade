#!/usr/bin/env python3
"""
Path Resolver for Plan Cascade

Provides unified path resolution for all runtime files.
Computes unique project identifiers and resolves paths to
platform-specific user directories.

Follows ADR-F001 (hash-based project IDs) and ADR-F002 (platform paths):
- Windows: %APPDATA%/plan-cascade/<project-id>/
- Unix/macOS: ~/.plan-cascade/<project-id>/
"""

import hashlib
import json
import os
import re
import sys
import time
from pathlib import Path


class PathResolver:
    """
    Resolves paths for Plan Cascade runtime files.

    Provides unified path resolution with:
    - Platform-specific base directories (APPDATA on Windows, ~/.plan-cascade on Unix)
    - Hash-based project identification for unique, filesystem-safe project IDs
    - Support for legacy mode (backward compatibility with existing projects)
    - Manifest file for reverse lookup of project roots
    """

    # Subdirectory names
    WORKTREE_DIR = ".worktree"
    LOCKS_DIR = ".locks"
    STATE_DIR = ".state"

    # Standard file names
    PRD_FILE = "prd.json"
    MEGA_PLAN_FILE = "mega-plan.json"
    MEGA_STATUS_FILE = ".mega-status.json"
    MEGA_FINDINGS_FILE = "mega-findings.md"
    MANIFEST_FILE = "manifest.json"

    def __init__(
        self,
        project_root: Path,
        legacy_mode: bool = False,
        data_dir_override: Path | None = None,
    ):
        """
        Initialize the path resolver.

        Args:
            project_root: Root directory of the project
            legacy_mode: If True, use project root directly for all paths (backward compatibility)
            data_dir_override: Override the default data directory (useful for testing)
        """
        self.project_root = Path(project_root).resolve()
        self._legacy_mode = legacy_mode
        self._data_dir_override = Path(data_dir_override) if data_dir_override else None
        self._project_id: str | None = None

    def is_legacy_mode(self) -> bool:
        """
        Check if running in legacy mode.

        Returns:
            True if using legacy mode (project root for all paths)
        """
        return self._legacy_mode

    def get_data_dir(self) -> Path:
        """
        Get the platform-specific data directory for Plan Cascade.

        Returns:
            Path to the data directory:
            - Windows: %APPDATA%/plan-cascade
            - Unix/macOS: ~/.plan-cascade

        Raises:
            OSError: If the data directory cannot be determined
        """
        if self._data_dir_override:
            return self._data_dir_override

        if sys.platform == "win32":
            # Windows: Use APPDATA
            appdata = os.environ.get("APPDATA")
            if appdata:
                return Path(appdata) / "plan-cascade"
            # Fallback to user home
            return Path.home() / "AppData" / "Roaming" / "plan-cascade"
        else:
            # Unix/macOS: Use home directory
            return Path.home() / ".plan-cascade"

    def get_project_id(self, project_root: Path | None = None) -> str:
        """
        Compute a unique, filesystem-safe project ID from the project root path.

        Uses ADR-F001 format: <sanitized-name>-<hash>
        where hash is the first 8 characters of SHA-256 of the full path.

        Args:
            project_root: Optional project root path (uses instance's project_root if not provided)

        Returns:
            Filesystem-safe project identifier (e.g., "my-project-a1b2c3d4")
        """
        root = Path(project_root).resolve() if project_root else self.project_root

        # Cache the project ID for the instance's project root
        if project_root is None and self._project_id:
            return self._project_id

        # Get the directory name and sanitize it
        name = root.name
        sanitized_name = self._sanitize_name(name)

        # Compute hash of full path for uniqueness
        path_str = str(root)
        # Normalize path separators for consistent hashing across platforms
        normalized_path = path_str.replace("\\", "/").lower()
        path_hash = hashlib.sha256(normalized_path.encode("utf-8")).hexdigest()[:8]

        project_id = f"{sanitized_name}-{path_hash}"

        # Cache if using instance's project root
        if project_root is None:
            self._project_id = project_id

        return project_id

    def _sanitize_name(self, name: str) -> str:
        """
        Sanitize a name for use in filesystem paths.

        Args:
            name: Original name

        Returns:
            Sanitized name with only alphanumeric characters, hyphens, and underscores
        """
        # Replace spaces with hyphens
        sanitized = name.replace(" ", "-")
        # Remove any characters that aren't alphanumeric, hyphen, or underscore
        sanitized = re.sub(r"[^a-zA-Z0-9\-_]", "", sanitized)
        # Convert to lowercase
        sanitized = sanitized.lower()
        # Remove consecutive hyphens
        sanitized = re.sub(r"-+", "-", sanitized)
        # Remove leading/trailing hyphens
        sanitized = sanitized.strip("-")
        # Ensure non-empty
        if not sanitized:
            sanitized = "project"
        # Limit length
        if len(sanitized) > 50:
            sanitized = sanitized[:50]
        return sanitized

    def get_project_dir(self) -> Path:
        """
        Get the project-specific directory for runtime files.

        Returns:
            Path to the project directory:
            - Normal mode: ~/.plan-cascade/<project-id>/
            - Legacy mode: project_root
        """
        if self._legacy_mode:
            return self.project_root

        return self.get_data_dir() / self.get_project_id()

    def get_prd_path(self) -> Path:
        """
        Get the path to the prd.json file.

        Returns:
            Path to prd.json in the project directory
        """
        return self.get_project_dir() / self.PRD_FILE

    def get_mega_plan_path(self) -> Path:
        """
        Get the path to the mega-plan.json file.

        Returns:
            Path to mega-plan.json in the project directory
        """
        return self.get_project_dir() / self.MEGA_PLAN_FILE

    def get_mega_status_path(self) -> Path:
        """
        Get the path to the .mega-status.json file.

        Returns:
            Path to .mega-status.json in the state directory
        """
        if self._legacy_mode:
            return self.project_root / self.MEGA_STATUS_FILE
        return self.get_state_dir() / self.MEGA_STATUS_FILE

    def get_mega_findings_path(self) -> Path:
        """
        Get the path to the mega-findings.md file.

        Note: mega-findings.md is a user-visible file, so it stays in the
        project root even in new mode, similar to findings.md.

        Returns:
            Path to mega-findings.md in the project root
        """
        return self.project_root / self.MEGA_FINDINGS_FILE

    def get_worktree_dir(self) -> Path:
        """
        Get the path to the worktree directory.

        Returns:
            Path to .worktree/ in the project directory
        """
        return self.get_project_dir() / self.WORKTREE_DIR

    def get_locks_dir(self) -> Path:
        """
        Get the path to the locks directory.

        Returns:
            Path to .locks/ in the project directory
        """
        return self.get_project_dir() / self.LOCKS_DIR

    def get_state_dir(self) -> Path:
        """
        Get the path to the state directory.

        Returns:
            Path to .state/ in the project directory
        """
        return self.get_project_dir() / self.STATE_DIR

    def get_state_file_path(self, name: str) -> Path:
        """
        Get the path to a state file.

        Args:
            name: Name of the state file (e.g., "iteration-state.json")

        Returns:
            Path to the state file in the state directory
        """
        return self.get_state_dir() / name

    def get_manifest_path(self) -> Path:
        """
        Get the path to the manifest.json file.

        Returns:
            Path to manifest.json in the project directory
        """
        return self.get_project_dir() / self.MANIFEST_FILE

    def ensure_directories(self) -> None:
        """
        Create all necessary directories if they don't exist.

        Creates:
        - Project directory
        - Worktree directory
        - Locks directory
        - State directory
        """
        dirs = [
            self.get_project_dir(),
            self.get_worktree_dir(),
            self.get_locks_dir(),
            self.get_state_dir(),
        ]
        for d in dirs:
            d.mkdir(parents=True, exist_ok=True)

    def write_manifest(self) -> None:
        """
        Write the manifest file containing project root path for reverse lookup.

        The manifest includes:
        - project_root: Absolute path to the project root
        - created_at: Timestamp of manifest creation
        - project_id: The computed project ID
        """
        if self._legacy_mode:
            return  # No manifest needed in legacy mode

        self.ensure_directories()

        manifest = {
            "project_root": str(self.project_root),
            "project_id": self.get_project_id(),
            "created_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "platform": sys.platform,
        }

        manifest_path = self.get_manifest_path()
        with open(manifest_path, "w", encoding="utf-8") as f:
            json.dump(manifest, f, indent=2)

    def read_manifest(self) -> dict | None:
        """
        Read the manifest file.

        Returns:
            Manifest dictionary or None if not found
        """
        manifest_path = self.get_manifest_path()
        if not manifest_path.exists():
            return None

        try:
            with open(manifest_path, encoding="utf-8") as f:
                return json.load(f)
        except (OSError, json.JSONDecodeError):
            return None

    @classmethod
    def find_project_by_id(
        cls, project_id: str, data_dir: Path | None = None
    ) -> Path | None:
        """
        Find the project root path for a given project ID.

        Args:
            project_id: The project ID to look up
            data_dir: Optional data directory override

        Returns:
            Project root path or None if not found
        """
        resolver = cls(Path.cwd(), data_dir_override=data_dir)
        data_dir_path = resolver.get_data_dir()

        project_dir = data_dir_path / project_id
        manifest_path = project_dir / cls.MANIFEST_FILE

        if manifest_path.exists():
            try:
                with open(manifest_path, encoding="utf-8") as f:
                    manifest = json.load(f)
                    root_str = manifest.get("project_root")
                    if root_str:
                        return Path(root_str)
            except (OSError, json.JSONDecodeError):
                pass

        return None

    @classmethod
    def list_projects(cls, data_dir: Path | None = None) -> list[dict]:
        """
        List all known projects.

        Args:
            data_dir: Optional data directory override

        Returns:
            List of project info dictionaries containing:
            - project_id: The project ID
            - project_root: The project root path
            - created_at: When the project was registered
        """
        resolver = cls(Path.cwd(), data_dir_override=data_dir)
        data_dir_path = resolver.get_data_dir()

        projects = []

        if not data_dir_path.exists():
            return projects

        for project_dir in data_dir_path.iterdir():
            if project_dir.is_dir():
                manifest_path = project_dir / cls.MANIFEST_FILE
                if manifest_path.exists():
                    try:
                        with open(manifest_path, encoding="utf-8") as f:
                            manifest = json.load(f)
                            projects.append({
                                "project_id": project_dir.name,
                                "project_root": manifest.get("project_root"),
                                "created_at": manifest.get("created_at"),
                            })
                    except (OSError, json.JSONDecodeError):
                        # Include even if manifest is corrupted
                        projects.append({
                            "project_id": project_dir.name,
                            "project_root": None,
                            "created_at": None,
                        })

        return projects

    def cleanup_project_data(self) -> None:
        """
        Remove all runtime data for the current project.

        This removes the entire project directory under the data directory.
        Does nothing in legacy mode.
        """
        if self._legacy_mode:
            return

        import shutil

        project_dir = self.get_project_dir()
        if project_dir.exists():
            shutil.rmtree(project_dir)


def main():
    """CLI interface for testing path resolver."""
    if len(sys.argv) < 2:
        print("Usage: path_resolver.py <command> [args]")
        print("Commands:")
        print("  data-dir                    - Show data directory")
        print("  project-id [path]           - Compute project ID")
        print("  project-dir                 - Show project directory")
        print("  prd-path                    - Show PRD path")
        print("  mega-plan-path              - Show mega-plan path")
        print("  worktree-dir                - Show worktree directory")
        print("  locks-dir                   - Show locks directory")
        print("  state-dir                   - Show state directory")
        print("  state-file <name>           - Show state file path")
        print("  init                        - Initialize directories and manifest")
        print("  list-projects               - List all known projects")
        print("  find-project <id>           - Find project root by ID")
        sys.exit(1)

    command = sys.argv[1]
    project_root = Path.cwd()

    # Check for --legacy flag
    legacy_mode = "--legacy" in sys.argv

    resolver = PathResolver(project_root, legacy_mode=legacy_mode)

    if command == "data-dir":
        print(resolver.get_data_dir())

    elif command == "project-id":
        if len(sys.argv) >= 3 and not sys.argv[2].startswith("--"):
            path = Path(sys.argv[2])
            print(resolver.get_project_id(path))
        else:
            print(resolver.get_project_id())

    elif command == "project-dir":
        print(resolver.get_project_dir())

    elif command == "prd-path":
        print(resolver.get_prd_path())

    elif command == "mega-plan-path":
        print(resolver.get_mega_plan_path())

    elif command == "worktree-dir":
        print(resolver.get_worktree_dir())

    elif command == "locks-dir":
        print(resolver.get_locks_dir())

    elif command == "state-dir":
        print(resolver.get_state_dir())

    elif command == "state-file" and len(sys.argv) >= 3:
        name = sys.argv[2]
        print(resolver.get_state_file_path(name))

    elif command == "init":
        resolver.ensure_directories()
        resolver.write_manifest()
        print(f"Initialized project: {resolver.get_project_id()}")
        print(f"Project directory: {resolver.get_project_dir()}")

    elif command == "list-projects":
        projects = PathResolver.list_projects()
        if not projects:
            print("No projects found")
        else:
            for proj in projects:
                print(f"{proj['project_id']}: {proj['project_root']}")

    elif command == "find-project" and len(sys.argv) >= 3:
        project_id = sys.argv[2]
        root = PathResolver.find_project_by_id(project_id)
        if root:
            print(root)
        else:
            print(f"Project not found: {project_id}")
            sys.exit(1)

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
