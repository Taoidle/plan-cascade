#!/usr/bin/env python3
"""
Project Link Manager for Plan Cascade

Provides lightweight link files in project root for project discovery and quick access.
Link files contain project metadata and enable scanning for active projects without
traversing user directories.

Follows ADR-F004 (link file for project discovery):
- Link file: .plan-cascade-link.json in project root
- Contains: project_id, data_path, timestamps
- Enables project discovery from any directory
- Supports orphan detection and cleanup
"""

import json
import os
import sys
import time
from pathlib import Path


# Constants
LINK_FILE_NAME = ".plan-cascade-link.json"


class ProjectLinkManager:
    """
    Manages project link files for project discovery and access.

    Link files are small JSON files placed in project roots that point to
    the centralized data directory. This enables:
    - Quick project discovery by scanning for link files
    - Project access without knowing the project ID
    - Orphan detection when data directories are deleted
    - Last-accessed tracking for project lifecycle management
    """

    def __init__(self, data_dir: Path | None = None):
        """
        Initialize the ProjectLinkManager.

        Args:
            data_dir: Optional data directory override. If not provided,
                     uses platform-specific default (~/.plan-cascade or %APPDATA%/plan-cascade)
        """
        self._data_dir_override = Path(data_dir) if data_dir else None

    def get_data_dir(self) -> Path:
        """
        Get the platform-specific data directory for Plan Cascade.

        Returns:
            Path to the data directory:
            - Windows: %APPDATA%/plan-cascade
            - Unix/macOS: ~/.plan-cascade
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

    def get_link_path(self, project_root: Path) -> Path:
        """
        Get the path to the link file for a project.

        Args:
            project_root: Root directory of the project

        Returns:
            Path to the .plan-cascade-link.json file
        """
        return Path(project_root).resolve() / LINK_FILE_NAME

    def create_link(
        self,
        project_root: Path,
        project_id: str,
        data_path: Path,
    ) -> None:
        """
        Create or update a link file in the project root.

        Args:
            project_root: Root directory of the project
            project_id: Unique project identifier
            data_path: Full path to the project's data directory

        Raises:
            OSError: If the link file cannot be written
        """
        project_root = Path(project_root).resolve()
        data_path = Path(data_path).resolve()
        link_path = self.get_link_path(project_root)

        now = time.strftime("%Y-%m-%dT%H:%M:%SZ")

        # Check if existing link file and preserve created_at
        existing = self.read_link(project_root)
        created_at = existing.get("created_at", now) if existing else now

        link_data = {
            "project_id": project_id,
            "data_path": str(data_path),
            "created_at": created_at,
            "last_accessed": now,
            "platform": sys.platform,
        }

        with open(link_path, "w", encoding="utf-8") as f:
            json.dump(link_data, f, indent=2)

    def read_link(self, project_root: Path) -> dict | None:
        """
        Read the link file from a project root.

        Args:
            project_root: Root directory of the project

        Returns:
            Link file contents as dictionary, or None if not found or invalid
        """
        link_path = self.get_link_path(project_root)

        if not link_path.exists():
            return None

        try:
            with open(link_path, encoding="utf-8") as f:
                return json.load(f)
        except (OSError, json.JSONDecodeError):
            return None

    def update_last_accessed(self, project_root: Path) -> bool:
        """
        Update the last_accessed timestamp in the link file.

        Args:
            project_root: Root directory of the project

        Returns:
            True if updated successfully, False if link file doesn't exist
        """
        existing = self.read_link(project_root)
        if not existing:
            return False

        self.create_link(
            project_root=project_root,
            project_id=existing["project_id"],
            data_path=Path(existing["data_path"]),
        )
        return True

    def delete_link(self, project_root: Path) -> bool:
        """
        Delete the link file from a project root.

        Args:
            project_root: Root directory of the project

        Returns:
            True if deleted successfully, False if not found
        """
        link_path = self.get_link_path(project_root)

        if not link_path.exists():
            return False

        try:
            link_path.unlink()
            return True
        except OSError:
            return False

    def is_orphaned(self, project_root: Path) -> bool:
        """
        Check if a link file is orphaned (data directory no longer exists).

        Args:
            project_root: Root directory of the project

        Returns:
            True if the link file exists but the data directory doesn't,
            False otherwise (including if link file doesn't exist)
        """
        link_data = self.read_link(project_root)
        if not link_data:
            return False

        data_path = Path(link_data.get("data_path", ""))
        return not data_path.exists()

    def discover_projects(
        self,
        search_paths: list[Path] | None = None,
        max_depth: int = 3,
    ) -> list[dict]:
        """
        Discover projects by scanning for link files.

        Args:
            search_paths: List of directories to search. If None, uses common locations:
                         - Current directory and parents
                         - Home directory subdirectories
            max_depth: Maximum directory depth to search (default: 3)

        Returns:
            List of project info dictionaries containing:
            - project_root: Path to the project root
            - project_id: The project ID
            - data_path: Path to the data directory
            - created_at: When the project was created
            - last_accessed: When the project was last accessed
            - is_orphaned: Whether the data directory exists
        """
        projects = []
        seen_roots: set[str] = set()

        if search_paths is None:
            search_paths = self._get_default_search_paths()

        for search_path in search_paths:
            search_path = Path(search_path).resolve()
            if not search_path.exists():
                continue

            # Search recursively up to max_depth
            self._scan_directory(search_path, max_depth, projects, seen_roots)

        return projects

    def discover_from_data_dir(self) -> list[dict]:
        """
        Discover all projects by scanning the data directory manifests
        and checking for corresponding link files.

        This provides a complete view by combining:
        - Projects with link files in their roots
        - Projects with manifests in the data directory

        Returns:
            List of project info dictionaries
        """
        projects = []
        data_dir = self.get_data_dir()

        if not data_dir.exists():
            return projects

        for project_dir in data_dir.iterdir():
            if not project_dir.is_dir():
                continue

            manifest_path = project_dir / "manifest.json"
            if not manifest_path.exists():
                continue

            try:
                with open(manifest_path, encoding="utf-8") as f:
                    manifest = json.load(f)

                project_root = Path(manifest.get("project_root", ""))
                project_id = manifest.get("project_id", project_dir.name)

                # Check for link file
                link_data = self.read_link(project_root) if project_root.exists() else None

                projects.append({
                    "project_root": str(project_root),
                    "project_id": project_id,
                    "data_path": str(project_dir),
                    "created_at": manifest.get("created_at"),
                    "last_accessed": link_data.get("last_accessed") if link_data else None,
                    "has_link_file": link_data is not None,
                    "project_root_exists": project_root.exists(),
                })

            except (OSError, json.JSONDecodeError):
                # Include even if manifest is corrupted
                projects.append({
                    "project_root": None,
                    "project_id": project_dir.name,
                    "data_path": str(project_dir),
                    "created_at": None,
                    "last_accessed": None,
                    "has_link_file": False,
                    "project_root_exists": False,
                })

        return projects

    def cleanup_orphans(
        self,
        search_paths: list[Path] | None = None,
        max_depth: int = 3,
        dry_run: bool = False,
    ) -> list[Path]:
        """
        Find and remove orphaned link files (where data directory no longer exists).

        Args:
            search_paths: List of directories to search for link files
            max_depth: Maximum directory depth to search
            dry_run: If True, only return orphaned paths without deleting

        Returns:
            List of paths to orphaned/deleted link files
        """
        orphaned: list[Path] = []

        if search_paths is None:
            search_paths = self._get_default_search_paths()

        for search_path in search_paths:
            search_path = Path(search_path).resolve()
            if not search_path.exists():
                continue

            self._find_orphans(search_path, max_depth, orphaned)

        # Delete orphaned link files unless dry_run
        if not dry_run:
            for link_path in orphaned:
                try:
                    link_path.unlink()
                except OSError:
                    pass  # Ignore deletion errors

        return orphaned

    def _get_default_search_paths(self) -> list[Path]:
        """
        Get default paths to search for link files.

        Returns:
            List of directories to search
        """
        paths = []

        # Current working directory
        cwd = Path.cwd()
        paths.append(cwd)

        # Parent directories (up to 5 levels)
        parent = cwd
        for _ in range(5):
            parent = parent.parent
            if parent != parent.parent:  # Not at root
                paths.append(parent)
            else:
                break

        # Home directory common project locations
        home = Path.home()
        common_dirs = [
            home / "projects",
            home / "Projects",
            home / "code",
            home / "Code",
            home / "dev",
            home / "Development",
            home / "work",
            home / "Work",
            home / "repos",
            home / "src",
        ]
        paths.extend([d for d in common_dirs if d.exists()])

        return paths

    def _scan_directory(
        self,
        directory: Path,
        depth: int,
        projects: list[dict],
        seen_roots: set[str],
    ) -> None:
        """
        Recursively scan a directory for link files.

        Args:
            directory: Directory to scan
            depth: Remaining depth to search
            projects: List to append found projects
            seen_roots: Set of already-seen project roots (to avoid duplicates)
        """
        if depth < 0:
            return

        # Check for link file in this directory
        link_path = directory / LINK_FILE_NAME
        if link_path.exists():
            root_str = str(directory)
            if root_str not in seen_roots:
                seen_roots.add(root_str)
                link_data = self.read_link(directory)
                if link_data:
                    data_path = Path(link_data.get("data_path", ""))
                    projects.append({
                        "project_root": root_str,
                        "project_id": link_data.get("project_id"),
                        "data_path": str(data_path),
                        "created_at": link_data.get("created_at"),
                        "last_accessed": link_data.get("last_accessed"),
                        "is_orphaned": not data_path.exists(),
                    })

        # Recurse into subdirectories
        if depth > 0:
            try:
                for child in directory.iterdir():
                    if child.is_dir() and not child.name.startswith("."):
                        self._scan_directory(child, depth - 1, projects, seen_roots)
            except PermissionError:
                pass  # Skip directories we can't access

    def _find_orphans(
        self,
        directory: Path,
        depth: int,
        orphaned: list[Path],
    ) -> None:
        """
        Recursively find orphaned link files.

        Args:
            directory: Directory to scan
            depth: Remaining depth to search
            orphaned: List to append orphaned link paths
        """
        if depth < 0:
            return

        # Check for orphaned link file
        if self.is_orphaned(directory):
            orphaned.append(self.get_link_path(directory))

        # Recurse into subdirectories
        if depth > 0:
            try:
                for child in directory.iterdir():
                    if child.is_dir() and not child.name.startswith("."):
                        self._find_orphans(child, depth - 1, orphaned)
            except PermissionError:
                pass


def main():
    """CLI interface for testing project link manager."""
    import argparse

    parser = argparse.ArgumentParser(description="Project Link Manager CLI")
    subparsers = parser.add_subparsers(dest="command", help="Command to run")

    # create command
    create_parser = subparsers.add_parser("create", help="Create a link file")
    create_parser.add_argument("project_root", type=Path, help="Project root directory")
    create_parser.add_argument("project_id", help="Project ID")
    create_parser.add_argument("data_path", type=Path, help="Data directory path")

    # read command
    read_parser = subparsers.add_parser("read", help="Read a link file")
    read_parser.add_argument(
        "project_root", type=Path, nargs="?", default=Path.cwd(), help="Project root directory"
    )

    # discover command
    discover_parser = subparsers.add_parser("discover", help="Discover projects")
    discover_parser.add_argument("--from-data-dir", action="store_true", help="Scan data directory")

    # cleanup command
    cleanup_parser = subparsers.add_parser("cleanup", help="Cleanup orphaned link files")
    cleanup_parser.add_argument("--dry-run", action="store_true", help="Don't actually delete")

    args = parser.parse_args()

    manager = ProjectLinkManager()

    if args.command == "create":
        manager.create_link(args.project_root, args.project_id, args.data_path)
        print(f"Created link file at {manager.get_link_path(args.project_root)}")

    elif args.command == "read":
        link_data = manager.read_link(args.project_root)
        if link_data:
            print(json.dumps(link_data, indent=2))
        else:
            print("No link file found")
            sys.exit(1)

    elif args.command == "discover":
        if args.from_data_dir:
            projects = manager.discover_from_data_dir()
        else:
            projects = manager.discover_projects()

        if not projects:
            print("No projects found")
        else:
            for proj in projects:
                print(f"{proj['project_id']}: {proj['project_root']}")
                print(f"  Data: {proj['data_path']}")
                if proj.get("is_orphaned"):
                    print("  [ORPHANED]")

    elif args.command == "cleanup":
        orphaned = manager.cleanup_orphans(dry_run=args.dry_run)
        if not orphaned:
            print("No orphaned link files found")
        else:
            action = "Would delete" if args.dry_run else "Deleted"
            for path in orphaned:
                print(f"{action}: {path}")

    else:
        parser.print_help()


if __name__ == "__main__":
    main()
