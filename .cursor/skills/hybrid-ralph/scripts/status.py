#!/usr/bin/env python3
"""
Status Monitoring Script for Hybrid Ralph

Shows execution status of stories in the PRD.
"""

import json
import re
import sys
from datetime import datetime
from pathlib import Path


def load_prd(prd_path: Path) -> dict:
    """Load PRD from file."""
    if not prd_path.exists():
        return None

    try:
        with open(prd_path, "r", encoding="utf-8") as f:
            return json.load(f)
    except (json.JSONDecodeError, IOError):
        return None


def load_progress(progress_path: Path) -> dict:
    """Load progress information from progress.txt."""
    if not progress_path.exists():
        return {}

    statuses = {}

    try:
        with open(progress_path, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()

                # Look for status markers
                if "[COMPLETE]" in line:
                    match = re.search(r'story-\d+', line)
                    if match:
                        statuses[match.group()] = "complete"

                elif "[IN_PROGRESS]" in line:
                    match = re.search(r'story-\d+', line)
                    if match:
                        statuses[match.group()] = "in_progress"

                elif "[PENDING]" in line:
                    match = re.search(r'story-\d+', line)
                    if match:
                        statuses[match.group()] = "pending"

                elif "[FAILED]" in line:
                    match = re.search(r'story-\d+', line)
                    if match:
                        statuses[match.group()] = "failed"

    except IOError:
        pass

    return statuses


def calculate_batches(prd: dict) -> list:
    """Calculate execution batches based on dependencies."""
    stories = prd.get("stories", [])
    if not stories:
        return []

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
            ready = [s for s in stories if s["id"] not in completed]

        # Sort by priority
        priority_order = {"high": 0, "medium": 1, "low": 2}
        ready.sort(key=lambda s: priority_order.get(s.get("priority", "medium"), 1))

        batches.append(ready)
        completed.update(s["id"] for s in ready)

    return batches


def display_status(prd: dict, progress: dict):
    """Display execution status."""
    print("=" * 60)
    print("EXECUTION STATUS")
    print("=" * 60)
    print()

    stories = prd.get("stories", [])

    if not stories:
        print("No stories found in PRD.")
        return

    # Calculate batches
    batches = calculate_batches(prd)

    # Count by status
    status_counts = {"complete": 0, "in_progress": 0, "pending": 0, "failed": 0}

    for story in stories:
        story_id = story["id"]
        status = progress.get(story_id, "pending")
        status_counts[status] = status_counts.get(status, 0) + 1

    # Summary
    print("## Summary")
    print()
    print(f"  Total Stories: {len(stories)}")
    print(f"  Total Batches: {len(batches)}")
    print()
    print(f"  Complete:     {status_counts['complete']} ✓")
    print(f"  In Progress:  {status_counts['in_progress']} ◐")
    print(f"  Pending:      {status_counts['pending']} ○")
    print(f"  Failed:       {status_counts['failed']} ✗")
    print()

    # Progress bar
    total = len(stories)
    done = status_counts["complete"]
    percentage = (done / total * 100) if total > 0 else 0

    bar_length = 40
    filled = int(bar_length * done / total) if total > 0 else 0

    print("## Progress")
    print()
    print(f"  [{('█' * filled) + ('░' * (bar_length - filled))}] {percentage:.1f}%")
    print()

    # Current batch
    current_batch = None
    for i, batch in enumerate(batches, 1):
        batch_complete = all(
            progress.get(s["id"], "pending") == "complete"
            for s in batch
        )

        if not batch_complete:
            current_batch = (i, batch)
            break

    if current_batch:
        batch_num, batch = current_batch
        print(f"## Current Batch: {batch_num}")
        print()

        for story in batch:
            story_id = story["id"]
            title = story["title"]
            status = progress.get(story_id, "pending")

            status_symbol = {
                "complete": "●",
                "in_progress": "◐",
                "pending": "○",
                "failed": "✗"
            }.get(status, "?")

            status_color = {
                "complete": "\033[92m",  # Green
                "in_progress": "\033[93m",  # Yellow
                "pending": "\033[90m",  # Gray
                "failed": "\033[91m"  # Red
            }.get(status, "")
            reset = "\033[0m" if status_color else ""

            print(f"  {status_symbol} {story_id}: {title} [{status_color}{status}{reset}]")

        print()

    # All batches
    print("## All Batches")
    print()

    for i, batch in enumerate(batches, 1):
        batch_complete = all(
            progress.get(s["id"], "pending") == "complete"
            for s in batch
        )

        batch_status = "✓" if batch_complete else "○"

        print(f"  Batch {i}: {batch_status} ({len(batch)} stories)")

        for story in batch:
            story_id = story["id"]
            status = progress.get(story_id, "pending")
            status_symbol = {
                "complete": "●",
                "in_progress": "◐",
                "pending": "○",
                "failed": "✗"
            }.get(status, "?")
            print(f"    {status_symbol} {story_id}: {story['title']}")

        print()

    # Timeline (from progress.txt)
    print("## Recent Activity")
    print()

    progress_path = Path.cwd() / "progress.txt"
    if progress_path.exists():
        try:
            with open(progress_path, "r", encoding="utf-8") as f:
                lines = f.readlines()

            # Show last 10 entries
            for line in lines[-10:]:
                line = line.strip()
                if line:
                    print(f"  {line}")

        except IOError:
            pass

    print()

    # Check for failures
    if status_counts["failed"] > 0:
        print("## Failed Stories")
        print()
        for story in stories:
            if progress.get(story["id"]) == "failed":
                print(f"  ✗ {story['id']}: {story['title']}")
        print()


def main():
    """Main entry point."""
    project_root = Path.cwd()

    # If path provided, use it
    if len(sys.argv) > 1:
        project_root = Path(sys.argv[1])

    prd_path = project_root / "prd.json"
    progress_path = project_root / "progress.txt"

    # Load PRD
    prd = load_prd(prd_path)
    if not prd:
        print("Error: No PRD found. Use /hybrid:auto or /hybrid:manual first.")
        sys.exit(1)

    # Load progress
    progress = load_progress(progress_path)

    # Display status
    display_status(prd, progress)

    print("=" * 60)
    print()

    # Check if complete
    stories = prd.get("stories", [])
    all_complete = all(
        progress.get(s["id"], "pending") == "complete"
        for s in stories
    )

    if all_complete:
        print("✓ All stories complete!")
    else:
        pending = sum(1 for s in stories if progress.get(s["id"], "pending") != "complete")
        print(f"○ {pending} story(s) remaining")


if __name__ == "__main__":
    main()
