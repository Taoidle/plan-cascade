#!/usr/bin/env python3
"""
External Skills CLI Commands for Plan Cascade

Provides CLI commands for external skill management:
- skills list: Show all configured skills from external-skills.json
- skills detect: Analyze project and show matching skills
- skills show: Display SKILL.md content for a specific skill
"""

import sys
from pathlib import Path
from typing import Optional

try:
    import typer
    from rich.console import Console
    from rich.markdown import Markdown
    from rich.panel import Panel
    from rich.table import Table

    HAS_TYPER = True
except ImportError:
    HAS_TYPER = False

if HAS_TYPER:
    # Create Typer app for skills commands
    skills_app = typer.Typer(
        name="skills",
        help="External skill management",
        no_args_is_help=True,
    )
    console = Console()

    # ========== Helper Functions ==========

    def _get_project_path(project_path: Optional[str]) -> Path:
        """Get the project path from argument or cwd."""
        return Path(project_path) if project_path else Path.cwd()

    def _print_header(title: str, subtitle: str | None = None) -> None:
        """Print a styled header."""
        console.print()
        console.print(f"[bold blue]{title}[/bold blue]")
        if subtitle:
            console.print(f"[dim]{subtitle}[/dim]")
        console.print()

    def _print_success(message: str) -> None:
        """Print a success message."""
        console.print(f"[green]v[/green] {message}")

    def _print_error(message: str) -> None:
        """Print an error message."""
        console.print(f"[red]x[/red] {message}")

    def _print_warning(message: str) -> None:
        """Print a warning message."""
        console.print(f"[yellow]![/yellow] {message}")

    def _print_info(message: str) -> None:
        """Print an info message."""
        console.print(f"[blue]i[/blue] {message}")

    def _get_skill_loader(project: Path):
        """Get an ExternalSkillLoader instance."""
        from ..core.external_skill_loader import ExternalSkillLoader
        return ExternalSkillLoader(project)

    # ========== CLI Commands ==========

    @skills_app.command("list")
    def list_skills(
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
        verbose: bool = typer.Option(False, "--verbose", "-v", help="Show detailed information"),
        json_output: bool = typer.Option(False, "--json", "-j", help="Output as JSON"),
    ):
        """
        List all configured external skills.

        Shows all skills defined in external-skills.json with their source,
        detection patterns, and injection phases.

        Examples:
            plan-cascade skills list
            plan-cascade skills list --verbose
            plan-cascade skills list --json
        """
        project = _get_project_path(project_path)

        loader = _get_skill_loader(project)
        config = loader.config

        if json_output:
            import json
            console.print(json.dumps(config, indent=2))
            return

        _print_header(
            "External Skills",
            f"Project: {project}"
        )

        # Check if config is empty
        skills = config.get("skills", {})
        if not skills:
            _print_warning("No skills configured")
            _print_info("Add skills to external-skills.json to enable framework-specific guidance")
            return

        # Build skills table
        table = Table(
            title=f"Configured Skills ({len(skills)} total)",
            show_header=True,
            header_style="bold cyan"
        )
        table.add_column("Name", style="cyan", width=25)
        table.add_column("Source", style="white", width=12)
        table.add_column("Priority", style="yellow", width=10, justify="right")
        table.add_column("Inject Into", style="dim", width=20)

        if verbose:
            table.add_column("Detection", style="dim", width=30)

        for skill_name, skill_config in sorted(skills.items(), key=lambda x: -x[1].get("priority", 0)):
            source = skill_config.get("source", "unknown")
            priority = str(skill_config.get("priority", 0))
            inject_into = ", ".join(skill_config.get("inject_into", ["implementation"]))

            row = [skill_name, source, priority, inject_into]

            if verbose:
                detect = skill_config.get("detect", {})
                files = detect.get("files", [])
                patterns = detect.get("patterns", [])
                detection_str = f"files: {', '.join(files[:2])}"
                if patterns:
                    patterns_preview = patterns[:2]
                    detection_str += f" | patterns: {', '.join(patterns_preview)}"
                    if len(patterns) > 2:
                        detection_str += "..."
                row.append(detection_str[:30])

            table.add_row(*row)

        console.print(table)

        # Show sources
        sources = config.get("sources", {})
        if sources:
            console.print()
            console.print("[bold]Skill Sources:[/bold]")
            for source_name, source_config in sources.items():
                source_type = source_config.get("type", "unknown")
                source_path = source_config.get("path", "N/A")
                repo = source_config.get("repository", "")
                console.print(f"  [cyan]{source_name}[/cyan]: {source_path} ({source_type})")
                if verbose and repo:
                    console.print(f"    [dim]Repository: {repo}[/dim]")

        # Show settings
        settings = config.get("settings", {})
        if settings and verbose:
            console.print()
            console.print("[bold]Settings:[/bold]")
            console.print(f"  Max skills per story: {settings.get('max_skills_per_story', 3)}")
            console.print(f"  Max content lines: {settings.get('max_content_lines', 200)}")
            console.print(f"  Cache enabled: {settings.get('cache_enabled', True)}")

    @skills_app.command("detect")
    def detect_skills(
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
        phase: str = typer.Option("implementation", "--phase", help="Execution phase to filter by"),
        verbose: bool = typer.Option(False, "--verbose", "-v", help="Show detailed detection info"),
    ):
        """
        Detect applicable skills for the current project.

        Analyzes the project directory to find skills that match based on
        file presence and content patterns (e.g., package.json with "react").

        Examples:
            plan-cascade skills detect
            plan-cascade skills detect --phase testing
            plan-cascade skills detect --verbose
        """
        project = _get_project_path(project_path)

        _print_header(
            "Skill Detection",
            f"Project: {project}"
        )

        loader = _get_skill_loader(project)

        # Detect applicable skills
        _print_info("Analyzing project...")
        applicable = loader.detect_applicable_skills(verbose=verbose)

        if not applicable:
            _print_warning("No applicable skills detected for this project")
            console.print()
            _print_info("Skills are detected based on project files and patterns:")
            console.print("  - React/Next.js: package.json with 'react' or 'next'")
            console.print("  - Vue: package.json with 'vue' or 'nuxt'")
            console.print("  - Rust: Cargo.toml with [package] or [dependencies]")
            console.print("  - Python: pyproject.toml or requirements.txt")
            return

        # Build detection results table
        table = Table(
            title=f"Detected Skills ({len(applicable)} applicable)",
            show_header=True,
            header_style="bold cyan"
        )
        table.add_column("Skill", style="cyan", width=25)
        table.add_column("Source", style="white", width=12)
        table.add_column("Priority", style="yellow", width=10, justify="right")
        table.add_column("Phases", style="dim", width=20)
        table.add_column("Status", width=12)

        skills_config = loader.config.get("skills", {})
        max_skills = loader.config.get("settings", {}).get("max_skills_per_story", 3)

        for i, skill_name in enumerate(applicable):
            skill_config = skills_config.get(skill_name, {})
            source = skill_config.get("source", "unknown")
            priority = str(skill_config.get("priority", 0))
            inject_into = skill_config.get("inject_into", ["implementation"])
            phases_str = ", ".join(inject_into)

            # Check if skill will be used (within max limit)
            if i < max_skills:
                status = "[green]Active[/green]"
            else:
                status = "[dim]Excluded[/dim]"

            # Check if skill applies to requested phase
            if phase not in inject_into:
                status = f"[dim]Not for {phase}[/dim]"

            table.add_row(skill_name, source, priority, phases_str, status)

        console.print(table)

        # Show phase-specific skills
        console.print()
        phase_skills = loader.get_skills_for_phase(phase)

        if phase_skills:
            _print_success(f"{len(phase_skills)} skill(s) will be injected for '{phase}' phase")
            for skill in phase_skills:
                console.print(f"  [green]v[/green] {skill.name} (priority: {skill.priority})")
        else:
            _print_info(f"No skills configured for '{phase}' phase")

        # Show settings info
        console.print()
        console.print(f"[dim]Max skills per story: {max_skills}[/dim]")
        console.print(f"[dim]Current phase filter: {phase}[/dim]")

    @skills_app.command("show")
    def show_skill(
        skill_name: str = typer.Argument(..., help="Name of the skill to display"),
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
        raw: bool = typer.Option(False, "--raw", "-r", help="Show raw markdown without rendering"),
    ):
        """
        Display the content of a specific skill's SKILL.md.

        Shows the full skill documentation including guidelines, patterns,
        and best practices for the specified framework/technology.

        Examples:
            plan-cascade skills show react-best-practices
            plan-cascade skills show rust-coding-guidelines
            plan-cascade skills show vue-best-practices --raw
        """
        project = _get_project_path(project_path)

        loader = _get_skill_loader(project)

        # Check if skill exists
        skills_config = loader.config.get("skills", {})
        if skill_name not in skills_config:
            _print_error(f"Skill '{skill_name}' not found")
            console.print()
            _print_info("Available skills:")
            for name in sorted(skills_config.keys()):
                console.print(f"  - {name}")
            raise typer.Exit(1)

        # Load skill content
        _print_header(
            f"Skill: {skill_name}",
            f"Project: {project}"
        )

        try:
            loaded_skill = loader.load_skill_content(skill_name)
        except (OSError, UnicodeEncodeError) as e:
            _print_error(f"Error loading skill content: {e}")
            _print_info("Try running: git submodule update --init --recursive")
            raise typer.Exit(1)

        if not loaded_skill:
            _print_error(f"Could not load skill content for '{skill_name}'")
            _print_info("The skill's SKILL.md file may be missing.")
            _print_info("Try running: git submodule update --init --recursive")
            raise typer.Exit(1)

        # Show skill metadata
        skill_config = skills_config[skill_name]
        console.print(Panel(
            f"[bold]Source:[/bold] {loaded_skill.source}\n"
            f"[bold]Priority:[/bold] {loaded_skill.priority}\n"
            f"[bold]Phases:[/bold] {', '.join(skill_config.get('inject_into', ['implementation']))}\n"
            f"[bold]Detection:[/bold] {', '.join(skill_config.get('detect', {}).get('files', []))}",
            title="Skill Metadata",
            border_style="cyan",
        ))

        console.print()

        # Display content
        if raw:
            console.print(loaded_skill.content)
        else:
            # Render as markdown
            try:
                md = Markdown(loaded_skill.content)
                console.print(md)
            except Exception:
                # Fallback to raw if markdown rendering fails
                console.print(loaded_skill.content)

        # Show content stats
        content_lines = len(loaded_skill.content.split("\n"))
        console.print()
        console.print(f"[dim]Content: {content_lines} lines[/dim]")

    @skills_app.command("summary")
    def skill_summary(
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
        phase: str = typer.Option("implementation", "--phase", help="Execution phase"),
    ):
        """
        Show a summary of skills that will be loaded for execution.

        Displays the same summary that appears during story execution,
        showing which skills will be injected based on project detection.

        Examples:
            plan-cascade skills summary
            plan-cascade skills summary --phase testing
        """
        project = _get_project_path(project_path)

        loader = _get_skill_loader(project)

        _print_header(
            "Skills Summary",
            f"Project: {project} | Phase: {phase}"
        )

        # Use the built-in summary display
        loader.display_skills_summary(phase)

        # Also show the formatted context that will be injected
        console.print()
        context = loader.get_skill_context(phase)

        if context:
            console.print("[bold]Prompt Context Preview:[/bold]")
            console.print()

            # Show truncated preview
            preview_lines = context.split("\n")[:20]
            preview = "\n".join(preview_lines)
            if len(context.split("\n")) > 20:
                preview += f"\n\n[dim]... ({len(context.split(chr(10))) - 20} more lines)[/dim]"

            console.print(Panel(
                preview,
                title="Injected Context",
                border_style="dim",
            ))
        else:
            _print_info("No skill context will be injected for this phase")

    @skills_app.command("validate")
    def validate_skills(
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
    ):
        """
        Validate external skills configuration and availability.

        Checks that all configured skills have valid paths, source repositories
        are initialized, and SKILL.md files exist.

        Examples:
            plan-cascade skills validate
        """
        project = _get_project_path(project_path)

        _print_header(
            "Skills Validation",
            f"Project: {project}"
        )

        loader = _get_skill_loader(project)
        config = loader.config

        issues = []
        validated = 0

        # Check config exists
        config_path = loader.plugin_root / "external-skills.json"
        if not config_path.exists():
            _print_error(f"Configuration file not found: {config_path}")
            raise typer.Exit(1)

        _print_success(f"Configuration file: {config_path}")

        # Validate sources
        console.print()
        console.print("[bold]Validating Sources:[/bold]")

        sources = config.get("sources", {})
        for source_name, source_config in sources.items():
            source_path = loader.plugin_root / source_config.get("path", "")

            if source_path.exists():
                _print_success(f"  {source_name}: {source_path}")
                validated += 1
            else:
                _print_error(f"  {source_name}: {source_path} (not found)")
                issues.append(f"Source '{source_name}' path not found: {source_path}")

        # Validate skills
        console.print()
        console.print("[bold]Validating Skills:[/bold]")

        skills = config.get("skills", {})
        for skill_name, skill_config in skills.items():
            source_name = skill_config.get("source")
            source_config = sources.get(source_name, {})

            if not source_config:
                _print_error(f"  {skill_name}: Unknown source '{source_name}'")
                issues.append(f"Skill '{skill_name}' has unknown source: {source_name}")
                continue

            # Check SKILL.md exists
            skill_md_path = (
                loader.plugin_root
                / source_config.get("path", "")
                / skill_config.get("skill_path", "")
                / "SKILL.md"
            )

            if skill_md_path.exists():
                _print_success(f"  {skill_name}: SKILL.md found")
                validated += 1
            else:
                _print_warning(f"  {skill_name}: SKILL.md not found at {skill_md_path}")
                issues.append(f"Skill '{skill_name}' missing SKILL.md: {skill_md_path}")

        # Summary
        console.print()
        if issues:
            _print_warning(f"Validation completed with {len(issues)} issue(s)")
            console.print()
            console.print("[bold]Issues:[/bold]")
            for issue in issues:
                console.print(f"  [red]-[/red] {issue}")
            console.print()
            _print_info("Run 'git submodule update --init --recursive' to initialize submodules")
        else:
            _print_success(f"All {validated} items validated successfully")

else:
    # Fallback when typer is not installed
    skills_app = None


def main():
    """CLI entry point for skills commands."""
    if HAS_TYPER:
        skills_app()
    else:
        print("Skills CLI requires 'typer' and 'rich' packages.")
        print("Install with: pip install typer rich")
        sys.exit(1)


if __name__ == "__main__":
    main()
