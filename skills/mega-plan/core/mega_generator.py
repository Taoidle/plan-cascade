#!/usr/bin/env python3
"""
Mega Plan Generator

Generates project-level mega-plan from user descriptions.
Breaks down complex projects into features that can be executed as hybrid:worktree tasks.
"""

import json
import re
from pathlib import Path
from typing import Dict, List, Optional, Tuple
from datetime import datetime


class MegaPlanGenerator:
    """Generates mega-plan from project descriptions."""

    def __init__(self, project_root: Path):
        """
        Initialize the mega-plan generator.

        Args:
            project_root: Root directory of the project
        """
        self.project_root = Path(project_root)
        self.mega_plan_path = self.project_root / "mega-plan.json"
        self.feature_counter = 0

    def generate_mega_plan(
        self,
        description: str,
        execution_mode: str = "auto",
        target_branch: str = "main",
        context: Optional[Dict] = None
    ) -> Dict:
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

    def add_feature(
        self,
        plan: Dict,
        name: str,
        title: str,
        description: str,
        priority: str = "medium",
        dependencies: Optional[List[str]] = None
    ) -> Dict:
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

    def generate_feature_batches(self, plan: Dict) -> List[List[Dict]]:
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

    def validate_mega_plan(self, plan: Dict) -> Tuple[bool, List[str]]:
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

    def _detect_dependency_cycle(self, plan: Dict) -> Optional[List[str]]:
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

    def get_feature_by_id(self, plan: Dict, feature_id: str) -> Optional[Dict]:
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

    def get_feature_by_name(self, plan: Dict, name: str) -> Optional[Dict]:
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

    def get_features_by_status(self, plan: Dict, status: str) -> List[Dict]:
        """
        Get all features with a specific status.

        Args:
            plan: Mega-plan dictionary
            status: Status to filter by

        Returns:
            List of features with the specified status
        """
        return [f for f in plan.get("features", []) if f.get("status") == status]

    def calculate_progress(self, plan: Dict) -> Dict:
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


def create_sample_mega_plan() -> Dict:
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
    import sys

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
        from mega_state import MegaStateManager
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
        from mega_state import MegaStateManager
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
        from mega_state import MegaStateManager
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
