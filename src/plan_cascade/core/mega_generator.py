#!/usr/bin/env python3
"""
Mega Plan Generator for Plan Cascade

Generates project-level mega-plan from user descriptions.
Breaks down complex projects into features that can be executed as hybrid:worktree tasks.
"""

import asyncio
import json
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from ..llm.base import LLMProvider
    from ..state.path_resolver import PathResolver


# Feature decomposition prompt template
FEATURE_DECOMPOSITION_PROMPT = """You are a technical project planner. Analyze the following project description and break it down into independent features that can be developed in parallel.

## Project Description
{description}

## Additional Context
{context}

## Instructions
Break down this project into 2-8 features. Each feature should be:
- Self-contained and independently deployable
- Small enough to complete in 1-3 days of work
- Named with lowercase alphanumeric characters and hyphens only (e.g., "user-auth", "api-endpoints")

For each feature, identify:
1. A clear title and description
2. Priority (high/medium/low) based on business value and technical dependencies
3. Dependencies on other features (by feature name)

## Output Format
Return ONLY a JSON array of features:
```json
[
  {{
    "name": "feature-name",
    "title": "Human Readable Title",
    "description": "Detailed description of the feature scope and requirements",
    "priority": "high|medium|low",
    "dependencies": ["other-feature-name"]
  }}
]
```

Important:
- Use lowercase with hyphens for names (e.g., "user-auth" not "UserAuth")
- Dependencies should reference feature names, not IDs
- High priority features should be foundational (auth, database, etc.)
- Return ONLY the JSON array, no additional text"""


class MegaPlanGenerator:
    """Generates mega-plan from project descriptions."""

    def __init__(
        self,
        project_root: Path,
        path_resolver: "PathResolver | None" = None,
        legacy_mode: bool | None = None,
    ):
        """
        Initialize the mega-plan generator.

        Args:
            project_root: Root directory of the project
            path_resolver: Optional PathResolver instance. If not provided,
                creates a default one based on legacy_mode setting.
            legacy_mode: If True, use project root for mega-plan path (backward compatible).
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

        # Use PathResolver for mega-plan path
        self.mega_plan_path = self._path_resolver.get_mega_plan_path()
        self.feature_counter = 0

    @property
    def path_resolver(self) -> "PathResolver":
        """Get the PathResolver instance."""
        return self._path_resolver

    def is_legacy_mode(self) -> bool:
        """Check if running in legacy mode."""
        return self._path_resolver.is_legacy_mode()

    def generate_mega_plan(
        self,
        description: str,
        execution_mode: str = "auto",
        target_branch: str = "main",
        context: dict | None = None
    ) -> dict:
        """
        Generate a mega-plan from a project description.

        This method should be called by an LLM with the project description.
        The LLM will analyze and break it into features.

        Args:
            description: User's project description
            execution_mode: "auto" or "manual"
            target_branch: Branch to merge into when complete
            context: Optional additional context

        Returns:
            Mega-plan dictionary
        """
        mega_plan = {
            "metadata": {
                "created_at": datetime.now().isoformat(),
                "version": "1.0.0"
            },
            "goal": self._extract_goal(description),
            "description": description,
            "execution_mode": execution_mode,
            "target_branch": target_branch,
            "features": []
        }

        return mega_plan

    async def generate_features_with_llm(
        self,
        plan: dict,
        llm: "LLMProvider",
        context: dict | None = None
    ) -> tuple[dict, list[str]]:
        """
        Generate features for a mega-plan using LLM analysis.

        Args:
            plan: Mega-plan dictionary to populate with features
            llm: LLM provider instance
            context: Optional additional context (design docs, constraints, etc.)

        Returns:
            Tuple of (updated plan, list of any errors/warnings)
        """
        errors: list[str] = []

        # Build context string
        context_str = "No additional context provided."
        if context:
            context_parts = []
            if context.get("design_doc"):
                context_parts.append(f"Design Document: {context.get('design_doc')}")
            if context.get("existing_features"):
                context_parts.append(f"Existing Features: {', '.join(context.get('existing_features', []))}")
            if context.get("constraints"):
                context_parts.append(f"Constraints: {context.get('constraints')}")
            if context.get("tech_stack"):
                context_parts.append(f"Tech Stack: {context.get('tech_stack')}")
            if context_parts:
                context_str = "\n".join(context_parts)

        # Build the prompt
        prompt = FEATURE_DECOMPOSITION_PROMPT.format(
            description=plan.get("description", ""),
            context=context_str
        )

        try:
            # Call LLM
            response = await llm.complete(
                messages=[{"role": "user", "content": prompt}],
                temperature=0.3,  # Lower temperature for more consistent output
                max_tokens=4096
            )

            # Parse the response
            features_data = self._parse_llm_features_response(response.content)

            if not features_data:
                errors.append("LLM returned no valid features")
                return plan, errors

            # Add features to the plan
            name_to_id: dict[str, str] = {}
            for feature_data in features_data:
                name = feature_data.get("name", "")
                if not name:
                    errors.append(f"Feature missing name: {feature_data}")
                    continue

                # Normalize name (ensure lowercase with hyphens)
                name = self._normalize_feature_name(name)

                # Add the feature
                plan = self.add_feature(
                    plan=plan,
                    name=name,
                    title=feature_data.get("title", name.replace("-", " ").title()),
                    description=feature_data.get("description", ""),
                    priority=feature_data.get("priority", "medium"),
                    dependencies=[]  # Will resolve after all features added
                )

                # Map name to ID for dependency resolution
                added_feature = plan["features"][-1]
                name_to_id[name] = added_feature["id"]

            # Resolve dependencies (convert names to IDs)
            for i, feature_data in enumerate(features_data):
                if i >= len(plan["features"]):
                    break

                feature = plan["features"][i]
                dep_names = feature_data.get("dependencies", [])

                resolved_deps = []
                for dep_name in dep_names:
                    dep_name = self._normalize_feature_name(dep_name)
                    if dep_name in name_to_id:
                        resolved_deps.append(name_to_id[dep_name])
                    else:
                        errors.append(f"Feature '{feature['name']}' has unknown dependency: '{dep_name}'")

                feature["dependencies"] = resolved_deps

        except Exception as e:
            errors.append(f"LLM generation failed: {str(e)}")

        return plan, errors

    def _parse_llm_features_response(self, content: str) -> list[dict[str, Any]]:
        """
        Parse LLM response to extract features JSON.

        Args:
            content: Raw LLM response content

        Returns:
            List of feature dictionaries
        """
        if not content:
            return []

        # Try to extract JSON from markdown code block
        json_match = re.search(r'```(?:json)?\s*([\s\S]*?)```', content)
        if json_match:
            content = json_match.group(1).strip()

        # Try to find JSON array directly
        array_match = re.search(r'\[\s*\{[\s\S]*\}\s*\]', content)
        if array_match:
            content = array_match.group(0)

        try:
            data = json.loads(content)
            if isinstance(data, list):
                return data
            elif isinstance(data, dict) and "features" in data:
                return data["features"]
            return []
        except json.JSONDecodeError:
            return []

    def _normalize_feature_name(self, name: str) -> str:
        """
        Normalize feature name to lowercase with hyphens.

        Args:
            name: Feature name to normalize

        Returns:
            Normalized name
        """
        # Remove 'feature-' prefix if present (will be added back)
        if name.startswith("feature-"):
            name = name[8:]

        # Convert to lowercase
        name = name.lower()

        # Replace spaces and underscores with hyphens
        name = re.sub(r'[\s_]+', '-', name)

        # Remove any non-alphanumeric characters except hyphens
        name = re.sub(r'[^a-z0-9-]', '', name)

        # Remove consecutive hyphens
        name = re.sub(r'-+', '-', name)

        # Remove leading/trailing hyphens
        name = name.strip('-')

        return name

    def add_feature(
        self,
        plan: dict,
        name: str,
        title: str,
        description: str,
        priority: str = "medium",
        dependencies: list[str] | None = None
    ) -> dict:
        """
        Add a feature to the mega-plan.

        Args:
            plan: Mega-plan dictionary
            name: Feature name (will be used for worktree directory, e.g., "feature-auth")
            title: Human-readable feature title
            description: Detailed feature description for PRD generation
            priority: Priority (high, medium, low)
            dependencies: List of feature IDs this feature depends on

        Returns:
            Updated mega-plan dictionary
        """
        self.feature_counter += 1
        feature_id = f"feature-{self.feature_counter:03d}"

        feature = {
            "id": feature_id,
            "name": name,
            "title": title,
            "description": description,
            "priority": priority,
            "dependencies": dependencies or [],
            "status": "pending"
        }

        plan["features"].append(feature)
        return plan

    def generate_feature_batches(self, plan: dict) -> list[list[dict]]:
        """
        Generate parallel execution batches from the mega-plan.

        Features with no dependencies run in parallel first.
        Dependent features wait for their dependencies to complete.

        Args:
            plan: Mega-plan dictionary

        Returns:
            List of batches, where each batch is a list of features
        """
        features = plan.get("features", [])
        if not features:
            return []

        # Create a map of feature dependencies
        feature_map = {f["id"]: f for f in features}
        completed = set()
        batches = []

        while len(completed) < len(features):
            # Find features ready to execute
            ready = []

            for feature in features:
                feature_id = feature["id"]

                if feature_id in completed:
                    continue

                # Check if all dependencies are complete
                deps = feature.get("dependencies", [])
                if all(dep in completed for dep in deps):
                    ready.append(feature)

            if not ready:
                # Circular dependency or error
                remaining = [f for f in features if f["id"] not in completed]
                print(f"Warning: Could not resolve dependencies for: {[f['id'] for f in remaining]}")
                # Add remaining as next batch anyway
                ready = remaining

            # Sort by priority within batch
            priority_order = {"high": 0, "medium": 1, "low": 2}
            ready.sort(key=lambda f: priority_order.get(f.get("priority", "medium"), 1))

            batches.append(ready)
            completed.update(f["id"] for f in ready)

        return batches

    def validate_mega_plan(self, plan: dict) -> tuple[bool, list[str]]:
        """
        Validate a mega-plan for correctness.

        Args:
            plan: Mega-plan dictionary

        Returns:
            Tuple of (is_valid, list_of_errors)
        """
        errors = []

        # Check required fields
        if "metadata" not in plan:
            errors.append("Missing 'metadata' section")

        if "goal" not in plan or not plan["goal"]:
            errors.append("Missing or empty 'goal'")

        if "execution_mode" not in plan:
            errors.append("Missing 'execution_mode'")
        elif plan["execution_mode"] not in ["auto", "manual"]:
            errors.append("'execution_mode' must be 'auto' or 'manual'")

        if "target_branch" not in plan or not plan["target_branch"]:
            errors.append("Missing or empty 'target_branch'")

        if "features" not in plan:
            errors.append("Missing 'features' section")
        elif not plan["features"]:
            errors.append("'features' list is empty - at least one feature required")
        else:
            # Validate each feature
            feature_ids = set()
            feature_names = set()

            for i, feature in enumerate(plan["features"]):
                # Check required fields
                if "id" not in feature:
                    errors.append(f"Feature {i}: Missing 'id'")
                else:
                    if feature["id"] in feature_ids:
                        errors.append(f"Duplicate feature ID: {feature['id']}")
                    feature_ids.add(feature["id"])

                if "name" not in feature or not feature["name"]:
                    errors.append(f"Feature {i}: Missing or empty 'name'")
                else:
                    if feature["name"] in feature_names:
                        errors.append(f"Duplicate feature name: {feature['name']}")
                    feature_names.add(feature["name"])
                    # Validate name format (valid directory name)
                    if not re.match(r'^[a-z0-9][a-z0-9-]*$', feature["name"]):
                        errors.append(f"Feature {feature.get('id', i)}: Invalid name format '{feature['name']}' - use lowercase alphanumeric with hyphens")

                if "title" not in feature or not feature["title"]:
                    errors.append(f"Feature {feature.get('id', i)}: Missing or empty 'title'")

                if "description" not in feature or not feature["description"]:
                    errors.append(f"Feature {feature.get('id', i)}: Missing or empty 'description'")

                # Validate dependencies exist
                for dep in feature.get("dependencies", []):
                    if dep not in feature_ids:
                        errors.append(f"Feature {feature.get('id', i)}: Unknown dependency '{dep}'")

                # Validate priority
                priority = feature.get("priority", "medium")
                if priority not in ["high", "medium", "low"]:
                    errors.append(f"Feature {feature.get('id', i)}: Invalid priority '{priority}'")

                # Validate status
                status = feature.get("status", "pending")
                valid_statuses = ["pending", "prd_generated", "approved", "in_progress", "complete", "failed"]
                if status not in valid_statuses:
                    errors.append(f"Feature {feature.get('id', i)}: Invalid status '{status}'")

        # Check for circular dependencies
        if "features" in plan and plan["features"]:
            cycle = self._detect_dependency_cycle(plan)
            if cycle:
                errors.append(f"Circular dependency detected: {' -> '.join(cycle)}")

        return (len(errors) == 0, errors)

    def _detect_dependency_cycle(self, plan: dict) -> list[str] | None:
        """
        Detect circular dependencies in the feature graph.

        Args:
            plan: Mega-plan dictionary

        Returns:
            List of feature IDs forming a cycle, or None if no cycle
        """
        features = plan.get("features", [])
        feature_map = {f["id"]: f for f in features}

        WHITE, GRAY, BLACK = 0, 1, 2
        color = {f["id"]: WHITE for f in features}
        parent = {}

        def dfs(fid):
            color[fid] = GRAY
            for dep in feature_map.get(fid, {}).get("dependencies", []):
                if dep not in color:
                    continue
                if color[dep] == GRAY:
                    # Found cycle - reconstruct path
                    cycle = [dep, fid]
                    current = fid
                    while current != dep and current in parent:
                        current = parent[current]
                        cycle.insert(1, current)
                    return cycle
                if color[dep] == WHITE:
                    parent[dep] = fid
                    result = dfs(dep)
                    if result:
                        return result
            color[fid] = BLACK
            return None

        for feature in features:
            if color[feature["id"]] == WHITE:
                result = dfs(feature["id"])
                if result:
                    return result

        return None

    def _extract_goal(self, description: str) -> str:
        """Extract the main goal from the description."""
        sentences = re.split(r'[.!?]', description)
        if sentences:
            first_sentence = sentences[0].strip()
            if len(first_sentence) > 200:
                return first_sentence[:200] + "..."
            return first_sentence
        return description[:200]

    def get_feature_by_id(self, plan: dict, feature_id: str) -> dict | None:
        """
        Get a feature by its ID.

        Args:
            plan: Mega-plan dictionary
            feature_id: Feature ID to find

        Returns:
            Feature dictionary or None
        """
        for feature in plan.get("features", []):
            if feature["id"] == feature_id:
                return feature
        return None

    def get_feature_by_name(self, plan: dict, name: str) -> dict | None:
        """
        Get a feature by its name.

        Args:
            plan: Mega-plan dictionary
            name: Feature name to find

        Returns:
            Feature dictionary or None
        """
        for feature in plan.get("features", []):
            if feature["name"] == name:
                return feature
        return None

    def get_features_by_status(self, plan: dict, status: str) -> list[dict]:
        """
        Get all features with a specific status.

        Args:
            plan: Mega-plan dictionary
            status: Status to filter by

        Returns:
            List of features with the specified status
        """
        return [f for f in plan.get("features", []) if f.get("status") == status]

    def calculate_progress(self, plan: dict) -> dict:
        """
        Calculate overall progress of the mega-plan.

        Args:
            plan: Mega-plan dictionary

        Returns:
            Progress dictionary with counts and percentage
        """
        features = plan.get("features", [])
        if not features:
            return {"total": 0, "completed": 0, "in_progress": 0, "pending": 0, "failed": 0, "percentage": 0}

        total = len(features)
        completed = len([f for f in features if f.get("status") == "complete"])
        in_progress = len([f for f in features if f.get("status") in ["prd_generated", "approved", "in_progress"]])
        failed = len([f for f in features if f.get("status") == "failed"])
        pending = total - completed - in_progress - failed

        percentage = int((completed / total) * 100) if total > 0 else 0

        return {
            "total": total,
            "completed": completed,
            "in_progress": in_progress,
            "pending": pending,
            "failed": failed,
            "percentage": percentage
        }


def create_sample_mega_plan() -> dict:
    """Create a sample mega-plan for demonstration."""
    return {
        "metadata": {
            "created_at": "2026-01-28T10:00:00Z",
            "version": "1.0.0"
        },
        "goal": "Build a complete e-commerce platform with user authentication, product management, and order processing",
        "description": "Build a complete e-commerce platform. It should include user authentication with JWT, product catalog with search and filtering, shopping cart functionality, and order processing with payment integration.",
        "execution_mode": "auto",
        "target_branch": "main",
        "features": [
            {
                "id": "feature-001",
                "name": "feature-auth",
                "title": "User Authentication System",
                "description": "Implement user authentication including registration, login, logout, password reset, and JWT token management. Include email verification for new accounts.",
                "priority": "high",
                "dependencies": [],
                "status": "pending"
            },
            {
                "id": "feature-002",
                "name": "feature-products",
                "title": "Product Catalog",
                "description": "Implement product management including CRUD operations, categories, search with Elasticsearch, filtering by category/price/rating, and product image handling.",
                "priority": "high",
                "dependencies": [],
                "status": "pending"
            },
            {
                "id": "feature-003",
                "name": "feature-cart",
                "title": "Shopping Cart",
                "description": "Implement shopping cart functionality including add/remove items, quantity adjustment, price calculation, cart persistence for logged-in users.",
                "priority": "medium",
                "dependencies": ["feature-001", "feature-002"],
                "status": "pending"
            },
            {
                "id": "feature-004",
                "name": "feature-orders",
                "title": "Order Processing",
                "description": "Implement order processing including checkout flow, payment integration with Stripe, order confirmation emails, order history, and status tracking.",
                "priority": "medium",
                "dependencies": ["feature-003"],
                "status": "pending"
            }
        ]
    }


def main():
    """CLI interface for testing mega-plan generator."""
    if len(sys.argv) < 2:
        print("Usage: mega_generator.py <command> [args]")
        print("Commands:")
        print("  validate                    - Validate existing mega-plan")
        print("  batches                     - Show execution batches")
        print("  sample                      - Create sample mega-plan")
        print("  progress                    - Show progress")
        sys.exit(1)

    command = sys.argv[1]
    project_root = Path.cwd()

    mg = MegaPlanGenerator(project_root)

    if command == "validate":
        from ..state.mega_state import MegaStateManager
        sm = MegaStateManager(project_root)
        plan = sm.read_mega_plan()
        if not plan:
            print("No mega-plan.json found")
            sys.exit(1)

        is_valid, errors = mg.validate_mega_plan(plan)
        if is_valid:
            print("Mega-plan is valid!")
        else:
            print("Mega-plan validation errors:")
            for error in errors:
                print(f"  - {error}")

    elif command == "batches":
        from ..state.mega_state import MegaStateManager
        sm = MegaStateManager(project_root)
        plan = sm.read_mega_plan()
        if not plan:
            print("No mega-plan.json found")
            sys.exit(1)

        batches = mg.generate_feature_batches(plan)
        print(f"Total batches: {len(batches)}")
        for i, batch in enumerate(batches, 1):
            print(f"\nBatch {i}:")
            for feature in batch:
                deps = feature.get("dependencies", [])
                dep_str = f" (depends on: {', '.join(deps)})" if deps else ""
                print(f"  - {feature['id']}: {feature['title']}{dep_str}")

    elif command == "sample":
        plan = create_sample_mega_plan()
        print(json.dumps(plan, indent=2))

    elif command == "progress":
        from ..state.mega_state import MegaStateManager
        sm = MegaStateManager(project_root)
        plan = sm.read_mega_plan()
        if not plan:
            print("No mega-plan.json found")
            sys.exit(1)

        progress = mg.calculate_progress(plan)
        print(f"Progress: {progress['percentage']}%")
        print(f"  Total: {progress['total']}")
        print(f"  Completed: {progress['completed']}")
        print(f"  In Progress: {progress['in_progress']}")
        print(f"  Pending: {progress['pending']}")
        print(f"  Failed: {progress['failed']}")

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
