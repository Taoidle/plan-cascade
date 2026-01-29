"""Tests for PRDGenerator module."""

import pytest
from pathlib import Path

from plan_cascade.core.prd_generator import PRDGenerator, create_sample_prd


class TestPRDGenerator:
    """Tests for PRDGenerator class."""

    def test_init(self, tmp_path: Path):
        """Test PRDGenerator initialization."""
        pg = PRDGenerator(tmp_path)
        assert pg.project_root == tmp_path

    def test_generate_prd(self, tmp_path: Path):
        """Test PRD generation from description."""
        pg = PRDGenerator(tmp_path)
        description = "Build a user authentication system with login and registration."

        prd = pg.generate_prd(description)

        assert "metadata" in prd
        assert "goal" in prd
        assert "stories" in prd
        assert prd["metadata"]["description"] == description

    def test_add_story(self, tmp_path: Path):
        """Test adding stories to PRD."""
        pg = PRDGenerator(tmp_path)
        prd = pg.generate_prd("Test project")

        prd = pg.add_story(
            prd,
            title="Implement login",
            description="Create login functionality",
            priority="high",
            acceptance_criteria=["Login form works", "Session created"]
        )

        assert len(prd["stories"]) == 1
        assert prd["stories"][0]["id"] == "story-001"
        assert prd["stories"][0]["title"] == "Implement login"
        assert prd["stories"][0]["priority"] == "high"

    def test_add_story_with_dependencies(self, tmp_path: Path):
        """Test adding stories with dependencies."""
        pg = PRDGenerator(tmp_path)
        prd = pg.generate_prd("Test project")

        prd = pg.add_story(prd, title="Create database schema", description="DB schema")
        prd = pg.add_story(
            prd,
            title="Implement user model",
            description="User model",
            dependencies=["story-001"]
        )

        assert prd["stories"][1]["dependencies"] == ["story-001"]

    def test_estimate_context_size(self, tmp_path: Path):
        """Test context size estimation."""
        pg = PRDGenerator(tmp_path)

        small = pg.estimate_context_size("Fix typo in README")
        assert small in ["small", "medium"]

        large = pg.estimate_context_size(
            "Implement a complete authentication system with JWT tokens, "
            "password hashing, email verification, and role-based access control. "
            "This requires refactoring the existing user model and adding new "
            "middleware for authentication."
        )
        assert large in ["medium", "large", "xlarge"]

    def test_generate_execution_batches(self, tmp_path: Path):
        """Test batch generation from PRD."""
        pg = PRDGenerator(tmp_path)

        prd = create_sample_prd()
        batches = pg.generate_execution_batches(prd)

        assert len(batches) > 0
        # First batch should have story with no dependencies
        first_batch_ids = [s["id"] for s in batches[0]]
        assert "story-001" in first_batch_ids

    def test_validate_prd_valid(self, tmp_path: Path):
        """Test PRD validation with valid PRD."""
        pg = PRDGenerator(tmp_path)
        prd = create_sample_prd()

        is_valid, errors = pg.validate_prd(prd)

        assert is_valid is True
        assert len(errors) == 0

    def test_validate_prd_missing_fields(self, tmp_path: Path):
        """Test PRD validation with missing fields."""
        pg = PRDGenerator(tmp_path)
        prd = {
            "stories": [
                {"id": "story-001"}  # Missing title and description
            ]
        }

        is_valid, errors = pg.validate_prd(prd)

        assert is_valid is False
        assert len(errors) > 0

    def test_validate_prd_duplicate_ids(self, tmp_path: Path):
        """Test PRD validation with duplicate story IDs."""
        pg = PRDGenerator(tmp_path)
        prd = {
            "metadata": {"description": "Test"},
            "goal": "Test",
            "stories": [
                {"id": "story-001", "title": "Test 1", "description": "Desc 1"},
                {"id": "story-001", "title": "Test 2", "description": "Desc 2"}
            ]
        }

        is_valid, errors = pg.validate_prd(prd)

        assert is_valid is False
        assert any("Duplicate" in e for e in errors)


class TestCreateSamplePRD:
    """Tests for create_sample_prd function."""

    def test_creates_valid_prd(self):
        """Test that sample PRD is valid."""
        prd = create_sample_prd()

        assert "metadata" in prd
        assert "goal" in prd
        assert "stories" in prd
        assert len(prd["stories"]) > 0

    def test_stories_have_required_fields(self):
        """Test that sample stories have required fields."""
        prd = create_sample_prd()

        for story in prd["stories"]:
            assert "id" in story
            assert "title" in story
            assert "description" in story
            assert "status" in story
            assert "dependencies" in story
