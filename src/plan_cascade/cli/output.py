"""
Rich Terminal Output for Plan Cascade CLI

Provides beautiful terminal output with progress bars, tables, panels, and styled text.
Uses the Rich library for all formatting.
"""

from contextlib import contextmanager
from typing import TYPE_CHECKING, Any, Optional

try:
    from rich.console import Console
    from rich.layout import Layout
    from rich.live import Live
    from rich.markup import escape
    from rich.panel import Panel
    from rich.progress import (
        BarColumn,
        Progress,
        SpinnerColumn,
        TaskProgressColumn,
        TextColumn,
        TimeElapsedColumn,
    )
    from rich.status import Status
    from rich.table import Table
    from rich.text import Text
    from rich.tree import Tree
    HAS_RICH = True
except ImportError:
    HAS_RICH = False

if TYPE_CHECKING:
    from ..core.expert_workflow import PRD
    from ..core.simple_workflow import ProgressEvent


class OutputManager:
    """
    Manages rich terminal output for Plan Cascade CLI.

    Provides consistent styling and formatting for:
    - Progress bars and spinners
    - Tables for PRD display
    - Panels for status information
    - Tree views for dependencies
    """

    # Color scheme
    COLORS = {
        "primary": "blue",
        "success": "green",
        "warning": "yellow",
        "error": "red",
        "muted": "dim",
        "highlight": "cyan",
    }

    # Status icons
    ICONS = {
        "pending": "[dim]o[/dim]",
        "in_progress": "[yellow]>[/yellow]",
        "complete": "[green]v[/green]",
        "failed": "[red]x[/red]",
        "running": "[cyan]*[/cyan]",
    }

    def __init__(self, console: Optional["Console"] = None):
        """
        Initialize the output manager.

        Args:
            console: Rich Console instance (creates one if not provided)
        """
        if HAS_RICH:
            self.console = console or Console()
            self._progress: Progress | None = None
            self._live: Live | None = None
        else:
            self.console = None
            self._progress = None
            self._live = None

    @property
    def is_available(self) -> bool:
        """Check if rich output is available."""
        return HAS_RICH and self.console is not None

    # ==================== Basic Output ====================

    def print(self, message: str = "", style: str | None = None) -> None:
        """
        Print a message with optional styling.

        Args:
            message: Message to print (defaults to empty string for blank line)
            style: Optional rich style string
        """
        if self.is_available:
            self.console.print(message, style=style)
        else:
            print(message)

    def print_header(self, title: str, subtitle: str | None = None) -> None:
        """
        Print a styled header.

        Args:
            title: Header title
            subtitle: Optional subtitle
        """
        if self.is_available:
            self.console.print()
            self.console.print(f"[bold blue]{title}[/bold blue]")
            if subtitle:
                self.console.print(f"[dim]{subtitle}[/dim]")
            self.console.print()
        else:
            print(f"\n{title}")
            if subtitle:
                print(subtitle)
            print()

    def print_success(self, message: str) -> None:
        """Print a success message."""
        if self.is_available:
            self.console.print(f"[green]v[/green] {message}")
        else:
            print(f"[OK] {message}")

    def print_error(self, message: str) -> None:
        """Print an error message."""
        if self.is_available:
            self.console.print(f"[red]x[/red] {message}")
        else:
            print(f"[ERROR] {message}")

    def print_warning(self, message: str) -> None:
        """Print a warning message."""
        if self.is_available:
            self.console.print(f"[yellow]![/yellow] {message}")
        else:
            print(f"[WARN] {message}")

    def print_info(self, message: str) -> None:
        """Print an info message."""
        if self.is_available:
            self.console.print(f"[blue]i[/blue] {message}")
        else:
            print(f"[INFO] {message}")

    # ==================== Panels ====================

    def panel(
        self,
        content: str,
        title: str | None = None,
        border_style: str = "blue"
    ) -> None:
        """
        Display content in a panel.

        Args:
            content: Panel content
            title: Optional panel title
            border_style: Border color style
        """
        if self.is_available:
            self.console.print(Panel(content, title=title, border_style=border_style))
        else:
            if title:
                print(f"=== {title} ===")
            print(content)
            print()

    def status_panel(self, status: dict[str, Any]) -> None:
        """
        Display a status panel.

        Args:
            status: Status dictionary
        """
        if self.is_available:
            lines = []
            for key, value in status.items():
                if isinstance(value, bool):
                    value_str = "[green]Yes[/green]" if value else "[red]No[/red]"
                else:
                    value_str = str(value)
                lines.append(f"[bold]{key}:[/bold] {value_str}")

            self.console.print(Panel(
                "\n".join(lines),
                title="Status",
                border_style="cyan"
            ))
        else:
            print("=== Status ===")
            for key, value in status.items():
                print(f"  {key}: {value}")
            print()

    # ==================== Tables ====================

    def prd_table(self, prd: "PRD") -> None:
        """
        Display PRD as a table.

        Args:
            prd: PRD object
        """
        if self.is_available:
            table = Table(title=f"PRD: {prd.goal[:50]}..." if len(prd.goal) > 50 else f"PRD: {prd.goal}")

            table.add_column("ID", style="cyan", width=12)
            table.add_column("Title", style="white", width=30)
            table.add_column("Priority", style="yellow", width=10)
            table.add_column("Status", width=12)
            table.add_column("Dependencies", style="dim", width=20)

            for story in prd.stories:
                status = story.get("status", "pending")
                status_style = {
                    "pending": "dim",
                    "in_progress": "yellow",
                    "complete": "green",
                    "failed": "red",
                }.get(status, "white")

                deps = ", ".join(story.get("dependencies", [])) or "-"

                table.add_row(
                    story.get("id", ""),
                    story.get("title", "")[:30],
                    story.get("priority", "medium"),
                    f"[{status_style}]{status}[/{status_style}]",
                    deps[:20],
                )

            self.console.print(table)
        else:
            print(f"\nPRD: {prd.goal}")
            print("-" * 80)
            for story in prd.stories:
                deps = ", ".join(story.get("dependencies", [])) or "-"
                print(f"  {story.get('id')}: {story.get('title')}")
                print(f"    Priority: {story.get('priority')} | Status: {story.get('status')} | Deps: {deps}")
            print()

    def stories_table(self, stories: list[dict[str, Any]], title: str = "Stories") -> None:
        """
        Display stories as a table.

        Args:
            stories: List of story dictionaries
            title: Table title
        """
        if self.is_available:
            table = Table(title=title)

            table.add_column("#", style="dim", width=4)
            table.add_column("ID", style="cyan", width=12)
            table.add_column("Title", style="white", width=40)
            table.add_column("Status", width=12)

            for i, story in enumerate(stories, 1):
                status = story.get("status", "pending")
                icon = self.ICONS.get(status, "")

                table.add_row(
                    str(i),
                    story.get("id", ""),
                    story.get("title", ""),
                    f"{icon} {status}",
                )

            self.console.print(table)
        else:
            print(f"\n{title}")
            print("-" * 60)
            for i, story in enumerate(stories, 1):
                print(f"  {i}. [{story.get('status')}] {story.get('id')}: {story.get('title')}")
            print()

    # ==================== Progress ====================

    @contextmanager
    def progress_context(self, description: str = "Working..."):
        """
        Context manager for progress display.

        Args:
            description: Progress description

        Yields:
            Progress tracker or None
        """
        if self.is_available:
            with Progress(
                SpinnerColumn(),
                TextColumn("[progress.description]{task.description}"),
                BarColumn(),
                TaskProgressColumn(),
                TimeElapsedColumn(),
                console=self.console,
            ) as progress:
                self._progress = progress
                yield progress
                self._progress = None
        else:
            print(description)
            yield None

    def create_progress_task(
        self,
        description: str,
        total: int = 100
    ) -> int | None:
        """
        Create a progress task.

        Args:
            description: Task description
            total: Total steps

        Returns:
            Task ID or None
        """
        if self._progress:
            return self._progress.add_task(description, total=total)
        return None

    def update_progress(
        self,
        task_id: int | None,
        advance: int = 1,
        description: str | None = None
    ) -> None:
        """
        Update progress task.

        Args:
            task_id: Task ID
            advance: Steps to advance
            description: Optional new description
        """
        if self._progress and task_id is not None:
            kwargs = {"advance": advance}
            if description:
                kwargs["description"] = description
            self._progress.update(task_id, **kwargs)

    @contextmanager
    def spinner(self, message: str = "Working..."):
        """
        Display a spinner during an operation.

        Args:
            message: Spinner message

        Yields:
            Status object or None
        """
        if self.is_available:
            with self.console.status(message) as status:
                yield status
        else:
            print(message)
            yield None

    # ==================== Dependency Tree ====================

    def dependency_tree(self, prd: "PRD") -> None:
        """
        Display dependency graph as a tree.

        Args:
            prd: PRD object
        """
        if self.is_available:
            tree = Tree("[bold blue]Story Dependencies[/bold blue]")

            # Build graph
            graph = prd.get_dependency_graph()
            story_map = {s["id"]: s for s in prd.stories}

            # Find roots
            roots = [sid for sid, deps in graph.items() if not deps]

            # Build reverse graph
            dependents: dict[str, list[str]] = {sid: [] for sid in graph}
            for sid, deps in graph.items():
                for dep in deps:
                    if dep in dependents:
                        dependents[dep].append(sid)

            def add_children(parent_node, story_id: str, visited: set):
                if story_id in visited:
                    return
                visited.add(story_id)

                story = story_map.get(story_id, {})
                status = story.get("status", "pending")
                icon = self.ICONS.get(status, "")
                title = story.get("title", "Unknown")

                node = parent_node.add(f"{icon} [cyan]{story_id}[/cyan]: {title}")

                for child_id in dependents.get(story_id, []):
                    add_children(node, child_id, visited)

            visited = set()
            for root_id in roots:
                add_children(tree, root_id, visited)

            # Add orphans
            orphans = [sid for sid in graph if sid not in visited]
            if orphans:
                orphan_node = tree.add("[yellow]Unconnected Stories[/yellow]")
                for oid in orphans:
                    story = story_map.get(oid, {})
                    status = story.get("status", "pending")
                    icon = self.ICONS.get(status, "")
                    title = story.get("title", "Unknown")
                    orphan_node.add(f"{icon} [cyan]{oid}[/cyan]: {title}")

            self.console.print(tree)
        else:
            print("\nDependency Tree:")
            print("-" * 40)
            for story in prd.stories:
                deps = story.get("dependencies", [])
                prefix = "  " if deps else ""
                print(f"{prefix}{story['id']}: {story['title']}")
                if deps:
                    print(f"    <- depends on: {', '.join(deps)}")
            print()

    # ==================== Strategy Display ====================

    def strategy_decision(self, decision: Any) -> None:
        """
        Display strategy decision.

        Args:
            decision: StrategyDecision object
        """
        if self.is_available:
            # Create info panel
            strategy_colors = {
                "direct": "green",
                "hybrid_auto": "yellow",
                "mega_plan": "red",
            }
            strategy_value = decision.strategy.value if hasattr(decision.strategy, 'value') else str(decision.strategy)
            color = strategy_colors.get(strategy_value, "white")

            lines = [
                f"[bold]Strategy:[/bold] [{color}]{strategy_value}[/{color}]",
                f"[bold]Confidence:[/bold] {decision.confidence:.0%}",
                f"[bold]Estimated Stories:[/bold] {decision.estimated_stories}",
                f"[bold]Use Worktree:[/bold] {'Yes' if decision.use_worktree else 'No'}",
                "",
                "[bold]Reasoning:[/bold]",
                f"  {decision.reasoning}",
            ]

            if decision.complexity_indicators:
                lines.append("")
                lines.append("[bold]Complexity Indicators:[/bold]")
                for indicator in decision.complexity_indicators:
                    lines.append(f"  - {indicator}")

            if decision.recommendations:
                lines.append("")
                lines.append("[bold]Recommendations:[/bold]")
                for rec in decision.recommendations:
                    lines.append(f"  - {rec}")

            self.console.print(Panel(
                "\n".join(lines),
                title="Strategy Analysis",
                border_style=color,
            ))
        else:
            print("\n=== Strategy Analysis ===")
            strategy_value = decision.strategy.value if hasattr(decision.strategy, 'value') else str(decision.strategy)
            print(f"  Strategy: {strategy_value}")
            print(f"  Confidence: {decision.confidence:.0%}")
            print(f"  Estimated Stories: {decision.estimated_stories}")
            print(f"  Reasoning: {decision.reasoning}")
            print()

    # ==================== Progress Events ====================

    def handle_progress_event(self, event: "ProgressEvent") -> None:
        """
        Handle a progress event and display appropriate output.

        Args:
            event: Progress event
        """
        event_type = event.type
        data = event.data

        if event_type == "strategy_decided":
            self.print_info(f"Strategy: {data.get('strategy', 'unknown')}")

        elif event_type == "execution_started":
            self.print_info(f"Execution started ({data.get('strategy', 'unknown')} mode)")

        elif event_type == "story_started":
            self.print(f"  [cyan]>[/cyan] {data.get('title', data.get('story_id', ''))}")

        elif event_type == "story_completed":
            self.print(f"  [green]v[/green] {data.get('title', data.get('story_id', ''))}")

        elif event_type == "story_failed":
            self.print(f"  [red]x[/red] {data.get('title', data.get('story_id', ''))}: {data.get('error', '')}")

        elif event_type == "batch_started":
            batch = data.get('batch', 0)
            total = data.get('total', 0)
            self.print(f"\n[bold]Batch {batch}/{total}[/bold]")

        elif event_type == "execution_completed":
            if data.get('success'):
                self.print_success("Execution completed successfully")
            else:
                self.print_error("Execution completed with errors")

    # ==================== Configuration Display ====================

    def config_display(self, config: dict[str, Any]) -> None:
        """
        Display configuration.

        Args:
            config: Configuration dictionary
        """
        if self.is_available:
            table = Table(title="Configuration")
            table.add_column("Setting", style="cyan")
            table.add_column("Value", style="white")

            for key, value in config.items():
                if key.lower() in ("api_key", "password", "secret"):
                    value = "***" if value else "(not set)"
                table.add_row(key, str(value))

            self.console.print(table)
        else:
            print("\n=== Configuration ===")
            for key, value in config.items():
                if key.lower() in ("api_key", "password", "secret"):
                    value = "***" if value else "(not set)"
                print(f"  {key}: {value}")
            print()

    # ==================== Workflow Result ====================

    def workflow_result(self, result: Any) -> None:
        """
        Display workflow result.

        Args:
            result: WorkflowResult object
        """
        if self.is_available:
            if result.success:
                title = "[green]Workflow Completed Successfully[/green]"
                border = "green"
            else:
                title = "[red]Workflow Failed[/red]"
                border = "red"

            strategy_value = result.strategy.value if hasattr(result.strategy, 'value') else str(result.strategy)

            lines = [
                f"[bold]Strategy:[/bold] {strategy_value}",
                f"[bold]Stories:[/bold] {result.stories_completed}/{result.stories_total}",
                f"[bold]Iterations:[/bold] {result.iterations}",
                f"[bold]Duration:[/bold] {result.duration_seconds:.1f}s",
            ]

            if result.error:
                lines.append(f"\n[bold red]Error:[/bold red] {result.error}")

            self.console.print(Panel(
                "\n".join(lines),
                title=title,
                border_style=border,
            ))
        else:
            status = "SUCCESS" if result.success else "FAILED"
            print(f"\n=== Workflow {status} ===")
            strategy_value = result.strategy.value if hasattr(result.strategy, 'value') else str(result.strategy)
            print(f"  Strategy: {strategy_value}")
            print(f"  Stories: {result.stories_completed}/{result.stories_total}")
            print(f"  Duration: {result.duration_seconds:.1f}s")
            if result.error:
                print(f"  Error: {result.error}")
            print()


# Singleton instance for convenience
_default_output: OutputManager | None = None


def get_output() -> OutputManager:
    """Get the default output manager."""
    global _default_output
    if _default_output is None:
        _default_output = OutputManager()
    return _default_output


def set_output(output: OutputManager) -> None:
    """Set the default output manager."""
    global _default_output
    _default_output = output
