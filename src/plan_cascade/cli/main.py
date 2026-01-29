#!/usr/bin/env python3
"""
Plan Cascade CLI

Command-line interface for Plan Cascade with dual-mode support:
- Simple mode (default): AI-driven automatic execution
- Expert mode (--expert): Interactive PRD editing and agent selection
"""

import sys
from pathlib import Path
from typing import Optional

try:
    import typer
    from rich.console import Console
    HAS_TYPER = True
except ImportError:
    HAS_TYPER = False

if HAS_TYPER:
    app = typer.Typer(
        name="plan-cascade",
        help="Plan Cascade - A structured approach to AI-driven development",
        add_completion=False,
    )
    console = Console()

    @app.command()
    def run(
        description: str = typer.Argument(..., help="Task description"),
        expert: bool = typer.Option(False, "--expert", "-e", help="Expert mode with PRD editing"),
        backend: Optional[str] = typer.Option(None, "--backend", "-b", help="Backend selection"),
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
    ):
        """
        Execute a development task.

        Simple mode (default): AI automatically analyzes, plans, and executes.
        Expert mode (--expert): Generate PRD for review/edit before execution.
        """
        project = Path(project_path) if project_path else Path.cwd()

        console.print(f"[blue]Plan Cascade v1.0.0[/blue]")
        console.print(f"[dim]Project: {project}[/dim]")
        console.print(f"[dim]Mode: {'Expert' if expert else 'Simple'}[/dim]")
        console.print()

        if expert:
            console.print("[yellow]Expert mode: PRD editing not yet implemented[/yellow]")
            console.print(f"Task: {description}")
        else:
            console.print("[yellow]Simple mode: Auto-execution not yet implemented[/yellow]")
            console.print(f"Task: {description}")

    @app.command()
    def config(
        show: bool = typer.Option(False, "--show", help="Show current configuration"),
        setup: bool = typer.Option(False, "--setup", help="Run configuration wizard"),
    ):
        """
        Configuration management.

        Use --show to display current settings.
        Use --setup to run the configuration wizard.
        """
        if show:
            console.print("[bold]Current Configuration[/bold]")
            console.print("  Backend: claude-code (default)")
            console.print("  Mode: simple")
        elif setup:
            console.print("[yellow]Configuration wizard not yet implemented[/yellow]")
        else:
            console.print("Use --show to view configuration or --setup to run wizard")

    @app.command()
    def status():
        """
        View execution status.

        Shows the current state of any running or recent tasks.
        """
        console.print("[dim]No tasks currently in progress[/dim]")

    @app.command()
    def version():
        """
        Show version information.
        """
        from .. import __version__
        console.print(f"Plan Cascade v{__version__}")

else:
    # Fallback when typer is not installed
    app = None

    def main():
        """Fallback main function when typer is not available."""
        print("Plan Cascade CLI requires 'typer' and 'rich' packages.")
        print("Install with: pip install typer rich")
        sys.exit(1)


def main():
    """CLI entry point."""
    if HAS_TYPER:
        app()
    else:
        print("Plan Cascade CLI requires 'typer' and 'rich' packages.")
        print("Install with: pip install typer rich")
        sys.exit(1)


if __name__ == "__main__":
    main()
