#!/usr/bin/env python3
"""
Context Filter for Plan Cascade

Extracts relevant context from prd.json and findings.md for specific stories.
Filters findings by tags, dependencies, and story relationships.
"""

import json
import re
import sys
from pathlib import Path
from typing import Dict, List, Optional


class ContextFilter:
    """Filters and extracts relevant context for a specific story."""

    def __init__(self, project_root: Path):
        """
        Initialize the context filter.

        Args:
            project_root: Root directory of the project
        """
        self.project_root = Path(project_root)
        self.prd_path = self.project_root / "prd.json"
        self.findings_path = self.project_root / "findings.md"

    def load_prd(self) -> Optional[Dict]:
        """
        Load the PRD JSON file.

        Returns:
            PRD dictionary or None if not found
        """
        if not self.prd_path.exists():
            return None

        try:
            with open(self.prd_path, "r", encoding="utf-8") as f:
                return json.load(f)
        except (json.JSONDecodeError, IOError) as e:
            print(f"Warning: Could not load PRD: {e}")
            return None

    def get_story(self, story_id: str) -> Optional[Dict]:
        """
        Get a specific story from the PRD.

        Args:
            story_id: Story ID (e.g., "story-001")

        Returns:
            Story dictionary or None if not found
        """
        prd = self.load_prd()
        if not prd:
            return None

        for story in prd.get("stories", []):
            if story.get("id") == story_id:
                return story

        return None

    def get_dependencies(self, story_id: str) -> List[str]:
        """
        Get the list of dependency story IDs for a story.

        Args:
            story_id: Story ID

        Returns:
            List of dependency story IDs
        """
        story = self.get_story(story_id)
        if not story:
            return []

        return story.get("dependencies", [])

    def get_dependent_stories(self, story_id: str) -> List[str]:
        """
        Get all stories that depend on the given story.

        Args:
            story_id: Story ID

        Returns:
            List of story IDs that depend on this story
        """
        prd = self.load_prd()
        if not prd:
            return []

        dependents = []
        for story in prd.get("stories", []):
            if story_id in story.get("dependencies", []):
                dependents.append(story.get("id"))

        return dependents

    def parse_findings_tags(self, content: str) -> Dict[str, List[str]]:
        """
        Parse findings.md to extract tagged sections.

        Looks for sections tagged with format:
            <!-- @tags: story-001,story-002 -->

        Args:
            content: Full findings.md content

        Returns:
            Dictionary mapping tag to list of section contents
        """
        tagged_sections: Dict[str, List[str]] = {}

        # Pattern to match tagged sections
        # Matches <!-- @tags: tag1,tag2 --> followed by content until next tag or end
        tag_pattern = r'<!--\s*@tags:\s*([^>]+)\s*-->'
        splits = re.split(tag_pattern, content)

        # First element is content before any tags (untagged)
        # Then pairs of (tags, content)

        i = 1  # Start after the initial content
        while i < len(splits):
            tags_str = splits[i]
            content_block = splits[i + 1] if i + 1 < len(splits) else ""

            # Parse comma-separated tags
            tags = [t.strip() for t in tags_str.split(",") if t.strip()]

            for tag in tags:
                if tag not in tagged_sections:
                    tagged_sections[tag] = []
                tagged_sections[tag].append(content_block.strip())

            i += 2

        return tagged_sections

    def get_context_for_story(self, story_id: str) -> Dict:
        """
        Get all relevant context for a specific story.

        Includes:
        - Story metadata from PRD
        - Summaries of completed dependencies
        - Tagged findings sections relevant to this story

        Args:
            story_id: Story ID

        Returns:
            Dictionary with story context
        """
        story = self.get_story(story_id)
        if not story:
            return {
                "error": f"Story {story_id} not found in PRD"
            }

        # Get dependency summaries
        dependency_ids = self.get_dependencies(story_id)
        dependency_summaries = []

        for dep_id in dependency_ids:
            dep_story = self.get_story(dep_id)
            if dep_story:
                # Check if dependency is marked complete in progress.txt
                status = self._get_story_status(dep_id)
                dependency_summaries.append({
                    "id": dep_id,
                    "title": dep_story.get("title", ""),
                    "status": status,
                    "summary": dep_story.get("description", "")
                })

        # Get tagged findings
        findings_context = self._get_tagged_findings(story_id)

        return {
            "story": story,
            "dependencies": dependency_summaries,
            "findings": findings_context,
            "context_estimate": story.get("context_estimate", "medium")
        }

    def _get_story_status(self, story_id: str) -> str:
        """
        Get the completion status of a story from progress.txt.

        Args:
            story_id: Story ID

        Returns:
            Status: "complete", "in_progress", "pending", or "unknown"
        """
        progress_path = self.project_root / "progress.txt"

        if not progress_path.exists():
            return "unknown"

        try:
            with open(progress_path, "r", encoding="utf-8") as f:
                content = f.read()

            # Look for completion marker
            if f"[COMPLETE] {story_id}" in content:
                return "complete"
            elif f"[IN_PROGRESS] {story_id}" in content:
                return "in_progress"
            elif f"[PENDING] {story_id}" in content:
                return "pending"

            return "unknown"
        except IOError:
            return "unknown"

    def _get_tagged_findings(self, story_id: str) -> List[str]:
        """
        Get all findings sections tagged with this story ID.

        Args:
            story_id: Story ID

        Returns:
            List of relevant findings sections
        """
        if not self.findings_path.exists():
            return []

        try:
            with open(self.findings_path, "r", encoding="utf-8") as f:
                content = f.read()
        except IOError:
            return []

        tagged_sections = self.parse_findings_tags(content)

        # Get sections tagged with this story ID
        relevant_sections = tagged_sections.get(story_id, [])

        # Also include untagged sections (general findings)
        if "" in tagged_sections:
            relevant_sections.extend(tagged_sections[""])

        return relevant_sections

    def get_execution_batch(self, batch_num: int) -> List[Dict]:
        """
        Get all stories that can be executed in a given batch.

        Batch 1: Stories with no dependencies
        Batch 2+: Stories whose dependencies are all complete

        Args:
            batch_num: Batch number (1-indexed)

        Returns:
            List of stories ready for execution in this batch
        """
        prd = self.load_prd()
        if not prd:
            return []

        stories = prd.get("stories", [])
        ready_stories = []

        for story in stories:
            deps = story.get("dependencies", [])

            # Check if all dependencies are complete
            all_complete = True
            for dep_id in deps:
                if self._get_story_status(dep_id) != "complete":
                    all_complete = False
                    break

            # Count how many dependencies the story has
            dep_count = len(deps)

            # Story is ready if:
            # - Batch 1: No dependencies
            # - Batch 2+: All dependencies complete
            if batch_num == 1:
                if dep_count == 0:
                    ready_stories.append(story)
            else:
                if all_complete and dep_count > 0:
                    ready_stories.append(story)

        return ready_stories

    def generate_batch_plan(self) -> List[List[Dict]]:
        """
        Generate the complete execution plan with all batches.

        Returns:
            List of batches, where each batch is a list of stories
        """
        batches = []
        batch_num = 1

        while True:
            batch = self.get_execution_batch(batch_num)
            if not batch:
                break
            batches.append(batch)
            batch_num += 1

        return batches


def main():
    """CLI interface for testing context filter."""
    if len(sys.argv) < 2:
        print("Usage: context_filter.py <command> [args]")
        print("Commands:")
        print("  get-story <story_id>     - Get story details")
        print("  get-context <story_id>   - Get context for a story")
        print("  get-batch <batch_num>    - Get stories in execution batch")
        print("  plan-batches              - Show full execution plan")
        sys.exit(1)

    command = sys.argv[1]
    project_root = Path.cwd()

    cf = ContextFilter(project_root)

    if command == "get-story" and len(sys.argv) >= 3:
        story_id = sys.argv[2]
        story = cf.get_story(story_id)
        print(json.dumps(story, indent=2))

    elif command == "get-context" and len(sys.argv) >= 3:
        story_id = sys.argv[2]
        context = cf.get_context_for_story(story_id)
        print(json.dumps(context, indent=2))

    elif command == "get-batch" and len(sys.argv) >= 3:
        batch_num = int(sys.argv[2])
        batch = cf.get_execution_batch(batch_num)
        print(json.dumps(batch, indent=2))

    elif command == "plan-batches":
        batches = cf.generate_batch_plan()
        print(f"Total batches: {len(batches)}")
        for i, batch in enumerate(batches, 1):
            print(f"\nBatch {i}:")
            for story in batch:
                print(f"  - {story.get('id')}: {story.get('title')}")

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
