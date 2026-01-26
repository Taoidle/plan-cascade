#!/usr/bin/env python3
"""
PRD Generation Helper for Hybrid Ralph

Interactive helper for generating PRDs from task descriptions.
This script sets up the structure and prompts for LLM to fill in stories.
"""

import json
import sys
from datetime import datetime
from pathlib import Path


def create_prd_template(description: str) -> dict:
    """Create a PRD template from the description."""
    return {
        "metadata": {
            "created_at": datetime.now().isoformat(),
            "version": "1.0.0",
            "description": description
        },
        "goal": extract_goal(description),
        "objectives": extract_objectives(description),
        "stories": []
    }


def extract_goal(description: str) -> str:
    """Extract the main goal from the description."""
    # First sentence or first 200 chars
    sentences = description.split(".")
    if sentences:
        first = sentences[0].strip()
        if len(first) > 200:
            return first[:200] + "..."
        return first
    return description[:200] + "..." if len(description) > 200 else description


def extract_objectives(description: str) -> list:
    """Extract objectives from the description."""
    # Look for bullet points or numbered lists
    objectives = []

    lines = description.split("\n")
    for line in lines:
        line = line.strip()
        if line.startswith("- ") or line.startswith("* "):
            objectives.append(line[2:].strip())
        elif line.startswith(f"{len(objectives)+1}.") or line.startswith(f"{len(objectives)+1})"):
            objectives.append(line.split(".", 1)[1].strip() if "." in line else line.split(")", 1)[1].strip())

    return objectives if objectives else ["Complete the described task"]


def save_prd(prd: dict, prd_path: Path):
    """Save PRD to file."""
    prd_path.parent.mkdir(parents=True, exist_ok=True)

    with open(prd_path, "w", encoding="utf-8") as f:
        json.dump(prd, f, indent=2)


def main():
    """Main entry point."""
    if len(sys.argv) < 2:
        print("Usage: prd-generate.py '<description>'")
        print()
        print("Creates a PRD template from your task description.")
        print("The template should be filled in by an LLM to create user stories.")
        sys.exit(1)

    description = " ".join(sys.argv[1:])
    project_root = Path.cwd()
    prd_path = project_root / "prd.json"

    # Create template
    prd = create_prd_template(description)

    # Save template
    save_prd(prd, prd_path)

    print(f"PRD template created at: {prd_path}")
    print()
    print("Next steps:")
    print("  1. Use an LLM to analyze the task and fill in stories")
    print("  2. Or use /hybrid:auto for automatic story generation")
    print()
    print("Template structure:")
    print(json.dumps(prd, indent=2))


if __name__ == "__main__":
    main()
