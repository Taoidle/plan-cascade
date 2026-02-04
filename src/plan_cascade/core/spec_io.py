#!/usr/bin/env python3
"""
Spec I/O helpers for Plan Cascade.

Writes/reads:
- spec.json
- spec.md
- .state/spec-interview.json
"""

from __future__ import annotations

import json
import os
from dataclasses import dataclass
from pathlib import Path
from uuid import uuid4

from .spec_models import Spec, SpecInterviewState


@dataclass(frozen=True)
class SpecPaths:
    """Resolved file paths for spec artifacts."""

    output_dir: Path
    spec_json_path: Path
    spec_md_path: Path
    interview_state_path: Path
    prd_json_path: Path


def resolve_prd_path(project_root: Path) -> Path:
    """
    Resolve the correct prd.json path for a project/worktree.

    - Legacy projects: <project_root>/prd.json
    - Migrated projects: <user-dir>/<project-id>/prd.json
    """
    from ..state.path_resolver import PathResolver, detect_project_mode

    mode = detect_project_mode(project_root)
    resolver = PathResolver(Path(project_root), legacy_mode=(mode == "legacy"))
    return resolver.get_prd_path()


def get_spec_paths(output_dir: Path) -> SpecPaths:
    out = Path(output_dir).resolve()
    return SpecPaths(
        output_dir=out,
        spec_json_path=out / "spec.json",
        spec_md_path=out / "spec.md",
        interview_state_path=out / ".state" / "spec-interview.json",
        prd_json_path=out / "prd.json",
    )


def _atomic_write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_name(f"{path.name}.{uuid4().hex}.tmp")
    tmp_path.write_text(content, encoding="utf-8")
    os.replace(tmp_path, path)


def write_json(path: Path, data: dict) -> None:
    text = json.dumps(data, indent=2, ensure_ascii=False) + "\n"
    _atomic_write_text(path, text)


def read_json(path: Path) -> dict | None:
    if not path.exists():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return None


def save_spec(spec: Spec, paths: SpecPaths) -> None:
    write_json(paths.spec_json_path, spec.to_dict())


def load_spec(paths: SpecPaths) -> Spec | None:
    data = read_json(paths.spec_json_path)
    if not isinstance(data, dict):
        return None
    return Spec.from_dict(data)


def save_spec_md(markdown: str, paths: SpecPaths) -> None:
    _atomic_write_text(paths.spec_md_path, markdown.rstrip() + "\n")


def load_interview_state(paths: SpecPaths) -> SpecInterviewState | None:
    data = read_json(paths.interview_state_path)
    if not isinstance(data, dict):
        return None
    return SpecInterviewState.from_dict(data)


def save_interview_state(state: SpecInterviewState, paths: SpecPaths) -> None:
    write_json(paths.interview_state_path, state.to_dict())
