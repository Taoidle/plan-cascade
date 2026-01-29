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
