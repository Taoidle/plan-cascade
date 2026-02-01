#!/usr/bin/env python3
"""
Migration Tool for Plan Cascade

Provides migration functionality to move existing planning files from
project root to user directory. Supports dry-run mode for preview,
rollback capability, and git worktree relocation.

Commands:
- plan-cascade migrate: Migrate planning files to user directory
- plan-cascade migrate --dry-run: Preview changes without migration
- plan-cascade migrate --rollback: Restore original structure
"""

import json
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING

try:
    import typer
    from rich.console import Console
    from rich.table import Table

    HAS_TYPER = True
except ImportError:
    HAS_TYPER = False

if TYPE_CHECKING:
    from .output import OutputManager


# Planning files that exist in legacy project root
PLANNING_FILES = [
    "prd.json",
    "mega-plan.json",
    ".mega-status.json",
    ".iteration-state.json",
    ".agent-status.json",
    ".retry-state.json",
    ".planning-config.json",
]

# Planning directories that exist in legacy project root
PLANNING_DIRS = [
    ".worktree",
    ".locks",
    ".state",
]

# Backup directory name
BACKUP_DIR_NAME = ".plan-cascade-backup"

# Migration state file
MIGRATION_STATE_FILE = ".migration-state.json"


@dataclass
class MigrationResult:
    """Result of a migration operation."""

    success: bool
    message: str
    files_migrated: list[str] = field(default_factory=list)
    dirs_migrated: list[str] = field(default_factory=list)
    worktrees_moved: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)


@dataclass
class DetectedFile:
    """Represents a detected planning file or directory."""

    path: Path
    is_dir: bool
    size: int = 0
    modified: str = ""


class MigrationManager:
    """
    Manages migration of planning files from project root to user directory.

    Supports:
    - Detection of existing planning files
    - Migration to user directory with unique project ID
    - Git worktree relocation
    - Backup and rollback
    - Dry-run mode for preview
    """

    def __init__(
        self,
        project_root: Path,
        data_dir_override: Path | None = None,
    ):
        """
        Initialize the migration manager.

        Args:
            project_root: Root directory of the project
            data_dir_override: Override data directory (useful for testing)
        """
        self.project_root = Path(project_root).resolve()
        self._data_dir_override = Path(data_dir_override) if data_dir_override else None

        # Lazy import to avoid circular dependencies
        from ..state.path_resolver import PathResolver
        from ..state.config_manager import ConfigManager
        from ..state.project_link import ProjectLinkManager

        self._path_resolver = PathResolver(
            project_root=self.project_root,
            legacy_mode=False,  # Use new paths
            data_dir_override=self._data_dir_override,
        )
        self._config_manager = ConfigManager(project_root=self.project_root)
        self._link_manager = ProjectLinkManager(data_dir=self._data_dir_override)

    def get_data_dir(self) -> Path:
        """Get the user data directory."""
        return self._path_resolver.get_data_dir()

    def get_project_id(self) -> str:
        """Get the unique project ID."""
        return self._path_resolver.get_project_id()

    def get_target_dir(self) -> Path:
        """Get the target project directory in user data."""
        return self._path_resolver.get_project_dir()

    def detect_existing_files(self) -> list[DetectedFile]:
        """
        Detect existing planning files in project root.

        Scans for known planning files and directories that should be migrated.

        Returns:
            List of DetectedFile objects for found files/directories
        """
        detected: list[DetectedFile] = []

        # Check for planning files
        for filename in PLANNING_FILES:
            file_path = self.project_root / filename
            if file_path.exists() and file_path.is_file():
                stat = file_path.stat()
                detected.append(DetectedFile(
                    path=file_path,
                    is_dir=False,
                    size=stat.st_size,
                    modified=time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(stat.st_mtime)),
                ))

        # Check for planning directories
        for dirname in PLANNING_DIRS:
            dir_path = self.project_root / dirname
            if dir_path.exists() and dir_path.is_dir():
                # Calculate total size
                total_size = sum(f.stat().st_size for f in dir_path.rglob("*") if f.is_file())
                stat = dir_path.stat()
                detected.append(DetectedFile(
                    path=dir_path,
                    is_dir=True,
                    size=total_size,
                    modified=time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(stat.st_mtime)),
                ))

        return detected

    def is_already_migrated(self) -> bool:
        """
        Check if project has already been migrated.

        Returns:
            True if a link file exists in project root
        """
        link_data = self._link_manager.read_link(self.project_root)
        return link_data is not None

    def migrate(self, dry_run: bool = False) -> MigrationResult:
        """
        Migrate planning files from project root to user directory.

        Args:
            dry_run: If True, only show what would be done

        Returns:
            MigrationResult with details of the migration
        """
        # Check if already migrated
        if self.is_already_migrated():
            return MigrationResult(
                success=False,
                message="Project already migrated. Use --rollback to restore original structure.",
            )

        # Detect files to migrate
        detected = self.detect_existing_files()
        if not detected:
            return MigrationResult(
                success=False,
                message="No planning files found in project root. Nothing to migrate.",
            )

        if dry_run:
            return self._dry_run_report(detected)

        # Create backup before migration
        backup_result = self._create_backup(detected)
        if not backup_result.success:
            return backup_result

        # Perform migration
        result = self._perform_migration(detected)

        if result.success:
            # Create link file in project root
            try:
                target_dir = self.get_target_dir()
                self._link_manager.create_link(
                    project_root=self.project_root,
                    project_id=self.get_project_id(),
                    data_path=target_dir,
                )
            except Exception as e:
                result.errors.append(f"Failed to create link file: {e}")

            # Write manifest to target directory
            try:
                self._path_resolver.write_manifest()
            except Exception as e:
                result.errors.append(f"Failed to write manifest: {e}")

            # Save migration state for rollback
            self._save_migration_state(detected, result)

        return result

    def _dry_run_report(self, detected: list[DetectedFile]) -> MigrationResult:
        """
        Generate a dry-run report of what would be migrated.

        Args:
            detected: List of detected files/directories

        Returns:
            MigrationResult with preview information
        """
        target_dir = self.get_target_dir()
        files_migrated = []
        dirs_migrated = []

        for item in detected:
            rel_path = item.path.name
            if item.is_dir:
                dirs_migrated.append(f"{rel_path}/ -> {target_dir / rel_path}")
            else:
                files_migrated.append(f"{rel_path} -> {target_dir / rel_path}")

        return MigrationResult(
            success=True,
            message=f"DRY RUN: Would migrate {len(files_migrated)} files and {len(dirs_migrated)} directories to {target_dir}",
            files_migrated=files_migrated,
            dirs_migrated=dirs_migrated,
        )

    def _create_backup(self, detected: list[DetectedFile]) -> MigrationResult:
        """
        Create a backup of files before migration.

        Args:
            detected: List of files to backup

        Returns:
            MigrationResult indicating backup success
        """
        backup_dir = self.project_root / BACKUP_DIR_NAME
        try:
            backup_dir.mkdir(parents=True, exist_ok=True)

            for item in detected:
                rel_path = item.path.name
                backup_path = backup_dir / rel_path

                if item.is_dir:
                    if backup_path.exists():
                        shutil.rmtree(backup_path)
                    shutil.copytree(item.path, backup_path)
                else:
                    shutil.copy2(item.path, backup_path)

            # Save backup metadata
            metadata = {
                "created_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
                "project_root": str(self.project_root),
                "files": [str(f.path) for f in detected if not f.is_dir],
                "directories": [str(f.path) for f in detected if f.is_dir],
            }
            with open(backup_dir / "backup-metadata.json", "w", encoding="utf-8") as f:
                json.dump(metadata, f, indent=2)

            return MigrationResult(
                success=True,
                message=f"Backup created at {backup_dir}",
            )

        except Exception as e:
            return MigrationResult(
                success=False,
                message=f"Failed to create backup: {e}",
                errors=[str(e)],
            )

    def _perform_migration(self, detected: list[DetectedFile]) -> MigrationResult:
        """
        Perform the actual migration.

        Args:
            detected: List of files/directories to migrate

        Returns:
            MigrationResult with migration details
        """
        target_dir = self.get_target_dir()
        files_migrated = []
        dirs_migrated = []
        worktrees_moved = []
        errors = []

        # Ensure target directories exist
        try:
            self._path_resolver.ensure_directories()
        except Exception as e:
            return MigrationResult(
                success=False,
                message=f"Failed to create target directories: {e}",
                errors=[str(e)],
            )

        # Migrate files and directories
        for item in detected:
            rel_name = item.path.name
            target_path = target_dir / rel_name

            try:
                if item.is_dir:
                    # Handle worktree directory specially
                    if rel_name == ".worktree":
                        worktree_result = self._move_worktrees(item.path, target_path)
                        if worktree_result:
                            worktrees_moved.extend(worktree_result)
                        dirs_migrated.append(str(rel_name))
                    else:
                        # Move directory
                        if target_path.exists():
                            shutil.rmtree(target_path)
                        shutil.move(str(item.path), str(target_path))
                        dirs_migrated.append(str(rel_name))
                else:
                    # Move file
                    shutil.move(str(item.path), str(target_path))
                    files_migrated.append(str(rel_name))

            except Exception as e:
                errors.append(f"Failed to migrate {rel_name}: {e}")

        success = len(errors) == 0
        if success:
            message = f"Successfully migrated {len(files_migrated)} files and {len(dirs_migrated)} directories"
        else:
            message = f"Migration completed with {len(errors)} error(s)"

        return MigrationResult(
            success=success,
            message=message,
            files_migrated=files_migrated,
            dirs_migrated=dirs_migrated,
            worktrees_moved=worktrees_moved,
            errors=errors,
        )

    def _move_worktrees(self, source_dir: Path, target_dir: Path) -> list[str]:
        """
        Move git worktrees to new location.

        Uses `git worktree move` to properly relocate worktrees.

        Args:
            source_dir: Source worktree directory (.worktree in project root)
            target_dir: Target worktree directory in user data

        Returns:
            List of moved worktree names
        """
        moved = []

        if not source_dir.exists():
            return moved

        # Ensure target directory exists
        target_dir.mkdir(parents=True, exist_ok=True)

        # Find all worktrees in the source directory
        try:
            for worktree_path in source_dir.iterdir():
                if worktree_path.is_dir():
                    new_path = target_dir / worktree_path.name

                    # Try to use git worktree move
                    try:
                        result = subprocess.run(
                            ["git", "worktree", "move", str(worktree_path), str(new_path)],
                            cwd=self.project_root,
                            capture_output=True,
                            text=True,
                            check=True,
                        )
                        moved.append(worktree_path.name)
                    except subprocess.CalledProcessError:
                        # Fallback: manual move (for non-git worktrees or testing)
                        if new_path.exists():
                            shutil.rmtree(new_path)
                        shutil.move(str(worktree_path), str(new_path))
                        moved.append(f"{worktree_path.name} (manual)")

            # Remove empty source directory
            if source_dir.exists() and not any(source_dir.iterdir()):
                source_dir.rmdir()

        except Exception as e:
            # If worktree move fails, fall back to regular move
            if target_dir.exists():
                shutil.rmtree(target_dir)
            shutil.move(str(source_dir), str(target_dir))

        return moved

    def _save_migration_state(self, detected: list[DetectedFile], result: MigrationResult) -> None:
        """
        Save migration state for rollback support.

        Args:
            detected: List of detected files that were migrated
            result: Migration result
        """
        target_dir = self.get_target_dir()
        state = {
            "migrated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "project_root": str(self.project_root),
            "target_dir": str(target_dir),
            "project_id": self.get_project_id(),
            "files_migrated": result.files_migrated,
            "dirs_migrated": result.dirs_migrated,
            "worktrees_moved": result.worktrees_moved,
            "backup_dir": str(self.project_root / BACKUP_DIR_NAME),
        }

        state_path = target_dir / MIGRATION_STATE_FILE
        with open(state_path, "w", encoding="utf-8") as f:
            json.dump(state, f, indent=2)

    def rollback(self) -> MigrationResult:
        """
        Rollback migration and restore original project root structure.

        Returns:
            MigrationResult with rollback details
        """
        # Check if project was migrated
        if not self.is_already_migrated():
            return MigrationResult(
                success=False,
                message="Project has not been migrated. Nothing to rollback.",
            )

        # Try to load migration state
        target_dir = self.get_target_dir()
        state_path = target_dir / MIGRATION_STATE_FILE

        if state_path.exists():
            try:
                with open(state_path, encoding="utf-8") as f:
                    state = json.load(f)
            except (OSError, json.JSONDecodeError):
                state = None
        else:
            state = None

        # Try to restore from backup first
        backup_dir = self.project_root / BACKUP_DIR_NAME
        if backup_dir.exists():
            return self._restore_backup(backup_dir, target_dir)

        # If no backup, try to move files back from target
        if state:
            return self._restore_from_target(state, target_dir)

        return MigrationResult(
            success=False,
            message="Cannot rollback: no backup or migration state found.",
        )

    def _restore_backup(self, backup_dir: Path, target_dir: Path) -> MigrationResult:
        """
        Restore files from backup directory.

        Args:
            backup_dir: Path to backup directory
            target_dir: Current target directory (to cleanup)

        Returns:
            MigrationResult with restoration details
        """
        restored_files = []
        restored_dirs = []
        errors = []

        try:
            # Read backup metadata
            metadata_path = backup_dir / "backup-metadata.json"
            if metadata_path.exists():
                with open(metadata_path, encoding="utf-8") as f:
                    metadata = json.load(f)
            else:
                metadata = {"files": [], "directories": []}

            # Restore files and directories
            for item in backup_dir.iterdir():
                if item.name == "backup-metadata.json":
                    continue

                target_path = self.project_root / item.name

                try:
                    if item.is_dir():
                        # Handle worktree directory
                        if item.name == ".worktree":
                            # Move worktrees back
                            self._restore_worktrees(item, target_path, target_dir / ".worktree")
                        else:
                            if target_path.exists():
                                shutil.rmtree(target_path)
                            shutil.copytree(item, target_path)
                        restored_dirs.append(item.name)
                    else:
                        shutil.copy2(item, target_path)
                        restored_files.append(item.name)
                except Exception as e:
                    errors.append(f"Failed to restore {item.name}: {e}")

            # Delete link file
            self._link_manager.delete_link(self.project_root)

            # Clean up backup directory
            try:
                shutil.rmtree(backup_dir)
            except Exception:
                pass

            # Clean up target directory
            try:
                if target_dir.exists():
                    shutil.rmtree(target_dir)
            except Exception:
                pass

            success = len(errors) == 0
            if success:
                message = f"Successfully restored {len(restored_files)} files and {len(restored_dirs)} directories"
            else:
                message = f"Rollback completed with {len(errors)} error(s)"

            return MigrationResult(
                success=success,
                message=message,
                files_migrated=restored_files,
                dirs_migrated=restored_dirs,
                errors=errors,
            )

        except Exception as e:
            return MigrationResult(
                success=False,
                message=f"Failed to restore from backup: {e}",
                errors=[str(e)],
            )

    def _restore_worktrees(self, backup_worktree: Path, target_path: Path, current_worktree: Path) -> None:
        """
        Restore worktrees from backup, handling git worktree relocation.

        Args:
            backup_worktree: Backup of original worktree directory
            target_path: Where to restore in project root
            current_worktree: Current worktree location in user data
        """
        target_path.mkdir(parents=True, exist_ok=True)

        for worktree in backup_worktree.iterdir():
            if worktree.is_dir():
                new_path = target_path / worktree.name
                current_path = current_worktree / worktree.name

                # Try to use git worktree move
                try:
                    if current_path.exists():
                        subprocess.run(
                            ["git", "worktree", "move", str(current_path), str(new_path)],
                            cwd=self.project_root,
                            capture_output=True,
                            text=True,
                            check=True,
                        )
                    else:
                        # Worktree was already removed, just copy backup
                        shutil.copytree(worktree, new_path)
                except subprocess.CalledProcessError:
                    # Fallback to manual copy
                    if new_path.exists():
                        shutil.rmtree(new_path)
                    shutil.copytree(worktree, new_path)

    def _restore_from_target(self, state: dict, target_dir: Path) -> MigrationResult:
        """
        Restore files by moving them back from target directory.

        Args:
            state: Migration state dictionary
            target_dir: Current target directory

        Returns:
            MigrationResult with restoration details
        """
        restored_files = []
        restored_dirs = []
        errors = []

        # Move files back
        for filename in state.get("files_migrated", []):
            source = target_dir / filename
            target = self.project_root / filename

            if source.exists():
                try:
                    shutil.move(str(source), str(target))
                    restored_files.append(filename)
                except Exception as e:
                    errors.append(f"Failed to restore {filename}: {e}")

        # Move directories back
        for dirname in state.get("dirs_migrated", []):
            source = target_dir / dirname
            target = self.project_root / dirname

            if source.exists():
                try:
                    # Handle worktree directory
                    if dirname == ".worktree":
                        self._restore_worktrees_from_target(source, target)
                    else:
                        shutil.move(str(source), str(target))
                    restored_dirs.append(dirname)
                except Exception as e:
                    errors.append(f"Failed to restore {dirname}: {e}")

        # Delete link file
        self._link_manager.delete_link(self.project_root)

        # Clean up target directory
        try:
            if target_dir.exists() and not any(target_dir.iterdir()):
                target_dir.rmdir()
        except Exception:
            pass

        success = len(errors) == 0
        if success:
            message = f"Successfully restored {len(restored_files)} files and {len(restored_dirs)} directories"
        else:
            message = f"Rollback completed with {len(errors)} error(s)"

        return MigrationResult(
            success=success,
            message=message,
            files_migrated=restored_files,
            dirs_migrated=restored_dirs,
            errors=errors,
        )

    def _restore_worktrees_from_target(self, source_dir: Path, target_dir: Path) -> None:
        """
        Restore worktrees from target directory back to project root.

        Args:
            source_dir: Current worktree location in user data
            target_dir: Original location in project root
        """
        target_dir.mkdir(parents=True, exist_ok=True)

        for worktree in source_dir.iterdir():
            if worktree.is_dir():
                new_path = target_dir / worktree.name

                # Try to use git worktree move
                try:
                    subprocess.run(
                        ["git", "worktree", "move", str(worktree), str(new_path)],
                        cwd=self.project_root,
                        capture_output=True,
                        text=True,
                        check=True,
                    )
                except subprocess.CalledProcessError:
                    # Fallback to manual move
                    if new_path.exists():
                        shutil.rmtree(new_path)
                    shutil.move(str(worktree), str(new_path))

        # Remove empty source directory
        if source_dir.exists() and not any(source_dir.iterdir()):
            source_dir.rmdir()


# ============================================================================
# Typer CLI Commands
# ============================================================================

if HAS_TYPER:
    migrate_app = typer.Typer(
        name="migrate",
        help="Migrate planning files from project root to user directory",
        no_args_is_help=False,
    )
    console = Console()

    def _get_output_manager() -> "OutputManager":
        """Get or create OutputManager instance."""
        from .output import OutputManager

        return OutputManager(console)

    @migrate_app.callback(invoke_without_command=True)
    def migrate_main(
        ctx: typer.Context,
        project_path: str = typer.Option(
            None,
            "--project",
            "-p",
            help="Project path (defaults to current directory)",
        ),
        dry_run: bool = typer.Option(
            False,
            "--dry-run",
            "-n",
            help="Show what would be migrated without making changes",
        ),
        rollback: bool = typer.Option(
            False,
            "--rollback",
            "-r",
            help="Rollback migration and restore original structure",
        ),
    ):
        """
        Migrate planning files from project root to user directory.

        Detects existing planning files (prd.json, mega-plan.json, .worktree/, etc.)
        and moves them to the platform-specific user directory:
        - Windows: %APPDATA%/plan-cascade/<project-id>/
        - Unix/macOS: ~/.plan-cascade/<project-id>/

        A link file (.plan-cascade-link.json) is created in the project root
        to enable quick project discovery.

        Examples:
            plan-cascade migrate                # Migrate current directory
            plan-cascade migrate --dry-run      # Preview changes
            plan-cascade migrate --rollback     # Restore original structure
            plan-cascade migrate -p /path/to/project
        """
        # Don't run if a subcommand was invoked
        if ctx.invoked_subcommand is not None:
            return

        output = _get_output_manager()
        project = Path(project_path) if project_path else Path.cwd()

        try:
            manager = MigrationManager(project)

            if rollback:
                # Rollback mode
                output.print_info(f"Rolling back migration for: {project}")
                result = manager.rollback()
            elif dry_run:
                # Dry run mode
                output.print_info(f"DRY RUN: Checking migration for: {project}")
                result = manager.migrate(dry_run=True)
            else:
                # Normal migration
                output.print_info(f"Migrating project: {project}")
                output.print(f"  [dim]Target:[/dim] {manager.get_target_dir()}")
                output.print(f"  [dim]Project ID:[/dim] {manager.get_project_id()}")
                output.print()

                # Detect and display files
                detected = manager.detect_existing_files()
                if detected:
                    _display_detected_files(output, detected)
                    output.print()

                result = manager.migrate(dry_run=False)

            # Display result
            if result.success:
                output.print_success(result.message)
            else:
                output.print_error(result.message)

            # Show details
            if result.files_migrated:
                output.print()
                output.print("[bold]Files:[/bold]")
                for f in result.files_migrated:
                    output.print(f"  - {f}")

            if result.dirs_migrated:
                output.print()
                output.print("[bold]Directories:[/bold]")
                for d in result.dirs_migrated:
                    output.print(f"  - {d}/")

            if result.worktrees_moved:
                output.print()
                output.print("[bold]Worktrees:[/bold]")
                for w in result.worktrees_moved:
                    output.print(f"  - {w}")

            if result.errors:
                output.print()
                output.print("[bold red]Errors:[/bold red]")
                for e in result.errors:
                    output.print(f"  [red]x[/red] {e}")

            if not result.success:
                raise typer.Exit(1)

        except Exception as e:
            output.print_error(f"Migration failed: {e}")
            raise typer.Exit(1)

    @migrate_app.command("status")
    def migrate_status(
        project_path: str = typer.Option(
            None,
            "--project",
            "-p",
            help="Project path (defaults to current directory)",
        ),
    ):
        """
        Show migration status for a project.

        Displays whether the project has been migrated and the current
        location of planning files.
        """
        output = _get_output_manager()
        project = Path(project_path) if project_path else Path.cwd()

        try:
            manager = MigrationManager(project)

            output.print_header("Migration Status", str(project))

            if manager.is_already_migrated():
                link_data = manager._link_manager.read_link(project)
                output.print_success("Project has been migrated")
                output.print()
                output.print(f"  [dim]Project ID:[/dim] {link_data.get('project_id', 'unknown')}")
                output.print(f"  [dim]Data Path:[/dim] {link_data.get('data_path', 'unknown')}")
                output.print(f"  [dim]Last Accessed:[/dim] {link_data.get('last_accessed', 'unknown')}")
            else:
                output.print_warning("Project has not been migrated")

                # Check for legacy files
                detected = manager.detect_existing_files()
                if detected:
                    output.print()
                    output.print("[bold]Detected planning files in project root:[/bold]")
                    _display_detected_files(output, detected)
                    output.print()
                    output.print("[dim]Run 'plan-cascade migrate' to migrate these files.[/dim]")
                else:
                    output.print()
                    output.print("[dim]No planning files found in project root.[/dim]")

        except Exception as e:
            output.print_error(f"Failed to check status: {e}")
            raise typer.Exit(1)

    def _display_detected_files(output: "OutputManager", detected: list[DetectedFile]) -> None:
        """Display detected files in a table format."""
        if not output.is_available:
            for item in detected:
                item_type = "dir" if item.is_dir else "file"
                output.print(f"  [{item_type}] {item.path.name} ({item.size} bytes, modified {item.modified})")
            return

        table = Table(show_header=True, header_style="bold")
        table.add_column("Type", style="cyan", width=6)
        table.add_column("Name", style="white")
        table.add_column("Size", justify="right")
        table.add_column("Modified", style="dim")

        for item in detected:
            item_type = "[blue]dir[/blue]" if item.is_dir else "[green]file[/green]"
            size_str = _format_size(item.size)
            table.add_row(
                item_type,
                item.path.name,
                size_str,
                item.modified,
            )

        console.print(table)

    def _format_size(size: int) -> str:
        """Format file size in human-readable form."""
        if size < 1024:
            return f"{size} B"
        elif size < 1024 * 1024:
            return f"{size / 1024:.1f} KB"
        else:
            return f"{size / (1024 * 1024):.1f} MB"

else:
    # Fallback when typer is not installed
    migrate_app = None


def main():
    """CLI entry point for migrate commands."""
    if HAS_TYPER:
        migrate_app()
    else:
        print("Migration commands require 'typer' and 'rich' packages.")
        print("Install with: pip install typer rich")
        sys.exit(1)


if __name__ == "__main__":
    main()
