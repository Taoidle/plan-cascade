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
from typing import TYPE_CHECKING

from ..core.external_skill_loader import ExternalSkillLoader

if TYPE_CHECKING:
    from .path_resolver import PathResolver


class ContextFilter:
    """Filters and extracts relevant context for a specific story."""

    def __init__(
        self,
        project_root: Path,
        plugin_root: Path = None,
        path_resolver: "PathResolver | None" = None,
        legacy_mode: bool | None = None,
    ):
        """
        Initialize the context filter.

        Args:
            project_root: Root directory of the project
            plugin_root: Root directory of the Plan Cascade plugin (optional)
            path_resolver: Optional PathResolver instance. If not provided,
                creates a default one based on legacy_mode setting.
            legacy_mode: If True, use project root for all paths (backward compatible).
                If None, defaults to True when path_resolver is not provided.
        """
        self.project_root = Path(project_root)

        # Set up PathResolver
        if path_resolver is not None:
            self._path_resolver = path_resolver
        else:
            # Default to legacy mode for backward compatibility
            if legacy_mode is None:
                legacy_mode = True
            from .path_resolver import PathResolver
            self._path_resolver = PathResolver(
                project_root=self.project_root,
                legacy_mode=legacy_mode,
            )

        # Use PathResolver for PRD path
        self.prd_path = self._path_resolver.get_prd_path()

        # findings.md and design_doc.json are user-visible files and stay in project root
        self.findings_path = self.project_root / "findings.md"
        self.design_doc_path = self.project_root / "design_doc.json"

        # Initialize external skill loader
        self.external_loader = ExternalSkillLoader(project_root, plugin_root)

    @property
    def path_resolver(self) -> "PathResolver":
        """Get the PathResolver instance."""
        return self._path_resolver

    def is_legacy_mode(self) -> bool:
        """Check if running in legacy mode."""
        return self._path_resolver.is_legacy_mode()

    def load_prd(self) -> dict | None:
        """
        Load the PRD JSON file.

        Returns:
            PRD dictionary or None if not found
        """
        if not self.prd_path.exists():
            return None

        try:
            with open(self.prd_path, encoding="utf-8") as f:
                return json.load(f)
        except (OSError, json.JSONDecodeError) as e:
            print(f"Warning: Could not load PRD: {e}")
            return None

    def load_design_doc(self) -> dict | None:
        """
        Load the design document JSON file.

        Returns:
            Design document dictionary or None if not found
        """
        if not self.design_doc_path.exists():
            return None

        try:
            with open(self.design_doc_path, encoding="utf-8") as f:
                return json.load(f)
        except (OSError, json.JSONDecodeError) as e:
            print(f"Warning: Could not load design document: {e}")
            return None

    def load_parent_design_doc(self) -> dict | None:
        """
        Load the parent (project-level) design document.

        Looks for design_doc.json in parent directories for worktree scenarios.

        Returns:
            Parent design document dictionary or None if not found
        """
        # Check parent directory
        parent_path = self.project_root.parent / "design_doc.json"
        if parent_path.exists():
            try:
                with open(parent_path, encoding="utf-8") as f:
                    doc = json.load(f)
                    # Only return if it's a project-level doc
                    if doc.get("metadata", {}).get("level") == "project":
                        return doc
            except (OSError, json.JSONDecodeError):
                pass

        # Check grandparent (for .worktree/feature-name/ structure)
        grandparent_path = self.project_root.parent.parent / "design_doc.json"
        if grandparent_path.exists():
            try:
                with open(grandparent_path, encoding="utf-8") as f:
                    doc = json.load(f)
                    if doc.get("metadata", {}).get("level") == "project":
                        return doc
            except (OSError, json.JSONDecodeError):
                pass

        return None

    def get_design_context_for_story(self, story_id: str) -> dict:
        """
        Get design context relevant to a specific story.

        Uses story_mappings to filter relevant components, decisions,
        interfaces, and patterns for the story. Also includes inherited
        context from project-level design document if available.

        Args:
            story_id: Story ID

        Returns:
            Dictionary with relevant design context:
            - overview: Feature overview
            - components: List of relevant component definitions
            - decisions: List of relevant ADRs (feature + inherited)
            - apis: List of relevant API definitions
            - data_models: List of relevant data models
            - patterns: List of architectural patterns to follow (feature + inherited)
            - data_flow: Data flow description
            - inherited: Inherited context from project-level doc
        """
        design_doc = self.load_design_doc()
        if not design_doc:
            # Try loading parent design doc directly
            parent_doc = self.load_parent_design_doc()
            if parent_doc:
                return self._get_project_context_only(parent_doc)
            return {}

        # Get inherited context from parent document
        parent_doc = self.load_parent_design_doc()
        inherited_context = self._get_inherited_context(design_doc, parent_doc)

        # Get story mapping
        story_mappings = design_doc.get("story_mappings", {})
        mapping = story_mappings.get(story_id, {})

        if not mapping:
            # If no specific mapping, return overview context with inherited
            return {
                "overview": design_doc.get("overview", {}),
                "patterns": design_doc.get("architecture", {}).get("patterns", []),
                "inherited": inherited_context
            }

        # Extract relevant components
        component_names = mapping.get("components", [])
        all_components = design_doc.get("architecture", {}).get("components", [])
        relevant_components = [
            c for c in all_components
            if c.get("name") in component_names
        ]

        # Extract relevant decisions (feature-level)
        decision_ids = mapping.get("decisions", [])
        all_decisions = design_doc.get("decisions", [])
        relevant_decisions = [
            d for d in all_decisions
            if d.get("id") in decision_ids
        ]

        # Extract relevant interfaces
        interface_ids = mapping.get("interfaces", [])
        all_apis = design_doc.get("interfaces", {}).get("apis", [])
        all_models = design_doc.get("interfaces", {}).get("data_models", [])

        relevant_apis = [
            api for api in all_apis
            if api.get("id") in interface_ids
        ]
        relevant_models = [
            model for model in all_models
            if model.get("name") in interface_ids
        ]

        # Always include architectural patterns (global guidance)
        patterns = design_doc.get("architecture", {}).get("patterns", [])

        return {
            "overview": design_doc.get("overview", {}),
            "components": relevant_components,
            "decisions": relevant_decisions,
            "apis": relevant_apis,
            "data_models": relevant_models,
            "patterns": patterns,
            "data_flow": design_doc.get("architecture", {}).get("data_flow", ""),
            "inherited": inherited_context
        }

    def _get_inherited_context(
        self,
        feature_doc: dict,
        parent_doc: dict | None
    ) -> dict:
        """
        Get inherited context from parent (project-level) design document.

        Args:
            feature_doc: Feature-level design document
            parent_doc: Project-level design document (or None)

        Returns:
            Dictionary with inherited context
        """
        if not parent_doc:
            # Check if feature doc has inherited_context section
            return feature_doc.get("inherited_context", {})

        inherited = {
            "project_overview": parent_doc.get("overview", {}),
            "patterns": [],
            "decisions": [],
            "shared_models": [],
            "api_standards": parent_doc.get("interfaces", {}).get("api_standards", {})
        }

        # Get feature ID from the feature doc
        feature_id = feature_doc.get("metadata", {}).get("feature_id")

        # Get patterns and decisions from feature_mappings
        if feature_id:
            feature_mappings = parent_doc.get("feature_mappings", {})
            if feature_id in feature_mappings:
                mapping = feature_mappings[feature_id]
                inherited_pattern_names = mapping.get("patterns", [])
                inherited_decision_ids = mapping.get("decisions", [])

                # Get full pattern definitions
                all_patterns = parent_doc.get("architecture", {}).get("patterns", [])
                inherited["patterns"] = [
                    p for p in all_patterns
                    if p.get("name") in inherited_pattern_names
                ]

                # Get full decision definitions
                all_decisions = parent_doc.get("decisions", [])
                inherited["decisions"] = [
                    d for d in all_decisions
                    if d.get("id") in inherited_decision_ids
                ]

        # Get shared data models
        shared_models = parent_doc.get("interfaces", {}).get("shared_data_models", [])
        inherited["shared_models"] = shared_models

        return inherited

    def _get_project_context_only(self, parent_doc: dict) -> dict:
        """
        Get context from project-level document when no feature doc exists.

        Args:
            parent_doc: Project-level design document

        Returns:
            Dictionary with project context
        """
        return {
            "overview": parent_doc.get("overview", {}),
            "patterns": parent_doc.get("architecture", {}).get("patterns", []),
            "decisions": parent_doc.get("decisions", []),
            "api_standards": parent_doc.get("interfaces", {}).get("api_standards", {}),
            "shared_models": parent_doc.get("interfaces", {}).get("shared_data_models", []),
            "data_flow": parent_doc.get("architecture", {}).get("data_flow", ""),
            "is_project_level": True
        }

    def get_story(self, story_id: str) -> dict | None:
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

    def get_dependencies(self, story_id: str) -> list[str]:
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

    def get_dependent_stories(self, story_id: str) -> list[str]:
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

    def parse_findings_tags(self, content: str) -> dict[str, list[str]]:
        """
        Parse findings.md to extract tagged sections.

        Looks for sections tagged with format:
            <!-- @tags: story-001,story-002 -->

        Args:
            content: Full findings.md content

        Returns:
            Dictionary mapping tag to list of section contents
        """
        tagged_sections: dict[str, list[str]] = {}

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

    def get_context_for_story(self, story_id: str) -> dict:
        """
        Get all relevant context for a specific story.

        Includes:
        - Story metadata from PRD
        - Summaries of completed dependencies
        - Tagged findings sections relevant to this story
        - Design context (components, decisions, patterns) if available

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

        # Get design context (if design_doc.json exists)
        design_context = self.get_design_context_for_story(story_id)

        # Get external skill context
        external_skill_context = self.external_loader.get_skill_context("implementation")

        # Include global execution configuration (if present in PRD)
        prd = self.load_prd() or {}
        tdd_config = prd.get("tdd_config")
        flow_config = prd.get("flow_config")

        return {
            "story": story,
            "dependencies": dependency_summaries,
            "findings": findings_context,
            "design": design_context,
            "external_skills": external_skill_context,
            "tdd_config": tdd_config,
            "flow_config": flow_config,
            "context_estimate": story.get("context_estimate", "medium")
        }

    def get_external_skill_context(self, phase: str = "implementation") -> str:
        """
        Get external skill context for a specific phase.

        Args:
            phase: Execution phase (implementation, retry, etc.)

        Returns:
            Formatted skill context string
        """
        return self.external_loader.get_skill_context(phase)

    def get_detected_frameworks(self) -> list[str]:
        """
        Get list of detected framework skills for the current project.

        Returns:
            List of applicable skill names
        """
        return self.external_loader.detect_applicable_skills()

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
            with open(progress_path, encoding="utf-8") as f:
                content = f.read()

            # Look for completion marker
            if f"[COMPLETE] {story_id}" in content:
                return "complete"
            elif f"[IN_PROGRESS] {story_id}" in content:
                return "in_progress"
            elif f"[PENDING] {story_id}" in content:
                return "pending"

            return "unknown"
        except OSError:
            return "unknown"

    def _get_tagged_findings(self, story_id: str) -> list[str]:
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
            with open(self.findings_path, encoding="utf-8") as f:
                content = f.read()
        except OSError:
            return []

        tagged_sections = self.parse_findings_tags(content)

        # Get sections tagged with this story ID
        relevant_sections = tagged_sections.get(story_id, [])

        # Also include untagged sections (general findings)
        if "" in tagged_sections:
            relevant_sections.extend(tagged_sections[""])

        return relevant_sections

    def get_execution_batch(self, batch_num: int) -> list[dict]:
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

    def generate_batch_plan(self) -> list[list[dict]]:
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
