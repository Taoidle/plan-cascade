#!/usr/bin/env python3
"""
Dependency Visualization Script for Hybrid Ralph

Shows dependency graph and analysis for stories in the PRD.
"""

import json
import sys
from collections import defaultdict
from pathlib import Path


def load_prd(prd_path: Path) -> dict:
    """Load PRD from file."""
    if not prd_path.exists():
        print(f"Error: PRD file not found: {prd_path}")
        sys.exit(1)

    try:
        with open(prd_path, "r", encoding="utf-8") as f:
            return json.load(f)
    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON in PRD file: {e}")
        sys.exit(1)


def build_dependency_graph(stories: list) -> dict:
    """Build dependency graph from stories."""
    graph = defaultdict(list)
    reverse_graph = defaultdict(list)
    story_map = {s["id"]: s for s in stories}

    for story in stories:
        story_id = story["id"]
        deps = story.get("dependencies", [])

        for dep in deps:
            if dep in story_map:
                graph[dep].append(story_id)
                reverse_graph[story_id].append(dep)

    return dict(graph), dict(reverse_graph), story_map


def calculate_depths(story_id: str, graph: dict, story_map: dict, memo: dict = None) -> int:
    """Calculate the depth of a story in the dependency tree."""
    if memo is None:
        memo = {}

    if story_id in memo:
        return memo[story_id]

    deps = story_map.get(story_id, {}).get("dependencies", [])

    if not deps:
        memo[story_id] = 0
        return 0

    max_dep_depth = max(calculate_depths(dep, graph, story_map, memo) for dep in deps)
    memo[story_id] = max_dep_depth + 1
    return memo[story_id]


def detect_cycles(graph: dict, story_map: dict) -> list:
    """Detect circular dependencies using DFS."""
    cycles = []
    visited = set()
    rec_stack = set()

    def dfs(node, path):
        if node in rec_stack:
            cycle_start = path.index(node)
            cycles.append(path[cycle_start:] + [node])
            return

        if node in visited:
            return

        visited.add(node)
        rec_stack.add(node)

        for neighbor in graph.get(node, []):
            dfs(neighbor, path + [node])

        rec_stack.remove(node)

    for story_id in story_map.keys():
        if story_id not in visited:
            dfs(story_id, [])

    return cycles


def find_bottlenecks(graph: dict) -> list:
    """Find stories that many other stories depend on."""
    bottlenecks = []

    for story_id, dependents in graph.items():
        if len(dependents) >= 2:
            bottlenecks.append((story_id, len(dependents)))

    bottlenecks.sort(key=lambda x: x[1], reverse=True)
    return bottlenecks


def find_orphans(reverse_graph: dict, story_map: dict) -> list:
    """Find stories with no dependencies and nothing depends on them."""
    orphans = []

    for story_id in story_map.keys():
        has_deps = bool(story_map[story_id].get("dependencies"))
        has_dependents = bool(reverse_graph.get(story_id))

        if not has_deps and not has_dependents:
            orphans.append(story_id)

    return orphans


def display_dependency_graph(prd: dict):
    """Display the dependency graph and analysis."""
    print("=" * 60)
    print("DEPENDENCY GRAPH")
    print("=" * 60)
    print()

    stories = prd.get("stories", [])
    if not stories:
        print("No stories found in PRD.")
        return

    # Build graph
    graph, reverse_graph, story_map = build_dependency_graph(stories)

    # Calculate depths
    depths = {}
    for story_id in story_map.keys():
        depths[story_id] = calculate_depths(story_id, graph, story_map)

    # Detect cycles
    cycles = detect_cycles(graph, story_map)

    # Find bottlenecks
    bottlenecks = find_bottlenecks(graph)

    # Find orphans
    orphans = find_orphans(reverse_graph, story_map)

    # Visual graph
    print("## Visual Graph")
    print()

    # Group by depth
    by_depth = defaultdict(list)
    for story_id, depth in depths.items():
        by_depth[depth].append(story_id)

    for depth in sorted(by_depth.keys()):
        stories_at_depth = by_depth[depth]
        depth_name = "Root" if depth == 0 else f"Depth {depth}"

        if depth == 0:
            print(f"### {depth_name} (can start immediately)")
        else:
            print(f"### {depth_name}")

        for story_id in sorted(stories_at_depth):
            story = story_map[story_id]
            deps = story.get("dependencies", [])
            title = story.get("title", "")

            if deps:
                print(f"  {story_id}: {title}")
                print(f"      depends on: {', '.join(deps)}")
            else:
                print(f"  {story_id}: {title}")

        print()

    # Dependency details
    print("## Dependency Details")
    print()

    for story in sorted(stories, key=lambda s: depths.get(s["id"], 0)):
        story_id = story["id"]
        title = story.get("title", "")
        deps = story.get("dependencies", [])
        depth = depths.get(story_id, 0)
        dependents = graph.get(story_id, [])

        print(f"{story_id}: {title}")
        print(f"  Dependencies: {', '.join(deps) if deps else 'None'}")
        print(f"  Depth: {depth}", end="")

        if depth == 0:
            print(" (Root)")
        elif not dependents:
            print(" (Endpoint)")
        else:
            print()

        if dependents:
            print(f"  Dependents: {', '.join(dependents)}")

            if len(dependents) >= 2:
                print(f"  └─ Bottleneck story ({len(dependents)} dependents)")

        if not deps and not dependents and len(stories) > 1:
            print(f"  └─ Orphan story")

        print()

    # Analysis
    print("## Analysis")
    print()

    max_depth = max(depths.values()) if depths else 0
    print(f"- Maximum depth: {max_depth} level{'s' if max_depth != 1 else ''}")

    # Find critical path
    critical_path = []
    current = max(depths, key=depths.get) if depths else None
    while current:
        critical_path.append(current)
        deps = story_map.get(current, {}).get("dependencies", [])
        if not deps:
            break
        # Follow dependency with highest depth
        current = max(deps, key=lambda d: depths.get(d, 0))

    if critical_path:
        print(f"- Critical path length: {len(critical_path)} stories")
        print(f"- Critical path: {' → '.join(critical_path)}")

    if bottlenecks:
        print(f"- Bottleneck stories:")
        for story_id, count in bottlenecks:
            print(f"  • {story_id} ({count} dependents)")
    else:
        print(f"- No bottleneck stories")

    if orphans:
        print(f"- Orphan stories: {', '.join(orphans)}")
    else:
        print(f"- No orphan stories")

    if cycles:
        print(f"- ⚠️ Circular dependencies detected:")
        for cycle in cycles:
            print(f"  • {' → '.join(cycle)}")
    else:
        print(f"- ✓ No circular dependencies")

    print()

    # Warnings
    if cycles:
        print("## ⚠️ Issues Detected")
        print()
        print("### Circular Dependencies")
        print()
        for cycle in cycles:
            print(f"Cycle: {' → '.join(cycle)}")
            print("This will prevent any story in the cycle from starting.")
            print("Break the cycle by removing one of the dependencies.")
            print()

    if bottlenecks:
        if not cycles:  # Only show this section if no cycles
            print("## ⚠️ Bottleneck Warnings")
            print()

        for story_id, count in bottlenecks:
            if count >= 3:
                story = story_map[story_id]
                print(f"{story_id}: {count} stories depend on this")
                print(f"  Title: {story.get('title', '')}")
                print(f"  Consider breaking this story into smaller pieces")
                print()

    if orphans:
        if not cycles and not any(count >= 3 for _, count in bottlenecks):
            print("## ℹ️ Information")
            print()

        for story_id in orphans:
            story = story_map[story_id]
            print(f"{story_id}: Orphan story")
            print(f"  Title: {story.get('title', '')}")
            print(f"  This story has no dependencies and nothing depends on it")
            print(f"  It may not be part of the main workflow")
            print()

    print("=" * 60)


def main():
    """Main entry point."""
    prd_path = Path.cwd() / "prd.json"

    if len(sys.argv) > 1:
        prd_path = Path(sys.argv[1])

    # Load PRD
    prd = load_prd(prd_path)

    # Display dependency graph
    display_dependency_graph(prd)


if __name__ == "__main__":
    main()
