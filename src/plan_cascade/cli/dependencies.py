#!/usr/bin/env python3
"""
Dependency Graph Visualization Commands for Plan Cascade CLI

Provides 'plan-cascade deps' command that displays a visual ASCII dependency graph
for all stories/features in the current PRD or mega-plan. Shows dependency details
including depth, critical path, and potential issues.

Commands:
- deps: Show dependency graph (alias: dependencies)
"""

import json
import sys
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from ..state.path_resolver import PathResolver

try:
    import typer
    from rich.console import Console
    from rich.panel import Panel
    from rich.table import Table
    from rich.tree import Tree

    HAS_TYPER = True
except ImportError:
    HAS_TYPER = False


class OutputFormat(str, Enum):
    """Output format options for dependency graph."""

    TREE = "tree"
    FLAT = "flat"
    JSON = "json"


@dataclass
class DependencyNode:
    """
    Node in dependency graph.

    Represents a story or feature with its dependency relationships.
    """

    id: str
    title: str = ""
    description: str = ""
    priority: str = "medium"
    status: str = "pending"
    dependencies: list[str] = field(default_factory=list)
    dependents: list[str] = field(default_factory=list)
    depth: int = 0
    is_bottleneck: bool = False
    is_orphan: bool = False
    on_critical_path: bool = False

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "id": self.id,
            "title": self.title,
            "priority": self.priority,
            "status": self.status,
            "dependencies": self.dependencies,
            "dependents": self.dependents,
            "depth": self.depth,
            "is_bottleneck": self.is_bottleneck,
            "is_orphan": self.is_orphan,
            "on_critical_path": self.on_critical_path,
        }


@dataclass
class DependencyGraphResult:
    """Result of dependency graph analysis."""

    nodes: dict[str, DependencyNode] = field(default_factory=dict)
    roots: list[str] = field(default_factory=list)
    critical_path: list[str] = field(default_factory=list)
    critical_path_length: int = 0
    bottlenecks: list[str] = field(default_factory=list)
    orphans: list[str] = field(default_factory=list)
    circular_dependencies: list[list[str]] = field(default_factory=list)
    has_issues: bool = False
    source_type: str = ""  # "prd" or "mega-plan"

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "source_type": self.source_type,
            "nodes": {nid: node.to_dict() for nid, node in self.nodes.items()},
            "roots": self.roots,
            "critical_path": self.critical_path,
            "critical_path_length": self.critical_path_length,
            "bottlenecks": self.bottlenecks,
            "orphans": self.orphans,
            "circular_dependencies": self.circular_dependencies,
            "has_issues": self.has_issues,
            "summary": {
                "total_nodes": len(self.nodes),
                "total_roots": len(self.roots),
                "total_bottlenecks": len(self.bottlenecks),
                "total_orphans": len(self.orphans),
                "total_circular": len(self.circular_dependencies),
            },
        }


class DependencyGraphAnalyzer:
    """Analyzes dependency graphs from PRD or mega-plan files."""

    BOTTLENECK_THRESHOLD = 2  # Number of dependents to be considered a bottleneck

    def __init__(
        self,
        project_root: Path,
        path_resolver: "PathResolver | None" = None,
    ):
        """
        Initialize the dependency graph analyzer.

        Args:
            project_root: Root directory of the project
            path_resolver: Optional PathResolver instance for locating plan files.
                          If not provided, uses legacy behavior (files in project_root).
        """
        self.project_root = Path(project_root)
        self._path_resolver = path_resolver

        # Use PathResolver if provided, otherwise fall back to legacy paths
        if path_resolver is not None:
            self.prd_path = path_resolver.get_prd_path()
            self.mega_plan_path = path_resolver.get_mega_plan_path()
        else:
            # Legacy mode: files in project root
            self.prd_path = self.project_root / "prd.json"
            self.mega_plan_path = self.project_root / "mega-plan.json"

    def analyze(self) -> DependencyGraphResult:
        """
        Analyze dependencies from PRD or mega-plan.

        Returns:
            DependencyGraphResult with analysis results

        Raises:
            FileNotFoundError: If neither prd.json nor mega-plan.json exists
        """
        # Try mega-plan first, then PRD
        if self.mega_plan_path.exists():
            return self._analyze_mega_plan()
        elif self.prd_path.exists():
            return self._analyze_prd()
        else:
            raise FileNotFoundError(
                "No prd.json or mega-plan.json found in the project directory."
            )

    def _analyze_prd(self) -> DependencyGraphResult:
        """Analyze dependencies from prd.json."""
        with open(self.prd_path, encoding="utf-8") as f:
            prd = json.load(f)

        result = DependencyGraphResult(source_type="prd")

        # Build nodes from stories
        stories = prd.get("stories", [])
        for story in stories:
            story_id = story.get("id", "")
            node = DependencyNode(
                id=story_id,
                title=story.get("title", ""),
                description=story.get("description", ""),
                priority=story.get("priority", "medium"),
                status=story.get("status", "pending"),
                dependencies=story.get("dependencies", []),
            )
            result.nodes[story_id] = node

        self._complete_analysis(result)
        return result

    def _analyze_mega_plan(self) -> DependencyGraphResult:
        """Analyze dependencies from mega-plan.json."""
        with open(self.mega_plan_path, encoding="utf-8") as f:
            plan = json.load(f)

        result = DependencyGraphResult(source_type="mega-plan")

        # Build nodes from features
        features = plan.get("features", [])
        for feature in features:
            feature_id = feature.get("id", "")
            node = DependencyNode(
                id=feature_id,
                title=feature.get("title", feature.get("name", "")),
                description=feature.get("description", ""),
                priority=feature.get("priority", "medium"),
                status=feature.get("status", "pending"),
                dependencies=feature.get("dependencies", []),
            )
            result.nodes[feature_id] = node

        self._complete_analysis(result)
        return result

    def _complete_analysis(self, result: DependencyGraphResult) -> None:
        """
        Complete the dependency graph analysis.

        Calculates dependents, depths, critical path, bottlenecks, orphans,
        and detects circular dependencies.
        """
        nodes = result.nodes

        # 1. Build dependents (reverse graph)
        for node_id, node in nodes.items():
            for dep_id in node.dependencies:
                if dep_id in nodes:
                    nodes[dep_id].dependents.append(node_id)

        # 2. Find roots (nodes with no dependencies)
        result.roots = [
            node_id for node_id, node in nodes.items() if not node.dependencies
        ]

        # 3. Detect circular dependencies
        result.circular_dependencies = self._detect_cycles(nodes)
        if result.circular_dependencies:
            result.has_issues = True

        # 4. Calculate depths using BFS (with cycle handling)
        self._calculate_depths(result)

        # 5. Find critical path (longest dependency chain)
        result.critical_path = self._find_critical_path(result)
        result.critical_path_length = len(result.critical_path)

        # Mark nodes on critical path
        for node_id in result.critical_path:
            if node_id in nodes:
                nodes[node_id].on_critical_path = True

        # 6. Identify bottlenecks (nodes with many dependents)
        for node_id, node in nodes.items():
            if len(node.dependents) >= self.BOTTLENECK_THRESHOLD:
                node.is_bottleneck = True
                result.bottlenecks.append(node_id)

        if result.bottlenecks:
            result.has_issues = True

        # 7. Identify orphans (no dependencies AND nothing depends on them)
        for node_id, node in nodes.items():
            if not node.dependencies and not node.dependents:
                node.is_orphan = True
                result.orphans.append(node_id)

    def _detect_cycles(
        self, nodes: dict[str, DependencyNode]
    ) -> list[list[str]]:
        """
        Detect circular dependencies using DFS.

        Returns:
            List of cycles, where each cycle is a list of node IDs
        """
        cycles = []
        visited = set()
        rec_stack = set()
        path = []

        def dfs(node_id: str) -> None:
            visited.add(node_id)
            rec_stack.add(node_id)
            path.append(node_id)

            node = nodes.get(node_id)
            if node:
                for dep_id in node.dependencies:
                    if dep_id not in visited:
                        dfs(dep_id)
                    elif dep_id in rec_stack:
                        # Found a cycle
                        cycle_start = path.index(dep_id)
                        cycle = path[cycle_start:] + [dep_id]
                        if cycle not in cycles:
                            cycles.append(cycle)

            path.pop()
            rec_stack.remove(node_id)

        for node_id in nodes:
            if node_id not in visited:
                dfs(node_id)

        return cycles

    def _calculate_depths(self, result: DependencyGraphResult) -> None:
        """
        Calculate depth for each node using BFS from roots.

        Depth represents the maximum distance from any root node.
        """
        nodes = result.nodes

        # Track nodes involved in cycles to avoid infinite loops
        cyclic_nodes = set()
        for cycle in result.circular_dependencies:
            cyclic_nodes.update(cycle)

        # BFS from roots
        queue = [(root_id, 0) for root_id in result.roots]
        visited_depths: dict[str, int] = {}

        while queue:
            node_id, depth = queue.pop(0)

            # Skip if we've already visited at a higher depth
            if node_id in visited_depths:
                if depth <= visited_depths[node_id]:
                    continue

            visited_depths[node_id] = depth

            if node_id in nodes:
                nodes[node_id].depth = depth

                for dependent_id in nodes[node_id].dependents:
                    # Skip cyclic dependencies to prevent infinite loops
                    if dependent_id in cyclic_nodes and node_id in cyclic_nodes:
                        continue
                    queue.append((dependent_id, depth + 1))

        # Handle nodes not reachable from roots (in cycles)
        for node_id in nodes:
            if node_id not in visited_depths:
                # Assign depth based on dependency depths
                max_dep_depth = -1
                for dep_id in nodes[node_id].dependencies:
                    if dep_id in nodes and dep_id in visited_depths:
                        max_dep_depth = max(max_dep_depth, nodes[dep_id].depth)
                nodes[node_id].depth = max_dep_depth + 1 if max_dep_depth >= 0 else 0

    def _find_critical_path(self, result: DependencyGraphResult) -> list[str]:
        """
        Find the critical path (longest dependency chain).

        Returns:
            List of node IDs representing the critical path
        """
        nodes = result.nodes

        if not nodes:
            return []

        # Find the node with maximum depth
        max_depth = -1
        deepest_node = None
        for node_id, node in nodes.items():
            if node.depth > max_depth:
                max_depth = node.depth
                deepest_node = node_id

        if deepest_node is None:
            return []

        # Build path from root to deepest node
        path = []
        current = deepest_node

        # Trace back to root
        reverse_path = [current]
        visited = {current}

        while True:
            node = nodes.get(current)
            if not node or not node.dependencies:
                break

            # Find the dependency with the highest depth
            best_dep = None
            best_depth = -1
            for dep_id in node.dependencies:
                if dep_id in nodes and dep_id not in visited:
                    dep_depth = nodes[dep_id].depth
                    if dep_depth > best_depth:
                        best_depth = dep_depth
                        best_dep = dep_id

            if best_dep is None:
                # Try any dependency not yet visited
                for dep_id in node.dependencies:
                    if dep_id in nodes and dep_id not in visited:
                        best_dep = dep_id
                        break

            if best_dep is None:
                break

            visited.add(best_dep)
            reverse_path.append(best_dep)
            current = best_dep

        # Reverse to get root -> deepest order
        path = list(reversed(reverse_path))
        return path


# ============================================================================
# Typer CLI Commands
# ============================================================================

if HAS_TYPER:
    # Note: This module provides the 'deps' command which is added to main app
    # It's not a separate Typer app like mega or worktree
    console = Console()

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

    def _display_tree_format(result: DependencyGraphResult) -> None:
        """Display dependency graph as a Rich tree."""
        source_label = "PRD Stories" if result.source_type == "prd" else "Mega-Plan Features"
        tree = Tree(f"[bold blue]Dependency Graph ({source_label})[/bold blue]")

        nodes = result.nodes

        # Status icons
        status_icons = {
            "pending": "[dim]o[/dim]",
            "in_progress": "[yellow]>[/yellow]",
            "prd_generated": "[yellow]~[/yellow]",
            "approved": "[yellow]~[/yellow]",
            "complete": "[green]v[/green]",
            "failed": "[red]x[/red]",
        }

        # Priority colors
        priority_colors = {
            "high": "red",
            "medium": "yellow",
            "low": "dim",
        }

        def add_node_to_tree(
            parent_tree: Tree,
            node_id: str,
            visited: set[str],
        ) -> None:
            """Recursively add nodes to the tree."""
            if node_id in visited:
                # Show circular reference indicator
                parent_tree.add(f"[red]-> {node_id} (circular)[/red]")
                return

            visited.add(node_id)

            node = nodes.get(node_id)
            if not node:
                return

            # Build node label
            icon = status_icons.get(node.status, "?")
            priority_color = priority_colors.get(node.priority, "white")

            # Add markers for special nodes
            markers = []
            if node.on_critical_path:
                markers.append("[bold magenta]*[/bold magenta]")
            if node.is_bottleneck:
                markers.append("[bold red]B[/bold red]")
            if node.is_orphan:
                markers.append("[dim]O[/dim]")

            marker_str = " ".join(markers)
            if marker_str:
                marker_str = f" {marker_str}"

            label = (
                f"{icon} [{priority_color}][{node.priority.upper()}][/{priority_color}] "
                f"[cyan]{node_id}[/cyan]: {node.title}{marker_str}"
            )

            # Add dependency info
            if node.dependencies:
                deps_str = ", ".join(node.dependencies)
                label += f"\n    [dim]Depends on: {deps_str}[/dim]"

            if node.dependents:
                blocks_str = ", ".join(node.dependents)
                label += f"\n    [dim]Blocks: {blocks_str}[/dim]"

            node_branch = parent_tree.add(label)

            # Add children (dependents)
            for dependent_id in node.dependents:
                add_node_to_tree(node_branch, dependent_id, visited.copy())

        # Add root nodes to tree
        visited: set[str] = set()
        for root_id in result.roots:
            add_node_to_tree(tree, root_id, visited)

        # Add orphans section if any
        orphans_not_shown = [o for o in result.orphans if o not in visited]
        if orphans_not_shown:
            orphan_branch = tree.add("[yellow]Orphan Nodes (isolated)[/yellow]")
            for orphan_id in orphans_not_shown:
                node = nodes.get(orphan_id)
                if node:
                    icon = status_icons.get(node.status, "?")
                    orphan_branch.add(
                        f"{icon} [cyan]{orphan_id}[/cyan]: {node.title} [dim](no dependencies, nothing depends on it)[/dim]"
                    )

        # Add cyclic nodes section if any
        cyclic_nodes = set()
        for cycle in result.circular_dependencies:
            cyclic_nodes.update(cycle)
        cyclic_not_shown = [c for c in cyclic_nodes if c not in visited]
        if cyclic_not_shown:
            cyclic_branch = tree.add("[red]Nodes in Circular Dependencies[/red]")
            for cyclic_id in cyclic_not_shown:
                node = nodes.get(cyclic_id)
                if node:
                    icon = status_icons.get(node.status, "?")
                    cyclic_branch.add(f"{icon} [cyan]{cyclic_id}[/cyan]: {node.title}")

        console.print(tree)

    def _display_flat_format(result: DependencyGraphResult) -> None:
        """Display dependency graph as a flat list with indentation."""
        source_label = "PRD Stories" if result.source_type == "prd" else "Mega-Plan Features"
        console.print(f"\n[bold blue]Dependency Graph ({source_label})[/bold blue]\n")

        nodes = result.nodes

        # Sort by depth for display
        sorted_nodes = sorted(nodes.values(), key=lambda n: (n.depth, n.id))

        for node in sorted_nodes:
            indent = "  " * node.depth

            # Status indicator
            status_char = {
                "pending": "o",
                "in_progress": ">",
                "complete": "v",
                "failed": "x",
            }.get(node.status, "?")

            # Priority indicator
            priority_char = {
                "high": "H",
                "medium": "M",
                "low": "L",
            }.get(node.priority, "?")

            # Markers
            markers = []
            if node.on_critical_path:
                markers.append("*")
            if node.is_bottleneck:
                markers.append("B")
            if node.is_orphan:
                markers.append("O")
            marker_str = "".join(markers)

            # Print node
            console.print(
                f"{indent}[{status_char}] [{priority_char}] {node.id}: {node.title}"
                + (f" [{marker_str}]" if marker_str else "")
            )

            # Print dependencies
            if node.dependencies:
                deps_str = ", ".join(node.dependencies)
                console.print(f"{indent}    <- depends on: {deps_str}")

            # Print dependents
            if node.dependents:
                blocks_str = ", ".join(node.dependents)
                console.print(f"{indent}    -> blocks: {blocks_str}")

    def _display_summary(result: DependencyGraphResult) -> None:
        """Display summary information including critical path and issues."""
        console.print()

        # Critical path
        if result.critical_path:
            path_str = " -> ".join(result.critical_path)
            console.print(
                f"[bold magenta]Critical Path:[/bold magenta] {path_str} "
                f"({result.critical_path_length} levels)"
            )

        # Bottlenecks
        if result.bottlenecks:
            for bottleneck_id in result.bottlenecks:
                node = result.nodes.get(bottleneck_id)
                if node:
                    console.print(
                        f"[yellow]![/yellow] [bold]Bottleneck:[/bold] {bottleneck_id} "
                        f"({len(node.dependents)} dependents)"
                    )

        # Orphans
        if result.orphans:
            orphans_str = ", ".join(result.orphans)
            console.print(
                f"[blue]i[/blue] [bold]Orphan nodes:[/bold] {orphans_str} "
                "(no dependencies, nothing depends on them)"
            )

        # Circular dependencies (errors)
        if result.circular_dependencies:
            console.print()
            console.print(
                f"[red][bold]CIRCULAR DEPENDENCIES DETECTED ({len(result.circular_dependencies)} cycle(s))[/bold][/red]"
            )
            console.print("[red]The following cycles must be resolved before execution:[/red]")
            console.print()
            for i, cycle in enumerate(result.circular_dependencies, 1):
                cycle_str = " -> ".join(cycle)
                console.print(f"  [red]Cycle {i}:[/red] {cycle_str}")
            console.print()
            console.print("[dim]Tip: Break cycles by removing or reordering dependencies[/dim]")

        # Legend
        console.print()
        console.print("[dim]Legend: * = critical path, B = bottleneck, O = orphan[/dim]")
        console.print("[dim]Status: o = pending, > = in progress, v = complete, x = failed[/dim]")

    def _display_table_format(result: DependencyGraphResult) -> None:
        """Display dependency graph as a table."""
        source_label = "PRD Stories" if result.source_type == "prd" else "Mega-Plan Features"

        table = Table(
            title=f"Dependency Graph ({source_label})",
            show_header=True,
            header_style="bold cyan",
        )

        table.add_column("ID", style="cyan", width=12)
        table.add_column("Title", style="white", width=30)
        table.add_column("Priority", width=8)
        table.add_column("Status", width=12)
        table.add_column("Depth", justify="center", width=6)
        table.add_column("Dependencies", style="dim", width=20)
        table.add_column("Dependents", style="dim", width=20)
        table.add_column("Flags", width=8)

        # Sort by depth
        sorted_nodes = sorted(result.nodes.values(), key=lambda n: (n.depth, n.id))

        for node in sorted_nodes:
            # Status style
            status_style = {
                "pending": "dim",
                "in_progress": "yellow",
                "complete": "green",
                "failed": "red",
            }.get(node.status, "white")

            # Priority style
            priority_style = {
                "high": "red",
                "medium": "yellow",
                "low": "dim",
            }.get(node.priority, "white")

            # Flags
            flags = []
            if node.on_critical_path:
                flags.append("[magenta]*[/magenta]")
            if node.is_bottleneck:
                flags.append("[red]B[/red]")
            if node.is_orphan:
                flags.append("[dim]O[/dim]")

            deps_str = ", ".join(node.dependencies) or "-"
            dependents_str = ", ".join(node.dependents) or "-"

            table.add_row(
                node.id,
                node.title[:28] + ".." if len(node.title) > 30 else node.title,
                f"[{priority_style}]{node.priority}[/{priority_style}]",
                f"[{status_style}]{node.status}[/{status_style}]",
                str(node.depth),
                deps_str[:18] + ".." if len(deps_str) > 20 else deps_str,
                dependents_str[:18] + ".." if len(dependents_str) > 20 else dependents_str,
                " ".join(flags) or "-",
            )

        console.print(table)

    def dependencies_command(
        ctx: typer.Context,
        format: str = typer.Option(
            "tree",
            "--format",
            "-f",
            help="Output format: tree, flat, table, json",
        ),
        project_path: str | None = typer.Option(
            None,
            "--project",
            "-p",
            help="Project path (defaults to current directory)",
        ),
        show_critical_path: bool = typer.Option(
            True,
            "--critical-path/--no-critical-path",
            help="Show critical path analysis",
        ),
        check_issues: bool = typer.Option(
            True,
            "--check/--no-check",
            help="Check for dependency issues",
        ),
        strict: bool = typer.Option(
            False,
            "--strict",
            help="Exit with error code if circular dependencies are detected",
        ),
    ) -> None:
        """
        Display dependency graph for PRD stories or mega-plan features.

        Analyzes dependencies and shows:
        - Visual tree/flat/table representation
        - Critical path (longest dependency chain)
        - Bottleneck nodes (many dependents)
        - Orphan nodes (isolated)
        - Circular dependency detection

        Examples:
            plan-cascade deps
            plan-cascade deps --format flat
            plan-cascade deps --format json
            plan-cascade deps --no-critical-path --no-check
            plan-cascade deps --strict  # Exit with error if cycles found
        """
        from .context import get_cli_context

        project = Path(project_path) if project_path else Path.cwd()

        # Get PathResolver from CLI context (if available)
        # This respects --legacy-mode flag and auto-detection
        cli_ctx = get_cli_context(ctx)

        # If project_path is explicitly provided, create a new PathResolver for that path
        # Otherwise, use the context's path resolver (which respects --legacy-mode)
        if project_path:
            from ..state.path_resolver import PathResolver, detect_project_mode

            detected_mode = detect_project_mode(project)
            path_resolver = PathResolver(project, legacy_mode=detected_mode == "legacy")
        else:
            path_resolver = cli_ctx.get_path_resolver()
            project = cli_ctx.project_root

        try:
            analyzer = DependencyGraphAnalyzer(project, path_resolver=path_resolver)
            result = analyzer.analyze()

            # JSON output
            if format.lower() == "json":
                console.print_json(json.dumps(result.to_dict(), indent=2))
                return

            # Display header
            console.print()
            console.print(f"[bold blue]Plan Cascade Dependency Analysis[/bold blue]")
            console.print(f"[dim]Project: {project}[/dim]")
            console.print(f"[dim]Source: {result.source_type}.json[/dim]")
            console.print()

            # Display based on format
            if format.lower() == "tree":
                _display_tree_format(result)
            elif format.lower() == "flat":
                _display_flat_format(result)
            elif format.lower() == "table":
                _display_table_format(result)
            else:
                _print_error(f"Unknown format: {format}. Use: tree, flat, table, json")
                raise typer.Exit(1)

            # Display summary with critical path and issues
            if show_critical_path or check_issues:
                _display_summary(result)

            # Handle circular dependencies
            if result.circular_dependencies:
                console.print()
                if check_issues:
                    if strict:
                        _print_error(
                            f"Dependency graph has {len(result.circular_dependencies)} circular "
                            "dependency cycle(s)."
                        )
                    else:
                        _print_error(
                            f"Found {len(result.circular_dependencies)} circular dependency cycle(s). "
                            "These must be resolved before execution."
                        )
                    raise typer.Exit(1)
                _print_warning(
                    f"Found {len(result.circular_dependencies)} circular dependency cycle(s). "
                    "These must be resolved before execution."
                )

        except FileNotFoundError as e:
            _print_error(str(e))
            _print_info("Create a prd.json or mega-plan.json first:")
            console.print("  - [cyan]plan-cascade run '<description>' --expert[/cyan]")
            console.print("  - [cyan]plan-cascade mega plan '<description>'[/cyan]")
            raise typer.Exit(1)
        except json.JSONDecodeError as e:
            _print_error(f"Invalid JSON in plan file: {e}")
            raise typer.Exit(1)
        except Exception as e:
            _print_error(f"Error analyzing dependencies: {e}")
            raise typer.Exit(1)


def main():
    """CLI entry point for dependencies command (standalone testing)."""
    if HAS_TYPER:
        # Create a simple app for standalone testing
        app = typer.Typer()
        app.command(name="deps")(dependencies_command)
        app()
    else:
        print("Dependencies command requires 'typer' and 'rich' packages.")
        print("Install with: pip install typer rich")
        sys.exit(1)


if __name__ == "__main__":
    main()
