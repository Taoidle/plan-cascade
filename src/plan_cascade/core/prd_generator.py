#!/usr/bin/env python3
"""
PRD Generator for Plan Cascade

Generates structured PRD (Product Requirements Document) from user descriptions.
Breaks down complex tasks into user stories with priorities and dependencies.
"""

import json
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ..state.path_resolver import PathResolver


class PRDGenerator:
    """Generates PRD from user task descriptions."""

    def __init__(
        self,
        project_root: Path,
        path_resolver: "PathResolver | None" = None,
        legacy_mode: bool | None = None,
    ):
        """
        Initialize the PRD generator.

        Args:
            project_root: Root directory of the project
            path_resolver: Optional PathResolver instance. If not provided,
                creates a default one based on legacy_mode setting.
            legacy_mode: If True, use project root for PRD path (backward compatible).
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
            from ..state.path_resolver import PathResolver
            self._path_resolver = PathResolver(
                project_root=self.project_root,
                legacy_mode=legacy_mode,
            )

        # Use PathResolver for PRD path
        self.prd_path = self._path_resolver.get_prd_path()
        self.story_counter = 0

    @property
    def path_resolver(self) -> "PathResolver":
        """Get the PathResolver instance."""
        return self._path_resolver

    def is_legacy_mode(self) -> bool:
        """Check if running in legacy mode."""
        return self._path_resolver.is_legacy_mode()

    def generate_prd(self, description: str, context: dict | None = None) -> dict:
        """
        Generate a PRD from a task description.

        This method should be called by an LLM with the task description.
        The LLM will analyze the description and break it into user stories.

        Args:
            description: User's task description
            context: Optional additional context (existing files, constraints, etc.)

        Returns:
            PRD dictionary
        """
        prd = {
            "metadata": {
                "created_at": datetime.now().isoformat(),
                "version": "1.0.0",
                "description": description
            },
            "goal": self._extract_goal(description),
            "objectives": self._extract_objectives(description),
            "stories": []
        }

        return prd

    def add_story(
        self,
        prd: dict,
        title: str,
        description: str,
        priority: str = "medium",
        dependencies: list[str] | None = None,
        acceptance_criteria: list[str] | None = None,
        context_estimate: str = "medium",
        tags: list[str] | None = None,
        test_expectations: dict | None = None,
    ) -> dict:
        """
        Add a user story to the PRD.

        Args:
            prd: PRD dictionary
            title: Story title
            description: Story description
            priority: Priority (high, medium, low)
            dependencies: List of story IDs this story depends on
            acceptance_criteria: List of acceptance criteria
            context_estimate: Estimated context size (small, medium, large, xlarge)
            tags: Optional tags for categorization
            test_expectations: Optional test expectations for TDD mode. Dictionary with:
                - required: bool - Whether tests are required for this story
                - test_types: list[str] - Types of tests expected (unit, integration, e2e)
                - coverage_areas: list[str] - Areas that should be covered by tests
                - min_tests: int - Minimum number of test cases expected

        Returns:
            Updated PRD dictionary
        """
        self.story_counter += 1
        story_id = f"story-{self.story_counter:03d}"

        story = {
            "id": story_id,
            "title": title,
            "description": description,
            "priority": priority,
            "dependencies": dependencies or [],
            "status": "pending",
            "acceptance_criteria": acceptance_criteria or [],
            "context_estimate": context_estimate,
            "tags": tags or []
        }

        # Add test_expectations if provided
        if test_expectations is not None:
            story["test_expectations"] = test_expectations

        prd["stories"].append(story)
        return prd

    def estimate_context_size(self, story_description: str, codebase_info: dict | None = None) -> str:
        """
        Estimate the context size needed for a story.

        Args:
            story_description: Description of the story
            codebase_info: Optional information about the codebase

        Returns:
            Context size estimate (small, medium, large, xlarge)
        """
        # Heuristic estimation based on description length and complexity
        description_lower = story_description.lower()

        # Check for complexity indicators
        complexity_keywords = {
            "xlarge": ["refactor", "architecture", "migration", "rewrite", "restructure"],
            "large": ["implement", "create", "build", "develop", "integrate", "multiple"],
            "medium": ["add", "update", "modify", "extend", "enhance"],
            "small": ["fix", "correct", "adjust", "tweak", "minor"]
        }

        # Count keyword matches
        scores = {"small": 0, "medium": 0, "large": 0, "xlarge": 0}

        for level, keywords in complexity_keywords.items():
            for keyword in keywords:
                if keyword in description_lower:
                    scores[level] += 1

        # Description length factor
        word_count = len(story_description.split())
        if word_count > 100:
            scores["large"] += 1
        elif word_count > 50:
            scores["medium"] += 1

        # Determine highest score
        max_score = max(scores.values())
        for size in ["xlarge", "large", "medium", "small"]:
            if scores[size] == max_score and max_score > 0:
                return size

        return "medium"  # Default

    def detect_dependencies(self, stories: list[dict]) -> list[dict]:
        """
        Detect dependencies between stories automatically.

        Analyzes story descriptions for dependency indicators.

        Args:
            stories: List of story dictionaries

        Returns:
            Updated stories with dependencies populated
        """
        dependency_keywords = [
            "after", "once", "depends on", "requires", "following",
            "based on", "building on", "extends"
        ]

        for i, story in enumerate(stories):
            description = story.get("description", "").lower()
            title = story.get("title", "").lower()

            deps = []

            # Check if this story mentions other stories
            for j, other_story in enumerate(stories):
                if i == j:
                    continue

                other_id = other_story.get("id", "")
                other_title = other_story.get("title", "").lower()

                # Check for explicit dependency keywords
                for keyword in dependency_keywords:
                    if keyword in description or keyword in title:
                        if other_title in description or other_title in title:
                            if other_id not in deps:
                                deps.append(other_id)

            stories[i]["dependencies"] = deps

        return stories

    def generate_execution_batches(self, prd: dict) -> list[list[dict]]:
        """
        Generate parallel execution batches from the PRD.

        Stories with no dependencies run in parallel first.
        Dependent stories wait for their dependencies to complete.

        Args:
            prd: PRD dictionary

        Returns:
            List of batches, where each batch is a list of stories
        """
        stories = prd.get("stories", [])
        if not stories:
            return []

        # Create a map of story dependencies
        story_map = {s["id"]: s for s in stories}
        completed = set()
        batches = []

        while len(completed) < len(stories):
            # Find stories ready to execute
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
                # Circular dependency or error
                remaining = [s for s in stories if s["id"] not in completed]
                print(f"Warning: Could not resolve dependencies for: {[s['id'] for s in remaining]}")
                # Add remaining as next batch anyway
                ready = remaining

            # Sort by priority within batch
            priority_order = {"high": 0, "medium": 1, "low": 2}
            ready.sort(key=lambda s: priority_order.get(s.get("priority", "medium"), 1))

            batches.append(ready)
            completed.update(s["id"] for s in ready)

        return batches

    def validate_prd(self, prd: dict) -> tuple[bool, list[str]]:
        """
        Validate a PRD for correctness.

        Args:
            prd: PRD dictionary

        Returns:
            Tuple of (is_valid, list_of_errors)
        """
        errors = []

        # Check required fields
        if "metadata" not in prd:
            errors.append("Missing 'metadata' section")
        elif "description" not in prd["metadata"]:
            errors.append("Missing 'description' in metadata")

        if "goal" not in prd or not prd["goal"]:
            errors.append("Missing or empty 'goal'")

        if "stories" not in prd:
            errors.append("Missing 'stories' section")
        else:
            # Validate each story
            story_ids = set()
            for i, story in enumerate(prd["stories"]):
                if "id" not in story:
                    errors.append(f"Story {i}: Missing 'id'")
                else:
                    if story["id"] in story_ids:
                        errors.append(f"Duplicate story ID: {story['id']}")
                    story_ids.add(story["id"])

                if "title" not in story or not story["title"]:
                    errors.append(f"Story {i}: Missing or empty 'title'")

                if "description" not in story or not story["description"]:
                    errors.append(f"Story {i}: Missing or empty 'description'")

                # Validate dependencies exist
                for dep in story.get("dependencies", []):
                    if dep not in story_ids:
                        errors.append(f"Story {story.get('id', i)}: Unknown dependency '{dep}'")

                # Validate test_expectations if present
                test_expectations = story.get("test_expectations")
                if test_expectations is not None:
                    if not isinstance(test_expectations, dict):
                        errors.append(f"Story {story.get('id', i)}: test_expectations must be a dictionary")
                    else:
                        # Validate test_types if present
                        test_types = test_expectations.get("test_types")
                        if test_types is not None and not isinstance(test_types, list):
                            errors.append(f"Story {story.get('id', i)}: test_expectations.test_types must be a list")
                        # Validate coverage_areas if present
                        coverage_areas = test_expectations.get("coverage_areas")
                        if coverage_areas is not None and not isinstance(coverage_areas, list):
                            errors.append(f"Story {story.get('id', i)}: test_expectations.coverage_areas must be a list")

        return (len(errors) == 0, errors)

    def _extract_goal(self, description: str) -> str:
        """Extract the main goal from the description."""
        # Simple extraction: first sentence or first 100 chars
        sentences = re.split(r'[.!?]', description)
        if sentences:
            first_sentence = sentences[0].strip()
            if len(first_sentence) > 200:
                return first_sentence[:200] + "..."
            return first_sentence
        return description[:200]

    def _extract_objectives(self, description: str) -> list[str]:
        """Extract objectives from the description."""
        # Look for bullet points or numbered lists
        objectives = []

        # Try bullet points
        bullet_pattern = r'^[\s]*[-*]\s+(.+)$'
        for line in description.split('\n'):
            match = re.match(bullet_pattern, line)
            if match:
                objectives.append(match.group(1).strip())

        # Try numbered lists
        if not objectives:
            number_pattern = r'^[\s]*\d+[.)\s]+(.+)$'
            for line in description.split('\n'):
                match = re.match(number_pattern, line)
                if match:
                    objectives.append(match.group(1).strip())

        return objectives


def create_sample_prd() -> dict:
    """Create a sample PRD for demonstration."""
    prd = {
        "metadata": {
            "created_at": "2024-01-15T10:00:00",
            "version": "1.0.0",
            "description": "Implement a user authentication system"
        },
        "goal": "Create a secure user authentication system with login, registration, and password reset functionality",
        "objectives": [
            "Allow users to register new accounts",
            "Allow existing users to log in",
            "Provide password reset functionality",
            "Ensure secure password storage"
        ],
        "stories": [
            {
                "id": "story-001",
                "title": "Design database schema for users",
                "description": "Create the database schema to store user information including email, password hash, and metadata.",
                "priority": "high",
                "dependencies": [],
                "status": "pending",
                "acceptance_criteria": [
                    "Users table with id, email, password_hash columns",
                    "Unique constraint on email",
                    "Timestamps for created_at and updated_at"
                ],
                "context_estimate": "small",
                "tags": ["database", "design"]
            },
            {
                "id": "story-002",
                "title": "Implement user registration",
                "description": "Create API endpoint for user registration with email validation and password hashing.",
                "priority": "high",
                "dependencies": ["story-001"],
                "status": "pending",
                "acceptance_criteria": [
                    "POST /api/auth/register endpoint",
                    "Email format validation",
                    "Password hashed with bcrypt",
                    "Returns JWT token on success"
                ],
                "context_estimate": "medium",
                "tags": ["api", "auth"]
            },
            {
                "id": "story-003",
                "title": "Implement user login",
                "description": "Create API endpoint for user login with JWT token generation.",
                "priority": "high",
                "dependencies": ["story-001"],
                "status": "pending",
                "acceptance_criteria": [
                    "POST /api/auth/login endpoint",
                    "Verifies password against hash",
                    "Returns JWT token on success",
                    "Returns 401 for invalid credentials"
                ],
                "context_estimate": "medium",
                "tags": ["api", "auth"]
            },
            {
                "id": "story-004",
                "title": "Implement password reset",
                "description": "Create password reset flow with email verification and token-based reset.",
                "priority": "medium",
                "dependencies": ["story-002", "story-003"],
                "status": "pending",
                "acceptance_criteria": [
                    "POST /api/auth/forgot-password endpoint",
                    "Email sent with reset token",
                    "POST /api/auth/reset-password endpoint",
                    "Token expires after 1 hour"
                ],
                "context_estimate": "large",
                "tags": ["api", "auth", "email"]
            }
        ]
    }
    return prd


def main():
    """CLI interface for testing PRD generator."""
    if len(sys.argv) < 2:
        print("Usage: prd_generator.py <command> [args]")
        print("Commands:")
        print("  generate <description>     - Generate PRD from description")
        print("  validate                   - Validate existing PRD")
        print("  batches                    - Show execution batches")
        print("  sample                     - Create sample PRD")
        sys.exit(1)

    command = sys.argv[1]
    project_root = Path.cwd()

    pg = PRDGenerator(project_root)

    if command == "generate" and len(sys.argv) >= 3:
        description = " ".join(sys.argv[2:])
        prd = pg.generate_prd(description)
        print(json.dumps(prd, indent=2))

    elif command == "validate":
        from ..state.state_manager import StateManager
        sm = StateManager(project_root)
        prd = sm.read_prd()
        if not prd:
            print("No PRD found")
            sys.exit(1)

        is_valid, errors = pg.validate_prd(prd)
        if is_valid:
            print("PRD is valid!")
        else:
            print("PRD validation errors:")
            for error in errors:
                print(f"  - {error}")

    elif command == "batches":
        from ..state.state_manager import StateManager
        sm = StateManager(project_root)
        prd = sm.read_prd()
        if not prd:
            print("No PRD found")
            sys.exit(1)

        batches = pg.generate_execution_batches(prd)
        print(f"Total batches: {len(batches)}")
        for i, batch in enumerate(batches, 1):
            print(f"\nBatch {i}:")
            for story in batch:
                deps = story.get("dependencies", [])
                dep_str = f" (depends on: {', '.join(deps)})" if deps else ""
                print(f"  - {story['id']}: {story['title']}{dep_str}")

    elif command == "sample":
        prd = create_sample_prd()
        print(json.dumps(prd, indent=2))

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
