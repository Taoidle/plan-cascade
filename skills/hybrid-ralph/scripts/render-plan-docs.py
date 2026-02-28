#!/usr/bin/env python3
"""
Render Plan Documents to Markdown

Programmatically converts JSON plan files to human-readable Markdown.
Non-LLM, deterministic conversion — output stays in sync with JSON.

Usage:
  python render-plan-docs.py --mode hybrid --project-root .
  python render-plan-docs.py --mode mega --project-root .
"""

import argparse
import json
import sys
from pathlib import Path


def _force_utf8_stdio() -> None:
    for stream_name in ("stdout", "stderr"):
        stream = getattr(sys, stream_name, None)
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except Exception:
            pass


_force_utf8_stdio()


def get_path_resolver(project_root: Path):
    """Get PathResolver if available, otherwise return None."""
    try:
        sys.path.insert(0, str(Path(__file__).parent.parent.parent.parent / "src"))
        from plan_cascade.state.path_resolver import PathResolver
        return PathResolver(project_root)
    except ImportError:
        return None


def load_json_file(filepath: Path) -> dict | None:
    """Load a JSON file if it exists."""
    if not filepath.exists():
        return None
    try:
        with open(filepath, encoding="utf-8-sig") as f:
            return json.load(f)
    except (json.JSONDecodeError, OSError):
        return None


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _render_list(items: list[str], prefix: str = "- ") -> str:
    if not items:
        return "_(none)_\n"
    return "\n".join(f"{prefix}{item}" for item in items) + "\n"


def _safe_str(value, default: str = "") -> str:
    if value is None:
        return default
    return str(value).strip()


def _safe_list(value) -> list:
    if isinstance(value, list):
        return value
    if isinstance(value, str):
        return [s.strip() for s in value.split(",") if s.strip()]
    return []


def _truncate(text: str, max_len: int) -> str:
    if len(text) <= max_len:
        return text
    return text[:max_len - 3] + "..."


# ---------------------------------------------------------------------------
# Batch calculation (same algorithm as unified-review.py)
# ---------------------------------------------------------------------------

def calculate_batches(items: list, id_key: str = "id") -> list[list]:
    if not items:
        return []
    completed: set[str] = set()
    batches: list[list] = []
    while len(completed) < len(items):
        ready = []
        for item in items:
            item_id = item[id_key]
            if item_id in completed:
                continue
            deps = item.get("dependencies", [])
            if all(dep in completed for dep in deps):
                ready.append(item)
        if not ready:
            ready = [item for item in items if item[id_key] not in completed]
        priority_order = {"high": 0, "medium": 1, "low": 2}
        ready.sort(key=lambda x: priority_order.get(x.get("priority", "medium"), 1))
        batches.append(ready)
        completed.update(item[id_key] for item in ready)
    return batches


# ---------------------------------------------------------------------------
# PRD -> prd.md
# ---------------------------------------------------------------------------

def render_prd_md(prd: dict) -> str:
    md: list[str] = []

    goal = _safe_str(prd.get("goal"), "Untitled")
    metadata = prd.get("metadata", {})
    stories = prd.get("stories", [])
    objectives = _safe_list(prd.get("objectives"))
    batches = calculate_batches(stories)

    md.append(f"# PRD: {goal}\n")
    md.append(f"> Generated: {metadata.get('created_at', 'N/A')} | Stories: {len(stories)} | Batches: {len(batches)}\n")

    # Objectives
    if objectives:
        md.append("## Objectives\n")
        md.append(_render_list([str(o).strip() for o in objectives if str(o).strip()]))

    # User Stories
    md.append("## User Stories\n")
    if not stories:
        md.append("_(none)_\n")
    else:
        for story in stories:
            sid = story.get("id", "story-???")
            title = _safe_str(story.get("title"), "Untitled")
            priority = story.get("priority", "medium").upper()
            description = _safe_str(story.get("description"))
            deps = story.get("dependencies", [])
            ac = _safe_list(story.get("acceptance_criteria"))
            size = story.get("context_estimate", "")

            md.append(f"### {sid}: {title} [{priority}]\n")
            if description:
                md.append(f"{description}\n")

            if ac:
                md.append("**Acceptance Criteria:**\n")
                md.append(_render_list([f"[ ] {c}" for c in ac]))

            if deps:
                md.append(f"**Dependencies:** {', '.join(deps)}\n")
            if size:
                md.append(f"**Estimated Size:** {size}\n")

            md.append("---\n")

    # Execution Plan
    md.append("## Execution Plan\n")
    for i, batch in enumerate(batches, 1):
        ids = ", ".join(item["id"] for item in batch)
        parallel = " (Parallel)" if len(batch) > 1 else ""
        md.append(f"- Batch {i}{parallel}: {ids}")
    md.append("")

    return "\n".join(line.rstrip() for line in md).rstrip() + "\n"


# ---------------------------------------------------------------------------
# Design Doc -> design_doc.md
# ---------------------------------------------------------------------------

def render_design_doc_md(doc: dict) -> str:
    md: list[str] = []

    metadata = doc.get("metadata", {})
    overview = doc.get("overview", {})
    arch = doc.get("architecture", {})
    interfaces = doc.get("interfaces", {})
    decisions = doc.get("decisions", [])

    components = arch.get("components", [])
    patterns = arch.get("patterns", [])

    level = metadata.get("level", "feature")
    title = _safe_str(overview.get("title"), "Technical Design")

    md.append(f"# Technical Design: {title}\n")
    md.append(f"> Level: {level} | Components: {len(components)} | Patterns: {len(patterns)} | ADRs: {len(decisions)}\n")

    # Overview
    summary = _safe_str(overview.get("summary"))
    goals = _safe_list(overview.get("goals"))
    non_goals = _safe_list(overview.get("non_goals"))

    if summary:
        md.append("## Overview\n")
        md.append(f"{summary}\n")

    if goals:
        md.append("### Goals\n")
        md.append(_render_list([str(g).strip() for g in goals if str(g).strip()]))

    if non_goals:
        md.append("### Non-Goals\n")
        md.append(_render_list([str(g).strip() for g in non_goals if str(g).strip()]))

    # Architecture
    md.append("## Architecture\n")

    # System overview (mega/project-level)
    system_overview = _safe_str(arch.get("system_overview"))
    if system_overview:
        md.append(f"{system_overview}\n")

    # Components
    if components:
        md.append("### Components\n")
        for comp in components:
            name = _safe_str(comp.get("name"), "Component")
            desc = _safe_str(comp.get("description"))
            responsibilities = _safe_list(comp.get("responsibilities"))
            deps = _safe_list(comp.get("dependencies"))
            files = _safe_list(comp.get("files"))

            md.append(f"#### {name}\n")
            if desc:
                md.append(f"{desc}\n")
            if responsibilities:
                md.append("- Responsibilities: " + ", ".join(str(r) for r in responsibilities))
            if deps:
                md.append("- Dependencies: " + ", ".join(str(d) for d in deps))
            if files:
                md.append("- Files: " + ", ".join(f"`{f}`" for f in files))
            md.append("")

    # Patterns
    if patterns:
        md.append("### Patterns\n")
        for p in patterns:
            name = _safe_str(p.get("name"), "Pattern")
            desc = _safe_str(p.get("description"))
            rationale = _safe_str(p.get("rationale"))
            line = f"- **{name}**: {desc}"
            if rationale:
                line += f" — _{rationale}_"
            md.append(line)
        md.append("")

    # Data Flow
    data_flow = _safe_str(arch.get("data_flow"))
    if data_flow:
        md.append("### Data Flow\n")
        md.append(f"{data_flow}\n")

    # Interfaces
    apis = interfaces.get("apis", [])
    data_models = interfaces.get("data_models", [])
    # Also check for shared_data_models (mega-level)
    if not data_models:
        data_models = interfaces.get("shared_data_models", [])

    if apis or data_models:
        md.append("## Interfaces\n")

    if apis:
        md.append("### APIs\n")
        md.append("| Method | Path | Description |")
        md.append("|--------|------|-------------|")
        for api in apis:
            method = _safe_str(api.get("method"), "GET")
            path = _safe_str(api.get("path"), "/")
            desc = _safe_str(api.get("description"))
            md.append(f"| {method} | {path} | {desc} |")
        md.append("")

    if data_models:
        md.append("### Data Models\n")
        for model in data_models:
            if isinstance(model, dict):
                name = _safe_str(model.get("name"), "Model")
                desc = _safe_str(model.get("description"))
                fields = model.get("fields", {})

                md.append(f"#### {name}\n")
                if desc:
                    md.append(f"{desc}\n")

                if isinstance(fields, dict) and fields:
                    md.append("| Field | Type |")
                    md.append("|-------|------|")
                    for field_name, field_type in fields.items():
                        md.append(f"| {field_name} | {field_type} |")
                    md.append("")
                elif isinstance(fields, list) and fields:
                    md.append("| Field | Type |")
                    md.append("|-------|------|")
                    for f in fields:
                        if isinstance(f, dict):
                            md.append(f"| {f.get('name', '')} | {f.get('type', '')} |")
                        else:
                            md.append(f"| {f} | - |")
                    md.append("")

    # Architecture Decisions (ADRs)
    if decisions:
        md.append("## Architecture Decisions\n")
        for d in decisions:
            adr_id = d.get("id", "ADR-???")
            adr_title = _safe_str(d.get("title"), "Untitled")
            status = d.get("status", "proposed")
            context = _safe_str(d.get("context"))
            decision = _safe_str(d.get("decision"))
            rationale = _safe_str(d.get("rationale"))
            alternatives = _safe_list(d.get("alternatives_considered"))

            md.append(f"### {adr_id}: {adr_title} [{status}]\n")
            if context:
                md.append(f"- **Context**: {context}")
            if decision:
                md.append(f"- **Decision**: {decision}")
            if rationale:
                md.append(f"- **Rationale**: {rationale}")
            if alternatives:
                md.append(f"- **Alternatives Considered**: {', '.join(str(a) for a in alternatives)}")
            md.append("")

    # Story/Feature Mappings
    story_mappings = doc.get("story_mappings", {})
    feature_mappings = doc.get("feature_mappings", {})
    mappings = story_mappings or feature_mappings

    if mappings:
        md.append("## Story Mappings\n")
        md.append("| Story | Components | Decisions | Interfaces |")
        md.append("|-------|-----------|-----------|------------|")
        for item_id, mapping in mappings.items():
            comps = ", ".join(mapping.get("components", [])) or "-"
            decs = ", ".join(mapping.get("decisions", [])) or "-"
            intfs = ", ".join(mapping.get("interfaces", [])) or "-"
            md.append(f"| {item_id} | {comps} | {decs} | {intfs} |")
        md.append("")

    return "\n".join(line.rstrip() for line in md).rstrip() + "\n"


# ---------------------------------------------------------------------------
# Mega Plan -> mega-plan.md
# ---------------------------------------------------------------------------

def render_mega_plan_md(mega: dict) -> str:
    md: list[str] = []

    goal = _safe_str(mega.get("goal"), "Untitled")
    metadata = mega.get("metadata", {})
    features = mega.get("features", [])
    execution_mode = mega.get("execution_mode", "auto")
    batches = calculate_batches(features)

    md.append(f"# Mega Plan: {goal}\n")
    md.append(f"> Features: {len(features)} | Batches: {len(batches)} | Mode: {execution_mode}\n")

    # Features
    md.append("## Features\n")
    if not features:
        md.append("_(none)_\n")
    else:
        for feature in features:
            fid = feature.get("id", "feature-???")
            title = _safe_str(feature.get("title"), "Untitled")
            priority = feature.get("priority", "medium").upper()
            description = _safe_str(feature.get("description"))
            deps = feature.get("dependencies", [])

            md.append(f"### {fid}: {title} [{priority}]\n")
            if description:
                md.append(f"{description}\n")
            if deps:
                md.append(f"**Dependencies:** {', '.join(deps)}\n")
            md.append("---\n")

    # Execution Batches
    md.append("## Execution Batches\n")
    for i, batch in enumerate(batches, 1):
        ids = ", ".join(item["id"] for item in batch)
        parallel = " (Parallel)" if len(batch) > 1 else ""
        suffix = ""
        if i < len(batches):
            suffix = " (merge to target branch)"
        md.append(f"- Batch {i}{parallel}: {ids}{suffix}")
    md.append("")

    return "\n".join(line.rstrip() for line in md).rstrip() + "\n"


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="Render plan JSON files to human-readable Markdown"
    )
    parser.add_argument(
        "--mode",
        choices=["hybrid", "mega"],
        default="hybrid",
        help="Mode: hybrid (prd+design_doc) or mega (mega-plan+design_doc)",
    )
    parser.add_argument(
        "--project-root",
        type=Path,
        default=Path.cwd(),
        help="Project root directory (default: cwd)",
    )
    args = parser.parse_args()

    project_root = args.project_root.resolve()
    resolver = get_path_resolver(project_root)
    output_files: list[str] = []

    if args.mode == "mega":
        # Load mega-plan.json
        if resolver:
            mega_path = resolver.get_mega_plan_path()
        else:
            mega_path = project_root / "mega-plan.json"
        mega = load_json_file(mega_path)
        if not mega:
            mega = load_json_file(project_root / "mega-plan.json")
        if not mega:
            print("Error: mega-plan.json not found", file=sys.stderr)
            sys.exit(1)

        # Render mega-plan.md
        mega_md = render_mega_plan_md(mega)
        out_path = project_root / "mega-plan.md"
        out_path.write_text(mega_md, encoding="utf-8")
        output_files.append(str(out_path))

    else:
        # Load prd.json
        if resolver:
            prd_path = resolver.get_prd_path()
        else:
            prd_path = project_root / "prd.json"
        prd = load_json_file(prd_path)
        if not prd:
            prd = load_json_file(project_root / "prd.json")
        if not prd:
            print("Error: prd.json not found", file=sys.stderr)
            sys.exit(1)

        # Render prd.md
        prd_md = render_prd_md(prd)
        out_path = project_root / "prd.md"
        out_path.write_text(prd_md, encoding="utf-8")
        output_files.append(str(out_path))

    # Load and render design_doc.json (both modes)
    design_doc = load_json_file(project_root / "design_doc.json")
    if design_doc:
        design_md = render_design_doc_md(design_doc)
        out_path = project_root / "design_doc.md"
        out_path.write_text(design_md, encoding="utf-8")
        output_files.append(str(out_path))
    else:
        print("Warning: design_doc.json not found, skipping design_doc.md", file=sys.stderr)

    # Output generated file paths (one per line)
    for f in output_files:
        print(f)


if __name__ == "__main__":
    main()
