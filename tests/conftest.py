"""Pytest configuration and fixtures for Plan Cascade tests."""

import json
import pytest
from pathlib import Path


@pytest.fixture
def tmp_path(tmp_path_factory):
    """Create a temporary directory for each test."""
    return tmp_path_factory.mktemp("test")


@pytest.fixture
def sample_prd():
    """Create a sample PRD for testing."""
    return {
        "metadata": {
            "created_at": "2026-01-28T10:00:00Z",
            "version": "1.0.0",
            "description": "Test PRD"
        },
        "goal": "Test goal",
        "objectives": ["Objective 1", "Objective 2"],
        "stories": [
            {
                "id": "story-001",
                "title": "First story",
                "description": "First story description",
                "priority": "high",
                "dependencies": [],
                "status": "pending",
                "acceptance_criteria": ["AC1", "AC2"]
            },
            {
                "id": "story-002",
                "title": "Second story",
                "description": "Second story description",
                "priority": "medium",
                "dependencies": ["story-001"],
                "status": "pending",
                "acceptance_criteria": ["AC1"]
            },
            {
                "id": "story-003",
                "title": "Third story",
                "description": "Third story description",
                "priority": "low",
                "dependencies": ["story-001"],
                "status": "pending",
                "acceptance_criteria": ["AC1"]
            },
            {
                "id": "story-004",
                "title": "Fourth story",
                "description": "Fourth story description",
                "priority": "medium",
                "dependencies": ["story-002", "story-003"],
                "status": "pending",
                "acceptance_criteria": ["AC1"]
            }
        ]
    }


@pytest.fixture
def prd_path(tmp_path, sample_prd):
    """Create a PRD file in the temp directory."""
    prd_file = tmp_path / "prd.json"
    with open(prd_file, "w") as f:
        json.dump(sample_prd, f)
    return prd_file


@pytest.fixture
def sample_mega_plan():
    """Create a sample mega-plan for testing."""
    return {
        "metadata": {
            "created_at": "2026-01-28T10:00:00Z",
            "version": "1.0.0"
        },
        "goal": "Build e-commerce platform",
        "description": "Build a complete e-commerce platform with multiple features",
        "execution_mode": "auto",
        "target_branch": "main",
        "features": [
            {
                "id": "feature-001",
                "name": "feature-auth",
                "title": "User Authentication",
                "description": "Implement user authentication system",
                "priority": "high",
                "dependencies": [],
                "status": "pending"
            },
            {
                "id": "feature-002",
                "name": "feature-products",
                "title": "Product Catalog",
                "description": "Implement product catalog",
                "priority": "high",
                "dependencies": [],
                "status": "pending"
            },
            {
                "id": "feature-003",
                "name": "feature-cart",
                "title": "Shopping Cart",
                "description": "Implement shopping cart",
                "priority": "medium",
                "dependencies": ["feature-001", "feature-002"],
                "status": "pending"
            }
        ]
    }
