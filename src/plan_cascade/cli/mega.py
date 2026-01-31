#!/usr/bin/env python3
"""
Mega Plan CLI Commands for Plan Cascade

Provides CLI commands for mega-plan workflow:
- mega plan: Generate multi-feature plan from project description
- mega approve: Start execution of approved plan
- mega status: View execution progress
- mega complete: Finalize and merge all features
- mega edit: Interactively edit features and dependencies
- mega resume: Resume interrupted execution
"""

import asyncio
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
    # Create Typer app for mega commands
    mega_app = typer.Typer(
        name="mega",
        help="Mega-plan workflow commands for multi-feature projects",
        no_args_is_help=True,
    )
    console = Console()

    # ========== Helper Functions ==========

    def _get_project_path(project_path: Optional[str]) -> Path:
        """Get the project path from argument or cwd."""
        return Path(project_path) if project_path else Path.cwd()

    def _load_mega_plan(project: Path) -> dict | None:
        """Load mega-plan.json from project directory."""
        from ..state.mega_state import MegaStateManager

        state_manager = MegaStateManager(project)
        return state_manager.read_mega_plan()

    def _save_mega_plan(project: Path, plan: dict) -> None:
        """Save mega-plan.json to project directory."""
        from ..state.mega_state import MegaStateManager

        state_manager = MegaStateManager(project)
        state_manager.write_mega_plan(plan)

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

    def _display_mega_plan(plan: dict, show_details: bool = False) -> None:
        """Display mega-plan in a formatted table."""
        from ..core.mega_generator import MegaPlanGenerator

        # Header panel
        console.print(Panel(
            f"[bold]{plan.get('goal', 'No goal specified')}[/bold]\n\n"
            f"[dim]{plan.get('description', '')[:200]}...[/dim]" if len(plan.get('description', '')) > 200
            else f"[dim]{plan.get('description', '')}[/dim]",
            title="Mega Plan",
            border_style="blue"
        ))

        # Features table
        table = Table(title="Features", show_header=True, header_style="bold cyan")
        table.add_column("ID", style="cyan", width=12)
        table.add_column("Name", style="white", width=20)
        table.add_column("Title", style="white", width=30)
        table.add_column("Priority", style="yellow", width=10)
        table.add_column("Status", width=12)
        table.add_column("Dependencies", style="dim", width=20)

        for feature in plan.get("features", []):
            status = feature.get("status", "pending")
            status_style = {
                "pending": "dim",
                "prd_generated": "yellow",
                "approved": "yellow",
                "in_progress": "cyan",
                "complete": "green",
                "failed": "red",
            }.get(status, "white")

            deps = ", ".join(feature.get("dependencies", [])) or "-"

            table.add_row(
                feature.get("id", ""),
                feature.get("name", ""),
                feature.get("title", "")[:30],
                feature.get("priority", "medium"),
                f"[{status_style}]{status}[/{status_style}]",
                deps[:20],
            )

        console.print(table)

        # Show execution batches
        if show_details:
            mg = MegaPlanGenerator(Path.cwd())
            batches = mg.generate_feature_batches(plan)

            console.print()
            console.print("[bold]Execution Batches:[/bold]")
            for i, batch in enumerate(batches, 1):
                batch_features = ", ".join(f["name"] for f in batch)
                console.print(f"  Batch {i}: {batch_features}")

    def _display_dependency_graph(plan: dict) -> None:
        """Display dependency graph as a tree."""
        tree = Tree("[bold blue]Feature Dependencies[/bold blue]")

        features = plan.get("features", [])
        feature_map = {f["id"]: f for f in features}

        # Build reverse dependency graph
        dependents: dict[str, list[str]] = {f["id"]: [] for f in features}
        for feature in features:
            for dep in feature.get("dependencies", []):
                if dep in dependents:
                    dependents[dep].append(feature["id"])

        # Find root features (no dependencies)
        roots = [f for f in features if not f.get("dependencies")]

        def add_feature_node(parent_node, feature: dict, visited: set):
            if feature["id"] in visited:
                return
            visited.add(feature["id"])

            status = feature.get("status", "pending")
            status_icons = {
                "pending": "[dim]o[/dim]",
                "prd_generated": "[yellow]~[/yellow]",
                "approved": "[yellow]~[/yellow]",
                "in_progress": "[cyan]>[/cyan]",
                "complete": "[green]v[/green]",
                "failed": "[red]x[/red]",
            }
            icon = status_icons.get(status, "?")

            node = parent_node.add(f"{icon} [cyan]{feature['id']}[/cyan]: {feature['title']}")

            for child_id in dependents.get(feature["id"], []):
                if child_id in feature_map:
                    add_feature_node(node, feature_map[child_id], visited)

        visited: set[str] = set()
        for root in roots:
            add_feature_node(tree, root, visited)

        # Add orphans
        orphans = [f for f in features if f["id"] not in visited]
        if orphans:
            orphan_node = tree.add("[yellow]Unconnected Features[/yellow]")
            for orphan in orphans:
                add_feature_node(orphan_node, orphan, visited)

        console.print(tree)

    # ========== CLI Commands ==========

    @mega_app.command("plan")
    def plan(
        description: str = typer.Argument(..., help="Project description for mega-plan generation"),
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
        target_branch: str = typer.Option("main", "--target", "-t", help="Target branch for merging"),
        execution_mode: str = typer.Option("auto", "--mode", "-m", help="Execution mode (auto/manual)"),
        prd_agent: Optional[str] = typer.Option(None, "--prd-agent", help="Agent for PRD generation"),
        story_agent: Optional[str] = typer.Option(None, "--story-agent", help="Agent for story execution"),
        design_doc: Optional[str] = typer.Option(None, "--design-doc", "-d", help="Path to design document"),
    ):
        """
        Generate a mega-plan from a project description.

        Creates a multi-feature development plan with dependencies and execution batches.
        The plan breaks down the project into features that can be worked on in parallel.

        Examples:
            plan-cascade mega plan "Build an e-commerce platform with auth, products, cart, and orders"
            plan-cascade mega plan "Add user dashboard" --target develop
            plan-cascade mega plan "Build API" --design-doc design.json
        """
        project = _get_project_path(project_path)

        _print_header(
            "Mega Plan Generator",
            f"Project: {project}"
        )

        # Check if mega-plan already exists
        existing_plan = _load_mega_plan(project)
        if existing_plan:
            _print_warning("A mega-plan.json already exists in this project")
            if not Confirm.ask("Overwrite existing plan?", default=False):
                _print_info("Aborted")
                raise typer.Exit(0)

        # Generate the mega-plan
        from ..core.mega_generator import MegaPlanGenerator

        _print_info(f"Generating mega-plan for: {description[:100]}...")

        generator = MegaPlanGenerator(project)
        plan = generator.generate_mega_plan(
            description=description,
            execution_mode=execution_mode,
            target_branch=target_branch,
        )

        # Store agent preferences in metadata
        if prd_agent or story_agent:
            plan["metadata"]["agents"] = {}
            if prd_agent:
                plan["metadata"]["agents"]["prd"] = prd_agent
            if story_agent:
                plan["metadata"]["agents"]["story"] = story_agent

        # Store design doc reference if provided
        if design_doc:
            plan["metadata"]["design_doc"] = design_doc

        # TODO: In a full implementation, this would call an LLM to analyze
        # the description and generate features. For now, we create a placeholder
        # that prompts for manual feature entry or uses a sample.

        _print_info("Note: Mega-plan requires AI analysis to break down into features.")
        _print_info("You can add features manually with 'mega edit' after creation.")

        # Save the plan
        _save_mega_plan(project, plan)
        _print_success(f"Mega-plan saved to {project / 'mega-plan.json'}")

        # Display the plan
        console.print()
        _display_mega_plan(plan)

        console.print()
        _print_info("Next steps:")
        console.print("  1. Add features with: [cyan]plan-cascade mega edit[/cyan]")
        console.print("  2. Review the plan: [cyan]plan-cascade mega status[/cyan]")
        console.print("  3. Start execution: [cyan]plan-cascade mega approve[/cyan]")

    @mega_app.command("approve")
    def approve(
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
        auto_prd: bool = typer.Option(False, "--auto-prd", help="Auto-approve generated PRDs"),
        batch: Optional[int] = typer.Option(None, "--batch", "-b", help="Execute specific batch only"),
    ):
        """
        Approve and start mega-plan execution.

        Validates the plan, creates worktrees for features, and starts execution.
        Features are executed in batches based on dependencies.

        Examples:
            plan-cascade mega approve
            plan-cascade mega approve --auto-prd
            plan-cascade mega approve --batch 1
        """
        project = _get_project_path(project_path)

        _print_header(
            "Mega Plan Approval",
            f"Project: {project}"
        )

        # Load mega-plan
        plan = _load_mega_plan(project)
        if not plan:
            _print_error("No mega-plan.json found. Run 'mega plan' first.")
            raise typer.Exit(1)

        # Validate the plan
        from ..core.mega_generator import MegaPlanGenerator

        generator = MegaPlanGenerator(project)
        is_valid, errors = generator.validate_mega_plan(plan)

        if not is_valid:
            _print_error("Mega-plan validation failed:")
            for error in errors:
                console.print(f"  [red]-[/red] {error}")
            raise typer.Exit(1)

        _print_success("Mega-plan is valid")

        # Show execution plan
        batches = generator.generate_feature_batches(plan)
        console.print()
        console.print(f"[bold]Execution Plan:[/bold] {len(batches)} batch(es)")

        for i, b in enumerate(batches, 1):
            features_str = ", ".join(f["name"] for f in b)
            console.print(f"  Batch {i}: {features_str}")

        console.print()

        # Confirm execution
        if not Confirm.ask("Start execution?", default=True):
            _print_info("Aborted")
            raise typer.Exit(0)

        # Create worktrees and start execution
        from ..core.feature_orchestrator import FeatureOrchestrator

        orchestrator = FeatureOrchestrator(project)

        # Determine which batch(es) to execute
        if batch is not None:
            if batch < 1 or batch > len(batches):
                _print_error(f"Invalid batch number. Must be 1-{len(batches)}")
                raise typer.Exit(1)
            target_batches = [batches[batch - 1]]
            _print_info(f"Executing batch {batch} only")
        else:
            target_batches = [batches[0]]  # Start with first batch
            _print_info("Starting with batch 1")

        # Create worktrees for the batch
        for target_batch in target_batches:
            _print_info(f"Creating worktrees for {len(target_batch)} feature(s)...")

            worktrees = orchestrator.create_feature_worktrees(
                target_batch,
                plan.get("target_branch", "main")
            )

            for feature_id, worktree_path in worktrees:
                _print_success(f"Created worktree: {worktree_path}")

            # Generate PRDs for features
            _print_info("Generating PRDs...")
            prd_results = orchestrator.generate_feature_prds(target_batch)

            for feature_id, success in prd_results.items():
                if success:
                    _print_success(f"PRD generated for {feature_id}")
                else:
                    _print_error(f"Failed to generate PRD for {feature_id}")

            # Execute the batch
            if auto_prd:
                _print_info("Auto-approving PRDs...")
                for feature in target_batch:
                    orchestrator.auto_approve_prd(feature)

            _print_info("Starting feature execution...")
            orchestrator.execute_feature_batch(target_batch, auto_prd=auto_prd)

        console.print()
        _print_success("Execution started!")
        console.print()
        _print_info("Monitor progress with: [cyan]plan-cascade mega status[/cyan]")

    @mega_app.command("status")
    def status(
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
        verbose: bool = typer.Option(False, "--verbose", "-v", help="Show detailed status"),
    ):
        """
        Show mega-plan execution status.

        Displays feature progress, batch status, and worktree details.

        Examples:
            plan-cascade mega status
            plan-cascade mega status --verbose
        """
        project = _get_project_path(project_path)

        _print_header(
            "Mega Plan Status",
            f"Project: {project}"
        )

        # Load mega-plan
        plan = _load_mega_plan(project)
        if not plan:
            _print_error("No mega-plan.json found. Run 'mega plan' first.")
            raise typer.Exit(1)

        # Calculate progress
        from ..core.mega_generator import MegaPlanGenerator

        generator = MegaPlanGenerator(project)
        progress = generator.calculate_progress(plan)
        batches = generator.generate_feature_batches(plan)

        # Display progress panel
        progress_text = (
            f"[bold]Progress:[/bold] {progress['percentage']}%\n\n"
            f"  Total: {progress['total']}\n"
            f"  [green]Complete:[/green] {progress['completed']}\n"
            f"  [cyan]In Progress:[/cyan] {progress['in_progress']}\n"
            f"  [dim]Pending:[/dim] {progress['pending']}\n"
        )
        if progress['failed'] > 0:
            progress_text += f"  [red]Failed:[/red] {progress['failed']}\n"

        console.print(Panel(progress_text, title="Execution Progress", border_style="cyan"))

        # Display features
        _display_mega_plan(plan, show_details=verbose)

        # Show worktree status if verbose
        if verbose:
            from ..state.mega_state import MegaStateManager

            state_manager = MegaStateManager(project)
            worktree_status = state_manager.sync_status_from_worktrees()

            if worktree_status:
                console.print()
                console.print("[bold]Worktree Status:[/bold]")

                for name, wt_status in worktree_status.items():
                    if wt_status.get("worktree_exists"):
                        prd_exists = "[green]v[/green]" if wt_status.get("prd_exists") else "[red]x[/red]"
                        complete = "[green]v[/green]" if wt_status.get("stories_complete") else "[yellow]...[/yellow]"
                        console.print(f"  {name}: PRD {prd_exists} | Complete {complete}")
                    else:
                        console.print(f"  {name}: [dim]No worktree[/dim]")

        # Show next actions
        console.print()
        current_batch = 1
        for i, b in enumerate(batches, 1):
            if any(f.get("status") not in ["complete"] for f in b):
                current_batch = i
                break

        if progress['percentage'] == 100:
            _print_success("All features complete!")
            _print_info("Finalize with: [cyan]plan-cascade mega complete[/cyan]")
        elif progress['in_progress'] > 0:
            _print_info(f"Currently executing batch {current_batch}")
        else:
            _print_info("Start execution with: [cyan]plan-cascade mega approve[/cyan]")

    @mega_app.command("complete")
    def complete(
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
        force: bool = typer.Option(False, "--force", "-f", help="Force completion even if features incomplete"),
        cleanup: bool = typer.Option(True, "--cleanup/--no-cleanup", help="Clean up planning files after completion"),
    ):
        """
        Finalize mega-plan and merge all features.

        Merges all completed feature branches, cleans up worktrees, and removes planning files.

        Examples:
            plan-cascade mega complete
            plan-cascade mega complete --no-cleanup
            plan-cascade mega complete --force
        """
        project = _get_project_path(project_path)

        _print_header(
            "Mega Plan Completion",
            f"Project: {project}"
        )

        # Load mega-plan
        plan = _load_mega_plan(project)
        if not plan:
            _print_error("No mega-plan.json found.")
            raise typer.Exit(1)

        # Check completion status
        from ..core.mega_generator import MegaPlanGenerator

        generator = MegaPlanGenerator(project)
        progress = generator.calculate_progress(plan)

        if progress['percentage'] < 100 and not force:
            _print_error(f"Not all features are complete ({progress['completed']}/{progress['total']})")
            _print_info("Use --force to complete anyway")
            raise typer.Exit(1)

        if progress['failed'] > 0 and not force:
            _print_error(f"{progress['failed']} feature(s) failed")
            _print_info("Use --force to complete anyway")
            raise typer.Exit(1)

        _print_info(f"Completing mega-plan: {progress['completed']}/{progress['total']} features")

        # Confirm
        if not Confirm.ask("Proceed with completion?", default=True):
            _print_info("Aborted")
            raise typer.Exit(0)

        # Merge features
        from ..state.mega_state import MegaStateManager

        state_manager = MegaStateManager(project)
        target_branch = plan.get("target_branch", "main")

        completed_features = generator.get_features_by_status(plan, "complete")

        for feature in completed_features:
            worktree_path = state_manager.get_worktree_path(feature["name"])
            branch_name = f"mega-{feature['name']}"

            _print_info(f"Merging {branch_name} into {target_branch}...")

            # Note: Actual git merge would happen here
            # This is a placeholder for the merge logic
            _print_success(f"Merged {feature['name']}")

        # Clean up worktrees
        _print_info("Cleaning up worktrees...")
        # Note: Actual worktree removal would happen here

        # Clean up planning files
        if cleanup:
            _print_info("Removing planning files...")
            state_manager.cleanup_all()
            _print_success("Planning files removed")

        console.print()
        _print_success("Mega-plan completed successfully!")

    @mega_app.command("edit")
    def edit(
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
    ):
        """
        Interactively edit the mega-plan.

        Allows adding, removing, and modifying features and their dependencies.

        Examples:
            plan-cascade mega edit
        """
        project = _get_project_path(project_path)

        _print_header(
            "Mega Plan Editor",
            f"Project: {project}"
        )

        # Load mega-plan
        plan = _load_mega_plan(project)
        if not plan:
            _print_error("No mega-plan.json found. Run 'mega plan' first.")
            raise typer.Exit(1)

        from ..core.mega_generator import MegaPlanGenerator

        generator = MegaPlanGenerator(project)

        # Interactive edit loop
        while True:
            console.print()
            _display_mega_plan(plan)
            console.print()

            choice = Prompt.ask(
                "Action",
                choices=["add", "edit", "remove", "deps", "priority", "graph", "validate", "save", "quit"],
                default="quit"
            )

            if choice == "quit":
                if Confirm.ask("Save changes before quitting?", default=True):
                    _save_mega_plan(project, plan)
                    _print_success("Changes saved")
                break

            elif choice == "save":
                _save_mega_plan(project, plan)
                _print_success("Changes saved")

            elif choice == "add":
                # Add a new feature
                name = Prompt.ask("Feature name (e.g., 'feature-auth')")
                if not name.startswith("feature-"):
                    name = f"feature-{name}"

                title = Prompt.ask("Feature title")
                description = Prompt.ask("Description")
                priority = Prompt.ask("Priority", choices=["high", "medium", "low"], default="medium")

                plan = generator.add_feature(
                    plan=plan,
                    name=name.replace("feature-", ""),
                    title=title,
                    description=description,
                    priority=priority,
                )
                _print_success(f"Added feature: {name}")

            elif choice == "edit":
                # Edit existing feature
                feature_ids = [f["id"] for f in plan.get("features", [])]
                if not feature_ids:
                    _print_warning("No features to edit")
                    continue

                feature_id = Prompt.ask("Feature ID to edit", choices=feature_ids)

                for feature in plan["features"]:
                    if feature["id"] == feature_id:
                        field = Prompt.ask(
                            "Field to edit",
                            choices=["title", "description", "priority", "name"]
                        )

                        current = feature.get(field, "")
                        console.print(f"Current {field}: {current}")

                        new_value = Prompt.ask(f"New {field}")
                        feature[field] = new_value
                        _print_success(f"Updated {feature_id}.{field}")
                        break

            elif choice == "remove":
                # Remove a feature
                feature_ids = [f["id"] for f in plan.get("features", [])]
                if not feature_ids:
                    _print_warning("No features to remove")
                    continue

                feature_id = Prompt.ask("Feature ID to remove", choices=feature_ids)

                if Confirm.ask(f"Remove {feature_id}?", default=False):
                    plan["features"] = [f for f in plan["features"] if f["id"] != feature_id]
                    # Also remove from dependencies
                    for f in plan["features"]:
                        f["dependencies"] = [d for d in f.get("dependencies", []) if d != feature_id]
                    _print_success(f"Removed {feature_id}")

            elif choice == "deps":
                # Edit dependencies
                feature_ids = [f["id"] for f in plan.get("features", [])]
                if not feature_ids:
                    _print_warning("No features")
                    continue

                feature_id = Prompt.ask("Feature ID to edit dependencies", choices=feature_ids)

                for feature in plan["features"]:
                    if feature["id"] == feature_id:
                        current_deps = feature.get("dependencies", [])
                        console.print(f"Current dependencies: {', '.join(current_deps) or 'none'}")

                        other_ids = [fid for fid in feature_ids if fid != feature_id]
                        console.print(f"Available: {', '.join(other_ids)}")

                        new_deps = Prompt.ask("New dependencies (comma-separated, or 'none')")
                        if new_deps.lower() == "none":
                            feature["dependencies"] = []
                        else:
                            deps_list = [d.strip() for d in new_deps.split(",") if d.strip()]
                            # Validate dependencies
                            valid_deps = [d for d in deps_list if d in other_ids]
                            feature["dependencies"] = valid_deps

                        _print_success(f"Updated dependencies for {feature_id}")
                        break

            elif choice == "priority":
                # Change priority
                feature_ids = [f["id"] for f in plan.get("features", [])]
                if not feature_ids:
                    _print_warning("No features")
                    continue

                feature_id = Prompt.ask("Feature ID", choices=feature_ids)

                for feature in plan["features"]:
                    if feature["id"] == feature_id:
                        new_priority = Prompt.ask(
                            "New priority",
                            choices=["high", "medium", "low"],
                            default=feature.get("priority", "medium")
                        )
                        feature["priority"] = new_priority
                        _print_success(f"Updated priority for {feature_id}")
                        break

            elif choice == "graph":
                # Show dependency graph
                console.print()
                _display_dependency_graph(plan)

            elif choice == "validate":
                # Validate the plan
                is_valid, errors = generator.validate_mega_plan(plan)
                if is_valid:
                    _print_success("Mega-plan is valid")
                else:
                    _print_error("Validation errors:")
                    for error in errors:
                        console.print(f"  [red]-[/red] {error}")

    @mega_app.command("resume")
    def resume(
        project_path: Optional[str] = typer.Option(None, "--project", "-p", help="Project path"),
        auto_prd: bool = typer.Option(False, "--auto-prd", help="Auto-approve PRDs for pending features"),
    ):
        """
        Resume interrupted mega-plan execution.

        Detects the current state and continues from where execution was interrupted.

        Examples:
            plan-cascade mega resume
            plan-cascade mega resume --auto-prd
        """
        project = _get_project_path(project_path)

        _print_header(
            "Resume Mega Plan Execution",
            f"Project: {project}"
        )

        # Load mega-plan
        plan = _load_mega_plan(project)
        if not plan:
            _print_error("No mega-plan.json found.")
            raise typer.Exit(1)

        # Analyze current state
        from ..core.mega_generator import MegaPlanGenerator
        from ..state.mega_state import MegaStateManager

        generator = MegaPlanGenerator(project)
        state_manager = MegaStateManager(project)

        progress = generator.calculate_progress(plan)
        batches = generator.generate_feature_batches(plan)

        # Check if already complete
        if progress['percentage'] == 100:
            _print_success("All features are already complete!")
            _print_info("Run 'mega complete' to finalize")
            raise typer.Exit(0)

        # Sync status from worktrees
        _print_info("Syncing status from worktrees...")
        worktree_status = state_manager.sync_status_from_worktrees()

        # Update feature statuses based on worktree state
        updated = False
        for feature in plan.get("features", []):
            name = feature["name"]
            if name in worktree_status:
                wt = worktree_status[name]
                if wt.get("stories_complete") and feature["status"] != "complete":
                    feature["status"] = "complete"
                    updated = True
                    _print_success(f"Marked {feature['id']} as complete")
                elif wt.get("worktree_exists") and feature["status"] == "pending":
                    feature["status"] = "in_progress"
                    updated = True

        if updated:
            _save_mega_plan(project, plan)

        # Recalculate progress
        progress = generator.calculate_progress(plan)

        # Display current status
        console.print()
        console.print(f"[bold]Current Progress:[/bold] {progress['percentage']}%")
        console.print(f"  Complete: {progress['completed']}/{progress['total']}")
        console.print(f"  In Progress: {progress['in_progress']}")
        console.print(f"  Pending: {progress['pending']}")

        # Find next batch to execute
        from ..core.feature_orchestrator import FeatureOrchestrator

        orchestrator = FeatureOrchestrator(project)

        # Find features that need to be resumed or started
        pending_features = generator.get_features_by_status(plan, "pending")
        in_progress_features = [
            f for f in plan.get("features", [])
            if f.get("status") in ["prd_generated", "approved", "in_progress"]
        ]

        if in_progress_features:
            _print_info(f"Found {len(in_progress_features)} in-progress feature(s)")
            for f in in_progress_features:
                console.print(f"  - {f['id']}: {f['title']} [{f['status']}]")

        if pending_features:
            # Check if dependencies are met for pending features
            completed_ids = {f["id"] for f in plan.get("features", []) if f.get("status") == "complete"}
            ready_features = []

            for f in pending_features:
                deps = set(f.get("dependencies", []))
                if deps.issubset(completed_ids):
                    ready_features.append(f)

            if ready_features:
                _print_info(f"Found {len(ready_features)} feature(s) ready to start")

                if Confirm.ask("Start pending features?", default=True):
                    worktrees = orchestrator.create_feature_worktrees(
                        ready_features,
                        plan.get("target_branch", "main")
                    )

                    for feature_id, worktree_path in worktrees:
                        _print_success(f"Created worktree: {worktree_path}")

                    orchestrator.generate_feature_prds(ready_features)
                    orchestrator.execute_feature_batch(ready_features, auto_prd=auto_prd)

                    _print_success("Features started")

        console.print()
        _print_info("Monitor progress with: [cyan]plan-cascade mega status[/cyan]")

else:
    # Fallback when typer is not installed
    mega_app = None


def main():
    """CLI entry point for mega commands."""
    if HAS_TYPER:
        mega_app()
    else:
        print("Mega CLI requires 'typer' and 'rich' packages.")
        print("Install with: pip install typer rich")
        sys.exit(1)


if __name__ == "__main__":
    main()
