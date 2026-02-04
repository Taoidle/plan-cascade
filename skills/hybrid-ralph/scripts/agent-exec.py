#!/usr/bin/env python3
"""
Agent Execution Helper for Hybrid Ralph

Helper script for agents executing individual stories.
Provides context filtering and progress tracking.
"""

import json
import sys
from pathlib import Path


def load_story_context(story_id: str, project_root: Path) -> dict:
    """Load context for a specific story."""
    prd_path = project_root / "prd.json"
    findings_path = project_root / "findings.md"
    progress_path = project_root / "progress.txt"

    # Load PRD
    try:
        with open(prd_path, encoding="utf-8") as f:
            prd = json.load(f)
    except (OSError, json.JSONDecodeError):
        return {"error": "Could not load PRD"}

    # Find the story
    story = None
    for s in prd.get("stories", []):
        if s.get("id") == story_id:
            story = s
            break

    if not story:
        return {"error": f"Story {story_id} not found in PRD"}

    # Get dependency summaries
    deps = story.get("dependencies", [])
    dependency_summaries = []

    for dep_id in deps:
        dep_story = None
        for s in prd.get("stories", []):
            if s.get("id") == dep_id:
                dep_story = s
                break

        if dep_story:
            # Check if complete
            status = "unknown"
            if progress_path.exists():
                try:
                    with open(progress_path, encoding="utf-8") as f:
                        content = f.read()
                    if f"[COMPLETE] {dep_id}" in content:
                        status = "complete"
                    elif f"[IN_PROGRESS] {dep_id}" in content:
                        status = "in_progress"
                except OSError:
                    pass

            dependency_summaries.append({
                "id": dep_id,
                "title": dep_story.get("title", ""),
                "status": status,
                "summary": dep_story.get("description", "")[:200] + "..."
            })

    # Get tagged findings
    findings_sections = []
    if findings_path.exists():
        try:
            with open(findings_path, encoding="utf-8") as f:
                content = f.read()

            # Simple extraction of tagged sections
            import re
            tag_pattern = rf'<!--\s*@tags:\s*([^>]*{re.escape(story_id)}[^>]*)\s*-->'
            matches = re.finditer(tag_pattern, content)

            for match in matches:
                # Get content after this tag until next tag or end
                start = match.end()
                next_tag = content.find("<!-- @tags:", start)
                if next_tag == -1:
                    section = content[start:]
                else:
                    section = content[start:next_tag]

                findings_sections.append(section.strip()[:500])

        except OSError:
            pass

    return {
        "story": story,
        "dependencies": dependency_summaries,
        "findings": findings_sections,
        "goal": prd.get("goal", ""),
        "objectives": prd.get("objectives", [])
    }


def display_story_context(context: dict):
    """Display story context for the agent."""
    if "error" in context:
        print(f"Error: {context['error']}")
        return

    story = context["story"]

    print("=" * 60)
    print(f"STORY: {story['id']} - {story['title']}")
    print("=" * 60)
    print()

    print("## Description")
    print(story.get("description", ""))
    print()

    if context.get("goal"):
        print("## Project Goal")
        print(context["goal"])
        print()

    if context.get("objectives"):
        print("## Project Objectives")
        for obj in context["objectives"]:
            print(f"  - {obj}")
        print()

    print("## Acceptance Criteria")
    for i, criterion in enumerate(story.get("acceptance_criteria", []), 1):
        print(f"  {i}. {criterion}")
    print()

    deps = context.get("dependencies", [])
    if deps:
        print("## Dependencies")
        for dep in deps:
            status_symbol = {
                "complete": "✓",
                "in_progress": "◐",
                "unknown": "?"
            }.get(dep.get("status", "unknown"), "?")

            print(f"  {status_symbol} {dep['id']}: {dep['title']} [{dep.get('status', 'unknown')}]")
            print(f"      {dep['summary']}")
        print()

    findings = context.get("findings", [])
    if findings:
        print("## Relevant Findings")
        for i, finding in enumerate(findings, 1):
            print(f"  {i}. {finding[:200]}...")
        print()

    print("=" * 60)
    print()


def mark_story_in_progress(story_id: str, project_root: Path):
    """Mark a story as in progress in progress.txt."""
    progress_path = project_root / "progress.txt"

    from datetime import datetime
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

    try:
        with open(progress_path, "a", encoding="utf-8") as f:
            f.write(f"[{timestamp}] {story_id}: [IN_PROGRESS] {story_id}\n")
    except OSError:
        pass


def mark_story_complete(story_id: str, project_root: Path):
    """Mark a story as complete in progress.txt."""
    progress_path = project_root / "progress.txt"

    from datetime import datetime
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

    try:
        with open(progress_path, "a", encoding="utf-8") as f:
            f.write(f"[{timestamp}] {story_id}: [COMPLETE] {story_id}\n")
    except OSError:
        pass


def main():
    """Main entry point."""
    if len(sys.argv) < 2:
        print("Usage: agent-exec.py <command> [args]")
        print("Commands:")
        print("  context <story_id>        - Show context for a story")
        print("  start <story_id>          - Mark story as in progress")
        print("  complete <story_id>       - Mark story as complete")
        print("  run <story_id>            - Start story and show context")
        sys.exit(1)

    command = sys.argv[1]
    project_root = Path.cwd()

    if len(sys.argv) > 3:
        project_root = Path(sys.argv[3])

    if command == "context" and len(sys.argv) >= 3:
        story_id = sys.argv[2]
        context = load_story_context(story_id, project_root)
        display_story_context(context)

    elif command == "start" and len(sys.argv) >= 3:
        story_id = sys.argv[2]
        mark_story_in_progress(story_id, project_root)
        print(f"Marked {story_id} as in progress")

    elif command == "complete" and len(sys.argv) >= 3:
        story_id = sys.argv[2]
        mark_story_complete(story_id, project_root)
        print(f"Marked {story_id} as complete")

    elif command == "run" and len(sys.argv) >= 3:
        story_id = sys.argv[2]
        mark_story_in_progress(story_id, project_root)
        context = load_story_context(story_id, project_root)
        display_story_context(context)

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
