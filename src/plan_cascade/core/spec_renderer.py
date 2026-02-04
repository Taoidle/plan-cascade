#!/usr/bin/env python3
"""Render spec.json into a human-readable spec.md."""

from __future__ import annotations

from .spec_models import Spec


def _render_list(items: list[str], prefix: str = "- ") -> str:
    if not items:
        return "_(none)_\n"
    return "\n".join(f"{prefix}{item}" for item in items) + "\n"


def render_spec_md(spec: Spec) -> str:
    spec.ensure_defaults()

    overview = spec.overview or {}
    scope = spec.scope or {}
    reqs = spec.requirements or {}
    nfr = reqs.get("non_functional") or {}
    interfaces = spec.interfaces or {}

    title = overview.get("title") or "Specification"
    goal = overview.get("goal") or ""
    problem = overview.get("problem") or ""

    success_metrics = overview.get("success_metrics") or []
    if isinstance(success_metrics, str):
        success_metrics = [s.strip() for s in success_metrics.split(",") if s.strip()]
    if not isinstance(success_metrics, list):
        success_metrics = []

    non_goals = overview.get("non_goals") or []
    if isinstance(non_goals, str):
        non_goals = [s.strip() for s in non_goals.split(",") if s.strip()]
    if not isinstance(non_goals, list):
        non_goals = []

    md: list[str] = []

    md.append(f"# Spec: {title}\n")
    md.append(f"**Schema:** `{spec.metadata.get('schema_version', '')}`  \n")
    md.append(f"**Created:** `{spec.metadata.get('created_at', '')}`  \n")
    md.append(f"**Updated:** `{spec.metadata.get('updated_at', '')}`\n")

    if goal:
        md.append("## Goal\n")
        md.append(f"{goal}\n")

    if problem:
        md.append("## Problem\n")
        md.append(f"{problem}\n")

    md.append("## Success Metrics\n")
    md.append(_render_list([str(s).strip() for s in success_metrics if str(s).strip()]))

    md.append("## Non-goals\n")
    md.append(_render_list([str(s).strip() for s in non_goals if str(s).strip()]))

    md.append("## Scope\n")
    md.append("### In scope\n")
    md.append(_render_list([str(s).strip() for s in (scope.get("in_scope") or []) if str(s).strip()]))
    md.append("### Out of scope\n")
    md.append(_render_list([str(s).strip() for s in (scope.get("out_of_scope") or []) if str(s).strip()]))
    md.append("### Do not touch\n")
    md.append(_render_list([str(s).strip() for s in (scope.get("do_not_touch") or []) if str(s).strip()]))
    md.append("### Assumptions\n")
    md.append(_render_list([str(s).strip() for s in (scope.get("assumptions") or []) if str(s).strip()]))

    md.append("## Requirements\n")
    md.append("### Functional\n")
    md.append(_render_list([str(s).strip() for s in (reqs.get("functional") or []) if str(s).strip()]))

    md.append("### Non-functional\n")
    md.append("#### Performance targets\n")
    md.append(_render_list([str(s).strip() for s in (nfr.get("performance_targets") or []) if str(s).strip()]))
    md.append("#### Security\n")
    md.append(_render_list([str(s).strip() for s in (nfr.get("security") or []) if str(s).strip()]))
    md.append("#### Reliability\n")
    md.append(_render_list([str(s).strip() for s in (nfr.get("reliability") or []) if str(s).strip()]))
    md.append("#### Scalability\n")
    md.append(_render_list([str(s).strip() for s in (nfr.get("scalability") or []) if str(s).strip()]))
    md.append("#### Accessibility\n")
    md.append(_render_list([str(s).strip() for s in (nfr.get("accessibility") or []) if str(s).strip()]))

    md.append("## Interfaces\n")
    api = interfaces.get("api") or []
    if not isinstance(api, list):
        api = []
    data_models = interfaces.get("data_models") or []
    if not isinstance(data_models, list):
        data_models = []

    md.append("### API\n")
    if api:
        for item in api:
            if isinstance(item, dict):
                name = item.get("name", "")
                notes = item.get("notes", "")
                md.append(f"- `{name}`{f' — {notes}' if notes else ''}")
            else:
                md.append(f"- {item}")
        md.append("")
    else:
        md.append("_(none)_\n")

    md.append("### Data models\n")
    if data_models:
        for item in data_models:
            if isinstance(item, dict):
                name = item.get("name", "")
                fields = item.get("fields", [])
                if not isinstance(fields, list):
                    fields = []
                md.append(f"- `{name}`: {', '.join(str(f) for f in fields)}")
            else:
                md.append(f"- {item}")
        md.append("")
    else:
        md.append("_(none)_\n")

    if spec.phases:
        md.append("## Phases\n")
        for phase in spec.phases:
            if not isinstance(phase, dict):
                continue
            name = phase.get("name", "phase")
            md.append(f"### {name}\n")
            cmds = phase.get("verification_commands") or []
            if not isinstance(cmds, list):
                cmds = []
            if cmds:
                md.append("**Verification commands:**\n")
                md.append(_render_list([str(c).strip() for c in cmds if str(c).strip()]))
            stories = phase.get("stories") or []
            if not isinstance(stories, list):
                stories = []
            if stories:
                md.append("**Stories:**\n")
                md.append(_render_list([str(s).strip() for s in stories if str(s).strip()]))

    md.append("## Stories\n")
    if not spec.stories:
        md.append("_(none)_\n")
    else:
        for story in spec.stories:
            md.append(f"### {story.id}: {story.title}\n")
            md.append(f"- **Category:** `{story.category}`")
            if story.priority:
                md.append(f"- **Priority:** `{story.priority}`")
            md.append(f"- **Context estimate:** `{story.context_estimate}`")
            if story.dependencies:
                md.append(f"- **Dependencies:** {', '.join(story.dependencies)}")
            md.append("")
            if story.description:
                md.append(story.description.strip() + "\n")
            md.append("**Acceptance criteria:**\n")
            md.append(_render_list(story.acceptance_criteria))
            verification = story.verification or {}
            commands = verification.get("commands") or []
            manual_steps = verification.get("manual_steps") or []
            if not isinstance(commands, list):
                commands = []
            if not isinstance(manual_steps, list):
                manual_steps = []
            md.append("**Verification commands:**\n")
            md.append(_render_list([str(c).strip() for c in commands if str(c).strip()]))
            if manual_steps:
                md.append("**Manual verification:**\n")
                md.append(_render_list([str(s).strip() for s in manual_steps if str(s).strip()]))
            md.append("---\n")

    md.append("## Open Questions\n")
    md.append(_render_list(spec.open_questions))

    if spec.decision_log:
        md.append("## Decision Log\n")
        for item in spec.decision_log:
            if not isinstance(item, dict):
                continue
            q = item.get("question", "")
            a = item.get("answer", "")
            if q:
                md.append(f"- **Q:** {q}")
                md.append(f"  - **A:** {a}")
        md.append("")

    return "\n".join(line.rstrip() for line in md).rstrip() + "\n"

