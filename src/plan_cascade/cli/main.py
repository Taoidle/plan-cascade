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
- plan-cascade deps: Display dependency graph visualization
- plan-cascade mega: Mega-plan workflow for multi-feature projects
  - mega plan: Generate multi-feature plan from project description
  - mega approve: Start execution of approved plan
  - mega status: View execution progress
  - mega complete: Finalize and merge all features
  - mega edit: Interactively edit features and dependencies
  - mega resume: Resume interrupted execution
- plan-cascade skills: External skill management
  - skills list: Show all configured skills
  - skills detect: Detect applicable skills for current project
  - skills show <name>: Display skill's SKILL.md content
  - skills summary: Show execution summary with loaded skills
  - skills validate: Validate skill configuration and availability
"""

import asyncio
import sys
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ..state.path_resolver import PathResolver

try:
    import typer
    from rich.console import Console
    from rich.prompt import Confirm, Prompt
    HAS_TYPER = True
except ImportError:
    HAS_TYPER = False

from .. import __version__
from .context import CLIContext
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

    @app.callback()
    def main_callback(
        ctx: typer.Context,
        legacy_mode: bool = typer.Option(
            False,
            "--legacy-mode/--no-legacy-mode",
            help="Use legacy mode for file paths (store files in project root instead of user directory)",
            envvar="PLAN_CASCADE_LEGACY_MODE",
        ),
    ):
        """
        Plan Cascade - AI-driven development made simple.

        Global Options:
            --legacy-mode: Store planning files (prd.json, mega-plan.json, etc.)
                           in the project root directory instead of the platform-specific
                           user directory (~/.plan-cascade on Unix, %APPDATA%/plan-cascade on Windows).
                           This is useful for backward compatibility with existing projects
                           or when you want planning files to be part of the project.
        """
        # Create CLI context with global options
        ctx.obj = CLIContext.from_options(
            legacy_mode=legacy_mode,
            project_root=Path.cwd(),
        )

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

        # Set up streaming output callback for real-time AI response display
        streaming_started = [False]  # Use list to allow mutation in closure

        def on_text(text: str):
            """Stream AI text output to console in real-time."""
            if not streaming_started[0]:
                # Add newline before first streaming output
                console.print()
                streaming_started[0] = True
            console.print(text, end="", highlight=False)
            sys.stdout.flush()  # Force flush for true real-time streaming

        def on_tool_call(data: dict):
            """Display tool calls when in verbose mode."""
            if verbose:
                tool_name = data.get("name", data.get("type", "unknown"))
                if tool_name == "tool_result":
                    is_error = data.get("is_error", False)
                    if is_error:
                        output.print_error(f"    Tool error")
                else:
                    output.print(f"  [dim]> Tool: {tool_name}[/dim]")

        def on_strategy_text(text: str):
            """Stream strategy analysis output in real-time."""
            console.print(text, end="", highlight=False)
            sys.stdout.flush()

        # Attach streaming callbacks to backend
        backend_instance.on_text = on_text
        backend_instance.on_tool_call = on_tool_call

        # Progress callback
        def on_progress(event: ProgressEvent):
            # Add newline after streaming output ends (before story_completed)
            if event.type in ("story_completed", "story_failed") and streaming_started[0]:
                console.print()  # End streaming output with newline
                streaming_started[0] = False  # Reset for next story
            output.handle_progress_event(event)

        # Create and run workflow
        workflow = SimpleWorkflow(
            backend=backend_instance,
            project_path=project,
            on_progress=on_progress,
            on_strategy_text=on_strategy_text,
            use_llm_strategy=True,  # Enable LLM-based strategy analysis
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
                    choices=["title", "description", "priority", "dependencies", "agent"],
                )

                story = workflow.state.prd.get_story(story_id)
                current = story.get(field, "")
                output.print(f"Current {field}: {current}")

                if field == "agent":
                    # Show available agents and let user select
                    available_agents = ["claude-code", "aider", "codex", "cursor-cli", "amp-code"]
                    output.print("[dim]Available agents:[/dim]")
                    for agent in available_agents:
                        marker = "[green]*[/green]" if agent == current else " "
                        output.print(f"  {marker} {agent}")
                    new_value = Prompt.ask(
                        f"Select agent",
                        choices=available_agents,
                        default=current if current in available_agents else "claude-code",
                    )
                elif field == "dependencies":
                    new_value = Prompt.ask(f"New {field}")
                    new_value = [d.strip() for d in new_value.split(",") if d.strip()]
                else:
                    new_value = Prompt.ask(f"New {field}")

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

        # Step 4: Quality Gate Configuration
        output.print("\n[bold]Step 4: Quality Gate Configuration[/bold]")
        output.print("Quality gates validate code after each story completion.")

        configure_gates = Confirm.ask("Configure quality gates?", default=True)

        if configure_gates:
            from ..settings.models import QualityGateConfig
            quality_gates = QualityGateConfig()

            # Toggle typecheck
            quality_gates.typecheck = Confirm.ask(
                "  Enable type checking (mypy/pyright)?",
                default=True
            )

            # Toggle test
            quality_gates.test = Confirm.ask(
                "  Enable test execution?",
                default=True
            )

            # Toggle lint
            quality_gates.lint = Confirm.ask(
                "  Enable code linting (ruff/flake8)?",
                default=True
            )

            # Custom validation script
            use_custom = Confirm.ask(
                "  Add custom validation script?",
                default=False
            )
            if use_custom:
                quality_gates.custom = True
                quality_gates.custom_script = Prompt.ask(
                    "    Script path",
                    default=""
                )
            else:
                quality_gates.custom = False
                quality_gates.custom_script = ""

            # Max retries
            max_retries_str = Prompt.ask(
                "  Max retry attempts on failure",
                default="3"
            )
            try:
                quality_gates.max_retries = int(max_retries_str)
            except ValueError:
                quality_gates.max_retries = 3
                output.print_warning("    Invalid number, using default: 3")
        else:
            quality_gates = None

        # Save configuration
        try:
            from ..settings.storage import SettingsStorage
            storage = SettingsStorage()
            settings = storage.load()
            settings.backend = backend
            if provider:
                settings.provider = provider
            if quality_gates:
                settings.quality_gates = quality_gates
            storage.save(settings)

            if api_key:
                storage.set_api_key(provider, api_key)

            output.print_success("\nConfiguration saved!")

            # Show quality gate summary if configured
            if quality_gates:
                output.print_info("Quality gates configured:")
                output.print(f"  Typecheck: {'enabled' if quality_gates.typecheck else 'disabled'}")
                output.print(f"  Test: {'enabled' if quality_gates.test else 'disabled'}")
                output.print(f"  Lint: {'enabled' if quality_gates.lint else 'disabled'}")
                if quality_gates.custom:
                    output.print(f"  Custom script: {quality_gates.custom_script}")
                output.print(f"  Max retries: {quality_gates.max_retries}")

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
        ctx: typer.Context,
        project_path: str | None = typer.Option(None, "--project", "-p", help="Project path"),
    ):
        """
        View execution status.

        Shows the current state of any running or recent tasks.
        """
        from .context import get_cli_context

        project = Path(project_path) if project_path else Path.cwd()

        # Get CLI context and configure PathResolver for correct project
        cli_ctx = get_cli_context(ctx)
        # Update project root if specified via --project
        if project_path:
            cli_ctx = CLIContext.from_options(
                legacy_mode=cli_ctx.legacy_mode,
                project_root=project,
            )
        path_resolver = cli_ctx.get_path_resolver()

        try:
            from ..state.state_manager import StateManager
            state_manager = StateManager(project, path_resolver=path_resolver)

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
    def agents(
        list_agents: bool = typer.Option(False, "--list", "-l", help="List all configured agents with status"),
        add: str | None = typer.Option(None, "--add", "-a", help="Add a new agent by name"),
        command: str | None = typer.Option(None, "--command", "-c", help="Command path for new agent (use with --add)"),
        remove: str | None = typer.Option(None, "--remove", "-r", help="Remove an agent by name"),
        default: str | None = typer.Option(None, "--default", "-d", help="Set default agent"),
        test: str | None = typer.Option(None, "--test", "-t", help="Test if an agent is available"),
    ):
        """
        Agent configuration management.

        Manage execution agents - list, add, remove, set defaults, and test availability.

        Examples:
            plan-cascade agents --list
            plan-cascade agents --add my-agent --command /path/to/agent
            plan-cascade agents --remove my-agent
            plan-cascade agents --default claude-code
            plan-cascade agents --test aider
        """
        # Handle --list option
        if list_agents:
            _list_agents()
            return

        # Handle --add option (requires --command)
        if add:
            if not command:
                output.print_error("--command is required when adding an agent")
                output.print("[dim]Usage: plan-cascade agents --add <name> --command <path>[/dim]")
                raise typer.Exit(1)
            _add_agent(add, command)
            return

        # Handle --remove option
        if remove:
            _remove_agent(remove)
            return

        # Handle --default option
        if default:
            _set_default_agent(default)
            return

        # Handle --test option
        if test:
            _test_agent(test)
            return

        # No option provided - show usage hint
        output.print("Agent management commands:")
        output.print("  [cyan]--list[/cyan]                    List all agents with availability status")
        output.print("  [cyan]--add <name> --command <path>[/cyan]  Add a new agent")
        output.print("  [cyan]--remove <name>[/cyan]           Remove an agent")
        output.print("  [cyan]--default <name>[/cyan]          Set the default agent")
        output.print("  [cyan]--test <name>[/cyan]             Test agent availability")
        output.print()
        output.print("[dim]Example: plan-cascade agents --list[/dim]")

    def _list_agents():
        """List all configured agents with availability status."""
        from rich.table import Table

        try:
            from ..backends.cross_platform_detector import CrossPlatformDetector
            from ..settings.storage import SettingsStorage

            storage = SettingsStorage()
            settings = storage.load()

            # Create detector for availability checks
            detector = CrossPlatformDetector()

            # Build table
            table = Table(title="Configured Agents", show_header=True, header_style="bold cyan")
            table.add_column("Name", style="cyan")
            table.add_column("Command", style="white")
            table.add_column("Enabled", justify="center")
            table.add_column("Default", justify="center")
            table.add_column("Available", justify="center")

            for agent in settings.agents:
                # Check availability using command or agent name
                agent_info = detector.detect_agent(agent.command or agent.name)

                enabled_icon = "[green]Yes[/green]" if agent.enabled else "[red]No[/red]"
                default_icon = "[yellow]*[/yellow]" if agent.is_default else ""
                available_icon = "[green]Yes[/green]" if agent_info.available else "[red]No[/red]"

                table.add_row(
                    agent.name,
                    agent.command or "(not set)",
                    enabled_icon,
                    default_icon,
                    available_icon,
                )

            console.print()
            console.print(table)
            console.print()

            # Show summary
            enabled_count = sum(1 for a in settings.agents if a.enabled)
            output.print(f"[dim]Total: {len(settings.agents)} agents, {enabled_count} enabled[/dim]")
            output.print(f"[dim]Default agent: {settings.default_agent}[/dim]")

        except ImportError as e:
            output.print_error(f"Required module not available: {e}")

    def _add_agent(name: str, command: str):
        """Add a new agent configuration."""
        try:
            from ..settings.models import AgentConfig
            from ..settings.storage import SettingsStorage

            storage = SettingsStorage()
            settings = storage.load()

            # Check if agent already exists
            existing = settings.get_agent_by_name(name)
            if existing:
                output.print_error(f"Agent '{name}' already exists")
                output.print("[dim]Use --remove first if you want to replace it[/dim]")
                return

            # Create new agent config
            new_agent = AgentConfig(
                name=name,
                enabled=True,
                command=command,
                is_default=False,
            )

            # Add to settings
            settings.agents.append(new_agent)
            storage.save(settings)

            output.print_success(f"Agent '{name}' added successfully")
            output.print(f"  [dim]Command: {command}[/dim]")

            # Test availability
            try:
                from ..backends.cross_platform_detector import CrossPlatformDetector
                detector = CrossPlatformDetector()
                agent_info = detector.detect_agent(command)
                if agent_info.available:
                    output.print_success(f"  Agent is available at: {agent_info.path}")
                    if agent_info.version:
                        output.print(f"  [dim]Version: {agent_info.version}[/dim]")
                else:
                    output.print_warning("  Agent command not found in PATH")
            except ImportError:
                pass

        except ImportError as e:
            output.print_error(f"Settings module not available: {e}")

    def _remove_agent(name: str):
        """Remove an agent configuration."""
        try:
            from ..settings.storage import SettingsStorage

            storage = SettingsStorage()
            settings = storage.load()

            # Check if agent exists
            existing = settings.get_agent_by_name(name)
            if not existing:
                output.print_error(f"Agent '{name}' not found")
                output.print("[dim]Use --list to see available agents[/dim]")
                return

            # Prevent removing default agent
            if existing.is_default:
                output.print_error(f"Cannot remove default agent '{name}'")
                output.print("[dim]Set a different default first with --default <name>[/dim]")
                return

            # Remove agent
            settings.agents = [a for a in settings.agents if a.name != name]
            storage.save(settings)

            output.print_success(f"Agent '{name}' removed successfully")

        except ImportError as e:
            output.print_error(f"Settings module not available: {e}")

    def _set_default_agent(name: str):
        """Set the default agent."""
        try:
            from ..settings.storage import SettingsStorage

            storage = SettingsStorage()
            settings = storage.load()

            # Check if agent exists
            existing = settings.get_agent_by_name(name)
            if not existing:
                output.print_error(f"Agent '{name}' not found")
                output.print("[dim]Use --list to see available agents, or --add to create one[/dim]")
                return

            # Check if agent is enabled
            if not existing.enabled:
                output.print_warning(f"Agent '{name}' is disabled. Enabling it...")
                existing.enabled = True

            # Update default flags
            for agent in settings.agents:
                agent.is_default = (agent.name == name)

            # Update settings
            settings.default_agent = name
            storage.save(settings)

            output.print_success(f"Default agent set to '{name}'")

        except ImportError as e:
            output.print_error(f"Settings module not available: {e}")

    def _test_agent(name: str):
        """Test if an agent is available and can be executed."""
        try:
            from ..backends.cross_platform_detector import CrossPlatformDetector
            from ..settings.storage import SettingsStorage

            storage = SettingsStorage()
            settings = storage.load()

            # Get agent config if it exists
            agent_config = settings.get_agent_by_name(name)
            command_to_test = agent_config.command if agent_config else name

            output.print_info(f"Testing agent: {name}")
            output.print(f"  [dim]Command: {command_to_test}[/dim]")

            # Use detector to check availability
            detector = CrossPlatformDetector()
            agent_info = detector.detect_agent(command_to_test, force_refresh=True)

            output.print()
            if agent_info.available:
                output.print_success("Agent is available!")
                output.print(f"  [cyan]Path:[/cyan] {agent_info.path}")
                if agent_info.version:
                    output.print(f"  [cyan]Version:[/cyan] {agent_info.version}")
                output.print(f"  [cyan]Platform:[/cyan] {agent_info.platform.value if agent_info.platform else 'unknown'}")
                output.print(f"  [cyan]Detection method:[/cyan] {agent_info.detection_method}")
            else:
                output.print_error("Agent is NOT available")
                output.print()
                output.print("[dim]Possible reasons:[/dim]")
                output.print("  - Command not found in PATH")
                output.print("  - Agent not installed")
                output.print("  - Incorrect command path")
                output.print()
                if not agent_config:
                    output.print("[dim]Tip: Add the agent with --add to configure a custom path[/dim]")

        except ImportError as e:
            output.print_error(f"Detection module not available: {e}")

    @app.command(name="auto-run")
    def auto_run(
        ctx: typer.Context,
        mode: str = typer.Option(
            "until_complete",
            "--mode", "-m",
            help="Iteration mode: until_complete, max_iterations, batch_complete"
        ),
        max_iterations: int = typer.Option(
            10,
            "--max-iterations",
            help="Maximum iterations (for max_iterations mode)"
        ),
        agent: str | None = typer.Option(
            None,
            "--agent", "-a",
            help="Default agent for story execution"
        ),
        impl_agent: str | None = typer.Option(
            None,
            "--impl-agent",
            help="Agent for implementation stories"
        ),
        retry_agent: str | None = typer.Option(
            None,
            "--retry-agent",
            help="Agent to use for retry attempts"
        ),
        dry_run: bool = typer.Option(
            False,
            "--dry-run",
            help="Show execution plan without actually running"
        ),
        no_quality_gates: bool = typer.Option(
            False,
            "--no-quality-gates",
            help="Disable quality gates (typecheck, test, lint)"
        ),
        no_fallback: bool = typer.Option(
            False,
            "--no-fallback",
            help="Disable agent fallback on failure"
        ),
        parallel: bool = typer.Option(
            False,
            "--parallel",
            help="Execute stories within batches in parallel"
        ),
        max_concurrency: int | None = typer.Option(
            None,
            "--max-concurrency",
            help="Maximum parallel stories (default: CPU count)"
        ),
        project_path: str | None = typer.Option(
            None,
            "--project", "-p",
            help="Project path"
        ),
        verify: bool = typer.Option(
            False,
            "--verify/--no-verify",
            help="Enable AI verification gate after quality gates pass"
        ),
        no_review: bool = typer.Option(
            False,
            "--no-review",
            help="Skip PRD display/confirmation prompts (useful for CI/CD)"
        ),
    ):
        """
        Auto-iterate through PRD batches until completion.

        Automatically executes all stories in dependency order with quality gates
        and retry management. Supports three iteration modes:

        - until_complete: Run until all stories are complete (default)
        - max_iterations: Run up to N iterations
        - batch_complete: Run a single batch only

        Use --parallel to execute stories within a batch concurrently.
        Use --verify to enable AI verification after quality gates pass.
        Use --no-review to skip PRD confirmation prompts (useful for CI/CD).

        Examples:
            plan-cascade auto-run
            plan-cascade auto-run --mode max_iterations --max-iterations 5
            plan-cascade auto-run --dry-run
            plan-cascade auto-run --no-quality-gates
            plan-cascade auto-run --agent aider --retry-agent claude-code
            plan-cascade auto-run --parallel --max-concurrency 4
            plan-cascade auto-run --verify
            plan-cascade auto-run --no-review
        """
        from .context import get_cli_context

        project = Path(project_path) if project_path else Path.cwd()

        # Get CLI context and configure PathResolver for correct project
        cli_ctx = get_cli_context(ctx)
        # Update project root if specified via --project
        if project_path:
            cli_ctx = CLIContext.from_options(
                legacy_mode=cli_ctx.legacy_mode,
                project_root=project,
            )
        path_resolver = cli_ctx.get_path_resolver()

        # Print header
        output.print_header(
            f"Plan Cascade v{__version__} - Auto-Run",
            f"Project: {project}"
        )

        # Run the auto-run logic
        asyncio.run(_run_auto(
            project=project,
            path_resolver=path_resolver,
            mode=mode,
            max_iterations=max_iterations,
            agent=agent,
            impl_agent=impl_agent,
            retry_agent=retry_agent,
            dry_run=dry_run,
            quality_gates_enabled=not no_quality_gates,
            fallback_enabled=not no_fallback,
            parallel=parallel,
            max_concurrency=max_concurrency,
            verify_enabled=verify,
            no_review=no_review,
        ))

    async def _run_auto(
        project: Path,
        path_resolver: "PathResolver",
        mode: str,
        max_iterations: int,
        agent: str | None,
        impl_agent: str | None,
        retry_agent: str | None,
        dry_run: bool,
        quality_gates_enabled: bool,
        fallback_enabled: bool,
        parallel: bool = False,
        max_concurrency: int | None = None,
        verify_enabled: bool = False,
        no_review: bool = False,
    ):
        """Execute auto-run iteration loop."""
        from rich.progress import BarColumn, Progress, SpinnerColumn, TaskProgressColumn, TextColumn

        from ..core.iteration_loop import (
            IterationCallbacks,
            IterationConfig,
            IterationLoop,
            IterationMode,
        )
        from ..core.orchestrator import Orchestrator
        from ..core.parallel_executor import (
            ParallelExecutionConfig,
            ParallelExecutor,
            ParallelProgressDisplay,
            StoryProgress,
        )
        from ..core.quality_gate import GateConfig, GateType, QualityGate
        from ..core.retry_manager import RetryConfig, RetryManager
        from ..state.state_manager import StateManager

        # Parse iteration mode
        mode_map = {
            "until_complete": IterationMode.UNTIL_COMPLETE,
            "max_iterations": IterationMode.MAX_ITERATIONS,
            "batch_complete": IterationMode.BATCH_COMPLETE,
        }
        iteration_mode = mode_map.get(mode.lower())
        if not iteration_mode:
            output.print_error(f"Invalid mode: {mode}")
            output.print("[dim]Valid modes: until_complete, max_iterations, batch_complete[/dim]")
            sys.exit(1)

        output.print_info(f"Mode: {iteration_mode.value}")
        if iteration_mode == IterationMode.MAX_ITERATIONS:
            output.print_info(f"Max Iterations: {max_iterations}")

        # Initialize state manager with PathResolver from CLI context
        state_manager = StateManager(project, path_resolver=path_resolver)

        # Load PRD
        prd = state_manager.read_prd()
        if not prd:
            output.print_error("No prd.json found in project")
            output.print("[dim]Generate a PRD first with: plan-cascade run --expert[/dim]")
            sys.exit(1)

        stories = prd.get("stories", [])
        if not stories:
            output.print_error("PRD has no stories")
            sys.exit(1)

        output.print_info(f"PRD loaded: {len(stories)} stories")

        # Show PRD details unless --no-review is specified
        if not no_review:
            output.print()
            output.print("[bold]Stories:[/bold]")
            for story in stories[:10]:  # Show first 10 stories
                status = story.get("status", "pending")
                status_icon = {"complete": "[green]v[/green]", "in_progress": "[yellow]>[/yellow]"}.get(status, "[dim]o[/dim]")
                output.print(f"  {status_icon} {story.get('id', '?')}: {story.get('title', 'Untitled')}")
            if len(stories) > 10:
                output.print(f"  ... and {len(stories) - 10} more stories")

        output.print()

        # Build agent selection configuration
        agent_config = _build_agent_config(
            default_agent=agent,
            impl_agent=impl_agent,
            retry_agent=retry_agent,
            fallback_enabled=fallback_enabled,
        )

        # Create orchestrator
        orchestrator = Orchestrator(
            project_root=project,
            state_manager=state_manager,
        )

        # Analyze dependencies and get batches
        batches = orchestrator.analyze_dependencies()
        total_batches = len(batches)
        total_stories = sum(len(batch) for batch in batches)

        output.print_info(f"Execution plan: {total_batches} batches, {total_stories} stories")
        output.print()

        # Show execution plan
        if dry_run:
            _show_execution_plan(orchestrator, batches, agent_config)
            output.print()
            output.print_success("Dry run complete - no changes made")
            return

        # Create iteration config
        iteration_config = IterationConfig(
            mode=iteration_mode,
            max_iterations=max_iterations,
            quality_gates_enabled=quality_gates_enabled,
            auto_retry_enabled=True,
            poll_interval_seconds=5,
            batch_timeout_seconds=3600,
        )

        # Create quality gate
        quality_gate = None
        if quality_gates_enabled:
            quality_gate = QualityGate.create_default(project)
            output.print_info("Quality gates: enabled (typecheck, test, lint)")

            # Add AI verification gate if --verify is enabled
            if verify_enabled:
                verify_gate_config = GateConfig(
                    name="ai-verification",
                    type=GateType.IMPLEMENTATION_VERIFY,
                    enabled=True,
                    required=True,
                    confidence_threshold=0.7,
                )
                quality_gate.gates.append(verify_gate_config)
                output.print_info("AI verification: [green]enabled[/green]")
        else:
            output.print_info("Quality gates: [yellow]disabled[/yellow]")
            if verify_enabled:
                output.print_warning("AI verification requires quality gates - use without --no-quality-gates")

        # Create retry manager
        retry_config = RetryConfig(
            max_retries=3,
            exponential_backoff=True,
            base_delay_seconds=5.0,
            switch_agent_on_retry=fallback_enabled,
            retry_agent_chain=_build_retry_chain(agent, retry_agent),
        )
        retry_manager = RetryManager(
            project_root=project,
            config=retry_config,
        )

        output.print_info(f"Retry: max 3 attempts with exponential backoff")

        # Show parallel execution info
        if parallel:
            import os
            concurrency = max_concurrency or os.cpu_count() or 4
            output.print_info(f"Parallel execution: [green]enabled[/green] (max {concurrency} concurrent)")
        output.print()

        # Use parallel or sequential execution based on flag
        if parallel:
            # Parallel execution path
            await _run_parallel_auto(
                project=project,
                batches=batches,
                orchestrator=orchestrator,
                state_manager=state_manager,
                quality_gate=quality_gate,
                retry_manager=retry_manager,
                max_concurrency=max_concurrency,
                iteration_mode=iteration_mode,
                max_iterations=max_iterations,
                output=output,
            )
        else:
            # Sequential execution path (existing behavior)
            # Create iteration loop
            iteration_loop = IterationLoop(
                project_root=project,
                config=iteration_config,
                orchestrator=orchestrator,
                quality_gate=quality_gate,
                retry_manager=retry_manager,
            )

            # Set up callbacks for progress display
            callbacks = _create_progress_callbacks(output, agent_config)

            # Show initial status
            output.print("[bold]Starting Auto-Run...[/bold]")
            output.print()

            try:
                # Start iteration loop
                final_state = iteration_loop.start(
                    callbacks=callbacks,
                    dry_run=False,
                )

                # Show final results
                output.print()
                _show_final_results(final_state, output)

            except KeyboardInterrupt:
                output.print_warning("\nInterrupted by user")
                iteration_loop.pause("User interruption")
                output.print_info("State saved - resume with: plan-cascade auto-run")

            except Exception as e:
                output.print_error(f"Auto-run failed: {e}")
                sys.exit(1)

    async def _run_parallel_auto(
        project: Path,
        batches: list[list[dict]],
        orchestrator,
        state_manager,
        quality_gate,
        retry_manager,
        max_concurrency: int | None,
        iteration_mode,
        max_iterations: int,
        output,
    ):
        """Execute auto-run with parallel story execution within batches."""
        from datetime import datetime
        from rich.panel import Panel

        from ..core.iteration_loop import IterationMode, IterationState, IterationStatus
        from ..core.parallel_executor import (
            BatchProgress,
            ParallelExecutionConfig,
            ParallelExecutor,
            ParallelProgressDisplay,
            StoryProgress,
            StoryStatus,
        )

        # Create parallel execution config
        parallel_config = ParallelExecutionConfig(
            max_concurrency=max_concurrency,
            poll_interval_seconds=1.0,
            timeout_seconds=3600,
            persist_progress=True,
            quality_gates_enabled=quality_gate is not None,
            auto_retry_enabled=retry_manager is not None,
        )

        # Create parallel executor
        executor = ParallelExecutor(
            project_root=project,
            config=parallel_config,
            orchestrator=orchestrator,
            state_manager=state_manager,
            quality_gate=quality_gate,
            retry_manager=retry_manager,
        )

        # Track overall state
        total_stories = sum(len(batch) for batch in batches)
        completed_stories = 0
        failed_stories = 0
        all_batch_results = []

        output.print("[bold]Starting Parallel Auto-Run...[/bold]")
        output.print()

        start_time = datetime.now()
        iteration = 0

        try:
            for batch_num, batch in enumerate(batches, 1):
                iteration += 1

                # Check iteration limit
                if iteration_mode == IterationMode.MAX_ITERATIONS:
                    if iteration > max_iterations:
                        output.print_warning(f"Max iterations ({max_iterations}) reached")
                        break

                # Skip empty batches
                if not batch:
                    continue

                output.print()
                output.print(f"[bold cyan]Batch {batch_num}/{len(batches)}[/bold cyan]: {len(batch)} stories")
                output.print("-" * 50)

                # Create progress display for this batch
                display = ParallelProgressDisplay(console)

                def on_story_progress(progress: StoryProgress):
                    """Handle individual story progress updates."""
                    status_icons = {
                        StoryStatus.PENDING: "[dim]o[/dim]",
                        StoryStatus.RUNNING: "[yellow]>[/yellow]",
                        StoryStatus.COMPLETE: "[green]v[/green]",
                        StoryStatus.FAILED: "[red]x[/red]",
                        StoryStatus.RETRYING: "[yellow]![/yellow]",
                    }
                    icon = status_icons.get(progress.status, "?")

                    if progress.status == StoryStatus.RUNNING:
                        output.print(f"  {icon} {progress.story_id} started...")
                    elif progress.status == StoryStatus.COMPLETE:
                        output.print(f"  {icon} {progress.story_id} completed")
                    elif progress.status == StoryStatus.FAILED:
                        error_msg = f": {progress.error}" if progress.error else ""
                        output.print(f"  {icon} {progress.story_id} failed{error_msg}")
                    elif progress.status == StoryStatus.RETRYING:
                        output.print(f"  {icon} {progress.story_id} retrying (attempt {progress.retry_count + 1})")

                def on_batch_progress(progress: BatchProgress):
                    """Handle batch-level progress updates."""
                    display.update(progress)

                # Execute batch in parallel with live display
                with display:
                    result = await executor.execute_batch(
                        stories=batch,
                        batch_num=batch_num,
                        on_progress=on_story_progress,
                        on_batch_progress=on_batch_progress,
                    )

                all_batch_results.append(result)
                completed_stories += result.stories_completed
                failed_stories += result.stories_failed

                # Show batch summary
                output.print()
                if result.success:
                    output.print_success(
                        f"Batch {batch_num} complete: {result.stories_completed}/{result.stories_launched} stories"
                    )
                else:
                    output.print_error(
                        f"Batch {batch_num} failed: {result.stories_completed} complete, "
                        f"{result.stories_failed} failed"
                    )

                if result.stories_retried > 0:
                    output.print_info(f"  Retried: {result.stories_retried} stories")
                if result.quality_gate_failures > 0:
                    output.print_warning(f"  Quality gate failures: {result.quality_gate_failures}")

                output.print(f"  [dim]Duration: {result.duration_seconds:.1f}s[/dim]")

                # Check if we should stop on batch mode
                if iteration_mode == IterationMode.BATCH_COMPLETE:
                    break

                # Check if all stories are complete
                if completed_stories >= total_stories:
                    break

            # Calculate final metrics
            total_duration = (datetime.now() - start_time).total_seconds()

            # Build final state for display
            final_status = IterationStatus.COMPLETED if failed_stories == 0 else IterationStatus.FAILED
            if iteration_mode == IterationMode.MAX_ITERATIONS and iteration > max_iterations:
                final_status = IterationStatus.STOPPED

            final_state = IterationState(
                status=final_status,
                started_at=start_time.isoformat(),
                completed_at=datetime.now().isoformat(),
                current_batch=len(all_batch_results),
                total_batches=len(batches),
                current_iteration=iteration,
                total_stories=total_stories,
                completed_stories=completed_stories,
                failed_stories=failed_stories,
            )

            # Show final results
            output.print()
            _show_parallel_results(final_state, all_batch_results, total_duration, output)

        except KeyboardInterrupt:
            output.print_warning("\nInterrupted by user")
            output.print_info("Progress saved - resume with: plan-cascade auto-run --parallel")

        except Exception as e:
            output.print_error(f"Parallel auto-run failed: {e}")
            import traceback
            traceback.print_exc()
            sys.exit(1)

    def _show_parallel_results(state, batch_results: list, total_duration: float, output):
        """Display final parallel execution results."""
        from rich.panel import Panel
        from ..core.iteration_loop import IterationStatus

        status_color = {
            IterationStatus.COMPLETED: "green",
            IterationStatus.FAILED: "red",
            IterationStatus.STOPPED: "yellow",
            IterationStatus.PAUSED: "cyan",
        }.get(state.status, "white")

        # Build result summary
        lines = [
            f"[bold]Status:[/bold] [{status_color}]{state.status.value}[/{status_color}]",
            f"[bold]Progress:[/bold] {state.progress_percent:.1f}%",
            "",
            f"[bold]Stories:[/bold]",
            f"  Completed: [green]{state.completed_stories}[/green]",
            f"  Failed: [red]{state.failed_stories}[/red]",
            f"  Total: {state.total_stories}",
            "",
            f"[bold]Batches:[/bold] {state.current_batch}/{state.total_batches}",
        ]

        # Add batch breakdown
        if batch_results:
            lines.append("")
            lines.append("[bold]Batch Results:[/bold]")
            for result in batch_results:
                status = "[green]v[/green]" if result.success else "[red]x[/red]"
                lines.append(
                    f"  {status} Batch {result.batch_num}: "
                    f"{result.stories_completed}/{result.stories_launched} stories "
                    f"({result.duration_seconds:.1f}s)"
                )

        lines.append("")
        lines.append(f"[bold]Total Duration:[/bold] {total_duration:.1f}s")

        if state.error:
            lines.extend([
                "",
                f"[bold red]Error:[/bold red] {state.error}",
            ])

        console.print(Panel(
            "\n".join(lines),
            title="Parallel Auto-Run Results",
            border_style=status_color,
        ))

        # Show next steps
        if state.status == IterationStatus.COMPLETED:
            output.print()
            output.print_success("All stories completed successfully!")
        elif state.status == IterationStatus.FAILED:
            output.print()
            output.print_error("Execution completed with failures")
            output.print("[dim]Retry with: plan-cascade auto-run --parallel[/dim]")

    def _build_agent_config(
        default_agent: str | None,
        impl_agent: str | None,
        retry_agent: str | None,
        fallback_enabled: bool,
    ) -> dict:
        """Build agent selection configuration."""
        return {
            "default": default_agent or "claude-code",
            "impl": impl_agent,
            "retry": retry_agent,
            "fallback_enabled": fallback_enabled,
            "priority_chain": ["command_flag", "story_agent", "type_override", "default"],
        }

    def _build_retry_chain(
        default_agent: str | None,
        retry_agent: str | None,
    ) -> list[str]:
        """Build agent retry chain."""
        chain = []
        if default_agent:
            chain.append(default_agent)
        else:
            chain.append("claude-code")

        if retry_agent and retry_agent not in chain:
            chain.append(retry_agent)

        # Add fallback agents
        for fallback in ["aider", "codex"]:
            if fallback not in chain:
                chain.append(fallback)

        return chain

    def _show_execution_plan(
        orchestrator,
        batches: list[list[dict]],
        agent_config: dict,
    ):
        """Display the execution plan in dry-run mode."""
        from rich.table import Table

        output.print("[bold cyan]Execution Plan (Dry Run)[/bold cyan]")
        output.print()

        # Show agent configuration
        output.print("[bold]Agent Configuration:[/bold]")
        output.print(f"  Default Agent: {agent_config['default']}")
        if agent_config['impl']:
            output.print(f"  Implementation Agent: {agent_config['impl']}")
        if agent_config['retry']:
            output.print(f"  Retry Agent: {agent_config['retry']}")
        output.print(f"  Fallback Enabled: {'Yes' if agent_config['fallback_enabled'] else 'No'}")
        output.print()

        # Show batches
        for batch_num, batch in enumerate(batches, 1):
            output.print(f"[bold]Batch {batch_num}:[/bold] {len(batch)} stories")

            table = Table(show_header=True, header_style="bold")
            table.add_column("Story", style="cyan", width=15)
            table.add_column("Title", width=35)
            table.add_column("Priority", width=10)
            table.add_column("Agent", width=15)
            table.add_column("Dependencies", style="dim", width=20)

            for story in batch:
                story_id = story.get("id", "unknown")
                title = story.get("title", "")[:35]
                priority = story.get("priority", "medium")

                # Determine agent based on priority chain
                story_agent = story.get("agent")
                if story_agent:
                    agent = story_agent
                elif agent_config['impl'] and "implement" in title.lower():
                    agent = agent_config['impl']
                else:
                    agent = agent_config['default']

                deps = story.get("dependencies", [])
                deps_str = ", ".join(deps) if deps else "-"

                table.add_row(story_id, title, priority, agent, deps_str[:20])

            console.print(table)
            output.print()

        # Summary
        total_stories = sum(len(b) for b in batches)
        output.print(f"[bold]Summary:[/bold]")
        output.print(f"  Total Batches: {len(batches)}")
        output.print(f"  Total Stories: {total_stories}")

    def _create_progress_callbacks(output, agent_config: dict):
        """Create callbacks for progress display during iteration."""
        from ..core.iteration_loop import IterationCallbacks

        def on_batch_start(batch_num: int, stories: list[dict]):
            output.print()
            output.print(f"[bold cyan]Batch {batch_num}[/bold cyan]: {len(stories)} stories")
            output.print("-" * 40)

        def on_batch_complete(result):
            status = "[green]Success[/green]" if result.success else "[red]Failed[/red]"
            output.print(f"Batch {result.batch_num} complete: {status}")
            output.print(f"  Completed: {result.stories_completed}, Failed: {result.stories_failed}")
            if result.quality_gate_failures > 0:
                output.print(f"  Quality Gate Failures: {result.quality_gate_failures}")
            if result.stories_retried > 0:
                output.print(f"  Retried: {result.stories_retried}")
            output.print(f"  Duration: {result.duration_seconds:.1f}s")

        def on_story_complete(story_id: str, success: bool):
            if success:
                output.print(f"  [green]v[/green] {story_id}")
            else:
                output.print(f"  [red]x[/red] {story_id}")

        def on_story_retry(story_id: str, attempt: int):
            output.print(f"  [yellow]![/yellow] {story_id} - retry attempt {attempt}")

        def on_quality_gate_run(story_id: str, results: dict):
            passed = all(r.get("passed", False) for r in results.values() if isinstance(r, dict))
            status = "[green]passed[/green]" if passed else "[yellow]failed[/yellow]"
            output.print(f"    Quality gates: {status}")

        def on_iteration_complete(state):
            output.print()
            output.print(f"[bold]Iteration {state.current_iteration} complete[/bold]")
            output.print(f"  Progress: {state.completed_stories}/{state.total_stories} stories")

        def on_error(context: str, error: Exception):
            output.print_error(f"Error in {context}: {error}")

        return IterationCallbacks(
            on_batch_start=on_batch_start,
            on_batch_complete=on_batch_complete,
            on_story_complete=on_story_complete,
            on_story_retry=on_story_retry,
            on_quality_gate_run=on_quality_gate_run,
            on_iteration_complete=on_iteration_complete,
            on_error=on_error,
        )

    def _show_final_results(state, output):
        """Display final iteration results."""
        from rich.panel import Panel

        status_color = {
            "completed": "green",
            "failed": "red",
            "stopped": "yellow",
            "paused": "cyan",
        }.get(state.status.value, "white")

        # Build result summary
        lines = [
            f"[bold]Status:[/bold] [{status_color}]{state.status.value}[/{status_color}]",
            f"[bold]Progress:[/bold] {state.progress_percent:.1f}%",
            "",
            f"[bold]Stories:[/bold]",
            f"  Completed: [green]{state.completed_stories}[/green]",
            f"  Failed: [red]{state.failed_stories}[/red]",
            f"  Total: {state.total_stories}",
            "",
            f"[bold]Batches:[/bold] {len(state.batch_results)}/{state.total_batches}",
            f"[bold]Iterations:[/bold] {state.current_iteration}",
        ]

        if state.error:
            lines.extend([
                "",
                f"[bold red]Error:[/bold red] {state.error}",
            ])

        # Calculate total duration
        if state.batch_results:
            total_duration = sum(b.duration_seconds for b in state.batch_results)
            lines.append(f"[bold]Total Duration:[/bold] {total_duration:.1f}s")

        console.print(Panel(
            "\n".join(lines),
            title="Auto-Run Results",
            border_style=status_color,
        ))

        # Show next steps
        if state.status.value == "completed":
            output.print()
            output.print_success("All stories completed successfully!")
        elif state.status.value == "paused":
            output.print()
            output.print_info("Execution paused - resume with: plan-cascade auto-run")
        elif state.status.value == "failed":
            output.print()
            output.print_error("Execution failed - check logs for details")
            output.print("[dim]Retry with: plan-cascade auto-run[/dim]")

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

    @app.command()
    def chat(
        project_path: str | None = typer.Option(None, "--project", "-p", help="Project path"),
        backend: str | None = typer.Option(None, "--backend", "-b", help="Backend selection"),
        provider: str | None = typer.Option(None, "--provider", help="LLM provider for builtin backend"),
        model: str | None = typer.Option(None, "--model", "-m", help="Model to use"),
        verbose: bool = typer.Option(False, "--verbose", "-v", help="Verbose output"),
    ):
        """
        Interactive REPL mode with continuous conversation.

        Start an interactive chat session that maintains context across messages.
        Use special commands like /exit, /clear, /help, /status, etc.

        Examples:
            plan-cascade chat
            plan-cascade chat --project ./my-project
        """
        project = Path(project_path) if project_path else Path.cwd()

        # Print header
        output.print_header(
            f"Plan Cascade v{__version__} - Interactive Mode",
            f"Project: {project}"
        )
        output.print("[dim]Type /help for available commands, /exit to quit[/dim]")
        output.print()

        asyncio.run(_run_chat(
            project=project,
            backend=backend,
            provider=provider,
            model=model,
            verbose=verbose,
        ))

    async def _run_chat(
        project: Path,
        backend: str | None = None,
        provider: str | None = None,
        model: str | None = None,
        verbose: bool = False,
    ):
        """Run the interactive REPL loop."""
        from ..core.intent_classifier import Intent, IntentClassifier
        from ..core.simple_workflow import SimpleWorkflow

        # Create backend
        backend_instance = _create_backend(backend, provider, model, project)

        # Create intent classifier with LLM support
        try:
            llm = backend_instance.get_llm()
        except Exception:
            llm = None
        intent_classifier = IntentClassifier(llm=llm)

        # Current mode (simple or expert)
        current_mode = "simple"

        # Auto intent detection (can be toggled)
        auto_intent = True

        # Conversation history for context-aware execution
        conversation_history: list[dict[str, str]] = []

        # Display welcome info
        output.print_info(f"Backend: {backend_instance.get_name()}")
        output.print_info(f"Mode: {current_mode}")
        output.print_info("Intent detection: [green]auto[/green]")
        output.print()

        while True:
            try:
                # Get user input
                user_input = Prompt.ask("\n[cyan]>[/cyan]")
            except (EOFError, KeyboardInterrupt):
                output.print("\n[dim]Goodbye![/dim]")
                break

            user_input = user_input.strip()
            if not user_input:
                continue

            # Handle special commands
            if user_input.lower() in ("exit", "quit", "/exit", "/quit"):
                output.print("[dim]Goodbye![/dim]")
                break

            elif user_input.lower() == "/clear":
                # Clear session context and conversation history
                if hasattr(backend_instance, 'clear_session'):
                    backend_instance.clear_session()
                conversation_history.clear()
                output.print_success("Session cleared, starting fresh conversation")
                continue

            elif user_input.lower() == "/help":
                _show_repl_help()
                continue

            elif user_input.lower() == "/status":
                status = backend_instance.get_status()
                status["mode"] = current_mode
                status["auto_intent"] = auto_intent
                status["conversation_messages"] = len(conversation_history)
                output.status_panel(status)
                continue

            elif user_input.lower() == "/history":
                if not conversation_history:
                    output.print_info("No conversation history yet")
                else:
                    output.print(f"[bold]Conversation History ({len(conversation_history)} messages):[/bold]\n")
                    for i, msg in enumerate(conversation_history):
                        role = "[cyan]You[/cyan]" if msg["role"] == "user" else "[green]AI[/green]"
                        content = msg["content"][:100] + "..." if len(msg["content"]) > 100 else msg["content"]
                        # Replace newlines with spaces for compact display
                        content = content.replace("\n", " ")
                        output.print(f"  {i+1}. {role}: {content}")
                continue

            elif user_input.lower() == "/version":
                output.print(f"Plan Cascade v{__version__}")
                continue

            elif user_input.lower() == "/mode":
                output.print(f"Current mode: {current_mode}")
                continue

            elif user_input.lower() == "/mode simple":
                current_mode = "simple"
                output.print_success("Switched to simple mode")
                continue

            elif user_input.lower() == "/mode expert":
                current_mode = "expert"
                output.print_success("Switched to expert mode")
                continue

            elif user_input.lower() == "/intent":
                status = "[green]on[/green]" if auto_intent else "[red]off[/red]"
                output.print(f"Auto intent detection: {status}")
                continue

            elif user_input.lower() == "/intent on":
                auto_intent = True
                output.print_success("Auto intent detection enabled")
                continue

            elif user_input.lower() == "/intent off":
                auto_intent = False
                output.print_success("Auto intent detection disabled (all inputs treated as tasks)")
                continue

            elif user_input.lower() == "/config":
                _show_config()
                continue

            elif user_input.lower() == "/config setup":
                _run_setup_wizard()
                continue

            elif user_input.lower() == "/project":
                output.print(f"Current project: {project}")
                continue

            elif user_input.lower().startswith("/project "):
                new_path = user_input[9:].strip()
                new_project = Path(new_path)
                if new_project.exists():
                    project = new_project
                    backend_instance.project_root = project
                    output.print_success(f"Switched to project: {project}")
                else:
                    output.print_error(f"Path does not exist: {new_path}")
                continue

            elif user_input.lower().startswith("/config backend "):
                new_backend = user_input[16:].strip()
                backend_instance = _create_backend(new_backend, provider, model, project)
                output.print_success(f"Backend changed to: {new_backend}")
                continue

            elif user_input.lower().startswith("/run "):
                # Execute with full workflow
                task = user_input[5:].strip()
                if task:
                    result = await _execute_in_repl(
                        task, backend_instance, project, current_mode, verbose,
                        conversation_history
                    )
                    if result:
                        conversation_history.append({"role": "user", "content": task})
                        conversation_history.append({"role": "assistant", "content": result})
                continue

            elif user_input.lower().startswith("/expert "):
                # Execute in expert mode
                task = user_input[8:].strip()
                if task:
                    result = await _execute_in_repl(
                        task, backend_instance, project, "expert", verbose,
                        conversation_history
                    )
                    if result:
                        conversation_history.append({"role": "user", "content": task})
                        conversation_history.append({"role": "assistant", "content": result})
                continue

            # Show command hints when just "/" is typed
            elif user_input == "/":
                _show_command_hints()
                continue

            # Unknown command
            elif user_input.startswith("/"):
                output.print_warning(f"Unknown command: {user_input}")
                output.print("[dim]Type /help for available commands[/dim]")
                continue

            # Regular message - use intent classification
            if auto_intent:
                # Classify intent
                intent_result = await intent_classifier.classify(
                    user_input,
                    conversation_history,
                    confidence_threshold=0.7
                )

                if verbose:
                    output.print(f"[dim]Intent: {intent_result.intent.value} "
                               f"(confidence: {intent_result.confidence:.0%})[/dim]")

                # Handle based on intent
                if intent_result.intent == Intent.UNCLEAR or not intent_result.is_confident(0.5):
                    # Ask user for clarification
                    output.print("\n[yellow]I'm not sure what you'd like me to do.[/yellow]")
                    choice = Prompt.ask(
                        "What would you like",
                        choices=["task", "query", "chat"],
                        default="task"
                    )
                    if choice == "task":
                        intent_result.intent = Intent.TASK
                    elif choice == "query":
                        intent_result.intent = Intent.QUERY
                    else:
                        intent_result.intent = Intent.CHAT

                # Execute based on intent
                if intent_result.intent == Intent.TASK:
                    # Use suggested mode for tasks
                    exec_mode = intent_result.suggested_mode if intent_result.suggested_mode else current_mode
                    if intent_result.suggested_mode == "expert" and current_mode != "expert":
                        output.print("[dim]This looks like a complex task, using expert mode[/dim]")

                    result = await _execute_in_repl(
                        user_input, backend_instance, project, exec_mode, verbose,
                        conversation_history
                    )
                elif intent_result.intent == Intent.QUERY:
                    # For queries, use simple mode (faster)
                    result = await _execute_in_repl(
                        user_input, backend_instance, project, "simple", verbose,
                        conversation_history
                    )
                else:
                    # For chat, just pass to AI without full workflow
                    result = await _execute_in_repl(
                        user_input, backend_instance, project, "simple", verbose,
                        conversation_history
                    )
            else:
                # Manual mode - always execute as task with current mode
                result = await _execute_in_repl(
                    user_input, backend_instance, project, current_mode, verbose,
                    conversation_history
                )

            if result:
                conversation_history.append({"role": "user", "content": user_input})
                conversation_history.append({"role": "assistant", "content": result})

    async def _execute_in_repl(
        task: str,
        backend,
        project: Path,
        mode: str,
        verbose: bool,
        conversation_history: list[dict[str, str]] | None = None
    ) -> str | None:
        """
        Execute a task within the REPL with conversation context.

        Args:
            task: The task description
            backend: Backend instance
            project: Project path
            mode: Execution mode (simple/expert)
            verbose: Verbose output flag
            conversation_history: Previous conversation for context

        Returns:
            The AI response text, or None if failed
        """
        from ..core.simple_workflow import SimpleWorkflow

        streaming_started = [False]
        collected_output: list[str] = []  # Collect output for history

        def on_text(text: str):
            """Stream AI text output to console in real-time."""
            if not streaming_started[0]:
                console.print()
                streaming_started[0] = True
            console.print(text, end="", highlight=False)
            sys.stdout.flush()
            collected_output.append(text)  # Collect for history

        def on_tool_call(data: dict):
            """Display tool calls when in verbose mode."""
            if verbose:
                tool_name = data.get("name", data.get("type", "unknown"))
                if tool_name != "tool_result":
                    output.print(f"  [dim]> Tool: {tool_name}[/dim]")

        def on_strategy_text(text: str):
            """Stream strategy analysis output."""
            console.print(text, end="", highlight=False)
            sys.stdout.flush()

        # Attach callbacks
        backend.on_text = on_text
        backend.on_tool_call = on_tool_call

        # Progress callback
        def on_progress(event):
            if event.type in ("story_completed", "story_failed") and streaming_started[0]:
                console.print()
                streaming_started[0] = False
            output.handle_progress_event(event)

        # Build context from conversation history
        context = ""
        if conversation_history:
            context_parts = ["## Previous Conversation Context"]
            for msg in conversation_history[-10:]:  # Last 10 messages for context
                role = "User" if msg["role"] == "user" else "Assistant"
                # Truncate long messages in context
                content = msg["content"][:500] + "..." if len(msg["content"]) > 500 else msg["content"]
                context_parts.append(f"**{role}:** {content}")
            context = "\n\n".join(context_parts)

            if verbose:
                output.print(f"[dim]Using {len(conversation_history)} messages as context[/dim]")

        # Create and run workflow
        workflow = SimpleWorkflow(
            backend=backend,
            project_path=project,
            on_progress=on_progress,
            on_strategy_text=on_strategy_text,
            use_llm_strategy=(mode == "expert"),  # Only use LLM strategy in expert mode
        )

        try:
            result = await workflow.run(task, context=context)
            if streaming_started[0]:
                console.print()  # End streaming with newline

            # Return collected output for history
            return "".join(collected_output) if collected_output else result.output

        except Exception as e:
            output.print_error(f"Error: {e}")
            return None

    def _show_repl_help():
        """Display REPL help information."""
        help_text = """
[bold cyan]Session Commands[/bold cyan]
  [cyan]exit[/cyan], [cyan]quit[/cyan], [cyan]/exit[/cyan]  Exit the REPL
  [cyan]/clear[/cyan]              Clear session context, start fresh
  [cyan]/help[/cyan]               Show this help message

[bold cyan]Status Commands[/bold cyan]
  [cyan]/status[/cyan]             Show current session status
  [cyan]/history[/cyan]            Show conversation history summary
  [cyan]/version[/cyan]            Show version information

[bold cyan]Mode Commands[/bold cyan]
  [cyan]/mode[/cyan]               Show current mode
  [cyan]/mode simple[/cyan]        Switch to simple mode
  [cyan]/mode expert[/cyan]        Switch to expert mode
  [cyan]/intent[/cyan]             Show/toggle auto intent detection
  [cyan]/intent on|off[/cyan]      Enable/disable smart intent detection

[bold cyan]Configuration Commands[/bold cyan]
  [cyan]/config[/cyan]             Show current configuration
  [cyan]/config setup[/cyan]       Run configuration wizard
  [cyan]/config backend <name>[/cyan]  Set backend (claude-code, builtin)

[bold cyan]Project Commands[/bold cyan]
  [cyan]/project[/cyan]            Show current project path
  [cyan]/project <path>[/cyan]     Switch to a different project

[bold cyan]Task Execution[/bold cyan]
  [cyan]/run <description>[/cyan]  Execute task with full workflow
  [cyan]/expert <description>[/cyan]  Execute in expert mode

[bold cyan]Regular Usage[/bold cyan]
  Just type your message to chat with the AI.
  The AI will analyze and respond to your request.
"""
        console.print(help_text)

    def _show_command_hints():
        """Display command hints with descriptions when '/' is typed."""
        hints = """[bold]Available Commands:[/bold]

[cyan]/exit[/cyan]                  Exit the REPL
[cyan]/clear[/cyan]                 Clear conversation context, start fresh
[cyan]/help[/cyan]                  Show full help with all details

[cyan]/status[/cyan]                Show session status (backend, mode, history count)
[cyan]/history[/cyan]               Show conversation history summary
[cyan]/version[/cyan]               Show Plan Cascade version

[cyan]/mode[/cyan]                  Show current mode (simple/expert)
[cyan]/mode simple[/cyan]           Switch to simple mode (fast heuristic analysis)
[cyan]/mode expert[/cyan]           Switch to expert mode (LLM strategy analysis)

[cyan]/intent[/cyan]                Show auto intent detection status
[cyan]/intent on[/cyan]             Enable auto intent detection (smart mode)
[cyan]/intent off[/cyan]            Disable auto intent (treat all as tasks)

[cyan]/config[/cyan]                Show current configuration
[cyan]/config setup[/cyan]          Run interactive configuration wizard
[cyan]/config backend[/cyan] <name> Change backend [dim](claude-code, builtin)[/dim]

[cyan]/project[/cyan]               Show current project path
[cyan]/project[/cyan] <path>        Switch to a different project directory

[cyan]/run[/cyan] <description>     Execute task with full workflow
[cyan]/expert[/cyan] <description>  Execute task in expert mode (with LLM analysis)

[dim]Conversation history is automatically used as context for each request.[/dim]
"""
        console.print(hints)

    # ==================== Resume Command ====================

    @app.command()
    def resume(
        ctx: typer.Context,
        auto: bool = typer.Option(
            False,
            "--auto", "-a",
            help="Non-interactive resume (continue without confirmation prompts)"
        ),
        project_path: str | None = typer.Option(
            None,
            "--project", "-p",
            help="Project path (defaults to current directory)"
        ),
        verbose: bool = typer.Option(
            False,
            "--verbose", "-v",
            help="Show detailed state information"
        ),
        json_output: bool = typer.Option(
            False,
            "--json", "-j",
            help="Output recovery plan in JSON format"
        ),
    ):
        """
        Auto-detect and resume any interrupted Plan Cascade task.

        Detects the context type (mega-plan, hybrid-worktree, or hybrid-auto)
        from project files and provides a recovery plan. Can automatically
        continue execution with the --auto flag.

        Context Detection:
        - mega-plan: Checks for mega-plan.json
        - hybrid-worktree: Checks for .worktree/ directories or .planning-config.json
        - hybrid-auto: Checks for prd.json in current directory

        State Analysis:
        - Analyzes PRD status (missing, corrupted, empty, valid)
        - Scans progress.txt for completion markers
        - Checks story statuses from PRD

        Examples:
            plan-cascade resume                    # Show recovery plan
            plan-cascade resume --auto             # Auto-resume without prompts
            plan-cascade resume --verbose          # Show detailed state
            plan-cascade resume --json             # Output as JSON
        """
        import json as json_module

        from .context import get_cli_context

        project = Path(project_path) if project_path else Path.cwd()

        # Get CLI context and configure PathResolver for correct project
        cli_ctx = get_cli_context(ctx)
        # Update project root if specified via --project
        if project_path:
            cli_ctx = CLIContext.from_options(
                legacy_mode=cli_ctx.legacy_mode,
                project_root=project,
            )
        path_resolver = cli_ctx.get_path_resolver()

        # Import context recovery
        try:
            from ..state.context_recovery import (
                ContextRecoveryManager,
                ContextType,
                TaskState,
            )
        except ImportError as e:
            output.print_error(f"Context recovery module not available: {e}")
            raise typer.Exit(1)

        # Create recovery manager with PathResolver from CLI context
        recovery_manager = ContextRecoveryManager(project, path_resolver=path_resolver)
        state = recovery_manager.detect_context()
        plan = recovery_manager.generate_recovery_plan(state)

        # Handle JSON output
        if json_output:
            console.print_json(json_module.dumps(plan.to_dict(), indent=2))
            return

        # Print header
        output.print_header(
            f"Plan Cascade v{__version__} - Context Recovery",
            f"Project: {project}"
        )

        # Display detected context
        _display_recovery_state(state, plan, verbose, output, console)

        # If no context found, exit
        if state.context_type == ContextType.UNKNOWN:
            output.print_warning("No task context found in this directory.")
            output.print()
            output.print_info("To start a new task:")
            output.print("  [cyan]plan-cascade run '<description>'[/cyan]")
            output.print("  [cyan]plan-cascade mega plan '<description>'[/cyan]")
            raise typer.Exit(0)

        # Show recovery actions
        output.print()
        output.print("[bold]Recovery Actions:[/bold]")
        for action in plan.actions:
            output.print(f"  {action.priority}. {action.description}")
            output.print(f"     [dim]{action.command}[/dim]")

        # Show warnings
        if plan.warnings:
            output.print()
            output.print("[bold yellow]Warnings:[/bold yellow]")
            for warning in plan.warnings:
                output.print_warning(warning)

        # Auto-resume or prompt
        if auto and plan.can_auto_resume:
            output.print()
            output.print_info("Auto-resume mode enabled. Continuing execution...")

            # Update context file
            recovery_manager.update_context_file(state)

            # Execute the primary recovery action
            _execute_recovery_action(state, plan, project, output, path_resolver)

        elif not auto:
            output.print()

            if plan.can_auto_resume:
                from rich.prompt import Confirm
                if Confirm.ask("Continue with recovery?", default=True):
                    # Update context file
                    recovery_manager.update_context_file(state)

                    # Execute recovery
                    _execute_recovery_action(state, plan, project, output, path_resolver)
            else:
                output.print_info("Manual intervention required. See actions above.")

        else:
            output.print()
            output.print_warning("Cannot auto-resume. Manual intervention required.")
            output.print_info("See recovery actions above for next steps.")

    def _display_recovery_state(state, plan, verbose: bool, output, console):
        """Display the detected recovery state."""
        from rich.panel import Panel
        from rich.table import Table

        from ..state.context_recovery import ContextType, PrdStatus, TaskState

        # Context type display
        context_colors = {
            ContextType.MEGA_PLAN: "red",
            ContextType.HYBRID_WORKTREE: "yellow",
            ContextType.HYBRID_AUTO: "green",
            ContextType.UNKNOWN: "dim",
        }
        context_color = context_colors.get(state.context_type, "white")

        # Task state display
        state_icons = {
            TaskState.NEEDS_PRD: "[dim]o[/dim]",
            TaskState.NEEDS_APPROVAL: "[yellow]~[/yellow]",
            TaskState.EXECUTING: "[cyan]>[/cyan]",
            TaskState.COMPLETE: "[green]v[/green]",
            TaskState.FAILED: "[red]x[/red]",
        }
        state_icon = state_icons.get(state.task_state, "?")

        # PRD status display
        prd_status_display = {
            PrdStatus.MISSING: "[dim]Missing[/dim]",
            PrdStatus.CORRUPTED: "[red]Corrupted[/red]",
            PrdStatus.EMPTY: "[yellow]Empty[/yellow]",
            PrdStatus.VALID: "[green]Valid[/green]",
        }

        # Build status panel content
        lines = [
            f"[bold]Context Type:[/bold] [{context_color}]{state.context_type.value}[/{context_color}]",
            f"[bold]Task State:[/bold] {state_icon} {state.task_state.value}",
            f"[bold]PRD Status:[/bold] {prd_status_display.get(state.prd_status, 'Unknown')}",
        ]

        if state.task_name:
            lines.append(f"[bold]Task:[/bold] {state.task_name}")

        if state.target_branch:
            lines.append(f"[bold]Target Branch:[/bold] {state.target_branch}")

        if state.worktree_path:
            lines.append(f"[bold]Worktree:[/bold] {state.worktree_path}")

        if state.last_activity:
            lines.append(f"[bold]Last Activity:[/bold] {state.last_activity}")

        # Progress section
        if state.total_stories > 0:
            lines.append("")
            lines.append(f"[bold]Progress:[/bold] {state.completion_percentage:.1f}%")
            lines.append(f"  Complete: [green]{len(state.completed_stories)}[/green]")
            lines.append(f"  In Progress: [cyan]{len(state.in_progress_stories)}[/cyan]")
            lines.append(f"  Failed: [red]{len(state.failed_stories)}[/red]")
            lines.append(f"  Pending: [dim]{len(state.pending_stories)}[/dim]")
            lines.append(f"  Total: {state.total_stories}")

        # Mega-plan specific progress
        if state.mega_plan_progress:
            lines.append("")
            lines.append("[bold]Feature Progress:[/bold]")
            lines.append(f"  Complete: [green]{state.mega_plan_progress.get('complete', 0)}[/green]")
            lines.append(f"  In Progress: [cyan]{state.mega_plan_progress.get('in_progress', 0)}[/cyan]")
            lines.append(f"  Failed: [red]{state.mega_plan_progress.get('failed', 0)}[/red]")
            lines.append(f"  Pending: [dim]{state.mega_plan_progress.get('pending', 0)}[/dim]")

        console.print(Panel(
            "\n".join(lines),
            title="Detected Context",
            border_style=context_color,
        ))

        # Verbose: show story details
        if verbose and (state.completed_stories or state.failed_stories or state.in_progress_stories):
            output.print()

            if state.completed_stories:
                output.print("[bold green]Completed:[/bold green]")
                for sid in state.completed_stories[:10]:
                    output.print(f"  [green]v[/green] {sid}")
                if len(state.completed_stories) > 10:
                    output.print(f"  [dim]... and {len(state.completed_stories) - 10} more[/dim]")

            if state.in_progress_stories:
                output.print("[bold cyan]In Progress:[/bold cyan]")
                for sid in state.in_progress_stories:
                    output.print(f"  [cyan]>[/cyan] {sid}")

            if state.failed_stories:
                output.print("[bold red]Failed:[/bold red]")
                for sid in state.failed_stories:
                    output.print(f"  [red]x[/red] {sid}")

            if state.pending_stories and len(state.pending_stories) <= 10:
                output.print("[bold dim]Pending:[/bold dim]")
                for sid in state.pending_stories:
                    output.print(f"  [dim]o[/dim] {sid}")
            elif state.pending_stories:
                output.print(f"[bold dim]Pending:[/bold dim] {len(state.pending_stories)} stories")

    def _execute_recovery_action(state, plan, project: Path, output, path_resolver: "PathResolver"):
        """Execute the primary recovery action based on context type."""
        from ..state.context_recovery import ContextType, TaskState

        primary_action = plan.actions[0] if plan.actions else None
        if not primary_action:
            output.print_warning("No recovery action available")
            return

        output.print()
        output.print_info(f"Executing: {primary_action.description}")

        # Route to appropriate handler based on context type
        if state.context_type == ContextType.MEGA_PLAN:
            _resume_mega_plan(state, project, output, path_resolver)

        elif state.context_type == ContextType.HYBRID_WORKTREE:
            if state.worktree_path:
                _resume_hybrid_worktree(state, state.worktree_path, output, path_resolver)
            else:
                output.print_warning("Multiple worktrees found. Please change to a worktree directory.")
                for wt in state.mega_plan_features[:3]:
                    output.print(f"  [cyan]cd {wt['path']}[/cyan]")

        elif state.context_type == ContextType.HYBRID_AUTO:
            _resume_hybrid_auto(state, project, output, path_resolver)

    def _resume_mega_plan(state, project: Path, output, path_resolver: "PathResolver"):
        """Resume mega-plan execution."""
        from ..state.context_recovery import TaskState

        if state.task_state == TaskState.COMPLETE:
            output.print_info("All features complete. Running completion...")
            # Import and run mega complete logic
            try:
                from ..state.mega_state import MegaStateManager
                from ..core.mega_generator import MegaPlanGenerator

                generator = MegaPlanGenerator(project, path_resolver=path_resolver)
                state_manager = MegaStateManager(project, path_resolver=path_resolver)

                mega_plan = state_manager.read_mega_plan()
                if mega_plan:
                    progress = generator.calculate_progress(mega_plan)
                    output.print_success(f"Mega-plan complete: {progress['completed']}/{progress['total']} features")
                    output.print()
                    output.print_info("Run [cyan]plan-cascade mega complete[/cyan] to finalize and merge")
            except ImportError as e:
                output.print_error(f"Could not load mega-plan modules: {e}")

        elif state.task_state == TaskState.NEEDS_APPROVAL:
            output.print_info("Mega-plan needs approval")
            output.print()
            output.print_info("Run [cyan]plan-cascade mega approve[/cyan] to start execution")

        elif state.task_state == TaskState.EXECUTING:
            output.print_info("Resuming mega-plan execution...")
            # Run the mega resume command programmatically
            try:
                from ..core.feature_orchestrator import FeatureOrchestrator
                from ..state.mega_state import MegaStateManager
                from ..core.mega_generator import MegaPlanGenerator

                state_manager = MegaStateManager(project, path_resolver=path_resolver)
                generator = MegaPlanGenerator(project, path_resolver=path_resolver)

                # Sync status from worktrees
                worktree_status = state_manager.sync_status_from_worktrees()

                mega_plan = state_manager.read_mega_plan()
                if mega_plan:
                    # Update feature statuses
                    updated = False
                    for feature in mega_plan.get("features", []):
                        name = feature["name"]
                        if name in worktree_status:
                            wt = worktree_status[name]
                            if wt.get("stories_complete") and feature["status"] != "complete":
                                feature["status"] = "complete"
                                updated = True
                                output.print_success(f"Marked {feature['id']} as complete")

                    if updated:
                        state_manager.write_mega_plan(mega_plan)

                    progress = generator.calculate_progress(mega_plan)
                    output.print()
                    output.print_success(f"Progress: {progress['percentage']:.0f}% ({progress['completed']}/{progress['total']})")

                    # Find features ready to start
                    pending = generator.get_features_by_status(mega_plan, "pending")
                    in_progress = [f for f in mega_plan.get("features", [])
                                   if f.get("status") in ["in_progress", "approved", "prd_generated"]]

                    if in_progress:
                        output.print_info(f"{len(in_progress)} feature(s) in progress")
                    if pending:
                        output.print_info(f"{len(pending)} feature(s) pending")

                    output.print()
                    output.print_info("Monitor with: [cyan]plan-cascade mega status[/cyan]")

            except ImportError as e:
                output.print_error(f"Could not load mega-plan modules: {e}")
                output.print_info("Run manually: [cyan]plan-cascade mega resume[/cyan]")

    def _resume_hybrid_worktree(state, worktree_path: Path, output, path_resolver: "PathResolver"):
        """Resume hybrid-worktree execution."""
        from ..state.context_recovery import TaskState
        # Note: path_resolver available for future use but not currently needed here

        if state.task_state == TaskState.COMPLETE:
            output.print_success("All stories complete!")
            output.print()
            output.print_info("Run [cyan]plan-cascade worktree complete[/cyan] to merge and cleanup")

        elif state.task_state == TaskState.NEEDS_PRD:
            output.print_info("PRD needs to be generated")
            output.print()
            output.print_info(f"Run [cyan]plan-cascade run '<description>' --project {worktree_path}[/cyan]")

        elif state.task_state == TaskState.NEEDS_APPROVAL:
            output.print_info("PRD ready for execution")
            output.print()
            output.print_info(f"Run [cyan]plan-cascade auto-run --project {worktree_path}[/cyan]")

        elif state.task_state == TaskState.EXECUTING:
            output.print_info(f"Resuming execution in worktree: {worktree_path}")
            output.print(f"  Progress: {state.completion_percentage:.0f}%")
            output.print()

            # Display what's remaining
            if state.in_progress_stories:
                output.print(f"  In progress: {', '.join(state.in_progress_stories[:3])}")
            if state.pending_stories:
                remaining = len(state.pending_stories)
                output.print(f"  Pending: {remaining} story(ies)")

            output.print()
            output.print_info(f"Run [cyan]plan-cascade auto-run --project {worktree_path}[/cyan]")

    def _resume_hybrid_auto(state, project: Path, output, path_resolver: "PathResolver"):
        """Resume hybrid-auto execution."""
        from ..state.context_recovery import TaskState
        # Note: path_resolver available for future use but not currently needed here

        if state.task_state == TaskState.COMPLETE:
            output.print_success("All stories complete!")
            output.print()
            output.print_info("View summary with: [cyan]plan-cascade status[/cyan]")

        elif state.task_state == TaskState.NEEDS_PRD:
            output.print_info("PRD needs to be generated")
            output.print()
            output.print_info("Run [cyan]plan-cascade run '<description>'[/cyan]")

        elif state.task_state == TaskState.NEEDS_APPROVAL:
            output.print_info("PRD ready for execution")
            output.print()
            output.print_info("Run [cyan]plan-cascade auto-run[/cyan]")

        elif state.task_state == TaskState.EXECUTING:
            output.print_info("Resuming execution...")
            output.print(f"  Progress: {state.completion_percentage:.0f}%")
            output.print()

            # Display what's remaining
            if state.in_progress_stories:
                output.print(f"  In progress: {', '.join(state.in_progress_stories[:3])}")
            if state.pending_stories:
                remaining = len(state.pending_stories)
                output.print(f"  Pending: {remaining} story(ies)")
            if state.failed_stories:
                output.print(f"  [red]Failed: {len(state.failed_stories)}[/red]")

            output.print()
            output.print_info("Run [cyan]plan-cascade auto-run[/cyan] to continue")

    # ==================== Register Subcommands ====================

    # Import and register mega-plan commands
    try:
        from .mega import mega_app

        if mega_app:
            app.add_typer(
                mega_app,
                name="mega",
                help="Mega-plan workflow for multi-feature projects",
            )
    except ImportError:
        pass  # mega module not available

    # Import and register worktree commands
    try:
        from .worktree import worktree_app

        if worktree_app:
            app.add_typer(
                worktree_app,
                name="worktree",
                help="Git worktree management for parallel task development",
            )
    except ImportError:
        pass  # worktree module not available

    # Import and register spec interview commands
    try:
        from .spec import spec_app

        if spec_app:
            app.add_typer(
                spec_app,
                name="spec",
                help="Spec interview workflow (spec.json/spec.md -> prd.json)",
            )
    except ImportError:
        pass  # spec module not available

    # Import and register design document commands
    try:
        from .design import design_app

        if design_app:
            app.add_typer(
                design_app,
                name="design",
                help="Design document management",
            )
    except ImportError:
        pass  # design module not available

    # Import and register skills commands
    try:
        from .skills import skills_app

        if skills_app:
            app.add_typer(
                skills_app,
                name="skills",
                help="External skill management",
            )
    except ImportError:
        pass  # skills module not available

    # Import and register dependencies command
    try:
        from .dependencies import dependencies_command

        app.command(name="deps")(dependencies_command)
        # Also register as 'dependencies' alias for discoverability
        app.command(name="dependencies", hidden=True)(dependencies_command)
    except ImportError:
        pass  # dependencies module not available

    # Import and register migrate command
    try:
        from .migrate import migrate_app

        if migrate_app:
            app.add_typer(
                migrate_app,
                name="migrate",
                help="Migrate planning files from project root to user directory",
            )
    except ImportError:
        pass  # migrate module not available

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
