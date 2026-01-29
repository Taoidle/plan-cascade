#!/usr/bin/env python3
"""
Plan Cascade CLI

Command-line interface for Plan Cascade with dual-mode support:
- Simple mode (default): AI-driven automatic execution
- Expert mode (--expert): Interactive PRD editing and agent selection

Commands:
- plan-cascade run <description>: Execute a development task
- plan-cascade config: Configuration management
- plan-cascade status: View execution status
"""

import asyncio
import sys
from pathlib import Path

try:
    import typer
    from rich.console import Console
    from rich.prompt import Confirm, Prompt
    HAS_TYPER = True
except ImportError:
    HAS_TYPER = False

from .. import __version__
from .output import OutputManager

if HAS_TYPER:
    app = typer.Typer(
        name="plan-cascade",
        help="Plan Cascade - AI-driven development made simple",
        add_completion=False,
        no_args_is_help=True,
    )
    console = Console()
    output = OutputManager(console)

    @app.command()
    def run(
        description: str = typer.Argument(..., help="Task description"),
        expert: bool = typer.Option(False, "--expert", "-e", help="Expert mode with PRD editing"),
        backend: str | None = typer.Option(None, "--backend", "-b", help="Backend selection (claude-code, builtin)"),
        project_path: str | None = typer.Option(None, "--project", "-p", help="Project path"),
        provider: str | None = typer.Option(None, "--provider", help="LLM provider for builtin backend"),
        model: str | None = typer.Option(None, "--model", "-m", help="Model to use"),
        verbose: bool = typer.Option(False, "--verbose", "-v", help="Verbose output"),
    ):
        """
        Execute a development task.

        Simple mode (default): AI automatically analyzes, plans, and executes.
        Expert mode (--expert): Generate PRD for review/edit before execution.

        Examples:
            plan-cascade run "Add login functionality"
            plan-cascade run "Build REST API" --expert
            plan-cascade run "Fix bug in auth" --backend builtin --provider openai
        """
        project = Path(project_path) if project_path else Path.cwd()

        # Print header
        output.print_header(
            f"Plan Cascade v{__version__}",
            f"Project: {project}"
        )
        output.print_info(f"Mode: {'Expert' if expert else 'Simple'}")

        if expert:
            asyncio.run(_run_expert(
                description=description,
                project=project,
                backend=backend,
                provider=provider,
                model=model,
                verbose=verbose,
            ))
        else:
            asyncio.run(_run_simple(
                description=description,
                project=project,
                backend=backend,
                provider=provider,
                model=model,
                verbose=verbose,
            ))

    async def _run_simple(
        description: str,
        project: Path,
        backend: str | None = None,
        provider: str | None = None,
        model: str | None = None,
        verbose: bool = False,
    ):
        """Execute in simple mode."""
        from ..core.simple_workflow import ProgressEvent, SimpleWorkflow

        # Create backend
        backend_instance = _create_backend(backend, provider, model, project)

        # Progress callback
        def on_progress(event: ProgressEvent):
            output.handle_progress_event(event)

        # Create and run workflow
        workflow = SimpleWorkflow(
            backend=backend_instance,
            project_path=project,
            on_progress=on_progress,
        )

        output.print_info(f"Processing: {description}")
        output.print()

        with output.spinner("Analyzing task..."):
            pass  # Spinner shows while strategy is being analyzed

        result = await workflow.run(description)

        output.print()
        output.workflow_result(result)

        if not result.success:
            sys.exit(1)

    async def _run_expert(
        description: str,
        project: Path,
        backend: str | None = None,
        provider: str | None = None,
        model: str | None = None,
        verbose: bool = False,
    ):
        """Execute in expert mode."""
        from ..core.expert_workflow import ExpertWorkflow
        from ..core.strategy_analyzer import ExecutionStrategy

        # Create backend
        backend_instance = _create_backend(backend, provider, model, project)

        # Create workflow
        workflow = ExpertWorkflow(
            backend=backend_instance,
            project_path=project,
        )

        # Start workflow
        await workflow.start(description)

        # Analyze strategy
        output.print_info("Analyzing task...")
        decision = await workflow.analyze_strategy()
        output.strategy_decision(decision)

        # Ask for strategy confirmation
        strategy_choices = ["direct", "hybrid", "mega"]
        selected = Prompt.ask(
            "\nSelect strategy",
            choices=strategy_choices,
            default=decision.strategy.value.replace("_auto", "").replace("_plan", ""),
        )

        strategy_map = {
            "direct": ExecutionStrategy.DIRECT,
            "hybrid": ExecutionStrategy.HYBRID_AUTO,
            "mega": ExecutionStrategy.MEGA_PLAN,
        }
        workflow.select_strategy(strategy_map[selected], "User selection")

        # Generate PRD
        output.print_info("\nGenerating PRD...")
        with output.spinner("Generating PRD..."):
            prd = await workflow.generate_prd()

        output.prd_table(prd)

        # Interactive menu
        while True:
            output.print()
            choice = Prompt.ask(
                "Action",
                choices=["view", "edit", "graph", "validate", "run", "save", "quit"],
                default="run",
            )

            if choice == "view":
                output.prd_table(workflow.state.prd)
                _show_story_details(workflow.state.prd)

            elif choice == "edit":
                await _edit_prd_interactive(workflow, output)

            elif choice == "graph":
                output.dependency_tree(workflow.state.prd)

            elif choice == "validate":
                errors = workflow.validate_prd()
                if errors:
                    output.print_error("Validation errors:")
                    for error in errors:
                        output.print(f"  - {error}")
                else:
                    output.print_success("PRD is valid")

            elif choice == "run":
                if Confirm.ask("Start execution?", default=True):
                    output.print_info("\nExecuting stories...")
                    summary = await workflow.execute_all()

                    output.print()
                    if summary["success"]:
                        output.print_success(
                            f"Completed {summary['completed']}/{summary['total_stories']} stories"
                        )
                    else:
                        output.print_error(
                            f"Failed: {summary['failed']} stories failed"
                        )
                    break

            elif choice == "save":
                path = workflow.save_prd()
                output.print_success(f"PRD saved to {path}")

            elif choice == "quit":
                break

    def _create_backend(
        backend: str | None,
        provider: str | None,
        model: str | None,
        project: Path,
    ):
        """Create a backend instance based on options."""
        # Try to import backend factory
        try:
            from ..backends.factory import BackendFactory

            config = {
                "backend": backend or "claude-code",
                "project_root": str(project),
            }

            if backend == "builtin":
                config["provider"] = provider or "claude"
                if model:
                    config["model"] = model

                # Try to get API key from environment or keyring
                import os
                provider_name = config["provider"]
                env_var_map = {
                    "claude": "ANTHROPIC_API_KEY",
                    "openai": "OPENAI_API_KEY",
                    "deepseek": "DEEPSEEK_API_KEY",
                }
                env_var = env_var_map.get(provider_name)
                if env_var:
                    config["api_key"] = os.environ.get(env_var, "")

            return BackendFactory.create(config)

        except ImportError:
            # Fallback to mock backend for testing
            return _create_mock_backend()

    def _create_mock_backend():
        """Create a mock backend for testing when real backends unavailable."""
        from dataclasses import dataclass
        from typing import Any

        @dataclass
        class MockResult:
            success: bool = True
            output: str = "Mock execution completed"
            iterations: int = 1
            error: str | None = None

        class MockLLM:
            async def complete(self, messages, **kwargs):
                @dataclass
                class Response:
                    content: str = '{"strategy": "hybrid_auto", "use_worktree": false, "estimated_stories": 3, "confidence": 0.8, "reasoning": "Mock analysis"}'

                return Response()

        class MockBackend:
            def __init__(self):
                self._llm = MockLLM()

            async def execute(self, story: dict[str, Any], context: str = "") -> MockResult:
                return MockResult()

            def get_llm(self):
                return self._llm

            def get_name(self) -> str:
                return "mock"

        return MockBackend()

    def _show_story_details(prd):
        """Show detailed story information."""
        for story in prd.stories:
            output.print()
            output.print(f"[bold cyan]{story['id']}[/bold cyan]: {story['title']}")
            output.print(f"  [dim]Priority:[/dim] {story.get('priority', 'medium')}")
            output.print(f"  [dim]Status:[/dim] {story.get('status', 'pending')}")

            deps = story.get('dependencies', [])
            if deps:
                output.print(f"  [dim]Dependencies:[/dim] {', '.join(deps)}")

            ac = story.get('acceptance_criteria', [])
            if ac:
                output.print("  [dim]Acceptance Criteria:[/dim]")
                for criterion in ac:
                    output.print(f"    - {criterion}")

    async def _edit_prd_interactive(workflow, output):
        """Interactive PRD editing."""
        while True:
            edit_choice = Prompt.ask(
                "Edit",
                choices=["story", "add", "remove", "back"],
                default="back",
            )

            if edit_choice == "back":
                break

            elif edit_choice == "story":
                story_ids = [s["id"] for s in workflow.state.prd.stories]
                story_id = Prompt.ask(
                    "Story ID to edit",
                    choices=story_ids,
                )

                field = Prompt.ask(
                    "Field to edit",
                    choices=["title", "description", "priority", "dependencies"],
                )

                story = workflow.state.prd.get_story(story_id)
                current = story.get(field, "")
                output.print(f"Current {field}: {current}")

                new_value = Prompt.ask(f"New {field}")

                if field == "dependencies":
                    new_value = [d.strip() for d in new_value.split(",") if d.strip()]

                workflow.edit_story(story_id, {field: new_value})
                output.print_success(f"Updated {story_id}.{field}")

            elif edit_choice == "add":
                title = Prompt.ask("Story title")
                description = Prompt.ask("Description")
                priority = Prompt.ask("Priority", choices=["high", "medium", "low"], default="medium")

                workflow.add_story({
                    "title": title,
                    "description": description,
                    "priority": priority,
                    "acceptance_criteria": [],
                })
                output.print_success("Story added")

            elif edit_choice == "remove":
                story_ids = [s["id"] for s in workflow.state.prd.stories]
                story_id = Prompt.ask("Story ID to remove", choices=story_ids)

                if Confirm.ask(f"Remove {story_id}?"):
                    workflow.remove_story(story_id)
                    output.print_success(f"Removed {story_id}")

    @app.command()
    def config(
        show: bool = typer.Option(False, "--show", help="Show current configuration"),
        setup: bool = typer.Option(False, "--setup", help="Run configuration wizard"),
        set_backend: str | None = typer.Option(None, "--backend", help="Set backend"),
        set_provider: str | None = typer.Option(None, "--provider", help="Set provider"),
        set_key: str | None = typer.Option(None, "--api-key", help="Set API key"),
    ):
        """
        Configuration management.

        Examples:
            plan-cascade config --show
            plan-cascade config --setup
            plan-cascade config --backend builtin --provider openai
        """
        if show:
            _show_config()
        elif setup:
            _run_setup_wizard()
        elif set_backend or set_provider or set_key:
            _update_config(set_backend, set_provider, set_key)
        else:
            output.print("Use --show to view configuration or --setup to run wizard")

    def _show_config():
        """Show current configuration."""
        try:
            from ..settings.storage import SettingsStorage
            storage = SettingsStorage()
            settings = storage.load()

            config = {
                "Backend": getattr(settings, "backend", "claude-code"),
                "Provider": getattr(settings, "provider", "claude"),
                "Model": getattr(settings, "model", "(default)") or "(default)",
                "Default Mode": getattr(settings, "default_mode", "simple"),
            }
            output.config_display(config)

        except ImportError:
            output.print_warning("Settings module not available")
            output.config_display({
                "Backend": "claude-code (default)",
                "Provider": "claude",
                "Model": "(default)",
            })

    def _run_setup_wizard():
        """Run the configuration wizard."""
        output.print_header("Plan Cascade Configuration Wizard")

        # Step 1: Backend selection
        output.print("[bold]Step 1: Select Backend[/bold]")
        output.print("  1. Claude Code (recommended, no API key needed)")
        output.print("  2. Builtin (direct LLM API, requires API key)")

        backend_choice = Prompt.ask("Selection", choices=["1", "2"], default="1")

        if backend_choice == "1":
            backend = "claude-code"
            provider = None
            api_key = None
        else:
            backend = "builtin"

            # Step 2: Provider selection
            output.print("\n[bold]Step 2: Select LLM Provider[/bold]")
            output.print("  1. Claude (Anthropic)")
            output.print("  2. OpenAI")
            output.print("  3. DeepSeek")
            output.print("  4. Ollama (local)")

            provider_choice = Prompt.ask("Selection", choices=["1", "2", "3", "4"], default="1")
            provider_map = {"1": "claude", "2": "openai", "3": "deepseek", "4": "ollama"}
            provider = provider_map[provider_choice]

            # Step 3: API Key (if needed)
            if provider != "ollama":
                output.print(f"\n[bold]Step 3: API Key for {provider}[/bold]")
                api_key = Prompt.ask("API Key", password=True)
            else:
                api_key = None

        # Save configuration
        try:
            from ..settings.storage import SettingsStorage
            storage = SettingsStorage()
            settings = storage.load()
            settings.backend = backend
            if provider:
                settings.provider = provider
            storage.save(settings)

            if api_key:
                storage.set_api_key(provider, api_key)

            output.print_success("\nConfiguration saved!")

        except ImportError:
            output.print_warning("\nSettings module not available - configuration not persisted")
            output.print_info(f"Selected: backend={backend}, provider={provider or 'N/A'}")

    def _update_config(backend: str | None, provider: str | None, api_key: str | None):
        """Update specific configuration values."""
        try:
            from ..settings.storage import SettingsStorage
            storage = SettingsStorage()
            settings = storage.load()

            if backend:
                settings.backend = backend
                output.print_success(f"Backend set to: {backend}")

            if provider:
                settings.provider = provider
                output.print_success(f"Provider set to: {provider}")

            storage.save(settings)

            if api_key and provider:
                storage.set_api_key(provider, api_key)
                output.print_success(f"API key set for: {provider}")

        except ImportError:
            output.print_error("Settings module not available")

    @app.command()
    def status(
        project_path: str | None = typer.Option(None, "--project", "-p", help="Project path"),
    ):
        """
        View execution status.

        Shows the current state of any running or recent tasks.
        """
        project = Path(project_path) if project_path else Path.cwd()

        try:
            from ..state.state_manager import StateManager
            state_manager = StateManager(project)

            # Read PRD status
            prd = state_manager.read_prd()
            if not prd:
                output.print_info("No PRD found in this project")
                return

            # Show PRD info
            output.print_header("Task Status", f"Project: {project}")

            # Get story statuses
            stories = prd.get("stories", [])
            completed = sum(1 for s in stories if s.get("status") == "complete")
            in_progress = sum(1 for s in stories if s.get("status") == "in_progress")
            failed = sum(1 for s in stories if s.get("status") == "failed")
            pending = sum(1 for s in stories if s.get("status") == "pending")

            status_info = {
                "Total Stories": len(stories),
                "Completed": completed,
                "In Progress": in_progress,
                "Failed": failed,
                "Pending": pending,
            }
            output.status_panel(status_info)

            # Show stories
            output.stories_table(stories)

            # Show agent status if available
            try:
                agent_status = state_manager.read_agent_status()
                running = agent_status.get("running", [])
                if running:
                    output.print("\n[bold]Running Agents:[/bold]")
                    for agent in running:
                        output.print(f"  - {agent['story_id']}: {agent['agent']}")
            except Exception:
                pass

        except ImportError:
            output.print_warning("State manager not available")
            _show_basic_status(project)

    def _show_basic_status(project: Path):
        """Show basic status from prd.json file."""
        prd_path = project / "prd.json"
        if not prd_path.exists():
            output.print_info("No prd.json found in this project")
            return

        import json
        with open(prd_path) as f:
            prd = json.load(f)

        stories = prd.get("stories", [])
        output.print_header("Task Status (Basic)", f"PRD: {prd_path}")

        for story in stories:
            status = story.get("status", "pending")
            icon = output.ICONS.get(status, "")
            output.print(f"  {icon} {story.get('id')}: {story.get('title')}")

    @app.command()
    def version():
        """Show version information."""
        try:
            from .. import __version__
            ver = __version__
        except ImportError:
            ver = "1.0.0"

        output.print(f"Plan Cascade v{ver}")
        output.print("[dim]AI-driven development made simple[/dim]")

else:
    # Fallback when typer is not installed
    app = None


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
