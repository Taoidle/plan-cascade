#!/usr/bin/env python3
"""Planning-time spec quality checks (shift-left gates)."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterable

from .spec_models import Spec


@dataclass
class SpecQualityResult:
    """Result of spec quality checks."""

    passed: bool = True
    errors: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)

    def add_error(self, message: str) -> None:
        self.passed = False
        self.errors.append(message)

    def add_warning(self, message: str) -> None:
        self.warnings.append(message)


_VAGUE_PHRASES_EN = (
    "works correctly",
    "works as expected",
    "is fast",
    "handles errors",
    "robust",
    "secure",
)
_VAGUE_PHRASES_ZH = (
    "正常工作",
    "符合预期",
    "性能好",
    "处理错误",
    "健壮",
    "安全",
)


def _iter_acceptance_criteria(spec: Spec) -> Iterable[tuple[str, str]]:
    """Yield (story_id, criterion)."""
    for story in spec.stories:
        for ac in story.acceptance_criteria:
            yield story.id, str(ac)


def check_spec_quality(spec: Spec, flow_level: str = "standard") -> SpecQualityResult:
    """
    Validate spec quality.

    In FULL flow, this enforces stronger completeness requirements to reduce
    execution-time confirmations.
    """
    spec.ensure_defaults()
    result = SpecQualityResult()

    flow_level = (flow_level or "standard").strip().lower()
    is_full = flow_level == "full"

    overview = spec.overview or {}
    scope = spec.scope or {}

    non_goals = overview.get("non_goals") or []
    if isinstance(non_goals, str):
        non_goals = [s.strip() for s in non_goals.split("\n") if s.strip()]
    if not isinstance(non_goals, list):
        non_goals = []

    out_of_scope = scope.get("out_of_scope") or []
    if isinstance(out_of_scope, str):
        out_of_scope = [s.strip() for s in out_of_scope.split("\n") if s.strip()]
    if not isinstance(out_of_scope, list):
        out_of_scope = []

    success_metrics = overview.get("success_metrics") or []
    if isinstance(success_metrics, str):
        success_metrics = [s.strip() for s in success_metrics.split("\n") if s.strip()]
    if not isinstance(success_metrics, list):
        success_metrics = []

    if is_full:
        if not non_goals:
            result.add_error("FULL flow requires overview.non_goals to be non-empty.")
        if not out_of_scope:
            result.add_error("FULL flow requires scope.out_of_scope to be non-empty.")
        if not success_metrics:
            result.add_error("FULL flow requires overview.success_metrics to be non-empty.")

    # Basic story checks
    if not spec.stories:
        if is_full:
            result.add_error("FULL flow requires at least 1 story in spec.stories.")
        else:
            result.add_warning("Spec has no stories.")
        return result

    seen_ids: set[str] = set()
    for story in spec.stories:
        if not story.id:
            result.add_error("Story missing id.")
            continue
        if story.id in seen_ids:
            result.add_error(f"Duplicate story id: {story.id}")
        seen_ids.add(story.id)

        if is_full:
            if len(story.acceptance_criteria) < 2:
                result.add_error(
                    f"{story.id}: FULL flow requires >= 2 acceptance criteria."
                )
            if (story.context_estimate or "").strip().lower() == "xlarge":
                result.add_error(f"{story.id}: FULL flow forbids context_estimate=xlarge.")

            verification = story.verification or {}
            commands = verification.get("commands") or []
            manual_steps = verification.get("manual_steps") or []
            has_commands = isinstance(commands, list) and any(str(c).strip() for c in commands)
            has_manual = isinstance(manual_steps, list) and any(str(s).strip() for s in manual_steps)
            if not (has_commands or has_manual):
                result.add_error(
                    f"{story.id}: FULL flow requires verification.commands or manual_steps."
                )

    # Vagueness check
    for story_id, ac in _iter_acceptance_criteria(spec):
        ac_lower = ac.lower()
        for phrase in _VAGUE_PHRASES_EN:
            if phrase in ac_lower:
                result.add_error(
                    f"{story_id}: acceptance_criteria contains vague phrase '{phrase}'."
                )
        for phrase in _VAGUE_PHRASES_ZH:
            if phrase in ac:
                result.add_error(
                    f"{story_id}: acceptance_criteria contains含糊表述 '{phrase}'."
                )

    # Right-sizing warnings
    if len(spec.stories) > 7:
        result.add_warning(
            f"Spec has {len(spec.stories)} stories; consider right-sizing to <= 7 for reviewability."
        )

    return result

