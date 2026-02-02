#!/usr/bin/env python3
"""
External Skills CLI Commands for Plan Cascade

Provides CLI commands for external skill management:
- skills list: Show all configured skills with source type and priority
- skills detect: Analyze project and show matching skills with overrides
- skills show: Display SKILL.md content for a specific skill
- skills add: Add a user-defined skill
- skills remove: Remove a user-defined skill
- skills validate: Validate all skill configurations
- skills refresh: Refresh cached remote skills
"""

import subprocess
import sys
from pathlib import Path
from typing import NamedTuple

try:
    import typer
    from rich.console import Console
    from rich.markdown import Markdown
    from rich.panel import Panel
    from rich.table import Table
    from rich.tree import Tree

    HAS_TYPER = True
except ImportError:
    HAS_TYPER = False


class VersionInfo(NamedTuple):
    """Version information for a skill."""

    current: str | None  # Currently installed version
    available: str | None  # Latest available version
    status: str  # "up-to-date", "update-available", "unknown"
    update_command: str | None  # Command to update if outdated

if HAS_TYPER:
    # Create Typer app for skills commands
    skills_app = typer.Typer(
        name="skills",
        help="External skill management",
        no_args_is_help=True,
    )
    console = Console()

    # ========== Helper Functions ==========

    def _get_project_path(project_path: str | None) -> Path:
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

    def _get_skill_loader(project: Path, verbose: bool = False):
        """Get an ExternalSkillLoader instance."""
        from ..core.external_skill_loader import ExternalSkillLoader
        return ExternalSkillLoader(project, verbose=verbose)

    def _get_user_skill_config(project: Path, verbose: bool = False):
        """Get a UserSkillConfig instance."""
        from ..core.user_skill_config import UserSkillConfig
        return UserSkillConfig(project, verbose=verbose)

    def _get_type_badge(source_type: str) -> str:
        """Get a styled badge for source type."""
        badges = {
            "builtin": "[blue]builtin[/blue]",
            "submodule": "[cyan]submodule[/cyan]",
            "user": "[green]user[/green]",
        }
        return badges.get(source_type, f"[dim]{source_type}[/dim]")

    def _get_priority_range_display(source_type: str) -> str:
        """Get priority range display for source type."""
        ranges = {
            "builtin": "1-50",
            "submodule": "51-100",
            "user": "101-200",
        }
        return ranges.get(source_type, "unknown")

    def _get_submodule_current_version(submodule_path: Path) -> str | None:
        """Get the current version (commit hash or tag) of a Git submodule.

        Args:
            submodule_path: Path to the submodule directory

        Returns:
            Version string (tag or short commit hash) or None if not available
        """
        if not submodule_path.exists():
            return None

        try:
            # First try to get a tag pointing to the current commit
            result = subprocess.run(
                ["git", "describe", "--tags", "--exact-match", "HEAD"],
                cwd=str(submodule_path),
                capture_output=True,
                text=True,
                timeout=10,
            )
            if result.returncode == 0 and result.stdout.strip():
                return result.stdout.strip()

            # Fall back to short commit hash
            result = subprocess.run(
                ["git", "rev-parse", "--short", "HEAD"],
                cwd=str(submodule_path),
                capture_output=True,
                text=True,
                timeout=10,
            )
            if result.returncode == 0 and result.stdout.strip():
                return result.stdout.strip()
        except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
            pass

        return None

    def _get_submodule_latest_version(submodule_path: Path) -> str | None:
        """Get the latest available version (tag or commit) from remote.

        Args:
            submodule_path: Path to the submodule directory

        Returns:
            Latest version string or None if not available
        """
        if not submodule_path.exists():
            return None

        try:
            # Fetch remote refs without updating
            result = subprocess.run(
                ["git", "ls-remote", "--tags", "--refs", "origin"],
                cwd=str(submodule_path),
                capture_output=True,
                text=True,
                timeout=30,
            )
            if result.returncode == 0 and result.stdout.strip():
                # Parse tags and find the latest semver-like tag
                tags = []
                for line in result.stdout.strip().split("\n"):
                    if "\t" in line:
                        _, ref = line.split("\t", 1)
                        tag = ref.replace("refs/tags/", "")
                        tags.append(tag)

                if tags:
                    # Sort tags, preferring semver-like versions
                    def version_key(tag: str) -> tuple:
                        # Remove 'v' prefix for sorting
                        clean = tag.lstrip("v")
                        parts = []
                        for part in clean.split("."):
                            try:
                                parts.append((0, int(part)))
                            except ValueError:
                                parts.append((1, part))
                        return parts

                    tags.sort(key=version_key, reverse=True)
                    return tags[0]

            # If no tags, get latest commit on default branch
            result = subprocess.run(
                ["git", "ls-remote", "origin", "HEAD"],
                cwd=str(submodule_path),
                capture_output=True,
                text=True,
                timeout=30,
            )
            if result.returncode == 0 and result.stdout.strip():
                commit_hash = result.stdout.strip().split()[0]
                return commit_hash[:7]  # Short hash

        except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
            pass

        return None

    def _get_skill_version_info(
        skill_name: str,
        skill_config: dict,
        sources: dict,
        plugin_root: Path,
        check_remote: bool = False,
    ) -> VersionInfo:
        """Get version information for a skill.

        Args:
            skill_name: Name of the skill
            skill_config: Skill configuration dict
            sources: Sources configuration dict
            plugin_root: Path to the plugin root
            check_remote: Whether to check remote for latest version

        Returns:
            VersionInfo with current/available versions and status
        """
        source_name = skill_config.get("source", "")
        source_config = sources.get(source_name, {})
        source_type = source_config.get("type", "submodule")

        current_version: str | None = None
        available_version: str | None = None
        update_command: str | None = None

        if source_type == "builtin":
            # Builtin skills use the Plan Cascade version
            current_version = "bundled"
            available_version = "bundled"
            return VersionInfo(
                current=current_version,
                available=available_version,
                status="up-to-date",
                update_command=None,
            )

        elif source_type == "submodule":
            source_path = source_config.get("path", "")
            submodule_path = plugin_root / source_path

            current_version = _get_submodule_current_version(submodule_path)

            if check_remote:
                available_version = _get_submodule_latest_version(submodule_path)
            else:
                available_version = current_version  # Assume up-to-date if not checking

            update_command = "git submodule update --remote --merge"

        elif source_type == "user":
            # User skills may have version in config or manifest
            user_version = skill_config.get("version")
            if user_version:
                current_version = user_version
                available_version = user_version  # Can't check remote for user skills
            else:
                current_version = "custom"
                available_version = "custom"

            return VersionInfo(
                current=current_version,
                available=available_version,
                status="up-to-date",
                update_command=None,
            )

        # Determine status
        if current_version is None:
            status = "unknown"
        elif available_version is None:
            status = "unknown"
        elif current_version == available_version:
            status = "up-to-date"
        else:
            # Compare versions - check if available is newer
            status = "update-available"

        return VersionInfo(
            current=current_version,
            available=available_version,
            status=status,
            update_command=update_command if status == "update-available" else None,
        )

    def _get_version_badge(status: str) -> str:
        """Get a styled badge for version status.

        Args:
            status: Version status string

        Returns:
            Rich-formatted status badge
        """
        badges = {
            "up-to-date": "[green]UP-TO-DATE[/green]",
            "update-available": "[yellow]UPDATE AVAILABLE[/yellow]",
            "unknown": "[dim]UNKNOWN[/dim]",
        }
        return badges.get(status, f"[dim]{status}[/dim]")

    # ========== CLI Commands ==========

    @skills_app.command("list")
    def list_skills(
        project_path: str | None = typer.Option(None, "--project", "-p", help="Project path"),
        verbose: bool = typer.Option(False, "--verbose", "-v", help="Show detailed information"),
        json_output: bool = typer.Option(False, "--json", "-j", help="Output as JSON"),
        group_by_type: bool = typer.Option(False, "--group", "-g", help="Group skills by source type"),
        check_updates: bool = typer.Option(False, "--check-updates", "-u", help="Check for available updates (requires network)"),
    ):
        """
        List all configured external skills.

        Shows all skills from three source types:
        - builtin: Built-in skills (priority 1-50)
        - submodule: External Git submodule skills (priority 51-100)
        - user: User-defined skills (priority 101-200)

        Examples:
            plan-cascade skills list
            plan-cascade skills list --group
            plan-cascade skills list --verbose
            plan-cascade skills list --json
            plan-cascade skills list --check-updates
        """
        project = _get_project_path(project_path)

        loader = _get_skill_loader(project, verbose=verbose)
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
            _print_info("Add skills to external-skills.json or .plan-cascade/skills.json")
            return

        # Get skills grouped by type
        skills_by_type = loader.get_skills_by_type()
        sources = config.get("sources", {})

        # Track outdated skills for update suggestions
        outdated_skills: list[tuple[str, str]] = []  # (skill_name, update_command)

        if check_updates:
            _print_info("Checking for updates (this may take a moment)...")
            console.print()

        if group_by_type:
            # Display skills grouped by source type using Tree
            tree = Tree("[bold]Skills by Source Type[/bold]")

            type_order = ["user", "submodule", "builtin"]  # Highest to lowest priority
            for source_type in type_order:
                skill_names = skills_by_type.get(source_type, [])
                if not skill_names:
                    continue

                type_badge = _get_type_badge(source_type)
                priority_range = _get_priority_range_display(source_type)
                type_branch = tree.add(
                    f"{type_badge} ({len(skill_names)} skills, priority {priority_range})"
                )

                # Sort skills by priority within type
                skill_list = []
                for name in skill_names:
                    cfg = skills.get(name, {})
                    skill_list.append((name, cfg.get("priority", 0)))
                skill_list.sort(key=lambda x: -x[1])

                for skill_name, priority in skill_list:
                    skill_cfg = skills.get(skill_name, {})
                    inject_into = ", ".join(skill_cfg.get("inject_into", ["implementation"]))

                    # Get version info if checking updates
                    version_str = ""
                    if check_updates or verbose:
                        version_info = _get_skill_version_info(
                            skill_name, skill_cfg, sources, loader.plugin_root, check_remote=check_updates
                        )
                        if version_info.current:
                            version_str = f" | v{version_info.current}"
                            if check_updates and version_info.status == "update-available":
                                version_str += f" -> {version_info.available}"
                                version_str += f" {_get_version_badge(version_info.status)}"
                                if version_info.update_command:
                                    outdated_skills.append((skill_name, version_info.update_command))

                    type_branch.add(f"[cyan]{skill_name}[/cyan] (priority: {priority}, phases: {inject_into}{version_str})")

            console.print(tree)
            console.print()

        else:
            # Build skills table with Type column
            table = Table(
                title=f"Configured Skills ({len(skills)} total)",
                show_header=True,
                header_style="bold cyan"
            )
            table.add_column("Name", style="cyan", width=25)
            table.add_column("Type", width=10)
            table.add_column("Source", style="white", width=15)
            table.add_column("Priority", style="yellow", width=10, justify="right")
            table.add_column("Inject Into", style="dim", width=18)

            if check_updates or verbose:
                table.add_column("Version", width=12)
                table.add_column("Status", width=18)

            if verbose and not check_updates:
                table.add_column("Detection", style="dim", width=25)

            # Get all skills with their info
            all_skills_info = loader.list_all_skills()

            for skill_info in all_skills_info:
                skill_name = skill_info["name"]
                source_type = skill_info["source_type"]
                source = skill_info["source"]
                priority = str(skill_info["priority"])
                inject_into = ", ".join(skill_info.get("inject_into", ["implementation"]))

                type_badge = _get_type_badge(source_type)

                row = [skill_name, type_badge, source, priority, inject_into]

                if check_updates or verbose:
                    skill_cfg = skills.get(skill_name, {})
                    version_info = _get_skill_version_info(
                        skill_name, skill_cfg, sources, loader.plugin_root, check_remote=check_updates
                    )
                    version_display = version_info.current or "-"
                    if check_updates and version_info.status == "update-available" and version_info.available:
                        version_display = f"{version_info.current} -> {version_info.available}"
                    row.append(version_display)
                    row.append(_get_version_badge(version_info.status))

                    if version_info.status == "update-available" and version_info.update_command:
                        outdated_skills.append((skill_name, version_info.update_command))

                if verbose and not check_updates:
                    skill_config = skills.get(skill_name, {})
                    detect = skill_config.get("detect", {})
                    files = detect.get("files", [])
                    patterns = detect.get("patterns", [])
                    detection_str = f"files: {', '.join(files[:2])}"
                    if patterns:
                        patterns_preview = patterns[:1]
                        detection_str += f" | {patterns_preview[0][:10]}..."
                    row.append(detection_str[:25])

                table.add_row(*row)

            console.print(table)

        # Show priority ranges summary
        console.print()
        console.print("[bold]Priority Ranges:[/bold]")
        for source_type in ["user", "submodule", "builtin"]:
            badge = _get_type_badge(source_type)
            range_str = _get_priority_range_display(source_type)
            count = len(skills_by_type.get(source_type, []))
            console.print(f"  {badge}: {range_str} ({count} skills)")

        # Show sources (sources variable already defined above)
        if sources and verbose:
            console.print()
            console.print("[bold]Skill Sources:[/bold]")
            for source_name, source_config in sources.items():
                source_type = source_config.get("type", "unknown")
                source_path = source_config.get("path", source_config.get("url", "N/A"))
                repo = source_config.get("repository", "")
                type_badge = _get_type_badge(source_type)
                console.print(f"  [cyan]{source_name}[/cyan]: {source_path} ({type_badge})")
                if repo:
                    console.print(f"    [dim]Repository: {repo}[/dim]")

        # Show settings
        settings = config.get("settings", {})
        if settings and verbose:
            console.print()
            console.print("[bold]Settings:[/bold]")
            console.print(f"  Max skills per story: {settings.get('max_skills_per_story', 3)}")
            console.print(f"  Max content lines: {settings.get('max_content_lines', 200)}")
            console.print(f"  Cache enabled: {settings.get('cache_enabled', True)}")

        # Show update suggestions if any skills are outdated
        if outdated_skills:
            console.print()
            console.print(Panel(
                "[yellow]Updates Available[/yellow]\n\n"
                f"{len(outdated_skills)} skill(s) have updates available.\n\n"
                "To update submodule skills, run:\n"
                "[bold]git submodule update --remote --merge[/bold]",
                title="Update Suggestions",
                border_style="yellow",
            ))

            # Show individual skill update info if verbose
            if verbose:
                console.print()
                console.print("[bold]Outdated Skills:[/bold]")
                for skill_name, update_cmd in outdated_skills:
                    console.print(f"  [yellow]-[/yellow] {skill_name}")
        elif check_updates:
            console.print()
            _print_success("All skills are up to date!")

    @skills_app.command("detect")
    def detect_skills(
        project_path: str | None = typer.Option(None, "--project", "-p", help="Project path"),
        phase: str = typer.Option("implementation", "--phase", help="Execution phase to filter by"),
        verbose: bool = typer.Option(False, "--verbose", "-v", help="Show detailed detection info"),
        show_overrides: bool = typer.Option(False, "--overrides", "-o", help="Show skill override details"),
    ):
        """
        Detect applicable skills for the current project.

        Analyzes the project directory to find skills that match based on
        file presence and content patterns. Shows the effective skills after
        deduplication, where higher priority skills override lower priority ones.

        Examples:
            plan-cascade skills detect
            plan-cascade skills detect --phase testing
            plan-cascade skills detect --overrides
            plan-cascade skills detect --verbose
        """
        project = _get_project_path(project_path)

        _print_header(
            "Skill Detection",
            f"Project: {project}"
        )

        loader = _get_skill_loader(project, verbose=verbose)

        # Detect applicable skills
        _print_info("Analyzing project...")

        # Get raw detection info before deduplication for override analysis
        skills_config = loader.config.get("skills", {})
        raw_matches = []
        for skill_name, skill_config in skills_config.items():
            if loader._skill_matches_project(skill_config):
                source = skill_config.get("source", "unknown")
                source_type = loader._get_source_type(source)
                priority = skill_config.get("priority", 0)
                raw_matches.append({
                    "name": skill_name,
                    "source": source,
                    "source_type": source_type,
                    "priority": priority,
                })

        # Sort by priority
        raw_matches.sort(key=lambda x: -x["priority"])

        # Detect with deduplication
        applicable = loader.detect_applicable_skills(verbose=verbose)

        if not raw_matches:
            _print_warning("No applicable skills detected for this project")
            console.print()
            _print_info("Skills are detected based on project files and patterns:")
            console.print("  - React/Next.js: package.json with 'react' or 'next'")
            console.print("  - Vue: package.json with 'vue' or 'nuxt'")
            console.print("  - Rust: Cargo.toml with [package] or [dependencies]")
            console.print("  - Python: pyproject.toml or requirements.txt")
            console.print("  - User skills: as configured in .plan-cascade/skills.json")
            return

        # Show override information if there are duplicates
        if show_overrides or verbose:
            # Find overridden skills (matched but not in applicable list)
            applicable_set = set(applicable)
            overridden = [m for m in raw_matches if m["name"] not in applicable_set]

            if overridden:
                console.print()
                console.print(Panel(
                    "[yellow]Skill Override Analysis[/yellow]\n\n"
                    "Skills with same base name are deduplicated. "
                    "Higher priority skills override lower priority ones.",
                    border_style="yellow",
                ))
                console.print()

                override_table = Table(
                    title="Overridden Skills",
                    show_header=True,
                    header_style="bold yellow"
                )
                override_table.add_column("Overridden Skill", style="dim", width=25)
                override_table.add_column("Type", width=10)
                override_table.add_column("Priority", width=10, justify="right")
                override_table.add_column("Overridden By", style="green", width=25)

                for item in overridden:
                    # Find the winner (skill with same base name in applicable)
                    base_name = loader._get_skill_base_name(item["name"])
                    winner = None
                    for app_skill in applicable:
                        if loader._get_skill_base_name(app_skill) == base_name:
                            winner = app_skill
                            break

                    type_badge = _get_type_badge(item["source_type"])
                    override_table.add_row(
                        item["name"],
                        type_badge,
                        str(item["priority"]),
                        winner or "N/A"
                    )

                console.print(override_table)
                console.print()

        # Build effective skills table
        table = Table(
            title=f"Effective Skills ({len(applicable)} after deduplication)",
            show_header=True,
            header_style="bold cyan"
        )
        table.add_column("Skill", style="cyan", width=25)
        table.add_column("Type", width=10)
        table.add_column("Source", style="white", width=12)
        table.add_column("Priority", style="yellow", width=10, justify="right")
        table.add_column("Phases", style="dim", width=18)
        table.add_column("Status", width=12)

        max_skills = loader.config.get("settings", {}).get("max_skills_per_story", 3)

        for i, skill_name in enumerate(applicable):
            skill_config = skills_config.get(skill_name, {})
            source = skill_config.get("source", "unknown")
            source_type = loader._get_source_type(source)
            priority = str(skill_config.get("priority", 0))
            inject_into = skill_config.get("inject_into", ["implementation"])
            phases_str = ", ".join(inject_into)

            type_badge = _get_type_badge(source_type)

            # Check if skill will be used (within max limit)
            if i < max_skills:
                status = "[green]Active[/green]"
            else:
                status = "[dim]Excluded[/dim]"

            # Check if skill applies to requested phase
            if phase not in inject_into:
                status = f"[dim]Not for {phase}[/dim]"

            table.add_row(skill_name, type_badge, source, priority, phases_str, status)

        console.print(table)

        # Show phase-specific skills
        console.print()
        phase_skills = loader.get_skills_for_phase(phase)

        if phase_skills:
            _print_success(f"{len(phase_skills)} skill(s) will be injected for '{phase}' phase")
            for skill in phase_skills:
                type_badge = _get_type_badge(skill.source_type)
                console.print(f"  [green]v[/green] {skill.name} ({type_badge}, priority: {skill.priority})")
        else:
            _print_info(f"No skills configured for '{phase}' phase")

        # Show settings info
        console.print()
        console.print(f"[dim]Max skills per story: {max_skills}[/dim]")
        console.print(f"[dim]Current phase filter: {phase}[/dim]")
        console.print(f"[dim]Total matched: {len(raw_matches)}, Effective after dedup: {len(applicable)}[/dim]")

    @skills_app.command("show")
    def show_skill(
        skill_name: str = typer.Argument(..., help="Name of the skill to display"),
        project_path: str | None = typer.Option(None, "--project", "-p", help="Project path"),
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
        project_path: str | None = typer.Option(None, "--project", "-p", help="Project path"),
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

    @skills_app.command("add")
    def add_skill(
        name: str = typer.Argument(..., help="Name of the skill to add"),
        path: str | None = typer.Option(None, "--path", help="Local path to skill directory (contains SKILL.md)"),
        url: str | None = typer.Option(None, "--url", help="Remote URL to skill repository"),
        priority: int = typer.Option(150, "--priority", help="Skill priority (101-200 for user skills)"),
        detect_files: str | None = typer.Option(None, "--detect-files", help="Comma-separated list of detection files"),
        detect_patterns: str | None = typer.Option(None, "--detect-patterns", help="Comma-separated list of detection patterns"),
        inject_into: str | None = typer.Option("implementation", "--inject-into", help="Comma-separated phases to inject into"),
        level: str = typer.Option("project", "--level", "-l", help="Config level: 'project' or 'user'"),
        project_path: str | None = typer.Option(None, "--project", "-p", help="Project path"),
    ):
        """
        Add a user-defined skill to the configuration.

        User skills have priority 101-200 and can override builtin/submodule skills
        with the same name. Skills can be loaded from local paths or remote URLs.

        Examples:
            plan-cascade skills add my-skill --path ./my-skills/custom
            plan-cascade skills add team-react --url https://github.com/team/skills/react --priority 180
            plan-cascade skills add python-best --path ./skills/python --detect-files pyproject.toml,setup.py
            plan-cascade skills add global-skill --path ~/skills/global --level user
        """
        project = _get_project_path(project_path)

        # Validate path/url
        if not path and not url:
            _print_error("Either --path or --url must be provided")
            raise typer.Exit(1)
        if path and url:
            _print_error("Only one of --path or --url should be provided, not both")
            raise typer.Exit(1)

        # Validate priority
        if not (101 <= priority <= 200):
            _print_error(f"Priority must be between 101 and 200 for user skills, got {priority}")
            raise typer.Exit(1)

        # Validate level
        if level not in ("project", "user"):
            _print_error(f"Level must be 'project' or 'user', got '{level}'")
            raise typer.Exit(1)

        _print_header(
            "Add User Skill",
            f"Level: {level}"
        )

        # Parse detection config
        detect = {}
        if detect_files:
            detect["files"] = [f.strip() for f in detect_files.split(",")]
        if detect_patterns:
            detect["patterns"] = [p.strip() for p in detect_patterns.split(",")]

        # Parse inject_into
        phases = [p.strip() for p in inject_into.split(",")]

        # Build skill entry
        skill_entry = {
            "name": name,
            "priority": priority,
            "detect": detect if detect else {"files": [], "patterns": []},
            "inject_into": phases,
        }
        if path:
            skill_entry["path"] = path
        if url:
            skill_entry["url"] = url

        # Get user skill config and add
        user_config = _get_user_skill_config(project)
        success = user_config.add_skill(skill_entry, level=level)

        if success:
            _print_success(f"Added skill '{name}' to {level} configuration")
            console.print()
            console.print("[bold]Skill Configuration:[/bold]")
            console.print(f"  Name: [cyan]{name}[/cyan]")
            console.print(f"  Priority: [yellow]{priority}[/yellow]")
            console.print(f"  Source: {path or url}")
            console.print(f"  Phases: {', '.join(phases)}")
            if detect:
                console.print(f"  Detection: {detect}")
            console.print()

            config_file = (
                Path.home() / ".plan-cascade" / "skills.json"
                if level == "user"
                else project / ".plan-cascade" / "skills.json"
            )
            _print_info(f"Config saved to: {config_file}")
        else:
            _print_error(f"Failed to add skill '{name}'")
            errors = user_config.get_validation_errors()
            if errors:
                console.print()
                console.print("[bold]Validation Errors:[/bold]")
                for error in errors:
                    console.print(f"  [red]-[/red] {error}")
            raise typer.Exit(1)

    @skills_app.command("remove")
    def remove_skill(
        name: str = typer.Argument(..., help="Name of the skill to remove"),
        level: str = typer.Option("project", "--level", "-l", help="Config level: 'project' or 'user'"),
        project_path: str | None = typer.Option(None, "--project", "-p", help="Project path"),
    ):
        """
        Remove a user-defined skill from the configuration.

        Only removes skills from user configuration files (.plan-cascade/skills.json).
        Builtin and submodule skills cannot be removed with this command.

        Examples:
            plan-cascade skills remove my-skill
            plan-cascade skills remove global-skill --level user
        """
        project = _get_project_path(project_path)

        # Validate level
        if level not in ("project", "user"):
            _print_error(f"Level must be 'project' or 'user', got '{level}'")
            raise typer.Exit(1)

        _print_header(
            "Remove User Skill",
            f"Level: {level}"
        )

        # Get user skill config and remove
        user_config = _get_user_skill_config(project)

        # Check if skill exists in user config
        skill = user_config.get_skill(name)
        if not skill:
            _print_warning(f"Skill '{name}' not found in user configuration")
            console.print()
            _print_info("This command only removes user-defined skills.")
            _print_info("Builtin and submodule skills are managed in external-skills.json")

            # List user skills
            user_skills = user_config.list_skills()
            if user_skills:
                console.print()
                console.print("[bold]User-defined skills:[/bold]")
                for s in user_skills:
                    src_level = s.get("_source_level", "unknown")
                    console.print(f"  - {s['name']} ({src_level})")
            raise typer.Exit(1)

        success = user_config.remove_skill(name, level=level)

        if success:
            _print_success(f"Removed skill '{name}' from {level} configuration")
            console.print()

            config_file = (
                Path.home() / ".plan-cascade" / "skills.json"
                if level == "user"
                else project / ".plan-cascade" / "skills.json"
            )
            _print_info(f"Config updated: {config_file}")
        else:
            _print_error(f"Failed to remove skill '{name}'")
            _print_info(f"Skill may not exist in {level} configuration")
            raise typer.Exit(1)

    @skills_app.command("refresh")
    def refresh_skills(
        skill_name: str | None = typer.Argument(None, help="Name of specific skill to refresh"),
        all_skills: bool = typer.Option(False, "--all", "-a", help="Refresh all cached remote skills"),
        clear: bool = typer.Option(False, "--clear", "-c", help="Clear cache instead of refreshing"),
        project_path: str | None = typer.Option(None, "--project", "-p", help="Project path"),
    ):
        """
        Refresh cached remote skills.

        Re-downloads remote skills to update the local cache. By default,
        refreshes all cached skills. Use --clear to only remove cache without re-downloading.

        The cache is stored in ~/.plan-cascade/cache/skills/ with a
        default TTL of 7 days.

        Examples:
            plan-cascade skills refresh --all
            plan-cascade skills refresh team-react
            plan-cascade skills refresh --all --clear
        """
        project = _get_project_path(project_path)

        _print_header(
            "Skill Cache Clear" if clear else "Skill Cache Refresh",
            f"Project: {project}"
        )

        if not skill_name and not all_skills:
            _print_error("Specify a skill name or use --all to refresh all cached skills")
            raise typer.Exit(1)

        loader = _get_skill_loader(project)

        # Get URL for specific skill if provided
        url = None
        if skill_name:
            skills_config = loader.config.get("skills", {})
            if skill_name not in skills_config:
                _print_error(f"Skill '{skill_name}' not found")
                raise typer.Exit(1)

            skill_config = skills_config[skill_name]
            source_name = skill_config.get("source", "")
            source_config = loader.config.get("sources", {}).get(source_name, {})
            url = source_config.get("url")

            if not url:
                _print_warning(f"Skill '{skill_name}' is not a remote URL skill")
                _print_info("Only URL-based skills have caches to refresh")
                return

            # Construct full URL
            skill_path = skill_config.get("skill_path", "")
            if skill_path:
                url = f"{url.rstrip('/')}/{skill_path}/SKILL.md"
            else:
                url = f"{url.rstrip('/')}/SKILL.md"

        if clear:
            # Clear cache without re-downloading
            results = loader.clear_skill_cache(url)
            if results["cleared"]:
                for cleared_url in results["cleared"]:
                    _print_success(f"Cleared: {cleared_url}")
                console.print()
                _print_success(f"Cleared {len(results['cleared'])} cached skill(s)")
            else:
                _print_info("No cached skills to clear")
            if results["failed"]:
                for failed_url in results["failed"]:
                    _print_error(f"Failed to clear: {failed_url}")
        else:
            # Refresh cache (re-download)
            results = loader.refresh_remote_skills(url)
            if results["refreshed"]:
                for refreshed_url in results["refreshed"]:
                    _print_success(f"Refreshed: {refreshed_url}")
                console.print()
                _print_success(f"Refreshed {len(results['refreshed'])} cached skill(s)")
            else:
                _print_info("No remote skills to refresh")
            if results["failed"]:
                console.print()
                for failed_url in results["failed"]:
                    _print_warning(f"Failed to refresh: {failed_url}")

    @skills_app.command("cache")
    def cache_stats(
        project_path: str | None = typer.Option(None, "--project", "-p", help="Project path"),
        verbose: bool = typer.Option(False, "--verbose", "-v", help="Show detailed cache entries"),
    ):
        """
        Show skill cache statistics and entries.

        Displays information about cached remote skills including:
        - Total number of cached skills
        - Cache size
        - Expired/valid entry counts
        - Cache directory location

        Examples:
            plan-cascade skills cache
            plan-cascade skills cache --verbose
        """
        project = _get_project_path(project_path)

        _print_header(
            "Skill Cache Statistics",
            f"Project: {project}"
        )

        loader = _get_skill_loader(project)
        stats = loader.get_skill_cache_stats()

        # Display stats
        console.print(f"[bold]Cache Directory:[/bold] {stats['cache_dir']}")
        console.print(f"[bold]TTL:[/bold] {stats['ttl_days']} days")
        console.print()

        table = Table(title="Cache Statistics", show_header=True, header_style="bold cyan")
        table.add_column("Metric", style="cyan")
        table.add_column("Value", style="white", justify="right")

        table.add_row("Total Cached", str(stats["total_cached"]))
        table.add_row("Valid Entries", f"[green]{stats['valid_count']}[/green]")
        table.add_row("Expired Entries", f"[yellow]{stats['expired_count']}[/yellow]" if stats["expired_count"] > 0 else "0")
        table.add_row("Total Size", f"{stats['total_size_mb']} MB")

        if stats["oldest_entry"]:
            table.add_row("Oldest Entry", stats["oldest_entry"])
        if stats["newest_entry"]:
            table.add_row("Newest Entry", stats["newest_entry"])

        console.print(table)

        # Show detailed entries if verbose
        if verbose:
            entries = loader.list_cached_skills()
            if entries:
                console.print()
                console.print("[bold]Cached Entries:[/bold]")

                entry_table = Table(show_header=True, header_style="bold cyan")
                entry_table.add_column("URL", style="cyan", width=50)
                entry_table.add_column("Status", width=10)
                entry_table.add_column("Cached At", width=20)
                entry_table.add_column("Expires At", width=20)
                entry_table.add_column("Size", justify="right", width=10)

                for entry in entries:
                    status = "[yellow]Expired[/yellow]" if entry.is_expired() else "[green]Valid[/green]"
                    size_str = f"{entry.size_bytes / 1024:.1f} KB" if entry.size_bytes >= 1024 else f"{entry.size_bytes} B"

                    # Truncate long URLs
                    url_display = entry.url if len(entry.url) <= 50 else entry.url[:47] + "..."

                    entry_table.add_row(
                        url_display,
                        status,
                        entry.cached_at,
                        entry.expires_at,
                        size_str,
                    )

                console.print(entry_table)
            else:
                _print_info("No cached skills")

    @skills_app.command("validate")
    def validate_skills(
        project_path: str | None = typer.Option(None, "--project", "-p", help="Project path"),
        verbose: bool = typer.Option(False, "--verbose", "-v", help="Show detailed validation info"),
    ):
        """
        Validate external skills configuration and availability.

        Checks all three source types (builtin, submodule, user) for:
        - Valid configuration files
        - Accessible source paths/URLs
        - Existing SKILL.md files
        - Priority range compliance
        - User config validation errors

        Examples:
            plan-cascade skills validate
            plan-cascade skills validate --verbose
        """
        project = _get_project_path(project_path)

        _print_header(
            "Skills Validation",
            f"Project: {project}"
        )

        loader = _get_skill_loader(project, verbose=verbose)
        config = loader.config

        issues = []
        warnings_list = []
        validated = 0

        # ========== 1. Validate base config file ==========
        console.print("[bold]1. Configuration Files:[/bold]")

        config_path = loader.plugin_root / "external-skills.json"
        if config_path.exists():
            _print_success(f"  Base config: {config_path}")
            validated += 1
        else:
            _print_warning(f"  Base config not found: {config_path}")
            warnings_list.append(f"Base configuration file not found: {config_path}")

        # Check user config files
        user_config = loader.get_user_skill_config()
        if user_config:
            project_config_path = project / ".plan-cascade" / "skills.json"
            user_home_config = Path.home() / ".plan-cascade" / "skills.json"

            if project_config_path.exists():
                _print_success(f"  Project user config: {project_config_path}")
                validated += 1
            else:
                _print_info("  Project user config: not configured (optional)")

            if user_home_config.exists():
                _print_success(f"  Global user config: {user_home_config}")
                validated += 1
            else:
                _print_info("  Global user config: not configured (optional)")

            # Check user config validation errors
            user_errors = user_config.get_validation_errors()
            if user_errors:
                for error in user_errors:
                    issues.append(f"User config: {error}")
        console.print()

        # ========== 2. Validate sources by type ==========
        console.print("[bold]2. Skill Sources by Type:[/bold]")

        sources = config.get("sources", {})
        skills_by_type = loader.get_skills_by_type()

        for source_type in ["builtin", "submodule", "user"]:
            type_badge = _get_type_badge(source_type)
            skill_count = len(skills_by_type.get(source_type, []))
            console.print(f"  {type_badge} ({skill_count} skills)")

            # Find sources of this type
            for source_name, source_config in sources.items():
                if source_config.get("type", "submodule") == source_type:
                    source_path = source_config.get("path", source_config.get("url", ""))
                    is_url = source_path.startswith(("http://", "https://"))

                    if is_url:
                        _print_info(f"    {source_name}: {source_path} (remote)")
                    else:
                        full_path = loader.plugin_root / source_path if source_path else None
                        if full_path and full_path.exists():
                            _print_success(f"    {source_name}: {full_path}")
                            validated += 1
                        elif full_path:
                            _print_error(f"    {source_name}: {full_path} (not found)")
                            issues.append(f"Source '{source_name}' path not found: {full_path}")
                        else:
                            _print_warning(f"    {source_name}: no path configured")
        console.print()

        # ========== 3. Validate skills ==========
        console.print("[bold]3. Skills Validation:[/bold]")

        skills = config.get("skills", {})

        for source_type in ["user", "submodule", "builtin"]:
            skill_names = skills_by_type.get(source_type, [])
            if not skill_names:
                continue

            type_badge = _get_type_badge(source_type)
            console.print(f"  {type_badge}:")

            for skill_name in sorted(skill_names):
                skill_config = skills.get(skill_name, {})
                source_name = skill_config.get("source")
                source_config = sources.get(source_name, {})

                if not source_config:
                    _print_error(f"    {skill_name}: Unknown source '{source_name}'")
                    issues.append(f"Skill '{skill_name}' has unknown source: {source_name}")
                    continue

                # Determine path/URL
                source_path = source_config.get("path", source_config.get("url", ""))
                is_url = source_path.startswith(("http://", "https://"))

                if is_url:
                    # Can't validate URL without making request
                    if verbose:
                        _print_info(f"    {skill_name}: remote URL (not validated)")
                    else:
                        _print_info(f"    {skill_name}: remote")
                else:
                    # Check SKILL.md exists
                    skill_md_path = (
                        loader.plugin_root
                        / source_config.get("path", "")
                        / skill_config.get("skill_path", "")
                        / "SKILL.md"
                    )

                    if skill_md_path.exists():
                        _print_success(f"    {skill_name}: SKILL.md found")
                        validated += 1
                    else:
                        _print_warning(f"    {skill_name}: SKILL.md not found")
                        warnings_list.append(f"Skill '{skill_name}' missing SKILL.md: {skill_md_path}")
        console.print()

        # ========== 4. Validate priorities ==========
        console.print("[bold]4. Priority Validation:[/bold]")

        priority_warnings = loader.validate_priorities()
        if priority_warnings:
            for warning in priority_warnings:
                _print_warning(f"  {warning}")
                warnings_list.append(warning)
        else:
            _print_success("  All skills have valid priorities for their source type")
            validated += 1
        console.print()

        # ========== Summary ==========
        console.print("[bold]Summary:[/bold]")
        total_issues = len(issues) + len(warnings_list)

        if issues:
            _print_error(f"  Errors: {len(issues)}")
            for issue in issues:
                console.print(f"    [red]-[/red] {issue}")

        if warnings_list:
            _print_warning(f"  Warnings: {len(warnings_list)}")
            if verbose:
                for warning in warnings_list:
                    console.print(f"    [yellow]-[/yellow] {warning}")

        if total_issues == 0:
            _print_success(f"  All {validated} items validated successfully")
        else:
            console.print()
            _print_info("Suggestions:")
            _print_info("  - Run 'git submodule update --init --recursive' to initialize submodules")
            _print_info("  - Check .plan-cascade/skills.json for user config errors")
            _print_info("  - Verify remote URLs are accessible")

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
