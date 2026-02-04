#!/usr/bin/env python3
"""
Spec Interview CLI commands for Plan Cascade.

Provides a resumable, interactive planning-time interview that produces:
- spec.json (structured spec)
- spec.md (rendered human-readable spec)
- prd.json (compiled Plan Cascade PRD)
"""

from __future__ import annotations

from pathlib import Path
from typing import Optional

try:
    import typer
    from rich.console import Console
    from rich.prompt import Confirm, Prompt

    HAS_TYPER = True
except ImportError:
    HAS_TYPER = False

from ..core.spec_compiler import CompileOptions, compile_spec_to_prd
from ..core.spec_io import (
    SpecPaths,
    get_spec_paths,
    load_interview_state,
    load_spec,
    save_interview_state,
    save_spec,
    save_spec_md,
)
from ..core.spec_models import Spec, SpecInterviewState, SpecStory, utc_now_iso
from ..core.spec_quality_gate import check_spec_quality
from ..core.spec_renderer import render_spec_md


def _parse_list(text: str) -> list[str]:
    text = (text or "").strip()
    if not text:
        return []
    # Allow comma or newline separated
    parts = []
    for line in text.replace("\r\n", "\n").split("\n"):
        parts.extend(p.strip() for p in line.split(","))
    return [p for p in parts if p]


def _ask_list(
    prompt: str,
    console: "Console",
    default: list[str] | None = None,
    required: bool = False,
) -> list[str]:
    default = default or []
    default_str = ", ".join(default) if default else ""
    while True:
        ans = Prompt.ask(prompt, default=default_str, console=console).strip()
        items = _parse_list(ans)
        if items or not required:
            return items
        console.print("[red]At least 1 item is required.[/red]")


def _ask_yes_no(prompt: str, console: "Console", default: bool = True) -> bool:
    return Confirm.ask(prompt, default=default, console=console)


def _ask_story_id(default: str, console: "Console", existing_ids: set[str]) -> str:
    while True:
        story_id = Prompt.ask("Story id", default=default, console=console).strip()
        if not story_id:
            console.print("[red]Story id is required.[/red]")
            continue
        if story_id in existing_ids and story_id != default:
            console.print("[red]Story id must be unique.[/red]")
            continue
        return story_id


def _prompt_story(
    console: "Console",
    story: SpecStory | None,
    index: int,
    flow_level: str,
    existing_ids: set[str],
) -> SpecStory:
    default_id = story.id if story else f"story-{index:03d}"
    story_id = _ask_story_id(default_id, console, existing_ids)
    existing_ids.add(story_id)

    category = Prompt.ask(
        "Category",
        default=(story.category if story else "core"),
        choices=["setup", "core", "integration", "polish", "test"],
        console=console,
    ).strip()

    title = Prompt.ask("Title", default=(story.title if story else ""), console=console).strip()
    description = Prompt.ask(
        "Description",
        default=(story.description if story else title),
        console=console,
    ).strip()

    priority_default = (story.priority if story else "") or ""
    priority_raw = Prompt.ask(
        "Priority (optional)",
        default=priority_default,
        console=console,
    ).strip()
    priority = priority_raw if priority_raw in ("high", "medium", "low") else (priority_raw or None)

    context_estimate = Prompt.ask(
        "Context estimate",
        default=(story.context_estimate if story else "medium"),
        choices=["small", "medium", "large", "xlarge"],
        console=console,
    ).strip()

    deps_default = ", ".join(story.dependencies) if story else ""
    deps = _parse_list(Prompt.ask("Dependencies (comma separated)", default=deps_default, console=console))

    # Acceptance criteria
    if story and story.acceptance_criteria:
        if _ask_yes_no("Replace acceptance criteria list?", console, default=False):
            acceptance_criteria: list[str] = []
        else:
            acceptance_criteria = list(story.acceptance_criteria)
    else:
        acceptance_criteria = []

    while True:
        if not acceptance_criteria:
            console.print("[dim]Add acceptance criteria (blank to finish)[/dim]")
        item = Prompt.ask("AC", default="", console=console).strip()
        if not item:
            break
        acceptance_criteria.append(item)

    # Verification commands
    verification = dict(story.verification) if story else {"commands": [], "manual_steps": []}
    commands_existing = verification.get("commands") or []
    if not isinstance(commands_existing, list):
        commands_existing = []
    if commands_existing and _ask_yes_no("Replace verification commands list?", console, default=False):
        commands: list[str] = []
    else:
        commands = [str(c).strip() for c in commands_existing if str(c).strip()]

    while True:
        if not commands:
            console.print("[dim]Add verification commands (blank to finish)[/dim]")
        cmd = Prompt.ask("Cmd", default="", console=console).strip()
        if not cmd:
            break
        commands.append(cmd)

    manual_steps_existing = verification.get("manual_steps") or []
    if not isinstance(manual_steps_existing, list):
        manual_steps_existing = []
    manual_steps = [str(s).strip() for s in manual_steps_existing if str(s).strip()]

    test_required_default = False
    if story and story.test_expectations and isinstance(story.test_expectations, dict):
        test_required_default = bool(story.test_expectations.get("required", False))
    test_required = _ask_yes_no("Tests required for this story?", console, default=test_required_default)
    coverage_areas: list[str] = []
    if test_required:
        existing_cov = []
        if story and story.test_expectations and isinstance(story.test_expectations, dict):
            cov = story.test_expectations.get("coverage_areas") or []
            if isinstance(cov, list):
                existing_cov = [str(c).strip() for c in cov if str(c).strip()]
        coverage_areas = _ask_list(
            "Coverage areas (comma/newline separated)",
            console,
            default=existing_cov,
            required=False,
        )

    test_expectations = {"required": test_required}
    if coverage_areas:
        test_expectations["coverage_areas"] = coverage_areas

    # FULL flow: enforce minimum completeness
    if flow_level == "full":
        if len(acceptance_criteria) < 2:
            console.print("[yellow]FULL flow requires >= 2 acceptance criteria.[/yellow]")
        if not commands and not manual_steps:
            console.print("[yellow]FULL flow requires at least 1 verification command.[/yellow]")

    return SpecStory(
        id=story_id,
        category=category,
        title=title,
        description=description,
        acceptance_criteria=acceptance_criteria,
        verification={"commands": commands, "manual_steps": manual_steps},
        test_expectations=test_expectations,
        dependencies=deps,
        context_estimate=context_estimate,
        priority=priority,
    )


def _refresh_drafts(spec: Spec, state: SpecInterviewState, paths: SpecPaths) -> None:
    spec.ensure_defaults()
    save_spec(spec, paths)
    save_spec_md(render_spec_md(spec), paths)
    state.last_draft_refresh_at_question = state.question_cursor
    state.output_paths = {
        "spec_json": str(paths.spec_json_path),
        "spec_md": str(paths.spec_md_path),
    }
    save_interview_state(state, paths)


def run_spec_interview(
    *,
    description: str,
    output_dir: Path,
    flow_level: str = "standard",
    mode: str = "on",
    first_principles: bool = False,
    max_questions: int = 18,
    feature_slug: str | None = None,
    title_hint: str | None = None,
    console: "Console",
) -> int:
    """
    Run interactive interview, writing spec.json/spec.md and interview state.

    Returns a process-style exit code (0 success, 1 errors).
    """
    paths = get_spec_paths(output_dir)
    paths.interview_state_path.parent.mkdir(parents=True, exist_ok=True)

    existing_state = load_interview_state(paths)
    existing_spec = load_spec(paths)

    if existing_state and existing_state.status == "in_progress":
        if not Confirm.ask(
            f"Existing interview state found in {paths.interview_state_path}. Resume?",
            default=True,
            console=console,
        ):
            existing_state = None

    spec = existing_spec or Spec()
    spec.ensure_defaults()

    # Ensure metadata baseline
    spec.metadata["schema_version"] = spec.metadata.get("schema_version") or "spec-0.1"
    spec.metadata["source"] = "spec-interview"
    spec.metadata["description"] = description
    if feature_slug:
        spec.metadata["feature_slug"] = feature_slug

    state = existing_state or SpecInterviewState(
        status="in_progress",
        mode=mode,
        flow_level=flow_level,
        first_principles=first_principles,
        max_questions=max_questions,
        question_cursor=0,
        last_draft_refresh_at_question=0,
        output_paths={},
        description=description,
    )

    # ========== Overview ==========
    console.print()
    console.print("[bold]Spec Interview[/bold]")
    console.print(f"[dim]Output dir:[/dim] {paths.output_dir}")
    console.print(f"[dim]Flow:[/dim] {flow_level}  [dim]First principles:[/dim] {first_principles}")
    console.print()

    def record(question: str, answer: str) -> None:
        state.question_cursor += 1
        state.history.append({"ts": utc_now_iso(), "question": question, "answer": answer})
        if state.question_cursor % 3 == 0:
            _refresh_drafts(spec, state, paths)
        else:
            save_interview_state(state, paths)

    overview = spec.overview
    overview["title"] = Prompt.ask(
        "Title",
        default=str(overview.get("title") or title_hint or "").strip(),
        console=console,
    ).strip()
    record("Title", str(overview["title"]))

    if first_principles:
        overview["problem"] = Prompt.ask(
            "Core problem (first principles)",
            default=str(overview.get("problem", "")).strip(),
            console=console,
        ).strip()
        record("Core problem", str(overview["problem"]))

    overview["goal"] = Prompt.ask(
        "Goal (one sentence)",
        default=str(overview.get("goal", "")).strip() or description,
        console=console,
    ).strip()
    record("Goal", str(overview["goal"]))

    overview["success_metrics"] = _ask_list(
        "Success metrics (comma/newline separated)",
        console,
        default=_parse_list(str(overview.get("success_metrics", ""))),
        required=(flow_level == "full"),
    )
    record("Success metrics", ", ".join(overview["success_metrics"]))

    overview["non_goals"] = _ask_list(
        "Non-goals / out of scope items (comma/newline separated)",
        console,
        default=_parse_list(str(overview.get("non_goals", ""))),
        required=(flow_level == "full"),
    )
    record("Non-goals", ", ".join(overview["non_goals"]))

    # ========== Scope ==========
    scope = spec.scope
    scope["in_scope"] = _ask_list(
        "In scope",
        console,
        default=_parse_list(str(scope.get("in_scope", ""))),
        required=False,
    )
    record("In scope", ", ".join(scope["in_scope"]))

    scope["out_of_scope"] = _ask_list(
        "Out of scope",
        console,
        default=_parse_list(str(scope.get("out_of_scope", ""))),
        required=(flow_level == "full"),
    )
    record("Out of scope", ", ".join(scope["out_of_scope"]))

    scope["do_not_touch"] = _ask_list(
        "Do not touch (modules/files/components)",
        console,
        default=_parse_list(str(scope.get("do_not_touch", ""))),
        required=False,
    )
    record("Do not touch", ", ".join(scope["do_not_touch"]))

    scope["assumptions"] = _ask_list(
        "Assumptions",
        console,
        default=_parse_list(str(scope.get("assumptions", ""))),
        required=False,
    )
    record("Assumptions", ", ".join(scope["assumptions"]))

    # ========== Requirements ==========
    reqs = spec.requirements
    reqs["functional"] = _ask_list(
        "Functional requirements",
        console,
        default=_parse_list(str(reqs.get("functional", ""))),
        required=False,
    )
    record("Functional requirements", ", ".join(reqs["functional"]))

    nfr = reqs.get("non_functional")
    if not isinstance(nfr, dict):
        nfr = {}
    reqs["non_functional"] = nfr

    nfr["performance_targets"] = _ask_list(
        "Non-functional: performance targets",
        console,
        default=_parse_list(str(nfr.get("performance_targets", ""))),
        required=False,
    )
    record("Performance targets", ", ".join(nfr["performance_targets"]))

    nfr["security"] = _ask_list(
        "Non-functional: security expectations",
        console,
        default=_parse_list(str(nfr.get("security", ""))),
        required=False,
    )
    record("Security expectations", ", ".join(nfr["security"]))

    nfr["reliability"] = _ask_list(
        "Non-functional: reliability expectations",
        console,
        default=_parse_list(str(nfr.get("reliability", ""))),
        required=False,
    )
    record("Reliability expectations", ", ".join(nfr["reliability"]))

    nfr["scalability"] = _ask_list(
        "Non-functional: scalability expectations",
        console,
        default=_parse_list(str(nfr.get("scalability", ""))),
        required=False,
    )
    record("Scalability expectations", ", ".join(nfr["scalability"]))

    nfr["accessibility"] = _ask_list(
        "Non-functional: accessibility expectations",
        console,
        default=_parse_list(str(nfr.get("accessibility", ""))),
        required=False,
    )
    record("Accessibility expectations", ", ".join(nfr["accessibility"]))

    # ========== Interfaces ==========
    interfaces = spec.interfaces
    api_list = interfaces.get("api")
    if not isinstance(api_list, list):
        api_list = []

    if _ask_yes_no("Edit API endpoints?", console, default=not bool(api_list)):
        api_list = []
        console.print("[dim]Add API endpoints in the form 'METHOD /path - notes' (blank to finish)[/dim]")
        while True:
            raw = Prompt.ask("API", default="", console=console).strip()
            if not raw:
                break
            name, notes = raw, ""
            if " - " in raw:
                name, notes = raw.split(" - ", 1)
            api_list.append({"name": name.strip(), "notes": notes.strip()})
        interfaces["api"] = api_list
        record("API endpoints", "; ".join([x.get("name", "") for x in api_list if isinstance(x, dict)]))

    data_models = interfaces.get("data_models")
    if not isinstance(data_models, list):
        data_models = []

    if _ask_yes_no("Edit data models?", console, default=not bool(data_models)):
        data_models = []
        console.print("[dim]Add data models in the form 'ModelName: field1, field2' (blank to finish)[/dim]")
        while True:
            raw = Prompt.ask("Model", default="", console=console).strip()
            if not raw:
                break
            name, fields_raw = raw, ""
            if ":" in raw:
                name, fields_raw = raw.split(":", 1)
            fields = _parse_list(fields_raw)
            data_models.append({"name": name.strip(), "fields": fields})
        interfaces["data_models"] = data_models
        record(
            "Data models",
            "; ".join([x.get("name", "") for x in data_models if isinstance(x, dict)]),
        )

    # ========== Stories ==========
    console.print()
    console.print("[bold]Stories[/bold]")
    console.print("[dim]You can keep/edit existing stories, or rebuild the list.[/dim]")
    console.print()

    if spec.stories and not _ask_yes_no("Rebuild story list from scratch?", console, default=False):
        stories = list(spec.stories)
    else:
        stories = []

    if not stories:
        default_count = 3 if flow_level != "full" else 5
        count = int(Prompt.ask("How many stories to define?", default=str(default_count), console=console))
        for i in range(1, count + 1):
            console.print()
            console.print(f"[bold]Story {i}/{count}[/bold]")
            stories.append(_prompt_story(console, None, i, flow_level, existing_ids=set(s.id for s in stories)))
    else:
        # Edit existing stories one by one
        edited: list[SpecStory] = []
        existing_ids: set[str] = set()
        for i, s in enumerate(stories, 1):
            console.print()
            console.print(f"[bold]Edit story {i}/{len(stories)}[/bold]  [dim]{s.id}: {s.title}[/dim]")
            edited.append(_prompt_story(console, s, i, flow_level, existing_ids))
        stories = edited

    spec.stories = stories
    record("Stories updated", f"{len(stories)} story(ies)")

    # ========== Open questions ==========
    spec.open_questions = _ask_list(
        "Open questions (comma/newline separated)",
        console,
        default=_parse_list(str(spec.open_questions)),
        required=False,
    )
    record("Open questions", ", ".join(spec.open_questions))

    # Final refresh + decision log
    spec.decision_log = [{"question": h.get("question", ""), "answer": h.get("answer", "")} for h in state.history]
    _refresh_drafts(spec, state, paths)

    # Quality gate
    q = check_spec_quality(spec, flow_level=flow_level)
    if q.errors:
        console.print()
        console.print("[red]Spec quality checks failed:[/red]")
        for e in q.errors:
            console.print(f"  [red]-[/red] {e}")
        if q.warnings:
            console.print()
            console.print("[yellow]Warnings:[/yellow]")
            for w in q.warnings:
                console.print(f"  [yellow]-[/yellow] {w}")
        console.print()
        if not Confirm.ask("Keep spec anyway (do not block)?", default=False, console=console):
            return 1

    if q.warnings:
        console.print()
        console.print("[yellow]Spec warnings:[/yellow]")
        for w in q.warnings:
            console.print(f"  [yellow]-[/yellow] {w}")

    state.status = "finalized"
    save_interview_state(state, paths)

    console.print()
    console.print("[green]✓ Spec written[/green]")
    console.print(f"  - {paths.spec_json_path}")
    console.print(f"  - {paths.spec_md_path}")
    console.print(f"  - {paths.interview_state_path}")
    return 0


if HAS_TYPER:
    spec_app = typer.Typer(
        name="spec",
        help="Spec interview workflow (spec.json/spec.md -> prd.json)",
        no_args_is_help=True,
    )
    console = Console()

    @spec_app.command("plan")
    def plan(
        description: str = typer.Argument(..., help="Specification description / intent"),
        output_dir: Optional[str] = typer.Option(None, "--output-dir", help="Output directory (default: cwd)"),
        flow: str = typer.Option("standard", "--flow", help="Flow level (quick|standard|full)"),
        mode: str = typer.Option("on", "--mode", help="Record mode (off|auto|on)"),
        first_principles: bool = typer.Option(False, "--first-principles", help="Ask first-principles questions first"),
        max_questions: int = typer.Option(18, "--max-questions", help="Soft cap for interview length (record only)"),
        feature_slug: Optional[str] = typer.Option(None, "--feature-slug", help="Feature slug (mega feature name)"),
        title: Optional[str] = typer.Option(None, "--title", help="Title hint"),
    ):
        """
        Start (or resume) a spec interview.

        Produces spec.json/spec.md and a resumable .state/spec-interview.json.
        """
        out = Path(output_dir) if output_dir else Path.cwd()
        code = run_spec_interview(
            description=description,
            output_dir=out,
            flow_level=flow,
            mode=mode,
            first_principles=first_principles,
            max_questions=max_questions,
            feature_slug=feature_slug,
            title_hint=title,
            console=console,
        )
        raise typer.Exit(code)

    @spec_app.command("resume")
    def resume(
        output_dir: Optional[str] = typer.Option(None, "--output-dir", help="Output directory (default: cwd)"),
        flow: str = typer.Option("standard", "--flow", help="Flow level (quick|standard|full)"),
    ):
        """Resume an in-progress spec interview in the target directory."""
        out = Path(output_dir) if output_dir else Path.cwd()
        paths = get_spec_paths(out)
        state = load_interview_state(paths)
        if not state:
            console.print("[red]No .state/spec-interview.json found.[/red]")
            raise typer.Exit(1)
        description = state.description or "Resume spec interview"
        code = run_spec_interview(
            description=description,
            output_dir=out,
            flow_level=flow or state.flow_level or "standard",
            mode=state.mode,
            first_principles=state.first_principles,
            max_questions=state.max_questions,
            feature_slug=(state.current_feature or {}).get("name") if state.current_feature else None,
            title_hint=None,
            console=console,
        )
        raise typer.Exit(code)

    @spec_app.command("check")
    def check(
        output_dir: Optional[str] = typer.Option(None, "--output-dir", help="Output directory (default: cwd)"),
        flow: str = typer.Option("standard", "--flow", help="Flow level (quick|standard|full)"),
    ):
        """Run planning-time SpecQualityGate checks for spec.json."""
        out = Path(output_dir) if output_dir else Path.cwd()
        paths = get_spec_paths(out)
        spec = load_spec(paths)
        if not spec:
            console.print("[red]spec.json not found.[/red]")
            raise typer.Exit(1)

        res = check_spec_quality(spec, flow_level=flow)
        if res.errors:
            console.print("[red]Spec check failed:[/red]")
            for e in res.errors:
                console.print(f"  [red]-[/red] {e}")
        if res.warnings:
            console.print("[yellow]Warnings:[/yellow]")
            for w in res.warnings:
                console.print(f"  [yellow]-[/yellow] {w}")
        raise typer.Exit(0 if not res.errors else 1)

    @spec_app.command("compile")
    def compile(
        output_dir: Optional[str] = typer.Option(None, "--output-dir", help="Output directory (default: cwd)"),
        prd_path: Optional[str] = typer.Option(None, "--prd-path", help="Output PRD path (default: <dir>/prd.json)"),
        flow: str = typer.Option("standard", "--flow", help="Flow level (quick|standard|full)"),
        tdd: str = typer.Option("auto", "--tdd", help="TDD mode (off|on|auto)"),
        confirm: bool = typer.Option(False, "--confirm", help="Enable batch confirmation"),
        no_confirm: bool = typer.Option(False, "--no-confirm", help="Disable batch confirmation (wins)"),
        description: str = typer.Option("", "--description", help="Override PRD description metadata"),
    ):
        """Compile spec.json into prd.json."""
        out = Path(output_dir) if output_dir else Path.cwd()
        paths = get_spec_paths(out)
        spec = load_spec(paths)
        if not spec:
            console.print("[red]spec.json not found.[/red]")
            raise typer.Exit(1)

        prd = compile_spec_to_prd(
            spec,
            options=CompileOptions(
                description=description or str(spec.metadata.get("description", "")) or "",
                flow_level=flow,
                tdd_mode=tdd,
                confirm_mode=confirm,
                no_confirm_mode=no_confirm,
            ),
        )

        prd_out = Path(prd_path) if prd_path else paths.prd_json_path
        prd_out.parent.mkdir(parents=True, exist_ok=True)
        prd_out.write_text(__import__("json").dumps(prd, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

        console.print("[green]✓ prd.json written[/green]")
        console.print(f"  - {prd_out}")
        raise typer.Exit(0)

    @spec_app.command("cleanup")
    def cleanup(
        output_dir: Optional[str] = typer.Option(None, "--output-dir", help="Output directory (default: cwd)"),
        all_files: bool = typer.Option(False, "--all", help="Also delete spec.json/spec.md"),
    ):
        """Cleanup interview state (and optionally spec artifacts)."""
        out = Path(output_dir) if output_dir else Path.cwd()
        paths = get_spec_paths(out)
        removed = []

        if paths.interview_state_path.exists():
            paths.interview_state_path.unlink()
            removed.append(str(paths.interview_state_path))

        if all_files:
            for p in (paths.spec_json_path, paths.spec_md_path):
                if p.exists():
                    p.unlink()
                    removed.append(str(p))

        if removed:
            console.print("[green]✓ Removed:[/green]")
            for r in removed:
                console.print(f"  - {r}")
        else:
            console.print("[dim]Nothing to remove.[/dim]")

else:
    spec_app = None

