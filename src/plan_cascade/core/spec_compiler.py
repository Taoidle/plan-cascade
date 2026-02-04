#!/usr/bin/env python3
"""Compile spec.json into a Plan Cascade PRD (prd.json)."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from typing import Any

from .spec_models import Spec


@dataclass(frozen=True)
class CompileOptions:
    """Options for compiling spec -> PRD."""

    description: str = ""
    flow_level: str | None = None  # quick|standard|full
    tdd_mode: str | None = None  # off|on|auto
    confirm_mode: bool | None = None
    no_confirm_mode: bool | None = None
    additional_metadata: dict[str, Any] | None = None


_CATEGORY_PRIORITY_DEFAULTS: dict[str, str] = {
    "setup": "high",
    "core": "high",
    "integration": "medium",
    "polish": "low",
    "test": "medium",
}


def compile_spec_to_prd(spec: Spec, options: CompileOptions | None = None) -> dict[str, Any]:
    """
    Compile a Spec into a Plan Cascade PRD dict.

    The generated PRD is intentionally backwards compatible: unknown fields are allowed.
    """
    options = options or CompileOptions()
    spec.ensure_defaults()

    overview = spec.overview or {}
    goal = str(overview.get("goal") or overview.get("title") or "").strip() or "Complete the task"

    # Objectives: prefer functional requirements (bounded), fallback to scope.in_scope
    requirements = spec.requirements or {}
    functional = requirements.get("functional") or []
    if isinstance(functional, str):
        functional = [s.strip() for s in functional.split("\n") if s.strip()]
    if not isinstance(functional, list):
        functional = []

    objectives: list[str] = [str(x).strip() for x in functional if str(x).strip()]
    if not objectives:
        scope = spec.scope or {}
        in_scope = scope.get("in_scope") or []
        if isinstance(in_scope, str):
            in_scope = [s.strip() for s in in_scope.split("\n") if s.strip()]
        if isinstance(in_scope, list):
            objectives = [str(x).strip() for x in in_scope if str(x).strip()]

    notes: list[str] = []
    if len(objectives) > 7:
        notes.append("Objectives truncated to 7; see spec.json for full list.")
        objectives = objectives[:7]

    prd: dict[str, Any] = {
        "metadata": {
            "created_at": datetime.now().isoformat(),
            "version": "1.0.0",
            "description": options.description or str(spec.metadata.get("description", "")) or "",
            "source": "spec-compile",
            "spec_schema_version": str(spec.metadata.get("schema_version", "")),
        },
        "goal": goal,
        "objectives": objectives,
        "stories": [],
    }

    if notes:
        prd["metadata"]["notes"] = notes

    # Merge any additional metadata (e.g., mega feature identifiers)
    if options.additional_metadata:
        for k, v in options.additional_metadata.items():
            # Don't overwrite core keys unless explicitly intended
            if k not in prd["metadata"]:
                prd["metadata"][k] = v

    # Compile stories
    for idx, story in enumerate(spec.stories, 1):
        story_id = story.id or f"story-{idx:03d}"
        category = (story.category or "core").strip() or "core"
        title = story.title.strip() or f"Story {idx}"
        description = story.description.strip() or title

        priority = story.priority or _CATEGORY_PRIORITY_DEFAULTS.get(category, "medium")

        tags: list[str] = []
        tags.append(f"category:{category}")

        prd_story: dict[str, Any] = {
            "id": story_id,
            "title": title,
            "description": description,
            "priority": priority,
            "dependencies": list(story.dependencies),
            "status": "pending",
            "acceptance_criteria": list(story.acceptance_criteria),
            "context_estimate": story.context_estimate or "medium",
            "tags": tags,
        }

        verification = story.verification or {}
        commands = verification.get("commands") or []
        manual_steps = verification.get("manual_steps") or []
        if isinstance(commands, list) and commands:
            prd_story["verification_commands"] = [str(c).strip() for c in commands if str(c).strip()]
        if isinstance(manual_steps, list) and manual_steps:
            prd_story["verification_manual_steps"] = [str(s).strip() for s in manual_steps if str(s).strip()]

        if story.test_expectations is not None:
            prd_story["test_expectations"] = dict(story.test_expectations)

        prd["stories"].append(prd_story)

    # Strict-mode fields
    if options.flow_level:
        prd["flow_config"] = {"level": options.flow_level, "source": "command-line"}
        if options.flow_level == "full":
            # Align with existing hybrid-auto guidance
            prd["verification_gate"] = {"enabled": True, "required": True}
            prd["code_review"] = {"enabled": True, "required": True}

    if options.tdd_mode:
        prd["tdd_config"] = {
            "mode": options.tdd_mode,
            "enforce_for_high_risk": True,
            "test_requirements": {
                "require_test_changes": options.tdd_mode == "on",
                "require_test_for_high_risk": True,
                "minimum_coverage_delta": 0.0,
                "test_patterns": ["test_", "_test.", ".test.", "tests/", "test/", "spec/"],
            },
        }

    # Confirm config: --no-confirm wins
    if options.no_confirm_mode:
        prd["execution_config"] = prd.get("execution_config", {})
        prd["execution_config"]["require_batch_confirm"] = False
        prd["execution_config"]["no_confirm_override"] = True
    elif options.confirm_mode:
        prd["execution_config"] = prd.get("execution_config", {})
        prd["execution_config"]["require_batch_confirm"] = True

    return prd

