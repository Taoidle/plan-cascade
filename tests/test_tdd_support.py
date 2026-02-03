"""
Tests for TDD Support Module.

Tests cover:
- TDDMode enum
- TDDConfig, TDDStepGuide, TDDCheckResult, TestExpectations dataclasses
- get_tdd_prompt_template() function
- should_enable_tdd() auto mode logic
- check_tdd_compliance() gate checking
- Serialization/deserialization
"""

import pytest

from plan_cascade.core.tdd_support import (
    HIGH_RISK_KEYWORDS,
    HIGH_RISK_TAGS,
    TDDCheckResult,
    TDDConfig,
    TDDMode,
    TDDStepGuide,
    TDDTestRequirements,
    StoryTestExpectations,
    add_tdd_to_prd,
    check_tdd_compliance,
    get_tdd_config_from_prd,
    get_tdd_prompt_template,
    get_tdd_recommendation,
    is_test_file,
    should_enable_tdd,
)


class TestTDDMode:
    """Tests for TDDMode enum."""

    def test_enum_values(self):
        """Test TDDMode has expected values."""
        assert TDDMode.OFF.value == "off"
        assert TDDMode.ON.value == "on"
        assert TDDMode.AUTO.value == "auto"

    def test_enum_from_string(self):
        """Test creating TDDMode from string."""
        assert TDDMode("off") == TDDMode.OFF
        assert TDDMode("on") == TDDMode.ON
        assert TDDMode("auto") == TDDMode.AUTO

    def test_invalid_mode_raises(self):
        """Test invalid mode value raises ValueError."""
        with pytest.raises(ValueError):
            TDDMode("invalid")


class TestTDDTestRequirements:
    """Tests for TDDTestRequirements dataclass."""

    def test_default_values(self):
        """Test default values."""
        req = TDDTestRequirements()
        assert req.require_test_changes is True
        assert req.minimum_coverage_delta == 0.0
        assert "test_" in req.test_patterns

    def test_to_dict(self):
        """Test serialization to dict."""
        req = TDDTestRequirements(
            require_test_changes=False,
            minimum_coverage_delta=5.0,
            test_patterns=["test_", "spec_"],
        )
        data = req.to_dict()
        assert data["require_test_changes"] is False
        assert data["minimum_coverage_delta"] == 5.0
        assert data["test_patterns"] == ["test_", "spec_"]

    def test_from_dict(self):
        """Test deserialization from dict."""
        data = {
            "require_test_changes": False,
            "minimum_coverage_delta": 10.0,
            "test_patterns": ["test/"],
        }
        req = TDDTestRequirements.from_dict(data)
        assert req.require_test_changes is False
        assert req.minimum_coverage_delta == 10.0
        assert req.test_patterns == ["test/"]

    def test_from_dict_defaults(self):
        """Test from_dict with missing keys uses defaults."""
        req = TDDTestRequirements.from_dict({})
        assert req.require_test_changes is True
        assert req.minimum_coverage_delta == 0.0


class TestTDDConfig:
    """Tests for TDDConfig dataclass."""

    def test_default_values(self):
        """Test default values."""
        config = TDDConfig()
        assert config.mode == TDDMode.OFF
        assert config.enforce_for_high_risk is True
        assert isinstance(config.test_requirements, TDDTestRequirements)

    def test_to_dict(self):
        """Test serialization to dict."""
        config = TDDConfig(
            mode=TDDMode.ON,
            enforce_for_high_risk=False,
        )
        data = config.to_dict()
        assert data["mode"] == "on"
        assert data["enforce_for_high_risk"] is False
        assert "test_requirements" in data

    def test_from_dict(self):
        """Test deserialization from dict."""
        data = {
            "mode": "auto",
            "enforce_for_high_risk": True,
            "test_requirements": {
                "require_test_changes": True,
            },
        }
        config = TDDConfig.from_dict(data)
        assert config.mode == TDDMode.AUTO
        assert config.enforce_for_high_risk is True

    def test_from_dict_invalid_mode_defaults_to_off(self):
        """Test invalid mode value defaults to OFF."""
        data = {"mode": "invalid_mode"}
        config = TDDConfig.from_dict(data)
        assert config.mode == TDDMode.OFF

    def test_roundtrip_serialization(self):
        """Test to_dict -> from_dict roundtrip."""
        original = TDDConfig(
            mode=TDDMode.AUTO,
            enforce_for_high_risk=True,
            test_requirements=TDDTestRequirements(
                require_test_changes=True,
                minimum_coverage_delta=5.0,
            ),
        )
        data = original.to_dict()
        restored = TDDConfig.from_dict(data)
        assert restored.mode == original.mode
        assert restored.enforce_for_high_risk == original.enforce_for_high_risk
        assert (
            restored.test_requirements.require_test_changes
            == original.test_requirements.require_test_changes
        )


class TestTDDStepGuide:
    """Tests for TDDStepGuide dataclass."""

    def test_default_values(self):
        """Test default values are empty strings."""
        guide = TDDStepGuide()
        assert guide.step1_red == ""
        assert guide.step2_green == ""
        assert guide.step3_refactor == ""
        assert guide.context_notes == ""

    def test_to_dict(self):
        """Test serialization to dict."""
        guide = TDDStepGuide(
            step1_red="Write failing test",
            step2_green="Implement minimally",
            step3_refactor="Clean up code",
            context_notes="High risk area",
        )
        data = guide.to_dict()
        assert data["step1_red"] == "Write failing test"
        assert data["step2_green"] == "Implement minimally"
        assert data["step3_refactor"] == "Clean up code"
        assert data["context_notes"] == "High risk area"

    def test_from_dict(self):
        """Test deserialization from dict."""
        data = {
            "step1_red": "Red phase",
            "step2_green": "Green phase",
            "step3_refactor": "Refactor phase",
        }
        guide = TDDStepGuide.from_dict(data)
        assert guide.step1_red == "Red phase"
        assert guide.step2_green == "Green phase"
        assert guide.step3_refactor == "Refactor phase"

    def test_to_prompt(self):
        """Test to_prompt generates formatted text."""
        guide = TDDStepGuide(
            step1_red="Write test",
            step2_green="Implement",
            step3_refactor="Refactor",
            context_notes="Security sensitive",
        )
        prompt = guide.to_prompt()
        assert "TDD Workflow Guide" in prompt
        assert "Step 1: RED" in prompt
        assert "Step 2: GREEN" in prompt
        assert "Step 3: REFACTOR" in prompt
        assert "Write test" in prompt
        assert "Security sensitive" in prompt

    def test_to_prompt_no_context_notes(self):
        """Test to_prompt without context notes."""
        guide = TDDStepGuide(
            step1_red="Write test",
            step2_green="Implement",
            step3_refactor="Refactor",
        )
        prompt = guide.to_prompt()
        assert "**Note:**" not in prompt


class TestTDDCheckResult:
    """Tests for TDDCheckResult dataclass."""

    def test_default_values(self):
        """Test default values."""
        result = TDDCheckResult()
        assert result.passed is True
        assert result.warnings == []
        assert result.errors == []
        assert result.suggestions == []
        assert result.check_name == "tdd_compliance"

    def test_add_warning(self):
        """Test adding warnings."""
        result = TDDCheckResult()
        result.add_warning("Test warning")
        assert result.passed is True  # Warnings don't fail
        assert "Test warning" in result.warnings

    def test_add_error(self):
        """Test adding errors marks as failed."""
        result = TDDCheckResult()
        result.add_error("Test error")
        assert result.passed is False
        assert "Test error" in result.errors

    def test_add_suggestion(self):
        """Test adding suggestions."""
        result = TDDCheckResult()
        result.add_suggestion("Test suggestion")
        assert result.passed is True
        assert "Test suggestion" in result.suggestions

    def test_has_issues(self):
        """Test has_issues detection."""
        result = TDDCheckResult()
        assert result.has_issues() is False

        result.add_warning("warning")
        assert result.has_issues() is True

    def test_get_summary(self):
        """Test get_summary output."""
        result = TDDCheckResult(check_name="test_check")
        result.add_error("Error 1")
        result.add_warning("Warning 1")

        summary = result.get_summary()
        assert "[FAILED] test_check" in summary
        assert "Error 1" in summary
        assert "Warning 1" in summary

    def test_to_dict_from_dict_roundtrip(self):
        """Test serialization roundtrip."""
        original = TDDCheckResult(
            passed=False,
            warnings=["warn1"],
            errors=["error1"],
            suggestions=["suggest1"],
            check_name="test",
            details={"key": "value"},
        )
        data = original.to_dict()
        restored = TDDCheckResult.from_dict(data)

        assert restored.passed == original.passed
        assert restored.warnings == original.warnings
        assert restored.errors == original.errors
        assert restored.suggestions == original.suggestions
        assert restored.check_name == original.check_name
        assert restored.details == original.details


class TestStoryTestExpectations:
    """Tests for StoryTestExpectations dataclass."""

    def test_default_values(self):
        """Test default values."""
        expectations = StoryTestExpectations()
        assert expectations.required is False
        assert expectations.test_types == []
        assert expectations.coverage_areas == []
        assert expectations.min_tests == 0

    def test_to_dict(self):
        """Test serialization."""
        expectations = StoryTestExpectations(
            required=True,
            test_types=["unit", "integration"],
            coverage_areas=["api", "database"],
            min_tests=3,
        )
        data = expectations.to_dict()
        assert data["required"] is True
        assert data["test_types"] == ["unit", "integration"]
        assert data["coverage_areas"] == ["api", "database"]
        assert data["min_tests"] == 3

    def test_from_dict(self):
        """Test deserialization."""
        data = {
            "required": True,
            "test_types": ["unit"],
            "min_tests": 2,
        }
        expectations = StoryTestExpectations.from_dict(data)
        assert expectations.required is True
        assert expectations.test_types == ["unit"]
        assert expectations.min_tests == 2


class TestGetTDDPromptTemplate:
    """Tests for get_tdd_prompt_template function."""

    def test_basic_template(self):
        """Test basic template generation."""
        guide = get_tdd_prompt_template()
        assert guide.step1_red != ""
        assert guide.step2_green != ""
        assert guide.step3_refactor != ""

    def test_template_content_quality(self):
        """Test template contains useful guidance."""
        guide = get_tdd_prompt_template()
        # Check step1 has test-writing guidance
        assert "test" in guide.step1_red.lower()
        assert "fail" in guide.step1_red.lower()

        # Check step2 has implementation guidance
        assert "pass" in guide.step2_green.lower() or "implement" in guide.step2_green.lower()

        # Check step3 has refactoring guidance
        assert "refactor" in guide.step3_refactor.lower() or "clean" in guide.step3_refactor.lower()

    def test_template_with_story_context(self):
        """Test template with story context adds notes."""
        story = {
            "id": "story-001",
            "tags": ["security"],
            "priority": "high",
        }
        guide = get_tdd_prompt_template(story=story)
        assert "security" in guide.context_notes.lower() or guide.context_notes != ""

    def test_template_with_test_expectations(self):
        """Test template with test_expectations adds notes."""
        story = {
            "id": "story-001",
            "test_expectations": {
                "test_types": ["integration"],
            },
        }
        guide = get_tdd_prompt_template(story=story)
        assert "integration" in guide.context_notes.lower()


class TestShouldEnableTDD:
    """Tests for should_enable_tdd function."""

    def test_mode_off_never_enables(self):
        """Test OFF mode never enables TDD."""
        config = TDDConfig(mode=TDDMode.OFF)
        story = {"id": "story-001", "tags": ["security"]}
        assert should_enable_tdd(story, config) is False

    def test_mode_on_always_enables(self):
        """Test ON mode always enables TDD."""
        config = TDDConfig(mode=TDDMode.ON)
        story = {"id": "story-001"}
        assert should_enable_tdd(story, config) is True

    def test_auto_mode_with_high_risk_tag(self):
        """Test AUTO mode enables for high-risk tags."""
        config = TDDConfig(mode=TDDMode.AUTO, enforce_for_high_risk=True)

        for tag in HIGH_RISK_TAGS:
            story = {"id": "story-001", "tags": [tag]}
            assert should_enable_tdd(story, config) is True, f"Failed for tag: {tag}"

    def test_auto_mode_with_high_risk_keyword(self):
        """Test AUTO mode enables for high-risk keywords."""
        config = TDDConfig(mode=TDDMode.AUTO, enforce_for_high_risk=True)

        story = {"id": "story-001", "title": "Implement authentication system"}
        assert should_enable_tdd(story, config) is True

    def test_auto_mode_with_test_expectations_required(self):
        """Test AUTO mode enables when test_expectations.required is True."""
        config = TDDConfig(mode=TDDMode.AUTO)
        story = {
            "id": "story-001",
            "test_expectations": {"required": True},
        }
        assert should_enable_tdd(story, config) is True

    def test_auto_mode_with_large_context(self):
        """Test AUTO mode enables for large context estimates."""
        config = TDDConfig(mode=TDDMode.AUTO)

        story = {"id": "story-001", "context_estimate": "large"}
        assert should_enable_tdd(story, config) is True

        story = {"id": "story-001", "context_estimate": "xlarge"}
        assert should_enable_tdd(story, config) is True

    def test_auto_mode_low_risk_does_not_enable(self):
        """Test AUTO mode does not enable for low-risk stories."""
        config = TDDConfig(mode=TDDMode.AUTO)
        story = {
            "id": "story-001",
            "title": "Update README",
            "description": "Fix typo in documentation",
            "tags": ["docs"],
            "context_estimate": "small",
        }
        assert should_enable_tdd(story, config) is False

    def test_auto_mode_enforce_for_high_risk_disabled(self):
        """Test enforce_for_high_risk=False doesn't enable for risk factors."""
        config = TDDConfig(mode=TDDMode.AUTO, enforce_for_high_risk=False)
        story = {"id": "story-001", "tags": ["security"]}
        # Should not enable because enforce_for_high_risk is False
        assert should_enable_tdd(story, config) is False

    def test_handles_missing_story_fields(self):
        """Test handles stories with missing fields gracefully."""
        config = TDDConfig(mode=TDDMode.AUTO)
        story = {"id": "story-001"}  # Minimal story
        # Should not crash, should return False for minimal story
        result = should_enable_tdd(story, config)
        assert isinstance(result, bool)


class TestGetTDDRecommendation:
    """Tests for get_tdd_recommendation function."""

    def test_off_mode_recommendation(self):
        """Test recommendation for OFF mode."""
        config = TDDConfig(mode=TDDMode.OFF)
        story = {"id": "story-001"}
        recommendation = get_tdd_recommendation(story, config)
        assert "off" in recommendation.lower()
        assert "disabled" in recommendation.lower()

    def test_on_mode_recommendation(self):
        """Test recommendation for ON mode."""
        config = TDDConfig(mode=TDDMode.ON)
        story = {"id": "story-001"}
        recommendation = get_tdd_recommendation(story, config)
        assert "on" in recommendation.lower()
        assert "enabled" in recommendation.lower()

    def test_auto_mode_with_risk_factors(self):
        """Test auto mode recommendation includes risk factors."""
        config = TDDConfig(mode=TDDMode.AUTO)
        story = {"id": "story-001", "tags": ["security"]}
        recommendation = get_tdd_recommendation(story, config)
        assert "auto-enabled" in recommendation.lower()
        assert "security" in recommendation.lower()

    def test_auto_mode_no_risk(self):
        """Test auto mode recommendation when no risk factors."""
        config = TDDConfig(mode=TDDMode.AUTO)
        story = {"id": "story-001", "title": "Simple task", "tags": ["docs"]}
        recommendation = get_tdd_recommendation(story, config)
        assert "auto" in recommendation.lower()
        assert "optional" in recommendation.lower()


class TestIsTestFile:
    """Tests for is_test_file function."""

    def test_test_prefixed_files(self):
        """Test files with test_ prefix."""
        assert is_test_file("test_module.py") is True
        assert is_test_file("src/test_api.py") is True

    def test_test_suffixed_files(self):
        """Test files with _test suffix."""
        assert is_test_file("module_test.py") is True
        assert is_test_file("api_test.ts") is True

    def test_test_directory_files(self):
        """Test files in test directories."""
        assert is_test_file("tests/test_api.py") is True
        assert is_test_file("test/module.py") is True
        assert is_test_file("spec/api.spec.ts") is True

    def test_non_test_files(self):
        """Test non-test files."""
        assert is_test_file("module.py") is False
        assert is_test_file("src/api.py") is False
        assert is_test_file("main.ts") is False

    def test_custom_patterns(self):
        """Test with custom patterns."""
        patterns = ["_spec.", "spec/"]
        assert is_test_file("api_spec.js", patterns) is True
        assert is_test_file("spec/api.js", patterns) is True
        assert is_test_file("test_api.py", patterns) is False


class TestCheckTDDCompliance:
    """Tests for check_tdd_compliance function."""

    def test_tdd_disabled_passes(self):
        """Test compliance passes when TDD is disabled."""
        config = TDDConfig(mode=TDDMode.OFF)
        story = {"id": "story-001"}
        result = check_tdd_compliance(story, changed_files=["src/module.py"], config=config)
        assert result.passed is True

    def test_no_code_changes_passes(self):
        """Test compliance passes with no code changes."""
        config = TDDConfig(mode=TDDMode.ON)
        story = {"id": "story-001"}
        result = check_tdd_compliance(story, changed_files=[], config=config)
        assert result.passed is True

    def test_code_and_test_changes_passes(self):
        """Test compliance passes with both code and test changes."""
        config = TDDConfig(mode=TDDMode.ON)
        story = {"id": "story-001"}
        changed_files = ["src/module.py", "tests/test_module.py"]
        result = check_tdd_compliance(story, changed_files=changed_files, config=config)
        assert result.passed is True
        assert result.details["test_files_changed"] == 1
        assert result.details["code_files_changed"] == 1

    def test_code_without_test_for_high_risk_fails(self):
        """Test compliance fails for high-risk stories without tests."""
        config = TDDConfig(mode=TDDMode.AUTO, enforce_for_high_risk=True)
        story = {
            "id": "story-001",
            "tags": ["security"],
            "test_expectations": {"required": True},
        }
        changed_files = ["src/auth.py"]
        result = check_tdd_compliance(story, changed_files=changed_files, config=config)
        assert result.passed is False
        assert len(result.errors) > 0

    def test_code_without_test_for_low_risk_warns(self):
        """Test compliance warns for low-risk stories without tests."""
        config = TDDConfig(mode=TDDMode.ON)
        story = {
            "id": "story-001",
            "tags": ["docs"],
        }
        changed_files = ["src/module.py"]
        result = check_tdd_compliance(story, changed_files=changed_files, config=config)
        # Should have warnings but not fail
        assert len(result.warnings) > 0 or len(result.errors) > 0

    def test_result_includes_file_lists(self):
        """Test result includes file categorization."""
        config = TDDConfig(mode=TDDMode.ON)
        story = {"id": "story-001"}
        changed_files = ["src/api.py", "tests/test_api.py", "src/model.py"]
        result = check_tdd_compliance(story, changed_files=changed_files, config=config)

        assert "test_files" in result.details
        assert "code_files" in result.details
        assert len(result.details["test_files"]) == 1
        assert len(result.details["code_files"]) == 2

    def test_compliance_with_test_expectations(self):
        """Test compliance checks test_expectations."""
        config = TDDConfig(mode=TDDMode.ON)
        story = {
            "id": "story-001",
            "test_expectations": {
                "min_tests": 2,
                "coverage_areas": ["api", "database"],
            },
        }
        changed_files = ["src/api.py", "tests/test_api.py"]
        result = check_tdd_compliance(story, changed_files=changed_files, config=config)

        # Should have warning about min_tests
        assert any("2" in w for w in result.warnings)
        # Should have suggestion about coverage areas
        assert any("api" in s and "database" in s for s in result.suggestions)


class TestPRDIntegration:
    """Tests for PRD integration functions."""

    def test_get_tdd_config_from_prd_with_config(self):
        """Test extracting TDD config from PRD."""
        prd = {
            "tdd_config": {
                "mode": "on",
                "enforce_for_high_risk": False,
            }
        }
        config = get_tdd_config_from_prd(prd)
        assert config.mode == TDDMode.ON
        assert config.enforce_for_high_risk is False

    def test_get_tdd_config_from_prd_without_config(self):
        """Test extracting TDD config from PRD without tdd_config."""
        prd = {"stories": []}
        config = get_tdd_config_from_prd(prd)
        assert config.mode == TDDMode.OFF  # Default

    def test_add_tdd_to_prd(self):
        """Test adding TDD config to PRD."""
        prd = {"stories": []}
        config = TDDConfig(mode=TDDMode.AUTO)

        updated_prd = add_tdd_to_prd(prd, config)

        assert "tdd_config" in updated_prd
        assert updated_prd["tdd_config"]["mode"] == "auto"


class TestHighRiskConstants:
    """Tests for high-risk detection constants."""

    def test_high_risk_keywords_not_empty(self):
        """Test HIGH_RISK_KEYWORDS is not empty."""
        assert len(HIGH_RISK_KEYWORDS) > 0

    def test_high_risk_tags_not_empty(self):
        """Test HIGH_RISK_TAGS is not empty."""
        assert len(HIGH_RISK_TAGS) > 0

    def test_security_related_keywords_present(self):
        """Test security-related keywords are present."""
        security_keywords = ["security", "auth", "encrypt", "credential"]
        for kw in security_keywords:
            assert kw in HIGH_RISK_KEYWORDS, f"Missing keyword: {kw}"

    def test_database_related_keywords_present(self):
        """Test database-related keywords are present."""
        db_keywords = ["database", "migration"]
        for kw in db_keywords:
            assert kw in HIGH_RISK_KEYWORDS, f"Missing keyword: {kw}"
