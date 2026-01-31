#!/usr/bin/env python3
"""
Design Document CLI Commands for Plan Cascade

Provides CLI commands for design document management:
- design generate: Create design_doc.json (auto-detects project vs feature level)
- design review: Display and interactively edit design documents
- design import: Convert external documents (Markdown, JSON, HTML)
- design show: Display design document in readable format
"""

import json
import sys
from pathlib import Path
from typing import Optional

try:
    import typer
    from rich.console import Console
    from rich.panel import Panel
    from rich.prompt import Confirm, Prompt
    from rich.table import Table
    from rich.tree import Tree

    HAS_TYPER = True
except ImportError:
    HAS_TYPER = False

if HAS_TYPER:
    # Create Typer app for design commands
    design_app = typer.Typer(
        name="design",
        help="Design document management commands",
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

    def _display_design_doc(design_doc: dict, verbose: bool = False) -> None:
        """Display design document in a formatted way."""
        metadata = design_doc.get("metadata", {})
        overview = design_doc.get("overview", {})
        architecture = design_doc.get("architecture", {})
        interfaces = design_doc.get("interfaces", {})
        decisions = design_doc.get("decisions", [])

        # Header panel
        level = metadata.get("level", "unknown")
        level_color = "blue" if level == "project" else "cyan"
        source = metadata.get("source", "unknown")

        header_text = f"[bold]{overview.get('title', 'Untitled')}[/bold]\n\n"
        if overview.get("summary"):
            summary = overview["summary"]
            if len(summary) > 200:
                summary = summary[:200] + "..."
            header_text += f"[dim]{summary}[/dim]\n\n"
        header_text += f"[dim]Level:[/dim] [{level_color}]{level}[/{level_color}]  |  "
        header_text += f"[dim]Source:[/dim] {source}  |  "
        header_text += f"[dim]Version:[/dim] {metadata.get('version', '1.0.0')}"

        console.print(Panel(header_text, title="Design Document", border_style=level_color))

        # Goals and Non-Goals
        goals = overview.get("goals", [])
        non_goals = overview.get("non_goals", [])

        if goals or non_goals:
            console.print()
            if goals:
                console.print("[bold]Goals:[/bold]")
                for goal in goals[:5]:  # Limit to 5 for brevity
                    console.print(f"  [green]+[/green] {goal}")
                if len(goals) > 5:
                    console.print(f"  [dim]... and {len(goals) - 5} more[/dim]")

            if non_goals:
                console.print()
                console.print("[bold]Non-Goals:[/bold]")
                for ng in non_goals[:3]:
                    console.print(f"  [red]-[/red] {ng}")
                if len(non_goals) > 3:
                    console.print(f"  [dim]... and {len(non_goals) - 3} more[/dim]")

        # Components table
        components = architecture.get("components", [])
        if components:
            console.print()
            table = Table(title="Components", show_header=True, header_style="bold cyan")
            table.add_column("Name", style="cyan", width=20)
            table.add_column("Description", style="white", width=40)
            table.add_column("Dependencies", style="dim", width=20)

            for comp in components[:10]:  # Limit to 10
                deps = ", ".join(comp.get("dependencies", [])[:3]) or "-"
                if len(comp.get("dependencies", [])) > 3:
                    deps += "..."

                table.add_row(
                    comp.get("name", ""),
                    comp.get("description", "")[:40] + ("..." if len(comp.get("description", "")) > 40 else ""),
                    deps,
                )

            console.print(table)
            if len(components) > 10:
                console.print(f"[dim]... and {len(components) - 10} more components[/dim]")

        # Patterns
        patterns = architecture.get("patterns", [])
        if patterns:
            console.print()
            console.print("[bold]Architectural Patterns:[/bold]")
            for pattern in patterns[:5]:
                console.print(f"  [yellow]*[/yellow] {pattern.get('name', 'Unknown')}")
                if verbose and pattern.get("description"):
                    console.print(f"      [dim]{pattern['description'][:60]}...[/dim]")
            if len(patterns) > 5:
                console.print(f"  [dim]... and {len(patterns) - 5} more patterns[/dim]")

        # Decisions (ADRs)
        if decisions:
            console.print()
            table = Table(title="Architecture Decisions", show_header=True, header_style="bold yellow")
            table.add_column("ID", style="yellow", width=10)
            table.add_column("Title", style="white", width=40)
            table.add_column("Status", width=10)

            for adr in decisions[:8]:
                status = adr.get("status", "accepted")
                status_style = {
                    "accepted": "green",
                    "proposed": "yellow",
                    "deprecated": "red",
                    "superseded": "dim",
                }.get(status, "white")

                table.add_row(
                    adr.get("id", ""),
                    adr.get("title", "")[:40] + ("..." if len(adr.get("title", "")) > 40 else ""),
                    f"[{status_style}]{status}[/{status_style}]",
                )

            console.print(table)
            if len(decisions) > 8:
                console.print(f"[dim]... and {len(decisions) - 8} more decisions[/dim]")

        # APIs (if any)
        apis = interfaces.get("apis", [])
        if apis and verbose:
            console.print()
            console.print("[bold]APIs:[/bold]")
            for api in apis[:5]:
                method = api.get("method", "GET")
                path = api.get("path", "/")
                console.print(f"  [{method}] {path}")
            if len(apis) > 5:
                console.print(f"  [dim]... and {len(apis) - 5} more endpoints[/dim]")

        # Data Models (if any)
        data_models = interfaces.get("data_models", []) or interfaces.get("shared_data_models", [])
        if data_models and verbose:
            console.print()
            console.print("[bold]Data Models:[/bold]")
            for model in data_models[:5]:
                console.print(f"  [cyan]{model.get('name', 'Unknown')}[/cyan]: {model.get('description', '')[:40]}")
            if len(data_models) > 5:
                console.print(f"  [dim]... and {len(data_models) - 5} more models[/dim]")

        # Story/Feature Mappings
        story_mappings = design_doc.get("story_mappings", {})
        feature_mappings = design_doc.get("feature_mappings", {})
        mappings = story_mappings or feature_mappings

        if mappings and verbose:
            console.print()
            mapping_type = "Story" if story_mappings else "Feature"
            console.print(f"[bold]{mapping_type} Mappings:[/bold]")
            for mid, mapping in list(mappings.items())[:5]:
                components_list = ", ".join(mapping.get("components", [])[:2]) or "-"
                console.print(f"  [cyan]{mid}[/cyan] -> {components_list}")
            if len(mappings) > 5:
                console.print(f"  [dim]... and {len(mappings) - 5} more mappings[/dim]")

    def _display_component_details(component: dict) -> None:
        """Display detailed component information."""
        console.print()
        console.print(f"[bold cyan]{component.get('name', 'Unknown')}[/bold cyan]")
        console.print()

        if component.get("description"):
            console.print(f"[bold]Description:[/bold] {component['description']}")
            console.print()

        responsibilities = component.get("responsibilities", [])
        if responsibilities:
            console.print("[bold]Responsibilities:[/bold]")
            for resp in responsibilities:
                console.print(f"  - {resp}")
            console.print()

        dependencies = component.get("dependencies", [])
        if dependencies:
            console.print(f"[bold]Dependencies:[/bold] {', '.join(dependencies)}")
            console.print()

        files = component.get("files", [])
        if files:
            console.print("[bold]Files:[/bold]")
            for f in files:
                console.print(f"  [dim]{f}[/dim]")

    def _display_decision_details(decision: dict) -> None:
        """Display detailed ADR information."""
        console.print()
        console.print(f"[bold yellow]{decision.get('id', '')}[/bold yellow]: {decision.get('title', '')}")
        console.print()

        if decision.get("context"):
            console.print("[bold]Context:[/bold]")
            console.print(f"  {decision['context']}")
            console.print()

        if decision.get("decision"):
            console.print("[bold]Decision:[/bold]")
            console.print(f"  {decision['decision']}")
            console.print()

        if decision.get("rationale"):
            console.print("[bold]Rationale:[/bold]")
            console.print(f"  {decision['rationale']}")
            console.print()

        alternatives = decision.get("alternatives_considered", [])
        if alternatives:
            console.print("[bold]Alternatives Considered:[/bold]")
            for alt in alternatives:
                console.print(f"  - {alt}")
            console.print()

        console.print(f"[bold]Status:[/bold] {decision.get('status', 'accepted')}")

    # ========== CLI Commands ==========

    @design_app.command("generate")
    def generate(
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
        level: Optional[str] = typer.Option(None, "--level", "-l", help="Force level (project/feature)"),
        feature_id: Optional[str] = typer.Option(None, "--feature-id", "-f", help="Feature ID for feature-level docs"),
        source: str = typer.Option("ai-generated", "--source", "-s", help="Source type"),
        force: bool = typer.Option(False, "--force", help="Overwrite existing design document"),
    ):
        """
        Generate a design document.

        Auto-detects level based on existing files:
        - Project level if mega-plan.json exists
        - Feature level if prd.json exists

        Examples:
            plan-cascade design generate
            plan-cascade design generate --level project
            plan-cascade design generate --feature-id feature-auth
        """
        project = _get_project_path(project_path)

        _print_header(
            "Design Document Generator",
            f"Project: {project}"
        )

        from ..core.design_doc_generator import DesignDocGenerator

        generator = DesignDocGenerator(project)

        # Check if design doc already exists
        existing = generator.load_design_doc()
        if existing and not force:
            _print_warning("design_doc.json already exists")
            if not Confirm.ask("Overwrite existing document?", default=False):
                _print_info("Aborted")
                raise typer.Exit(0)

        # Detect or use specified level
        if level:
            detected_level = level
            _print_info(f"Using specified level: {level}")
        else:
            detected_level = generator.detect_level()
            if detected_level == "unknown":
                _print_warning("Cannot auto-detect level (no mega-plan.json or prd.json found)")
                detected_level = Prompt.ask(
                    "Select level",
                    choices=["project", "feature"],
                    default="feature"
                )
            else:
                _print_info(f"Auto-detected level: {detected_level}")

        # Generate the design document
        _print_info("Generating design document...")

        if detected_level == "project":
            design_doc = generator.generate_project_design_doc(source=source)
        else:
            design_doc = generator.generate_feature_design_doc(
                feature_id=feature_id,
                source=source
            )

        # Save the document
        if generator.save_design_doc(design_doc):
            _print_success(f"Design document saved to {generator.design_doc_path}")
        else:
            _print_error("Failed to save design document")
            raise typer.Exit(1)

        # Display the generated document
        console.print()
        _display_design_doc(design_doc)

        console.print()
        _print_info("Next steps:")
        console.print("  1. Review the document: [cyan]plan-cascade design review[/cyan]")
        console.print("  2. Add components: [cyan]plan-cascade design review[/cyan] then 'add component'")
        console.print("  3. Add decisions: [cyan]plan-cascade design review[/cyan] then 'add decision'")

    @design_app.command("show")
    def show(
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
        verbose: bool = typer.Option(False, "--verbose", "-v", help="Show detailed information"),
        json_output: bool = typer.Option(False, "--json", "-j", help="Output as JSON"),
    ):
        """
        Display the current design document.

        Shows the design document in a readable format with components,
        patterns, decisions, and mappings.

        Examples:
            plan-cascade design show
            plan-cascade design show --verbose
            plan-cascade design show --json
        """
        project = _get_project_path(project_path)

        from ..core.design_doc_generator import DesignDocGenerator

        generator = DesignDocGenerator(project)
        design_doc = generator.load_design_doc()

        if not design_doc:
            _print_error("No design_doc.json found")
            _print_info("Generate one with: [cyan]plan-cascade design generate[/cyan]")
            raise typer.Exit(1)

        if json_output:
            console.print(json.dumps(design_doc, indent=2))
        else:
            _print_header(
                "Design Document",
                f"Project: {project}"
            )
            _display_design_doc(design_doc, verbose=verbose)

    @design_app.command("review")
    def review(
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
    ):
        """
        Interactively review and edit the design document.

        Allows adding, editing, and removing components, patterns,
        decisions, APIs, and data models.

        Examples:
            plan-cascade design review
        """
        project = _get_project_path(project_path)

        _print_header(
            "Design Document Review",
            f"Project: {project}"
        )

        from ..core.design_doc_generator import DesignDocGenerator

        generator = DesignDocGenerator(project)
        design_doc = generator.load_design_doc()

        if not design_doc:
            _print_error("No design_doc.json found")
            if Confirm.ask("Generate a new design document?", default=True):
                level = generator.detect_level()
                if level == "unknown":
                    level = Prompt.ask("Select level", choices=["project", "feature"], default="feature")

                if level == "project":
                    design_doc = generator.generate_project_design_doc()
                else:
                    design_doc = generator.generate_feature_design_doc()

                generator.save_design_doc(design_doc)
                _print_success("Generated new design document")
            else:
                raise typer.Exit(1)

        # Interactive edit loop
        while True:
            console.print()
            _display_design_doc(design_doc, verbose=False)
            console.print()

            choice = Prompt.ask(
                "Action",
                choices=[
                    "view", "add", "edit", "remove",
                    "validate", "save", "quit"
                ],
                default="quit"
            )

            if choice == "quit":
                if Confirm.ask("Save changes before quitting?", default=True):
                    generator.save_design_doc(design_doc)
                    _print_success("Changes saved")
                break

            elif choice == "save":
                generator.save_design_doc(design_doc)
                _print_success("Changes saved")

            elif choice == "view":
                # View detailed information
                view_type = Prompt.ask(
                    "View",
                    choices=["components", "patterns", "decisions", "apis", "models", "full"],
                    default="full"
                )

                if view_type == "full":
                    _display_design_doc(design_doc, verbose=True)

                elif view_type == "components":
                    components = design_doc.get("architecture", {}).get("components", [])
                    if not components:
                        _print_info("No components defined")
                    else:
                        for comp in components:
                            _display_component_details(comp)

                elif view_type == "patterns":
                    patterns = design_doc.get("architecture", {}).get("patterns", [])
                    if not patterns:
                        _print_info("No patterns defined")
                    else:
                        for pattern in patterns:
                            console.print()
                            console.print(f"[yellow]*[/yellow] [bold]{pattern.get('name', '')}[/bold]")
                            if pattern.get("description"):
                                console.print(f"    {pattern['description']}")
                            if pattern.get("rationale"):
                                console.print(f"    [dim]Rationale: {pattern['rationale']}[/dim]")

                elif view_type == "decisions":
                    decisions = design_doc.get("decisions", [])
                    if not decisions:
                        _print_info("No decisions defined")
                    else:
                        adr_id = Prompt.ask(
                            "Decision ID (or 'all')",
                            default="all"
                        )
                        if adr_id == "all":
                            for dec in decisions:
                                _display_decision_details(dec)
                        else:
                            for dec in decisions:
                                if dec.get("id") == adr_id:
                                    _display_decision_details(dec)
                                    break
                            else:
                                _print_warning(f"Decision {adr_id} not found")

                elif view_type == "apis":
                    apis = design_doc.get("interfaces", {}).get("apis", [])
                    if not apis:
                        _print_info("No APIs defined")
                    else:
                        for api in apis:
                            console.print(f"  [{api.get('method', 'GET')}] {api.get('path', '/')}")
                            if api.get("description"):
                                console.print(f"      [dim]{api['description']}[/dim]")

                elif view_type == "models":
                    models = design_doc.get("interfaces", {}).get("data_models", [])
                    if not models:
                        _print_info("No data models defined")
                    else:
                        for model in models:
                            console.print(f"  [cyan]{model.get('name', '')}[/cyan]")
                            if model.get("description"):
                                console.print(f"      {model['description']}")

            elif choice == "add":
                # Add new elements
                add_type = Prompt.ask(
                    "Add",
                    choices=["component", "pattern", "decision", "api", "model"],
                    default="component"
                )

                if add_type == "component":
                    name = Prompt.ask("Component name")
                    description = Prompt.ask("Description")
                    responsibilities = Prompt.ask("Responsibilities (comma-separated)")
                    dependencies = Prompt.ask("Dependencies (comma-separated)", default="")
                    files = Prompt.ask("Files (comma-separated)", default="")

                    resp_list = [r.strip() for r in responsibilities.split(",") if r.strip()]
                    deps_list = [d.strip() for d in dependencies.split(",") if d.strip()] if dependencies else []
                    files_list = [f.strip() for f in files.split(",") if f.strip()] if files else []

                    generator.add_component(
                        design_doc,
                        name=name,
                        description=description,
                        responsibilities=resp_list,
                        dependencies=deps_list,
                        files=files_list,
                    )
                    _print_success(f"Added component: {name}")

                elif add_type == "pattern":
                    name = Prompt.ask("Pattern name")
                    description = Prompt.ask("Description")
                    rationale = Prompt.ask("Rationale")

                    generator.add_pattern(
                        design_doc,
                        name=name,
                        description=description,
                        rationale=rationale,
                    )
                    _print_success(f"Added pattern: {name}")

                elif add_type == "decision":
                    title = Prompt.ask("Decision title")
                    context = Prompt.ask("Context (why this decision is needed)")
                    decision_text = Prompt.ask("Decision (what was decided)")
                    rationale = Prompt.ask("Rationale (why this option)")
                    alternatives = Prompt.ask("Alternatives considered (comma-separated)", default="")
                    status = Prompt.ask(
                        "Status",
                        choices=["accepted", "proposed", "deprecated", "superseded"],
                        default="accepted"
                    )

                    alt_list = [a.strip() for a in alternatives.split(",") if a.strip()] if alternatives else []

                    generator.add_decision(
                        design_doc,
                        title=title,
                        context=context,
                        decision=decision_text,
                        rationale=rationale,
                        alternatives=alt_list,
                        status=status,
                    )
                    _print_success(f"Added decision: {title}")

                elif add_type == "api":
                    method = Prompt.ask(
                        "HTTP method",
                        choices=["GET", "POST", "PUT", "DELETE", "PATCH"],
                        default="GET"
                    )
                    path = Prompt.ask("Path (e.g., /api/users/{id})")
                    description = Prompt.ask("Description")

                    generator.add_api(
                        design_doc,
                        method=method,
                        path=path,
                        description=description,
                    )
                    _print_success(f"Added API: {method} {path}")

                elif add_type == "model":
                    name = Prompt.ask("Model name")
                    description = Prompt.ask("Description")
                    fields_input = Prompt.ask("Fields (name:type pairs, comma-separated)")

                    fields = {}
                    for field in fields_input.split(","):
                        if ":" in field:
                            fname, ftype = field.split(":", 1)
                            fields[fname.strip()] = ftype.strip()

                    generator.add_data_model(
                        design_doc,
                        name=name,
                        description=description,
                        fields=fields,
                    )
                    _print_success(f"Added data model: {name}")

            elif choice == "edit":
                # Edit existing elements
                edit_type = Prompt.ask(
                    "Edit",
                    choices=["overview", "component", "pattern", "decision"],
                    default="overview"
                )

                if edit_type == "overview":
                    overview = design_doc.get("overview", {})
                    field = Prompt.ask(
                        "Field",
                        choices=["title", "summary"],
                        default="title"
                    )

                    current = overview.get(field, "")
                    console.print(f"Current {field}: {current}")

                    new_value = Prompt.ask(f"New {field}")
                    design_doc["overview"][field] = new_value
                    _print_success(f"Updated overview.{field}")

                elif edit_type == "component":
                    components = design_doc.get("architecture", {}).get("components", [])
                    if not components:
                        _print_warning("No components to edit")
                        continue

                    comp_names = [c["name"] for c in components]
                    comp_name = Prompt.ask("Component name", choices=comp_names)

                    for comp in components:
                        if comp["name"] == comp_name:
                            field = Prompt.ask(
                                "Field",
                                choices=["description", "responsibilities", "dependencies", "files"],
                                default="description"
                            )

                            if field == "description":
                                console.print(f"Current: {comp.get('description', '')}")
                                comp["description"] = Prompt.ask("New description")
                            elif field == "responsibilities":
                                console.print(f"Current: {', '.join(comp.get('responsibilities', []))}")
                                new_resp = Prompt.ask("New responsibilities (comma-separated)")
                                comp["responsibilities"] = [r.strip() for r in new_resp.split(",") if r.strip()]
                            elif field == "dependencies":
                                console.print(f"Current: {', '.join(comp.get('dependencies', []))}")
                                new_deps = Prompt.ask("New dependencies (comma-separated)")
                                comp["dependencies"] = [d.strip() for d in new_deps.split(",") if d.strip()]
                            elif field == "files":
                                console.print(f"Current: {', '.join(comp.get('files', []))}")
                                new_files = Prompt.ask("New files (comma-separated)")
                                comp["files"] = [f.strip() for f in new_files.split(",") if f.strip()]

                            _print_success(f"Updated {comp_name}.{field}")
                            break

                elif edit_type == "pattern":
                    patterns = design_doc.get("architecture", {}).get("patterns", [])
                    if not patterns:
                        _print_warning("No patterns to edit")
                        continue

                    pattern_names = [p["name"] for p in patterns]
                    pattern_name = Prompt.ask("Pattern name", choices=pattern_names)

                    for pattern in patterns:
                        if pattern["name"] == pattern_name:
                            field = Prompt.ask(
                                "Field",
                                choices=["description", "rationale"],
                                default="description"
                            )

                            console.print(f"Current: {pattern.get(field, '')}")
                            pattern[field] = Prompt.ask(f"New {field}")
                            _print_success(f"Updated {pattern_name}.{field}")
                            break

                elif edit_type == "decision":
                    decisions = design_doc.get("decisions", [])
                    if not decisions:
                        _print_warning("No decisions to edit")
                        continue

                    adr_ids = [d["id"] for d in decisions]
                    adr_id = Prompt.ask("Decision ID", choices=adr_ids)

                    for dec in decisions:
                        if dec["id"] == adr_id:
                            field = Prompt.ask(
                                "Field",
                                choices=["title", "context", "decision", "rationale", "status"],
                                default="status"
                            )

                            if field == "status":
                                dec["status"] = Prompt.ask(
                                    "New status",
                                    choices=["accepted", "proposed", "deprecated", "superseded"],
                                    default=dec.get("status", "accepted")
                                )
                            else:
                                console.print(f"Current: {dec.get(field, '')}")
                                dec[field] = Prompt.ask(f"New {field}")

                            _print_success(f"Updated {adr_id}.{field}")
                            break

            elif choice == "remove":
                # Remove elements
                remove_type = Prompt.ask(
                    "Remove",
                    choices=["component", "pattern", "decision", "api", "model"],
                    default="component"
                )

                if remove_type == "component":
                    components = design_doc.get("architecture", {}).get("components", [])
                    if not components:
                        _print_warning("No components to remove")
                        continue

                    comp_names = [c["name"] for c in components]
                    comp_name = Prompt.ask("Component name to remove", choices=comp_names)

                    if Confirm.ask(f"Remove component '{comp_name}'?", default=False):
                        design_doc["architecture"]["components"] = [
                            c for c in components if c["name"] != comp_name
                        ]
                        _print_success(f"Removed component: {comp_name}")

                elif remove_type == "pattern":
                    patterns = design_doc.get("architecture", {}).get("patterns", [])
                    if not patterns:
                        _print_warning("No patterns to remove")
                        continue

                    pattern_names = [p["name"] for p in patterns]
                    pattern_name = Prompt.ask("Pattern name to remove", choices=pattern_names)

                    if Confirm.ask(f"Remove pattern '{pattern_name}'?", default=False):
                        design_doc["architecture"]["patterns"] = [
                            p for p in patterns if p["name"] != pattern_name
                        ]
                        _print_success(f"Removed pattern: {pattern_name}")

                elif remove_type == "decision":
                    decisions = design_doc.get("decisions", [])
                    if not decisions:
                        _print_warning("No decisions to remove")
                        continue

                    adr_ids = [d["id"] for d in decisions]
                    adr_id = Prompt.ask("Decision ID to remove", choices=adr_ids)

                    if Confirm.ask(f"Remove decision '{adr_id}'?", default=False):
                        design_doc["decisions"] = [d for d in decisions if d["id"] != adr_id]
                        _print_success(f"Removed decision: {adr_id}")

                elif remove_type == "api":
                    apis = design_doc.get("interfaces", {}).get("apis", [])
                    if not apis:
                        _print_warning("No APIs to remove")
                        continue

                    api_ids = [f"{a.get('id', '')} ({a.get('method', '')} {a.get('path', '')})" for a in apis]
                    api_choice = Prompt.ask("API to remove", choices=api_ids)
                    api_id = api_choice.split(" ")[0]

                    if Confirm.ask(f"Remove API '{api_id}'?", default=False):
                        design_doc["interfaces"]["apis"] = [a for a in apis if a.get("id") != api_id]
                        _print_success(f"Removed API: {api_id}")

                elif remove_type == "model":
                    models = design_doc.get("interfaces", {}).get("data_models", [])
                    if not models:
                        _print_warning("No data models to remove")
                        continue

                    model_names = [m["name"] for m in models]
                    model_name = Prompt.ask("Model name to remove", choices=model_names)

                    if Confirm.ask(f"Remove model '{model_name}'?", default=False):
                        design_doc["interfaces"]["data_models"] = [
                            m for m in models if m["name"] != model_name
                        ]
                        _print_success(f"Removed model: {model_name}")

            elif choice == "validate":
                # Validate the design document
                is_valid, errors = generator.validate_design_doc(design_doc)
                if is_valid:
                    _print_success("Design document is valid")
                else:
                    _print_error("Validation errors:")
                    for error in errors:
                        console.print(f"  [red]-[/red] {error}")

    @design_app.command("import")
    def import_doc(
        input_file: str = typer.Argument(..., help="Path to the input document"),
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
        format_type: Optional[str] = typer.Option(None, "--format", "-f", help="Force format (markdown/json/html)"),
        force: bool = typer.Option(False, "--force", help="Overwrite existing design document"),
    ):
        """
        Import and convert an external document to design_doc.json.

        Supports:
        - Markdown files (.md) - parses headings and sections
        - JSON files (.json) - validates or maps fields
        - HTML files (.html) - extracts from Confluence/Notion exports

        Examples:
            plan-cascade design import design.md
            plan-cascade design import architecture.json
            plan-cascade design import confluence-export.html
            plan-cascade design import design.md --format markdown
        """
        project = _get_project_path(project_path)

        _print_header(
            "Design Document Import",
            f"Project: {project}"
        )

        input_path = Path(input_file)
        if not input_path.is_absolute():
            input_path = project / input_path

        if not input_path.exists():
            _print_error(f"Input file not found: {input_path}")
            raise typer.Exit(1)

        from ..core.design_doc_converter import DesignDocConverter
        from ..core.design_doc_generator import DesignDocGenerator

        converter = DesignDocConverter(project)
        generator = DesignDocGenerator(project)

        # Check if design doc already exists
        existing = generator.load_design_doc()
        if existing and not force:
            _print_warning("design_doc.json already exists")
            if not Confirm.ask("Overwrite existing document?", default=False):
                _print_info("Aborted")
                raise typer.Exit(0)

        # Detect format
        if format_type:
            detected_format = format_type
            _print_info(f"Using specified format: {format_type}")
        else:
            detected_format = converter._detect_format(input_path)
            _print_info(f"Detected format: {detected_format}")

        # Convert the document
        _print_info(f"Converting {input_path.name}...")

        try:
            design_doc = converter.convert(input_path, format_type=detected_format)
        except ValueError as e:
            _print_error(f"Conversion failed: {e}")
            raise typer.Exit(1)
        except FileNotFoundError as e:
            _print_error(f"File not found: {e}")
            raise typer.Exit(1)

        # Validate the converted document
        is_valid, errors = generator.validate_design_doc(design_doc)
        if not is_valid:
            _print_warning("Converted document has validation issues:")
            for error in errors:
                console.print(f"  [yellow]-[/yellow] {error}")

            if not Confirm.ask("Save anyway?", default=True):
                _print_info("Aborted")
                raise typer.Exit(0)

        # Save the document
        if converter.save_design_doc(design_doc):
            _print_success(f"Design document saved to {converter.design_doc_path}")
        else:
            _print_error("Failed to save design document")
            raise typer.Exit(1)

        # Display the converted document
        console.print()
        _display_design_doc(design_doc)

        console.print()
        _print_info("Review and edit with: [cyan]plan-cascade design review[/cyan]")

    @design_app.command("validate")
    def validate(
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
    ):
        """
        Validate the current design document.

        Checks for required sections, valid structure, and consistency.

        Examples:
            plan-cascade design validate
        """
        project = _get_project_path(project_path)

        _print_header(
            "Design Document Validation",
            f"Project: {project}"
        )

        from ..core.design_doc_generator import DesignDocGenerator

        generator = DesignDocGenerator(project)
        design_doc = generator.load_design_doc()

        if not design_doc:
            _print_error("No design_doc.json found")
            raise typer.Exit(1)

        is_valid, errors = generator.validate_design_doc(design_doc)

        if is_valid:
            _print_success("Design document is valid!")

            # Show summary
            console.print()
            metadata = design_doc.get("metadata", {})
            arch = design_doc.get("architecture", {})
            interfaces = design_doc.get("interfaces", {})
            decisions = design_doc.get("decisions", [])

            console.print("[bold]Summary:[/bold]")
            console.print(f"  Level: {metadata.get('level', 'unknown')}")
            console.print(f"  Components: {len(arch.get('components', []))}")
            console.print(f"  Patterns: {len(arch.get('patterns', []))}")
            console.print(f"  Decisions: {len(decisions)}")
            console.print(f"  APIs: {len(interfaces.get('apis', []))}")
            console.print(f"  Data Models: {len(interfaces.get('data_models', []))}")
        else:
            _print_error("Design document validation failed:")
            for error in errors:
                console.print(f"  [red]-[/red] {error}")
            raise typer.Exit(1)

else:
    # Fallback when typer is not installed
    design_app = None


def main():
    """CLI entry point for design commands."""
    if HAS_TYPER:
        design_app()
    else:
        print("Design CLI requires 'typer' and 'rich' packages.")
        print("Install with: pip install typer rich")
        sys.exit(1)


if __name__ == "__main__":
    main()
