#!/usr/bin/env python3
"""
Design Document Tools for Plan Cascade MCP Server

Provides MCP tools for design document lifecycle management:
- design_generate: Generate design_doc.json from PRD or mega-plan
- design_import: Import external documents into design_doc.json schema
- design_review: Review design_doc.json for completeness and consistency
- design_get: Read design_doc.json contents with optional story filtering
"""

import json
import sys
from pathlib import Path
from typing import Any, Dict, Optional

# Add project source to path for imports
PLUGIN_ROOT = Path(__file__).parent.parent.parent
SRC_CORE = PLUGIN_ROOT / "src" / "plan_cascade" / "core"

# Add core directory to path so imports work
if str(SRC_CORE) not in sys.path:
    sys.path.insert(0, str(SRC_CORE))

from design_doc_generator import DesignDocGenerator
from design_doc_converter import DesignDocConverter


def register_design_tools(mcp: Any, project_root: Path) -> None:
    """
    Register all design document tools with the MCP server.

    Args:
        mcp: FastMCP server instance
        project_root: Root directory of the project
    """

    @mcp.tool()
    def design_generate(
        level: Optional[str] = None,
        prd_path: Optional[str] = None,
        description: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        Generate a design_doc.json from the current PRD or mega-plan.

        Auto-detects level (feature vs project) based on available files.
        Uses existing PRD stories and codebase context to produce the
        design document structure with metadata, overview, architecture,
        interfaces, decisions, and story_mappings sections.

        Args:
            level: Design level - 'auto' (default, detect from files), 'feature', or 'project'
            prd_path: Override path to PRD file (defaults to prd.json in project root)
            description: Optional description override for design context

        Returns:
            Generated design document with file path
        """
        try:
            generator = DesignDocGenerator(project_root)

            # Handle custom PRD path first (affects level detection)
            prd_data = None
            if prd_path:
                prd_file = Path(prd_path)
                if not prd_file.exists():
                    return {
                        "success": False,
                        "error": f"PRD file not found: {prd_path}"
                    }
                with open(prd_file, encoding="utf-8") as f:
                    prd_data = json.load(f)

            # Resolve the effective level
            effective_level = level if level and level != "auto" else None

            if effective_level is None:
                if prd_data is not None:
                    # Custom PRD provided, default to feature level
                    effective_level = "feature"
                else:
                    detected = generator.detect_level()
                    if detected == "unknown":
                        return {
                            "success": False,
                            "error": "Cannot determine design level. No prd.json or mega-plan.json found. "
                                     "Run prd_generate or mega_generate first."
                        }
                    effective_level = detected

            # Generate the design document
            if effective_level == "project":
                design_doc = generator.generate_project_design_doc()
            else:
                design_doc = generator.generate_feature_design_doc(prd=prd_data)

            # Apply description override to overview if provided
            if description:
                design_doc["overview"]["summary"] = description

            # Save to file
            generator.save_design_doc(design_doc)

            return {
                "success": True,
                "message": f"Design document ({effective_level}) generated successfully",
                "design_doc": design_doc,
                "file_path": str(project_root / "design_doc.json"),
                "level": effective_level
            }

        except Exception as e:
            return {
                "success": False,
                "error": f"Failed to generate design document: {str(e)}"
            }

    @mcp.tool()
    def design_import(
        source_path: str,
        level: Optional[str] = "feature"
    ) -> Dict[str, Any]:
        """
        Import an external design document and convert it to design_doc.json schema.

        Detects format from file extension (.md, .json, .html) and extracts
        structured sections into the unified design_doc.json format.

        Args:
            source_path: Path to external document (.md, .json, .html)
            level: Design level - 'feature' (default) or 'project'

        Returns:
            Converted design document with format information
        """
        try:
            converter = DesignDocConverter(project_root)
            input_path = Path(source_path)

            if not input_path.exists():
                return {
                    "success": False,
                    "error": f"Source file not found: {source_path}"
                }

            # Detect format
            source_format = converter._detect_format(input_path)

            # Convert the document
            design_doc = converter.convert(input_path)

            # Set the level in metadata
            effective_level = level or "feature"
            design_doc["metadata"]["level"] = effective_level

            # Ensure level-specific sections exist
            if effective_level == "project":
                if "feature_mappings" not in design_doc:
                    design_doc["feature_mappings"] = {}
                # Remove feature-specific sections if present
                design_doc.pop("story_mappings", None)
            else:
                if "story_mappings" not in design_doc:
                    design_doc["story_mappings"] = {}

            # Save to file
            converter.save_design_doc(design_doc)

            # Collect conversion warnings
            warnings = []
            if not design_doc["overview"]["title"]:
                warnings.append("No title could be extracted from the source document")
            if not design_doc["overview"]["summary"]:
                warnings.append("No summary/overview section found in the source document")
            if not design_doc["architecture"].get("components"):
                warnings.append("No components could be extracted from the source document")
            if not design_doc.get("decisions"):
                warnings.append("No architectural decisions found in the source document")

            return {
                "success": True,
                "message": f"Design document imported from {source_format} format",
                "design_doc": design_doc,
                "file_path": str(project_root / "design_doc.json"),
                "source_format": source_format,
                "warnings": warnings
            }

        except FileNotFoundError as e:
            return {
                "success": False,
                "error": str(e)
            }
        except Exception as e:
            return {
                "success": False,
                "error": f"Failed to import design document: {str(e)}"
            }

    @mcp.tool()
    def design_review() -> Dict[str, Any]:
        """
        Review the current design_doc.json for completeness and consistency.

        Checks for:
        - Schema validation (required fields, types)
        - Story mapping coverage (all PRD stories mapped)
        - Component completeness (components have files listed)
        - Decision completeness (decisions have rationale)

        Returns:
            Validation report with coverage metrics and warnings
        """
        try:
            generator = DesignDocGenerator(project_root)

            # Load design document
            design_doc = generator.load_design_doc()
            if not design_doc:
                return {
                    "success": False,
                    "error": "No design_doc.json found. Run design_generate or design_import first."
                }

            # Run schema validation
            is_valid, validation_errors = generator.validate_design_doc(design_doc)

            # Compute story mapping coverage
            story_mapping_coverage = _compute_story_mapping_coverage(
                design_doc, project_root
            )

            # Compute component completeness
            components = design_doc.get("architecture", {}).get("components", [])
            components_with_files = [
                c for c in components
                if c.get("files") and len(c["files"]) > 0
            ]
            components_missing_files = [
                c["name"] for c in components
                if not c.get("files") or len(c["files"]) == 0
            ]

            component_completeness = {
                "total": len(components),
                "with_files": len(components_with_files),
                "missing_files": components_missing_files
            }

            # Compute decision completeness
            decisions = design_doc.get("decisions", [])
            decisions_with_rationale = [
                d for d in decisions
                if d.get("rationale") and d["rationale"].strip()
            ]
            decisions_missing_rationale = [
                d.get("id", "unknown") for d in decisions
                if not d.get("rationale") or not d["rationale"].strip()
            ]

            decision_completeness = {
                "total": len(decisions),
                "with_rationale": len(decisions_with_rationale),
                "missing_rationale": decisions_missing_rationale
            }

            # Collect warnings
            warnings = []
            if story_mapping_coverage.get("unmapped"):
                warnings.append(
                    f"{len(story_mapping_coverage['unmapped'])} stories not mapped in design doc: "
                    f"{', '.join(story_mapping_coverage['unmapped'])}"
                )
            if components_missing_files:
                warnings.append(
                    f"{len(components_missing_files)} components missing file references: "
                    f"{', '.join(components_missing_files)}"
                )
            if decisions_missing_rationale:
                warnings.append(
                    f"{len(decisions_missing_rationale)} decisions missing rationale: "
                    f"{', '.join(decisions_missing_rationale)}"
                )

            return {
                "success": True,
                "is_valid": is_valid,
                "errors": validation_errors,
                "story_mapping_coverage": story_mapping_coverage,
                "component_completeness": component_completeness,
                "decision_completeness": decision_completeness,
                "warnings": warnings
            }

        except Exception as e:
            return {
                "success": False,
                "error": f"Failed to review design document: {str(e)}"
            }

    @mcp.tool()
    def design_get(
        story_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        Read and return the current design_doc.json contents.

        When called without story_id, returns the full design document.
        When called with a story_id, returns only the design context
        relevant to that story (components, decisions, interfaces from
        story_mappings).

        Args:
            story_id: Optional story ID to filter design context (e.g., 'story-001')

        Returns:
            Full or filtered design document content
        """
        try:
            generator = DesignDocGenerator(project_root)

            # Load design document
            design_doc = generator.load_design_doc()
            if not design_doc:
                return {
                    "success": False,
                    "error": "No design_doc.json found. Run design_generate or design_import first."
                }

            # Return full doc if no story_id
            if not story_id:
                return {
                    "success": True,
                    "design_doc": design_doc,
                    "filtered": False,
                    "story_id": None
                }

            # Filter by story_id
            story_mappings = design_doc.get("story_mappings", {})
            if story_id not in story_mappings:
                available = list(story_mappings.keys())
                return {
                    "success": False,
                    "error": f"Story {story_id} not found in design document story_mappings. "
                             f"Available stories: {available}"
                }

            mapping = story_mappings[story_id]

            # Extract relevant components
            all_components = design_doc.get("architecture", {}).get("components", [])
            mapped_component_names = mapping.get("components", [])
            relevant_components = [
                c for c in all_components
                if c.get("name") in mapped_component_names
            ]

            # Extract relevant decisions
            all_decisions = design_doc.get("decisions", [])
            mapped_decision_ids = mapping.get("decisions", [])
            relevant_decisions = [
                d for d in all_decisions
                if d.get("id") in mapped_decision_ids
            ]

            # Extract relevant interfaces/APIs
            all_apis = design_doc.get("interfaces", {}).get("apis", [])
            mapped_interface_refs = mapping.get("interfaces", [])
            relevant_apis = [
                a for a in all_apis
                if a.get("id") in mapped_interface_refs
            ]

            # Extract relevant data models
            all_models = design_doc.get("interfaces", {}).get("data_models", [])
            relevant_models = [
                m for m in all_models
                if m.get("name") in mapped_interface_refs
            ]

            # Build filtered context
            filtered_doc = {
                "metadata": design_doc.get("metadata", {}),
                "overview": design_doc.get("overview", {}),
                "story_mapping": mapping,
                "components": relevant_components,
                "decisions": relevant_decisions,
                "apis": relevant_apis,
                "data_models": relevant_models,
                "patterns": design_doc.get("architecture", {}).get("patterns", [])
            }

            return {
                "success": True,
                "design_doc": filtered_doc,
                "filtered": True,
                "story_id": story_id
            }

        except Exception as e:
            return {
                "success": False,
                "error": f"Failed to read design document: {str(e)}"
            }


def _compute_story_mapping_coverage(
    design_doc: Dict[str, Any],
    project_root: Path
) -> Dict[str, Any]:
    """
    Compute story mapping coverage by comparing design doc mappings to PRD stories.

    Args:
        design_doc: The design document dictionary
        project_root: Project root path for finding prd.json

    Returns:
        Coverage report with total_stories, mapped_stories, and unmapped list
    """
    story_mappings = design_doc.get("story_mappings", {})

    # Try to load PRD for story list
    prd_path = project_root / "prd.json"
    prd_story_ids = set()

    if prd_path.exists():
        try:
            with open(prd_path, encoding="utf-8") as f:
                prd = json.load(f)
            prd_story_ids = {s["id"] for s in prd.get("stories", [])}
        except (json.JSONDecodeError, OSError):
            pass

    # If no PRD, use the story_mappings keys as the total
    if not prd_story_ids:
        prd_story_ids = set(story_mappings.keys())

    mapped_ids = set(story_mappings.keys())
    unmapped = sorted(prd_story_ids - mapped_ids)

    return {
        "total_stories": len(prd_story_ids),
        "mapped_stories": len(mapped_ids & prd_story_ids),
        "unmapped": unmapped
    }
