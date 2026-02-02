#!/usr/bin/env python3
"""
Tests for design document schema validation.

Tests the enhanced validate_design_doc method in DesignDocGenerator,
which validates design_doc.json against the expected schema with
field-path error reporting.
"""

import pytest
from pathlib import Path
import tempfile
import json

from src.plan_cascade.core.design_doc_generator import DesignDocGenerator


@pytest.fixture
def temp_project_dir():
    """Create a temporary project directory."""
    with tempfile.TemporaryDirectory() as tmpdir:
        yield Path(tmpdir)


@pytest.fixture
def generator(temp_project_dir):
    """Create a DesignDocGenerator instance."""
    return DesignDocGenerator(temp_project_dir)


class TestDesignDocValidation:
    """Test cases for design document schema validation."""

    def test_valid_feature_level_doc(self, generator):
        """Test that a valid feature-level document passes validation."""
        doc = {
            "metadata": {
                "created_at": "2024-01-01T00:00:00Z",
                "version": "1.0.0",
                "source": "ai-generated",
                "level": "feature",
                "prd_reference": "prd.json"
            },
            "overview": {
                "title": "Test Feature",
                "summary": "A test feature",
                "goals": ["Goal 1", "Goal 2"],
                "non_goals": []
            },
            "architecture": {
                "components": [
                    {
                        "name": "Component1",
                        "description": "A component",
                        "responsibilities": ["resp1"],
                        "dependencies": [],
                        "files": ["file1.py"]
                    }
                ],
                "data_flow": "Component flow",
                "patterns": [
                    {"name": "Pattern1", "description": "A pattern", "rationale": "Because"}
                ]
            },
            "interfaces": {
                "apis": [
                    {"method": "GET", "path": "/api/test", "description": "Test endpoint"}
                ],
                "data_models": [
                    {"name": "Model1", "description": "A model", "fields": {"id": "string"}}
                ]
            },
            "decisions": [
                {
                    "id": "ADR-F001",
                    "title": "Decision 1",
                    "context": "Some context",
                    "decision": "Some decision",
                    "rationale": "Some rationale",
                    "alternatives_considered": ["Alt 1"],
                    "status": "accepted"
                }
            ],
            "story_mappings": {
                "story-001": {
                    "components": ["Component1"],
                    "decisions": ["ADR-F001"],
                    "interfaces": ["API-001"]
                }
            }
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is True, f"Expected valid, got errors: {errors}"
        assert len(errors) == 0

    def test_valid_project_level_doc(self, generator):
        """Test that a valid project-level document passes validation."""
        doc = {
            "metadata": {
                "created_at": "2024-01-01T00:00:00Z",
                "version": "1.0.0",
                "source": "ai-generated",
                "level": "project",
                "mega_plan_reference": "mega-plan.json"
            },
            "overview": {
                "title": "Test Project",
                "summary": "A test project",
                "goals": ["Goal 1"],
                "non_goals": ["Non-goal 1"]
            },
            "architecture": {
                "components": [],
                "data_flow": "",
                "patterns": []
            },
            "interfaces": {
                "apis": [],
                "shared_data_models": []
            },
            "decisions": [],
            "feature_mappings": {
                "feature-1": {
                    "components": [],
                    "patterns": [],
                    "decisions": [],
                    "description": "Feature 1 description"
                }
            }
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is True, f"Expected valid, got errors: {errors}"
        assert len(errors) == 0

    def test_missing_metadata_section(self, generator):
        """Test validation fails when metadata section is missing."""
        doc = {
            "overview": {"title": "Test"},
            "architecture": {"components": []},
            "decisions": [],
            "story_mappings": {}
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False
        assert any("metadata" in e and "missing" in e.lower() for e in errors)

    def test_missing_required_metadata_fields(self, generator):
        """Test validation fails for missing required metadata fields."""
        doc = {
            "metadata": {
                "created_at": "2024-01-01T00:00:00Z"
                # Missing 'level' and 'source'
            },
            "overview": {"title": "Test"},
            "architecture": {"components": []},
            "decisions": [],
            "story_mappings": {}
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False
        assert any("metadata.level" in e for e in errors)
        assert any("metadata.source" in e for e in errors)

    def test_invalid_level_value(self, generator):
        """Test validation fails for invalid level value."""
        doc = {
            "metadata": {
                "level": "invalid_level",
                "source": "test"
            },
            "overview": {"title": "Test"},
            "architecture": {"components": []},
            "decisions": [],
            "story_mappings": {}
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False
        assert any("level" in e and "project" in e and "feature" in e for e in errors)

    def test_missing_overview_title(self, generator):
        """Test validation fails when overview title is missing or empty."""
        doc = {
            "metadata": {"level": "feature", "source": "test"},
            "overview": {"summary": "No title here"},
            "architecture": {"components": []},
            "decisions": [],
            "story_mappings": {}
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False
        assert any("overview.title" in e for e in errors)

    def test_invalid_component_type(self, generator):
        """Test validation fails when component has wrong type."""
        doc = {
            "metadata": {"level": "feature", "source": "test"},
            "overview": {"title": "Test"},
            "architecture": {
                "components": [
                    {"name": 123}  # name should be string
                ]
            },
            "decisions": [],
            "story_mappings": {}
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False
        assert any("architecture.components[0].name" in e and "str" in e for e in errors)

    def test_component_missing_required_name(self, generator):
        """Test validation fails when component is missing name."""
        doc = {
            "metadata": {"level": "feature", "source": "test"},
            "overview": {"title": "Test"},
            "architecture": {
                "components": [
                    {"description": "No name field"}
                ]
            },
            "decisions": [],
            "story_mappings": {}
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False
        assert any("components[0].name" in e and "required" in e.lower() for e in errors)

    def test_duplicate_adr_ids(self, generator):
        """Test validation fails for duplicate ADR IDs."""
        doc = {
            "metadata": {"level": "feature", "source": "test"},
            "overview": {"title": "Test"},
            "architecture": {"components": []},
            "decisions": [
                {"id": "ADR-001", "title": "First"},
                {"id": "ADR-001", "title": "Duplicate"}
            ],
            "story_mappings": {}
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False
        assert any("duplicate" in e.lower() and "ADR-001" in e for e in errors)

    def test_invalid_decision_status(self, generator):
        """Test validation fails for invalid ADR status."""
        doc = {
            "metadata": {"level": "feature", "source": "test"},
            "overview": {"title": "Test"},
            "architecture": {"components": []},
            "decisions": [
                {"id": "ADR-001", "title": "Test", "status": "invalid_status"}
            ],
            "story_mappings": {}
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False
        assert any("status" in e and "invalid_status" in e for e in errors)

    def test_valid_decision_status_values(self, generator):
        """Test that all valid status values pass validation."""
        valid_statuses = ["accepted", "proposed", "deprecated", "superseded"]

        for status in valid_statuses:
            doc = {
                "metadata": {"level": "feature", "source": "test"},
                "overview": {"title": "Test"},
                "architecture": {"components": []},
                "decisions": [
                    {"id": "ADR-001", "title": "Test", "status": status}
                ],
                "story_mappings": {}
            }

            is_valid, errors = generator.validate_design_doc(doc)
            assert is_valid is True, f"Status '{status}' should be valid, got errors: {errors}"

    def test_feature_level_missing_story_mappings(self, generator):
        """Test feature-level doc requires story_mappings."""
        doc = {
            "metadata": {"level": "feature", "source": "test"},
            "overview": {"title": "Test"},
            "architecture": {"components": []},
            "decisions": []
            # Missing story_mappings
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False
        assert any("story_mappings" in e for e in errors)

    def test_project_level_missing_feature_mappings(self, generator):
        """Test project-level doc requires feature_mappings."""
        doc = {
            "metadata": {"level": "project", "source": "test"},
            "overview": {"title": "Test"},
            "architecture": {"components": []},
            "decisions": []
            # Missing feature_mappings
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False
        assert any("feature_mappings" in e for e in errors)

    def test_invalid_api_missing_method(self, generator):
        """Test API validation requires method."""
        doc = {
            "metadata": {"level": "feature", "source": "test"},
            "overview": {"title": "Test"},
            "architecture": {"components": []},
            "interfaces": {
                "apis": [
                    {"path": "/api/test"}  # Missing method
                ]
            },
            "decisions": [],
            "story_mappings": {}
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False
        assert any("apis[0].method" in e and "required" in e.lower() for e in errors)

    def test_goals_must_be_strings(self, generator):
        """Test that goals array items must be strings."""
        doc = {
            "metadata": {"level": "feature", "source": "test"},
            "overview": {
                "title": "Test",
                "goals": ["Valid goal", 123, {"invalid": "object"}]
            },
            "architecture": {"components": []},
            "decisions": [],
            "story_mappings": {}
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False
        assert any("goals[1]" in e and "string" in e for e in errors)
        assert any("goals[2]" in e and "string" in e for e in errors)

    def test_graceful_degradation_optional_fields(self, generator):
        """Test that missing optional fields don't cause errors."""
        # Minimal valid document with only required fields
        doc = {
            "metadata": {"level": "feature", "source": "test"},
            "overview": {"title": "Minimal Test"},
            "architecture": {},  # No components, patterns, etc.
            "decisions": [],
            "story_mappings": {}
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is True, f"Minimal doc should be valid, got errors: {errors}"

    def test_null_values_for_optional_fields(self, generator):
        """Test that null values for nullable fields are accepted."""
        doc = {
            "metadata": {
                "level": "feature",
                "source": "test",
                "prd_reference": None,
                "parent_design_doc": None,
                "feature_id": None
            },
            "overview": {"title": "Test"},
            "architecture": {"components": [], "data_flow": None},
            "decisions": [],
            "story_mappings": {}
        }

        is_valid, errors = generator.validate_design_doc(doc)
        # Should be valid since these fields are nullable
        assert is_valid is True, f"Null optional fields should be valid, got errors: {errors}"

    def test_invalid_story_mapping_structure(self, generator):
        """Test validation of story mapping structure."""
        doc = {
            "metadata": {"level": "feature", "source": "test"},
            "overview": {"title": "Test"},
            "architecture": {"components": []},
            "decisions": [],
            "story_mappings": {
                "story-001": {
                    "components": "should be array"  # Wrong type
                }
            }
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False
        assert any("story_mappings.story-001.components" in e and "array" in e for e in errors)

    def test_invalid_feature_mapping_structure(self, generator):
        """Test validation of feature mapping structure."""
        doc = {
            "metadata": {"level": "project", "source": "test"},
            "overview": {"title": "Test"},
            "architecture": {"components": []},
            "decisions": [],
            "feature_mappings": {
                "feature-1": "should be object"  # Wrong type
            }
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False
        assert any("feature_mappings.feature-1" in e and "object" in e for e in errors)

    def test_error_messages_include_field_paths(self, generator):
        """Test that all error messages include clear field paths."""
        doc = {
            "metadata": {"level": "feature"},  # Missing source
            "overview": {},  # Missing title
            "architecture": {
                "components": [
                    {}  # Missing name
                ]
            },
            "decisions": [
                {"title": "No ID"}  # Missing id
            ],
            "story_mappings": {}
        }

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is False

        # Check that errors have proper field paths
        error_text = " ".join(errors)
        assert "metadata.source" in error_text
        assert "overview.title" in error_text
        assert "architecture.components[0].name" in error_text
        assert "decisions[0].id" in error_text


class TestDesignDocGeneratorIntegration:
    """Integration tests for DesignDocGenerator validation."""

    def test_generated_project_doc_is_valid(self, generator, temp_project_dir):
        """Test that generated project-level docs pass validation when mega-plan exists."""
        # Create a minimal mega-plan.json to provide context
        mega_plan = {
            "goal": "Test Project Goal",
            "features": [
                {"id": "feature-1", "description": "Test feature"}
            ]
        }
        with open(temp_project_dir / "mega-plan.json", "w") as f:
            json.dump(mega_plan, f)

        doc = generator.generate_project_design_doc(source="test")
        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is True, f"Generated project doc should be valid: {errors}"

    def test_generated_feature_doc_is_valid(self, generator, temp_project_dir):
        """Test that generated feature-level docs pass validation when PRD exists."""
        # Create a minimal prd.json to provide context
        prd = {
            "metadata": {"description": "Test Feature"},
            "goal": "Test feature goal",
            "objectives": ["Objective 1"],
            "stories": [{"id": "story-001", "title": "Test story"}]
        }
        with open(temp_project_dir / "prd.json", "w") as f:
            json.dump(prd, f)

        doc = generator.generate_feature_design_doc(source="test")
        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is True, f"Generated feature doc should be valid: {errors}"

    def test_doc_with_added_components_is_valid(self, generator, temp_project_dir):
        """Test that docs with added components remain valid."""
        # Create a minimal prd.json to provide context
        prd = {
            "metadata": {"description": "Test Feature"},
            "goal": "Test feature goal",
            "objectives": [],
            "stories": []
        }
        with open(temp_project_dir / "prd.json", "w") as f:
            json.dump(prd, f)

        doc = generator.generate_feature_design_doc(source="test")

        generator.add_component(
            doc,
            name="TestComponent",
            description="A test component",
            responsibilities=["Do stuff"],
            dependencies=["OtherComponent"],
            files=["test.py"]
        )

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is True, f"Doc with component should be valid: {errors}"

    def test_doc_with_added_decisions_is_valid(self, generator, temp_project_dir):
        """Test that docs with added ADRs remain valid."""
        # Create a minimal prd.json to provide context
        prd = {
            "metadata": {"description": "Test Feature"},
            "goal": "Test feature goal",
            "objectives": [],
            "stories": []
        }
        with open(temp_project_dir / "prd.json", "w") as f:
            json.dump(prd, f)

        doc = generator.generate_feature_design_doc(source="test")

        generator.add_decision(
            doc,
            title="Test Decision",
            context="We need to decide",
            decision="We decided this",
            rationale="Because reasons",
            alternatives=["Other option"],
            status="accepted"
        )

        is_valid, errors = generator.validate_design_doc(doc)
        assert is_valid is True, f"Doc with decision should be valid: {errors}"

    def test_empty_generated_doc_has_validation_issues(self, generator):
        """Test that empty generated docs (without source files) have validation issues."""
        # Generate without any source files - should have empty title
        doc = generator.generate_feature_design_doc(source="test")
        is_valid, errors = generator.validate_design_doc(doc)

        # Empty title is expected since there's no prd.json
        assert is_valid is False
        assert any("overview.title" in e for e in errors)
