#!/usr/bin/env python3
"""
PRD Validation Script for Hybrid Ralph

Validates a prd.json file and displays review information.
"""

import json
import sys
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


def validate_prd(prd: dict) -> tuple[bool, list[str]]:
    """Validate PRD structure and content."""
    errors = []

    # Check metadata
    if "metadata" not in prd:
        errors.append("Missing 'metadata' section")
    else:
        if "description" not in prd["metadata"]:
            errors.append("Missing 'description' in metadata")

    # Check goal
    if "goal" not in prd or not prd["goal"]:
        errors.append("Missing or empty 'goal'")

    # Check stories
    if "stories" not in prd:
        errors.append("Missing 'stories' section")
    else:
        story_ids = set()
        for i, story in enumerate(prd["stories"]):
            if "id" not in story:
                errors.append(f"Story {i}: Missing 'id'")
            else:
                story_id = story["id"]
                if story_id in story_ids:
                    errors.append(f"Duplicate story ID: {story_id}")
                story_ids.add(story_id)

            if "title" not in story or not story["title"]:
                errors.append(f"Story {i}: Missing or empty 'title'")

            if "description" not in story or not story["description"]:
                errors.append(f"Story {i}: Missing or empty 'description'")

            # Validate dependencies exist
            for dep in story.get("dependencies", []):
                if dep not in story_ids and dep not in [s.get("id") for s in prd["stories"]]:
                    errors.append(f"Story {story.get('id', i)}: Unknown dependency '{dep}'")

    return (len(errors) == 0, errors)


def display_review(prd: dict):
    """Display PRD review."""
    print("=" * 60)
    print("PRD REVIEW")
    print("=" * 60)
    print()

    # Overview
    print("## Overview")
    print(f"**Goal:** {prd.get('goal', 'N/A')}")
    print()

    description = prd.get("metadata", {}).get("description", "N/A")
    print(f"**Description:** {description}")
    print()

    stories = prd.get("stories", [])
    print(f"**Total Stories:** {len(stories)}")
    print()

    # Calculate batches
    batches = calculate_batches(stories)
    print(f"**Estimated Batches:** {len(batches)}")
    print()

    # Stories by priority
    high = [s for s in stories if s.get("priority") == "high"]
    medium = [s for s in stories if s.get("priority") == "medium"]
    low = [s for s in stories if s.get("priority") == "low"]

    print("**Stories by Priority:**")
    print(f"  - High: {len(high)}")
    print(f"  - Medium: {len(medium)}")
    print(f"  - Low: {len(low)}")
    print()

    # Stories
    print("## User Stories")
    print()

    for story in stories:
        print(f"### {story['id']}: {story['title']}")
        print()
        print(f"**Priority:** {story.get('priority', 'N/A')}")
        print(f"**Status:** {story.get('status', 'N/A')}")
        print(f"**Context Estimate:** {story.get('context_estimate', 'N/A')}")
        print()

        print(f"**Description:**")
        print(f"  {story.get('description', 'N/A')}")
        print()

        acceptance_criteria = story.get("acceptance_criteria", [])
        if acceptance_criteria:
            print(f"**Acceptance Criteria:**")
            for criterion in acceptance_criteria:
                print(f"  - {criterion}")
            print()

        dependencies = story.get("dependencies", [])
        if dependencies:
            print(f"**Dependencies:** {', '.join(dependencies)}")
        else:
            print(f"**Dependencies:** None")
        print()

        tags = story.get("tags", [])
        if tags:
            print(f"**Tags:** {', '.join(tags)}")
        else:
            print(f"**Tags:** None")
        print()

        print("---")
        print()

    # Dependency graph
    if stories:
        print("## Dependency Graph")
        print()
        print("```")
        for story in stories:
            deps = story.get("dependencies", [])
            if deps:
                print(f"{story['id']} -> {', '.join(deps)}")
            else:
                print(f"{story['id']} (no dependencies)")
        print("```")
        print()

    # Execution plan
    if batches:
        print("## Execution Plan")
        print()
        for i, batch in enumerate(batches, 1):
            print(f"### Batch {i}:")
            if i == 1:
                print("  (Independent Stories - Parallel Execution)")
            else:
                print("  (Dependent Stories)")
            for story in batch:
                deps = story.get("dependencies", [])
                dep_str = f" (depends on: {', '.join(deps)})" if deps else ""
                print(f"  - {story['id']}: {story['title']}{dep_str}")
            print()


def calculate_batches(stories: list) -> list:
    """Calculate execution batches based on dependencies."""
    story_map = {s["id"]: s for s in stories}
    completed = set()
    batches = []

    while len(completed) < len(stories):
        ready = []

        for story in stories:
            story_id = story["id"]

            if story_id in completed:
                continue

            # Check if all dependencies are complete
            deps = story.get("dependencies", [])
            if all(dep in completed for dep in deps):
                ready.append(story)

        if not ready:
            # Circular dependency or error - add remaining
            ready = [s for s in stories if s["id"] not in completed]

        # Sort by priority
        priority_order = {"high": 0, "medium": 1, "low": 2}
        ready.sort(key=lambda s: priority_order.get(s.get("priority", "medium"), 1))

        batches.append(ready)
        completed.update(s["id"] for s in ready)

    return batches


def main():
    """Main entry point."""
    prd_path = Path.cwd() / "prd.json"

    if len(sys.argv) > 1:
        prd_path = Path(sys.argv[1])

    # Load PRD
    prd = load_prd(prd_path)

    # Validate PRD
    is_valid, errors = validate_prd(prd)

    if not is_valid:
        print("PRD Validation Errors:")
        for error in errors:
            print(f"  - {error}")
        print()
        sys.exit(1)

    print("PRD is valid!")
    print()

    # Display review
    display_review(prd)

    print()
    print("=" * 60)
    print("ACTIONS")
    print("=" * 60)
    print()
    print("  /approve    - Accept this PRD and begin execution")
    print("  /edit       - Modify the PRD")
    print()


if __name__ == "__main__":
    main()
