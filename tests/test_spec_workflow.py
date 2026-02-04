"""Tests for spec interview core utilities (spec.json -> prd.json)."""

import json
from pathlib import Path

from typer.testing import CliRunner

from plan_cascade.core.spec_compiler import CompileOptions, compile_spec_to_prd
from plan_cascade.core.spec_models import Spec, SpecStory
from plan_cascade.core.spec_quality_gate import check_spec_quality
from src.plan_cascade.cli.spec import spec_app


runner = CliRunner()


def _make_minimal_good_spec() -> Spec:
    spec = Spec()
    spec.overview = {
        "title": "Example Feature",
        "goal": "Implement example feature",
        "problem": "Users cannot do X",
        "success_metrics": ["Tests pass", "Endpoint returns 200"],
        "non_goals": ["No UI redesign"],
    }
    spec.scope = {
        "in_scope": ["Add endpoint"],
        "out_of_scope": ["Rewrite auth"],
        "do_not_touch": [],
        "assumptions": [],
    }
    spec.requirements = {"functional": ["User can do X"], "non_functional": {}}
    spec.interfaces = {"api": [{"name": "POST /x", "notes": ""}], "data_models": []}
    spec.stories = [
        SpecStory(
            id="story-001",
            category="core",
            title="Implement X",
            description="Add X end-to-end",
            acceptance_criteria=["Returns 200", "Handles invalid input with 400"],
            verification={"commands": ["uv run pytest -q"], "manual_steps": []},
            test_expectations={"required": True, "coverage_areas": ["x edge cases"]},
            dependencies=[],
            context_estimate="medium",
        )
    ]
    spec.ensure_defaults()
    return spec


class TestSpecCompiler:
    def test_compile_maps_category_to_tags_and_configs(self):
        spec = _make_minimal_good_spec()

        prd = compile_spec_to_prd(
            spec,
            options=CompileOptions(
                description="Example",
                flow_level="full",
                tdd_mode="on",
                confirm_mode=True,
                additional_metadata={"mega_feature_id": "feature-001"},
            ),
        )

        assert prd["goal"] == "Implement example feature"
        assert prd["flow_config"]["level"] == "full"
        assert prd["tdd_config"]["mode"] == "on"
        assert prd["execution_config"]["require_batch_confirm"] is True
        assert prd["metadata"]["mega_feature_id"] == "feature-001"

        story = prd["stories"][0]
        assert "category:core" in story["tags"]
        assert story["verification_commands"] == ["uv run pytest -q"]


class TestSpecQualityGate:
    def test_full_flow_requires_completeness(self):
        spec = Spec()
        spec.overview = {"title": "X", "goal": "Y"}
        spec.scope = {"in_scope": ["a"]}
        spec.stories = [
            SpecStory(
                id="story-001",
                title="Do thing",
                description="...",
                acceptance_criteria=["works correctly"],
                verification={"commands": [], "manual_steps": []},
                context_estimate="xlarge",
            )
        ]
        spec.ensure_defaults()

        res = check_spec_quality(spec, flow_level="full")
        assert res.errors, "Expected FULL flow to produce errors"
        assert any("overview.non_goals" in e for e in res.errors)
        assert any("scope.out_of_scope" in e for e in res.errors)
        assert any("success_metrics" in e for e in res.errors)
        assert any(">= 2 acceptance criteria" in e for e in res.errors)
        assert any("context_estimate=xlarge" in e for e in res.errors)
        assert any("verification.commands" in e for e in res.errors)
        assert any("vague phrase" in e or "含糊" in e for e in res.errors)


class TestSpecCLI:
    def test_compile_command_writes_prd(self, tmp_path: Path):
        spec = _make_minimal_good_spec()
        (tmp_path / "spec.json").write_text(json.dumps(spec.to_dict(), indent=2), encoding="utf-8")

        result = runner.invoke(
            spec_app,
            [
                "compile",
                "--output-dir",
                str(tmp_path),
                "--flow",
                "full",
                "--tdd",
                "on",
            ],
        )

        assert result.exit_code == 0, result.output
        prd_path = tmp_path / "prd.json"
        assert prd_path.exists()
        prd = json.loads(prd_path.read_text(encoding="utf-8"))
        assert prd["flow_config"]["level"] == "full"
        assert prd["tdd_config"]["mode"] == "on"

