#!/usr/bin/env python3
"""
CLI Context for Plan Cascade

Provides a context object to hold global CLI state including:
- legacy_mode: Whether to use legacy mode (files in project root)
- path_resolver: PathResolver instance configured for the project

This follows the Typer Context Pattern (ADR-F001) where a custom context
object is passed to all commands via ctx.obj.
"""

import logging
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ..state.path_resolver import PathResolver

logger = logging.getLogger(__name__)


@dataclass
class CLIContext:
    """
    Context object for Plan Cascade CLI.

    Holds global state that is shared across all commands.

    Attributes:
        legacy_mode: If True, PathResolver uses project root for all paths
                     (backward compatibility). If False, uses platform-specific
                     user directories (new mode). If None, auto-detect based on
                     presence of .plan-cascade-link.json file (ADR-F002).
        project_root: Root directory of the project (defaults to cwd).
        _path_resolver: Cached PathResolver instance (lazily created).
        _resolved_legacy_mode: Cached result of auto-detection (internal use).
    """

    legacy_mode: bool | None = None
    project_root: Path = field(default_factory=Path.cwd)
    _path_resolver: "PathResolver | None" = field(default=None, repr=False)
    _resolved_legacy_mode: bool | None = field(default=None, repr=False)

    def get_resolved_legacy_mode(self) -> bool:
        """
        Get the resolved legacy mode, applying auto-detection if needed.

        When legacy_mode is None (not explicitly set), auto-detect by checking
        for .plan-cascade-link.json. If found and valid, use new mode (False).
        Otherwise, default to legacy mode (True) for backward compatibility.

        Returns:
            True if legacy mode should be used, False for new mode.
        """
        if self._resolved_legacy_mode is not None:
            return self._resolved_legacy_mode

        if self.legacy_mode is not None:
            # Explicit mode set by user
            self._resolved_legacy_mode = self.legacy_mode
            return self._resolved_legacy_mode

        # Auto-detect mode based on migration link file
        from ..state.path_resolver import detect_project_mode

        detected_mode = detect_project_mode(self.project_root)

        if detected_mode == "migrated":
            logger.info(
                f"Auto-detected migrated project at {self.project_root}, "
                "using new path mode"
            )
            self._resolved_legacy_mode = False
        else:
            logger.debug(
                f"No migration detected at {self.project_root}, "
                "using legacy path mode"
            )
            self._resolved_legacy_mode = True

        return self._resolved_legacy_mode

    def get_path_resolver(self) -> "PathResolver":
        """
        Get or create a PathResolver configured with the context settings.

        If legacy_mode is None, auto-detection is performed to determine
        the appropriate mode based on the presence of .plan-cascade-link.json.

        Returns:
            PathResolver instance configured for legacy_mode and project_root.
        """
        if self._path_resolver is None:
            from ..state.path_resolver import PathResolver

            resolved_mode = self.get_resolved_legacy_mode()

            self._path_resolver = PathResolver(
                project_root=self.project_root,
                legacy_mode=resolved_mode,
            )
        return self._path_resolver

    @classmethod
    def from_options(
        cls,
        legacy_mode: bool | None = None,
        project_root: Path | None = None,
    ) -> "CLIContext":
        """
        Create a CLIContext from CLI options.

        Args:
            legacy_mode: Whether to use legacy mode for file paths.
                        If None, auto-detect based on .plan-cascade-link.json.
                        If True, force legacy mode (files in project root).
                        If False, force new mode (files in user directory).
            project_root: Project root directory (defaults to cwd).

        Returns:
            Configured CLIContext instance.
        """
        return cls(
            legacy_mode=legacy_mode,
            project_root=project_root or Path.cwd(),
        )


def get_cli_context(ctx) -> CLIContext:
    """
    Extract CLIContext from a Typer context.

    Args:
        ctx: Typer Context object (typer.Context).

    Returns:
        CLIContext from ctx.obj, or a new default CLIContext if not set.

    Example:
        @app.command()
        def my_command(ctx: typer.Context):
            cli_ctx = get_cli_context(ctx)
            resolver = cli_ctx.get_path_resolver()
    """
    if ctx.obj is not None and isinstance(ctx.obj, CLIContext):
        return ctx.obj
    # Return default context if not set
    return CLIContext()
