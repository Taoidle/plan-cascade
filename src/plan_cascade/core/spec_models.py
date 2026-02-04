#!/usr/bin/env python3
"""
Specification (spec.*) models for Plan Cascade.

This module defines lightweight dataclasses for:
- spec.json (planning-time structured spec)
- .state/spec-interview.json (resumable interview state)

These are intentionally minimal and permissive (backward/forward compatible).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any


SPEC_SCHEMA_VERSION = "spec-0.1"
SPEC_INTERVIEW_SCHEMA_VERSION = "spec-interview-0.1"


def utc_now_iso() -> str:
    """Return current UTC time as ISO-8601 string."""
    return datetime.now(timezone.utc).isoformat()


def _as_list(value: Any) -> list[str]:
    if value is None:
        return []
    if isinstance(value, list):
        return [str(v).strip() for v in value if str(v).strip()]
    if isinstance(value, str):
        # Split comma/newline separated input
        parts = [p.strip() for p in value.replace("\r\n", "\n").split("\n")]
        if len(parts) <= 1:
            parts = [p.strip() for p in value.split(",")]
        return [p for p in parts if p]
    return [str(value).strip()] if str(value).strip() else []


def _as_dict(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


@dataclass
class SpecStory:
    """A single story in spec.json."""

    id: str
    category: str = "core"  # setup|core|integration|polish|test
    title: str = ""
    description: str = ""
    acceptance_criteria: list[str] = field(default_factory=list)
    verification: dict[str, Any] = field(
        default_factory=lambda: {"commands": [], "manual_steps": []}
    )
    test_expectations: dict[str, Any] | None = None
    dependencies: list[str] = field(default_factory=list)
    context_estimate: str = "medium"  # small|medium|large|xlarge
    priority: str | None = None  # optional, but useful for PRD ordering

    def to_dict(self) -> dict[str, Any]:
        d: dict[str, Any] = {
            "id": self.id,
            "category": self.category,
            "title": self.title,
            "description": self.description,
            "acceptance_criteria": list(self.acceptance_criteria),
            "verification": dict(self.verification or {}),
            "dependencies": list(self.dependencies),
            "context_estimate": self.context_estimate,
        }
        if self.test_expectations is not None:
            d["test_expectations"] = dict(self.test_expectations)
        if self.priority is not None:
            d["priority"] = self.priority
        return d

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "SpecStory":
        verification = _as_dict(data.get("verification"))
        commands = _as_list(verification.get("commands"))
        manual_steps = _as_list(verification.get("manual_steps"))
        verification = {"commands": commands, "manual_steps": manual_steps}

        return cls(
            id=str(data.get("id", "")).strip(),
            category=str(data.get("category", "core")).strip() or "core",
            title=str(data.get("title", "")).strip(),
            description=str(data.get("description", "")).strip(),
            acceptance_criteria=_as_list(data.get("acceptance_criteria")),
            verification=verification,
            test_expectations=_as_dict(data.get("test_expectations"))
            if data.get("test_expectations") is not None
            else None,
            dependencies=_as_list(data.get("dependencies")),
            context_estimate=str(data.get("context_estimate", "medium")).strip()
            or "medium",
            priority=str(data.get("priority")).strip() if data.get("priority") is not None else None,
        )


@dataclass
class Spec:
    """Top-level spec.json model."""

    metadata: dict[str, Any] = field(default_factory=dict)
    overview: dict[str, Any] = field(default_factory=dict)
    scope: dict[str, Any] = field(default_factory=dict)
    requirements: dict[str, Any] = field(default_factory=dict)
    interfaces: dict[str, Any] = field(default_factory=dict)
    stories: list[SpecStory] = field(default_factory=list)
    phases: list[dict[str, Any]] = field(default_factory=list)
    decision_log: list[dict[str, str]] = field(default_factory=list)
    open_questions: list[str] = field(default_factory=list)

    def ensure_defaults(self) -> None:
        self.metadata = _as_dict(self.metadata)
        self.overview = _as_dict(self.overview)
        self.scope = _as_dict(self.scope)
        self.requirements = _as_dict(self.requirements)
        self.interfaces = _as_dict(self.interfaces)
        self.phases = self.phases if isinstance(self.phases, list) else []
        self.decision_log = self.decision_log if isinstance(self.decision_log, list) else []
        self.open_questions = _as_list(self.open_questions)

        if not self.metadata.get("schema_version"):
            self.metadata["schema_version"] = SPEC_SCHEMA_VERSION
        if not self.metadata.get("created_at"):
            self.metadata["created_at"] = utc_now_iso()
        self.metadata["updated_at"] = utc_now_iso()
        if not self.metadata.get("source"):
            self.metadata["source"] = "spec-interview"

    def to_dict(self) -> dict[str, Any]:
        self.ensure_defaults()
        return {
            "metadata": dict(self.metadata),
            "overview": dict(self.overview),
            "scope": dict(self.scope),
            "requirements": dict(self.requirements),
            "interfaces": dict(self.interfaces),
            "stories": [s.to_dict() for s in self.stories],
            "phases": list(self.phases),
            "decision_log": list(self.decision_log),
            "open_questions": list(self.open_questions),
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "Spec":
        stories_raw = data.get("stories", [])
        stories: list[SpecStory] = []
        if isinstance(stories_raw, list):
            for item in stories_raw:
                if isinstance(item, dict):
                    story = SpecStory.from_dict(item)
                    if story.id:
                        stories.append(story)

        spec = cls(
            metadata=_as_dict(data.get("metadata")),
            overview=_as_dict(data.get("overview")),
            scope=_as_dict(data.get("scope")),
            requirements=_as_dict(data.get("requirements")),
            interfaces=_as_dict(data.get("interfaces")),
            stories=stories,
            phases=data.get("phases", []) if isinstance(data.get("phases", []), list) else [],
            decision_log=data.get("decision_log", [])
            if isinstance(data.get("decision_log", []), list)
            else [],
            open_questions=_as_list(data.get("open_questions")),
        )
        spec.ensure_defaults()
        return spec


@dataclass
class SpecInterviewState:
    """Resumable spec-interview state (.state/spec-interview.json)."""

    schema_version: str = SPEC_INTERVIEW_SCHEMA_VERSION
    status: str = "in_progress"  # in_progress|finalized
    mode: str = "on"  # off|auto|on (record only)
    flow_level: str = "standard"  # quick|standard|full (record only)
    first_principles: bool = False
    max_questions: int = 18
    question_cursor: int = 0
    history: list[dict[str, str]] = field(default_factory=list)  # {ts, question, answer}
    last_draft_refresh_at_question: int = 0
    current_feature: dict[str, str] | None = None  # mega: {id,name,title}
    output_paths: dict[str, str] = field(default_factory=dict)  # {spec_json,spec_md}
    description: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "status": self.status,
            "mode": self.mode,
            "flow_level": self.flow_level,
            "first_principles": self.first_principles,
            "max_questions": self.max_questions,
            "question_cursor": self.question_cursor,
            "history": list(self.history),
            "last_draft_refresh_at_question": self.last_draft_refresh_at_question,
            "current_feature": dict(self.current_feature) if self.current_feature else None,
            "output_paths": dict(self.output_paths),
            "description": self.description,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "SpecInterviewState":
        history_raw = data.get("history", [])
        history: list[dict[str, str]] = []
        if isinstance(history_raw, list):
            for h in history_raw:
                if not isinstance(h, dict):
                    continue
                q = str(h.get("question", "")).strip()
                a = str(h.get("answer", "")).strip()
                ts = str(h.get("ts", "")).strip() or utc_now_iso()
                if q:
                    history.append({"ts": ts, "question": q, "answer": a})

        current_feature = data.get("current_feature")
        if not isinstance(current_feature, dict):
            current_feature = None

        output_paths = _as_dict(data.get("output_paths"))

        return cls(
            schema_version=str(data.get("schema_version", SPEC_INTERVIEW_SCHEMA_VERSION)),
            status=str(data.get("status", "in_progress")),
            mode=str(data.get("mode", "on")),
            flow_level=str(data.get("flow_level", "standard")),
            first_principles=bool(data.get("first_principles", False)),
            max_questions=int(data.get("max_questions", 18) or 18),
            question_cursor=int(data.get("question_cursor", 0) or 0),
            history=history,
            last_draft_refresh_at_question=int(data.get("last_draft_refresh_at_question", 0) or 0),
            current_feature={str(k): str(v) for k, v in current_feature.items()} if current_feature else None,
            output_paths={str(k): str(v) for k, v in output_paths.items()},
            description=str(data.get("description", "")),
        )

