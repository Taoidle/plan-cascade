#!/usr/bin/env python3
"""
Design Document Generator for Plan Cascade

Generates structured technical design documents at two levels:
- Project level (from mega-plan.json): Global architecture, cross-feature patterns
- Feature level (from prd.json): Feature-specific components, APIs, decisions

These documents provide architectural context to story execution agents.
"""

import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Literal


class DesignDocGenerator:
    """Generates design documents from mega-plan or PRD content."""

    def __init__(self, project_root: Path):
        """
        Initialize the design document generator.

        Args:
            project_root: Root directory of the project (or worktree)
        """
        self.project_root = Path(project_root)
        self.prd_path = self.project_root / "prd.json"
        self.mega_plan_path = self.project_root / "mega-plan.json"
        self.design_doc_path = self.project_root / "design_doc.json"

    def detect_level(self) -> Literal["project", "feature", "unknown"]:
        """
        Auto-detect the appropriate design document level.

        Returns:
            "project" if mega-plan.json exists (project root)
            "feature" if prd.json exists (worktree or standalone feature)
            "unknown" if neither exists
        """
        if self.mega_plan_path.exists():
            return "project"
        elif self.prd_path.exists():
            return "feature"
        return "unknown"

    def load_mega_plan(self) -> dict | None:
        """Load the mega-plan.json file."""
        if not self.mega_plan_path.exists():
            return None

        try:
            with open(self.mega_plan_path, encoding="utf-8") as f:
                return json.load(f)
        except (OSError, json.JSONDecodeError) as e:
            print(f"Warning: Could not load mega-plan: {e}")
            return None

    def load_prd(self) -> dict | None:
        """Load the prd.json file."""
        if not self.prd_path.exists():
            return None

        try:
            with open(self.prd_path, encoding="utf-8") as f:
                return json.load(f)
        except (OSError, json.JSONDecodeError) as e:
            print(f"Warning: Could not load PRD: {e}")
            return None

    def load_parent_design_doc(self) -> dict | None:
        """
        Load parent design document for feature-level inheritance.

        Looks for design_doc.json in parent directory (project root).
        """
        parent_path = self.project_root.parent / "design_doc.json"
        if not parent_path.exists():
            # Try going up from .worktree/feature-name/
            grandparent_path = self.project_root.parent.parent / "design_doc.json"
            if grandparent_path.exists():
                parent_path = grandparent_path
            else:
                return None

        try:
            with open(parent_path, encoding="utf-8") as f:
                return json.load(f)
        except (OSError, json.JSONDecodeError):
            return None

    def generate_project_design_doc(
        self,
        mega_plan: dict | None = None,
        source: str = "ai-generated"
    ) -> dict:
        """
        Generate a project-level design document from mega-plan.

        Args:
            mega_plan: Mega-plan dictionary (loads from file if not provided)
            source: Source type (ai-generated, user-provided, converted)

        Returns:
            Project-level design document dictionary
        """
        if mega_plan is None:
            mega_plan = self.load_mega_plan()

        if mega_plan is None:
            return self._create_empty_project_design_doc(source)

        features = mega_plan.get("features", [])

        design_doc = {
            "metadata": {
                "created_at": datetime.now(timezone.utc).isoformat(),
                "version": "1.0.0",
                "source": source,
                "level": "project",
                "mega_plan_reference": "mega-plan.json"
            },
            "overview": {
                "title": mega_plan.get("goal", "Untitled Project"),
                "summary": mega_plan.get("goal", ""),
                "goals": [],
                "non_goals": []
            },
            "architecture": {
                "system_overview": "",
                "components": [],
                "data_flow": "",
                "patterns": [],
                "infrastructure": {}
            },
            "interfaces": {
                "api_standards": {},
                "shared_data_models": []
            },
            "decisions": [],
            "feature_mappings": self._generate_feature_mappings(features)
        }

        return design_doc

    def generate_feature_design_doc(
        self,
        prd: dict | None = None,
        feature_id: str | None = None,
        source: str = "ai-generated"
    ) -> dict:
        """
        Generate a feature-level design document from PRD.

        Args:
            prd: PRD dictionary (loads from file if not provided)
            feature_id: Feature ID (for mega-plan context)
            source: Source type

        Returns:
            Feature-level design document dictionary
        """
        if prd is None:
            prd = self.load_prd()

        if prd is None:
            return self._create_empty_feature_design_doc(source, feature_id)

        # Try to load parent design doc for inheritance
        parent_doc = self.load_parent_design_doc()
        inherited_context = self._extract_inherited_context(parent_doc, feature_id)

        design_doc = {
            "metadata": {
                "created_at": datetime.now(timezone.utc).isoformat(),
                "version": "1.0.0",
                "source": source,
                "level": "feature",
                "prd_reference": "prd.json",
                "parent_design_doc": "../design_doc.json" if parent_doc else None,
                "feature_id": feature_id
            },
            "overview": self._extract_overview(prd),
            "inherited_context": inherited_context,
            "architecture": {
                "components": [],
                "data_flow": "",
                "patterns": []
            },
            "interfaces": {
                "apis": [],
                "data_models": []
            },
            "decisions": [],
            "story_mappings": self._generate_story_mappings(prd)
        }

        return design_doc

    def _create_empty_project_design_doc(self, source: str) -> dict:
        """Create an empty project-level design document structure."""
        return {
            "metadata": {
                "created_at": datetime.now(timezone.utc).isoformat(),
                "version": "1.0.0",
                "source": source,
                "level": "project",
                "mega_plan_reference": None
            },
            "overview": {
                "title": "",
                "summary": "",
                "goals": [],
                "non_goals": []
            },
            "architecture": {
                "system_overview": "",
                "components": [],
                "data_flow": "",
                "patterns": [],
                "infrastructure": {}
            },
            "interfaces": {
                "api_standards": {},
                "shared_data_models": []
            },
            "decisions": [],
            "feature_mappings": {}
        }

    def _create_empty_feature_design_doc(
        self,
        source: str,
        feature_id: str | None
    ) -> dict:
        """Create an empty feature-level design document structure."""
        return {
            "metadata": {
                "created_at": datetime.now(timezone.utc).isoformat(),
                "version": "1.0.0",
                "source": source,
                "level": "feature",
                "prd_reference": None,
                "parent_design_doc": None,
                "feature_id": feature_id
            },
            "overview": {
                "title": "",
                "summary": "",
                "goals": [],
                "non_goals": []
            },
            "inherited_context": {},
            "architecture": {
                "components": [],
                "data_flow": "",
                "patterns": []
            },
            "interfaces": {
                "apis": [],
                "data_models": []
            },
            "decisions": [],
            "story_mappings": {}
        }

    def _extract_overview(self, prd: dict) -> dict:
        """Extract overview section from PRD."""
        metadata = prd.get("metadata", {})
        return {
            "title": metadata.get("description", "Untitled Feature"),
            "summary": prd.get("goal", ""),
            "goals": prd.get("objectives", []),
            "non_goals": []
        }

    def _extract_inherited_context(
        self,
        parent_doc: dict | None,
        feature_id: str | None
    ) -> dict:
        """Extract context to inherit from parent design document."""
        if not parent_doc:
            return {}

        inherited = {
            "description": "Context inherited from project-level design document",
            "patterns": [],
            "decisions": [],
            "shared_models": []
        }

        # Get feature mapping from parent if available
        feature_mappings = parent_doc.get("feature_mappings", {})
        if feature_id and feature_id in feature_mappings:
            mapping = feature_mappings[feature_id]
            inherited["patterns"] = mapping.get("patterns", [])
            inherited["decisions"] = mapping.get("decisions", [])

        # Add shared data models
        shared_models = parent_doc.get("interfaces", {}).get("shared_data_models", [])
        inherited["shared_models"] = [m.get("name", "") for m in shared_models]

        return inherited

    def _generate_feature_mappings(self, features: list) -> dict:
        """Generate feature mappings from mega-plan features."""
        mappings = {}
        for feature in features:
            feature_id = feature.get("id", "")
            if feature_id:
                mappings[feature_id] = {
                    "components": [],
                    "patterns": [],
                    "decisions": [],
                    "description": feature.get("description", "")
                }
        return mappings

    def _generate_story_mappings(self, prd: dict) -> dict:
        """Generate story mappings from PRD stories."""
        mappings = {}
        for story in prd.get("stories", []):
            story_id = story.get("id", "")
            if story_id:
                mappings[story_id] = {
                    "components": [],
                    "decisions": [],
                    "interfaces": []
                }
        return mappings

    # Component/Pattern/Decision manipulation methods

    def add_component(
        self,
        design_doc: dict,
        name: str,
        description: str,
        responsibilities: list[str],
        dependencies: list[str] | None = None,
        files: list[str] | None = None,
        features: list[str] | None = None
    ) -> dict:
        """Add a component to the design document."""
        component = {
            "name": name,
            "description": description,
            "responsibilities": responsibilities,
            "dependencies": dependencies or [],
            "files": files or []
        }
        if features:  # Project-level component
            component["features"] = features

        design_doc["architecture"]["components"].append(component)
        return design_doc

    def add_pattern(
        self,
        design_doc: dict,
        name: str,
        description: str,
        rationale: str,
        applies_to: list[str] | None = None
    ) -> dict:
        """Add an architectural pattern to the design document."""
        pattern = {
            "name": name,
            "description": description,
            "rationale": rationale
        }
        if applies_to:  # Project-level pattern
            pattern["applies_to"] = applies_to

        design_doc["architecture"]["patterns"].append(pattern)
        return design_doc

    def add_decision(
        self,
        design_doc: dict,
        title: str,
        context: str,
        decision: str,
        rationale: str,
        alternatives: list[str] | None = None,
        status: str = "accepted",
        applies_to: list[str] | None = None
    ) -> dict:
        """Add an Architecture Decision Record (ADR)."""
        level = design_doc.get("metadata", {}).get("level", "feature")
        existing_count = len(design_doc.get("decisions", []))

        # Use different ID prefixes for project vs feature level
        if level == "project":
            adr_id = f"ADR-{existing_count + 1:03d}"
        else:
            adr_id = f"ADR-F{existing_count + 1:03d}"

        adr = {
            "id": adr_id,
            "title": title,
            "context": context,
            "decision": decision,
            "rationale": rationale,
            "alternatives_considered": alternatives or [],
            "status": status
        }
        if applies_to:  # Project-level decision
            adr["applies_to"] = applies_to

        design_doc["decisions"].append(adr)
        return design_doc

    def add_api(
        self,
        design_doc: dict,
        method: str,
        path: str,
        description: str,
        request_body: dict | None = None,
        response: dict | None = None
    ) -> dict:
        """Add an API endpoint to the design document."""
        existing_count = len(design_doc.get("interfaces", {}).get("apis", []))
        api_id = f"API-{existing_count + 1:03d}"

        api = {
            "id": api_id,
            "method": method,
            "path": path,
            "description": description,
            "request_body": request_body or {},
            "response": response or {}
        }
        design_doc["interfaces"]["apis"].append(api)
        return design_doc

    def add_data_model(
        self,
        design_doc: dict,
        name: str,
        description: str,
        fields: dict,
        used_by: list[str] | None = None
    ) -> dict:
        """Add a data model to the design document."""
        model = {
            "name": name,
            "description": description,
            "fields": fields
        }
        if used_by:  # Project-level shared model
            model["used_by"] = used_by
            if "shared_data_models" not in design_doc["interfaces"]:
                design_doc["interfaces"]["shared_data_models"] = []
            design_doc["interfaces"]["shared_data_models"].append(model)
        else:
            if "data_models" not in design_doc["interfaces"]:
                design_doc["interfaces"]["data_models"] = []
            design_doc["interfaces"]["data_models"].append(model)

        return design_doc

    def map_story_to_component(
        self,
        design_doc: dict,
        story_id: str,
        component_name: str
    ) -> dict:
        """Map a story to a component."""
        if story_id not in design_doc["story_mappings"]:
            design_doc["story_mappings"][story_id] = {
                "components": [],
                "decisions": [],
                "interfaces": []
            }

        if component_name not in design_doc["story_mappings"][story_id]["components"]:
            design_doc["story_mappings"][story_id]["components"].append(component_name)

        return design_doc

    def map_feature_to_component(
        self,
        design_doc: dict,
        feature_id: str,
        component_name: str
    ) -> dict:
        """Map a feature to a component (project-level)."""
        if "feature_mappings" not in design_doc:
            design_doc["feature_mappings"] = {}

        if feature_id not in design_doc["feature_mappings"]:
            design_doc["feature_mappings"][feature_id] = {
                "components": [],
                "patterns": [],
                "decisions": [],
                "description": ""
            }

        if component_name not in design_doc["feature_mappings"][feature_id]["components"]:
            design_doc["feature_mappings"][feature_id]["components"].append(component_name)

        return design_doc

    def save_design_doc(self, design_doc: dict) -> bool:
        """Save design document to file."""
        try:
            with open(self.design_doc_path, "w", encoding="utf-8") as f:
                json.dump(design_doc, f, indent=2)
            return True
        except OSError as e:
            print(f"Error saving design document: {e}")
            return False

    def load_design_doc(self) -> dict | None:
        """Load existing design document from file."""
        if not self.design_doc_path.exists():
            return None

        try:
            with open(self.design_doc_path, encoding="utf-8") as f:
                return json.load(f)
        except (OSError, json.JSONDecodeError) as e:
            print(f"Warning: Could not load design document: {e}")
            return None

    def validate_design_doc(self, design_doc: dict) -> tuple[bool, list[str]]:
        """
        Validate a design document against the expected schema.

        Performs comprehensive validation including:
        - Required fields at each level
        - Type checking for all fields
        - Structural validation (arrays, nested objects)
        - Level-specific requirements (project vs feature)
        - Optional field graceful degradation

        Args:
            design_doc: The design document dictionary to validate

        Returns:
            Tuple of (is_valid, list of error messages with field paths)
        """
        errors = []

        # Schema definitions for validation
        METADATA_SCHEMA = {
            "required": ["level", "source"],
            "optional": ["created_at", "version", "prd_reference", "mega_plan_reference",
                        "parent_design_doc", "feature_id"],
            "types": {
                "created_at": str,
                "version": str,
                "source": str,
                "level": str,
                "prd_reference": (str, type(None)),
                "mega_plan_reference": (str, type(None)),
                "parent_design_doc": (str, type(None)),
                "feature_id": (str, type(None)),
            }
        }

        OVERVIEW_SCHEMA = {
            "required": ["title"],
            "optional": ["summary", "goals", "non_goals"],
            "types": {
                "title": str,
                "summary": str,
                "goals": list,
                "non_goals": list,
            }
        }

        COMPONENT_SCHEMA = {
            "required": ["name"],
            "optional": ["description", "responsibilities", "dependencies", "files", "features"],
            "types": {
                "name": str,
                "description": str,
                "responsibilities": list,
                "dependencies": list,
                "files": list,
                "features": list,
            }
        }

        PATTERN_SCHEMA = {
            "required": ["name"],
            "optional": ["description", "rationale", "applies_to"],
            "types": {
                "name": str,
                "description": str,
                "rationale": str,
                "applies_to": list,
            }
        }

        DECISION_SCHEMA = {
            "required": ["id", "title"],
            "optional": ["context", "decision", "rationale", "alternatives_considered",
                        "status", "applies_to"],
            "types": {
                "id": str,
                "title": str,
                "context": str,
                "decision": str,
                "rationale": str,
                "alternatives_considered": list,
                "status": str,
                "applies_to": list,
            }
        }

        API_SCHEMA = {
            "required": ["method", "path"],
            "optional": ["id", "description", "request_body", "response"],
            "types": {
                "id": str,
                "method": str,
                "path": str,
                "description": str,
                "request_body": dict,
                "response": dict,
            }
        }

        DATA_MODEL_SCHEMA = {
            "required": ["name"],
            "optional": ["description", "fields", "used_by"],
            "types": {
                "name": str,
                "description": str,
                "fields": dict,
                "used_by": list,
            }
        }

        def validate_field_type(value, expected_type, path: str) -> list[str]:
            """Validate a field value against expected type(s)."""
            if isinstance(expected_type, tuple):
                if not isinstance(value, expected_type):
                    type_names = " or ".join(t.__name__ if t != type(None) else "null"
                                            for t in expected_type)
                    return [f"{path}: expected {type_names}, got {type(value).__name__}"]
            else:
                if not isinstance(value, expected_type):
                    return [f"{path}: expected {expected_type.__name__}, got {type(value).__name__}"]
            return []

        def validate_object(obj: dict, schema: dict, path: str) -> list[str]:
            """Validate an object against a schema definition."""
            errs = []

            # Check required fields
            for field in schema.get("required", []):
                if field not in obj:
                    errs.append(f"{path}.{field}: required field is missing")
                elif obj[field] is None or obj[field] == "":
                    errs.append(f"{path}.{field}: required field is empty")

            # Type check all present fields
            type_map = schema.get("types", {})
            for field, value in obj.items():
                if field in type_map and value is not None:
                    field_path = f"{path}.{field}"
                    errs.extend(validate_field_type(value, type_map[field], field_path))

            return errs

        def validate_array_of_objects(arr: list, item_schema: dict, path: str) -> list[str]:
            """Validate an array of objects."""
            errs = []
            if not isinstance(arr, list):
                errs.append(f"{path}: expected array, got {type(arr).__name__}")
                return errs

            for i, item in enumerate(arr):
                item_path = f"{path}[{i}]"
                if not isinstance(item, dict):
                    errs.append(f"{item_path}: expected object, got {type(item).__name__}")
                else:
                    errs.extend(validate_object(item, item_schema, item_path))

            return errs

        # ========== Top-level validation ==========

        # Validate metadata section
        if "metadata" not in design_doc:
            errors.append("metadata: required section is missing")
        elif not isinstance(design_doc["metadata"], dict):
            errors.append(f"metadata: expected object, got {type(design_doc['metadata']).__name__}")
        else:
            errors.extend(validate_object(design_doc["metadata"], METADATA_SCHEMA, "metadata"))

            # Validate level value
            level = design_doc["metadata"].get("level", "")
            if level and level not in ("project", "feature"):
                errors.append(f"metadata.level: must be 'project' or 'feature', got '{level}'")

        # Validate overview section
        if "overview" not in design_doc:
            errors.append("overview: required section is missing")
        elif not isinstance(design_doc["overview"], dict):
            errors.append(f"overview: expected object, got {type(design_doc['overview']).__name__}")
        else:
            errors.extend(validate_object(design_doc["overview"], OVERVIEW_SCHEMA, "overview"))

            # Validate goals/non_goals arrays contain strings
            for list_field in ["goals", "non_goals"]:
                if list_field in design_doc["overview"]:
                    arr = design_doc["overview"][list_field]
                    if isinstance(arr, list):
                        for i, item in enumerate(arr):
                            if not isinstance(item, str):
                                errors.append(f"overview.{list_field}[{i}]: expected string, got {type(item).__name__}")

        # Validate architecture section
        if "architecture" not in design_doc:
            errors.append("architecture: required section is missing")
        elif not isinstance(design_doc["architecture"], dict):
            errors.append(f"architecture: expected object, got {type(design_doc['architecture']).__name__}")
        else:
            arch = design_doc["architecture"]

            # Validate components array
            if "components" in arch:
                errors.extend(validate_array_of_objects(
                    arch["components"], COMPONENT_SCHEMA, "architecture.components"
                ))

            # Validate patterns array
            if "patterns" in arch:
                errors.extend(validate_array_of_objects(
                    arch["patterns"], PATTERN_SCHEMA, "architecture.patterns"
                ))

            # Validate data_flow is string if present
            if "data_flow" in arch and arch["data_flow"] is not None:
                if not isinstance(arch["data_flow"], str):
                    errors.append(f"architecture.data_flow: expected string, got {type(arch['data_flow']).__name__}")

        # Validate interfaces section (optional but should be object if present)
        if "interfaces" in design_doc:
            if not isinstance(design_doc["interfaces"], dict):
                errors.append(f"interfaces: expected object, got {type(design_doc['interfaces']).__name__}")
            else:
                interfaces = design_doc["interfaces"]

                # Validate apis array
                if "apis" in interfaces:
                    errors.extend(validate_array_of_objects(
                        interfaces["apis"], API_SCHEMA, "interfaces.apis"
                    ))

                # Validate data_models array
                if "data_models" in interfaces:
                    errors.extend(validate_array_of_objects(
                        interfaces["data_models"], DATA_MODEL_SCHEMA, "interfaces.data_models"
                    ))

                # Validate shared_data_models array (project-level)
                if "shared_data_models" in interfaces:
                    errors.extend(validate_array_of_objects(
                        interfaces["shared_data_models"], DATA_MODEL_SCHEMA, "interfaces.shared_data_models"
                    ))

        # Validate decisions section
        if "decisions" not in design_doc:
            errors.append("decisions: required section is missing")
        elif not isinstance(design_doc["decisions"], list):
            errors.append(f"decisions: expected array, got {type(design_doc['decisions']).__name__}")
        else:
            errors.extend(validate_array_of_objects(
                design_doc["decisions"], DECISION_SCHEMA, "decisions"
            ))

            # Validate ADR IDs are unique
            adr_ids = set()
            for i, adr in enumerate(design_doc["decisions"]):
                if isinstance(adr, dict):
                    adr_id = adr.get("id", "")
                    if adr_id and adr_id in adr_ids:
                        errors.append(f"decisions[{i}].id: duplicate ADR ID '{adr_id}'")
                    adr_ids.add(adr_id)

                    # Validate status values
                    status = adr.get("status")
                    if status and status not in ("accepted", "proposed", "deprecated", "superseded"):
                        errors.append(f"decisions[{i}].status: invalid value '{status}', "
                                     "expected one of: accepted, proposed, deprecated, superseded")

        # Level-specific validation
        level = design_doc.get("metadata", {}).get("level", "")

        if level == "project":
            if "feature_mappings" not in design_doc:
                errors.append("feature_mappings: required for project-level document")
            elif not isinstance(design_doc["feature_mappings"], dict):
                errors.append(f"feature_mappings: expected object, got {type(design_doc['feature_mappings']).__name__}")
            else:
                # Validate each feature mapping
                for feature_id, mapping in design_doc["feature_mappings"].items():
                    if not isinstance(mapping, dict):
                        errors.append(f"feature_mappings.{feature_id}: expected object, got {type(mapping).__name__}")
                    else:
                        for arr_field in ["components", "patterns", "decisions"]:
                            if arr_field in mapping and not isinstance(mapping[arr_field], list):
                                errors.append(f"feature_mappings.{feature_id}.{arr_field}: "
                                            f"expected array, got {type(mapping[arr_field]).__name__}")

        elif level == "feature":
            if "story_mappings" not in design_doc:
                errors.append("story_mappings: required for feature-level document")
            elif not isinstance(design_doc["story_mappings"], dict):
                errors.append(f"story_mappings: expected object, got {type(design_doc['story_mappings']).__name__}")
            else:
                # Validate each story mapping
                for story_id, mapping in design_doc["story_mappings"].items():
                    if not isinstance(mapping, dict):
                        errors.append(f"story_mappings.{story_id}: expected object, got {type(mapping).__name__}")
                    else:
                        for arr_field in ["components", "decisions", "interfaces"]:
                            if arr_field in mapping and not isinstance(mapping[arr_field], list):
                                errors.append(f"story_mappings.{story_id}.{arr_field}: "
                                            f"expected array, got {type(mapping[arr_field]).__name__}")

        return (len(errors) == 0, errors)


def main():
    """CLI interface for testing design document generator."""
    if len(sys.argv) < 2:
        print("Usage: design_doc_generator.py <command> [args]")
        print("Commands:")
        print("  detect               - Detect appropriate level")
        print("  generate [level]     - Generate design doc (project/feature/auto)")
        print("  validate             - Validate existing design doc")
        print("  show                 - Show current design doc")
        sys.exit(1)

    command = sys.argv[1]
    project_root = Path.cwd()

    dg = DesignDocGenerator(project_root)

    if command == "detect":
        level = dg.detect_level()
        print(f"Detected level: {level}")

    elif command == "generate":
        level = sys.argv[2] if len(sys.argv) > 2 else "auto"

        if level == "auto":
            level = dg.detect_level()
            print(f"Auto-detected level: {level}")

        if level == "project":
            design_doc = dg.generate_project_design_doc()
        elif level == "feature":
            design_doc = dg.generate_feature_design_doc()
        else:
            print("Cannot determine level. Specify 'project' or 'feature'.")
            sys.exit(1)

        if dg.save_design_doc(design_doc):
            print(f"Design document ({level}) generated and saved")
            print(json.dumps(design_doc, indent=2))
        else:
            print("Failed to save design document")
            sys.exit(1)

    elif command == "validate":
        design_doc = dg.load_design_doc()
        if not design_doc:
            print("No design document found")
            sys.exit(1)

        is_valid, errors = dg.validate_design_doc(design_doc)
        if is_valid:
            print("Design document is valid!")
        else:
            print("Design document validation errors:")
            for error in errors:
                print(f"  - {error}")

    elif command == "show":
        design_doc = dg.load_design_doc()
        if not design_doc:
            print("No design document found")
            sys.exit(1)

        print(json.dumps(design_doc, indent=2))

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
