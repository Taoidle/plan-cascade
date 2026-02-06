#!/usr/bin/env python3
"""
Spec Interview Tools for Plan Cascade MCP Server

Provides MCP tools for specification interview lifecycle management:
- spec_start: Start a new spec interview session
- spec_resume: Resume an interrupted spec interview
- spec_submit_answers: Submit answers to interview questions
- spec_get_status: Get current interview status
- spec_cleanup: Clean up interview state files
"""

import json
import logging
from pathlib import Path
from typing import Any, Dict, List, Optional

from plan_cascade.core.spec_models import (
    Spec,
    SpecInterviewState,
    SpecStory,
    utc_now_iso,
)
from plan_cascade.core.spec_io import (
    get_spec_paths,
    save_interview_state,
    load_interview_state,
    save_spec,
    save_spec_md,
    write_json,
    read_json,
)
from plan_cascade.core.spec_renderer import render_spec_md
from plan_cascade.core.spec_compiler import compile_spec_to_prd, CompileOptions

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Question generation helpers
# ---------------------------------------------------------------------------

# Questions are organized by interview section / topic.
# Each flow level uses a different subset.

_OVERVIEW_QUESTIONS = [
    {"id": "overview_goal", "text": "What is the primary goal of this feature/project?", "section": "overview"},
    {"id": "overview_problem", "text": "What problem does this solve for users?", "section": "overview"},
    {"id": "overview_success_metrics", "text": "What are the measurable success metrics?", "section": "overview"},
    {"id": "overview_non_goals", "text": "What is explicitly NOT a goal (non-goals)?", "section": "overview"},
]

_SCOPE_QUESTIONS = [
    {"id": "scope_in_scope", "text": "What is in scope for this work?", "section": "scope"},
    {"id": "scope_out_of_scope", "text": "What is out of scope?", "section": "scope"},
    {"id": "scope_do_not_touch", "text": "Are there areas of the codebase that should NOT be modified?", "section": "scope"},
    {"id": "scope_assumptions", "text": "What assumptions are you making?", "section": "scope"},
]

_REQUIREMENTS_QUESTIONS = [
    {"id": "req_functional", "text": "What are the key functional requirements?", "section": "requirements"},
    {"id": "req_performance", "text": "Are there any performance targets or constraints?", "section": "requirements"},
    {"id": "req_security", "text": "Are there security requirements to consider?", "section": "requirements"},
]

_INTERFACE_QUESTIONS = [
    {"id": "iface_api", "text": "What APIs or endpoints will be created/modified?", "section": "interfaces"},
    {"id": "iface_data_models", "text": "What data models or schemas are involved?", "section": "interfaces"},
]

_STORY_QUESTIONS = [
    {"id": "stories_breakdown", "text": "How would you break this work into discrete stories/tasks?", "section": "stories"},
    {"id": "stories_dependencies", "text": "Which tasks depend on others? What is the execution order?", "section": "stories"},
    {"id": "stories_verification", "text": "How will each task be verified (test commands, manual checks)?", "section": "stories"},
]

_FIRST_PRINCIPLES_QUESTIONS = [
    {"id": "fp_constraints", "text": "What are the fundamental constraints you cannot change?", "section": "first_principles"},
    {"id": "fp_tradeoffs", "text": "What key trade-offs are you willing to make?", "section": "first_principles"},
]

# Flow -> question pool mapping
_FLOW_QUESTION_POOLS: Dict[str, List[Dict[str, str]]] = {
    "quick": _OVERVIEW_QUESTIONS[:2] + _SCOPE_QUESTIONS[:1] + _REQUIREMENTS_QUESTIONS[:1] + _STORY_QUESTIONS[:1],
    "standard": _OVERVIEW_QUESTIONS + _SCOPE_QUESTIONS[:2] + _REQUIREMENTS_QUESTIONS + _INTERFACE_QUESTIONS[:1] + _STORY_QUESTIONS,
    "full": _OVERVIEW_QUESTIONS + _SCOPE_QUESTIONS + _REQUIREMENTS_QUESTIONS + _INTERFACE_QUESTIONS + _STORY_QUESTIONS + _FIRST_PRINCIPLES_QUESTIONS,
}


def _get_questions_for_flow(
    flow: str,
    first_principles: bool,
    max_questions: int,
    cursor: int = 0,
) -> List[Dict[str, str]]:
    """Return the next batch of questions based on flow level and cursor position."""
    pool = list(_FLOW_QUESTION_POOLS.get(flow, _FLOW_QUESTION_POOLS["standard"]))

    # Add first principles questions if requested and not already in pool
    if first_principles and flow != "full":
        fp_ids = {q["id"] for q in _FIRST_PRINCIPLES_QUESTIONS}
        existing_ids = {q["id"] for q in pool}
        for q in _FIRST_PRINCIPLES_QUESTIONS:
            if q["id"] not in existing_ids:
                pool.append(q)

    # Limit to max_questions total
    pool = pool[:max_questions]

    # Return questions from cursor onward (batch of remaining)
    remaining = pool[cursor:]
    return remaining


def _resolve_output_dir(project_root: Path, output_dir: Optional[str]) -> Path:
    """Resolve the output directory for spec artifacts."""
    if output_dir:
        return Path(output_dir).resolve()
    return Path(project_root).resolve()


def _get_interview_state_path(output_dir: Path) -> Path:
    """Get path to the spec-interview.json state file."""
    return output_dir / ".state" / "spec-interview.json"


def _load_state(output_dir: Path) -> Optional[SpecInterviewState]:
    """Load interview state from disk."""
    paths = get_spec_paths(output_dir)
    return load_interview_state(paths)


def _save_state(state: SpecInterviewState, output_dir: Path) -> None:
    """Save interview state to disk."""
    paths = get_spec_paths(output_dir)
    save_interview_state(state, paths)


def _build_spec_from_history(state: SpecInterviewState) -> Spec:
    """Build a Spec object from interview history answers."""
    spec = Spec()

    overview: Dict[str, Any] = {}
    scope: Dict[str, Any] = {}
    requirements: Dict[str, Any] = {"functional": [], "non_functional": {}}
    interfaces: Dict[str, Any] = {"api": [], "data_models": []}
    stories_raw: List[str] = []
    deps_raw: str = ""
    verification_raw: str = ""

    for entry in state.history:
        q = entry.get("question", "")
        a = entry.get("answer", "")
        if not a.strip():
            continue

        # Map question IDs to spec sections
        if "overview_goal" in q:
            overview["goal"] = a
        elif "overview_problem" in q:
            overview["problem"] = a
        elif "overview_success_metrics" in q:
            overview["success_metrics"] = [s.strip() for s in a.split(",") if s.strip()]
        elif "overview_non_goals" in q:
            overview["non_goals"] = [s.strip() for s in a.split(",") if s.strip()]
        elif "scope_in_scope" in q:
            scope["in_scope"] = [s.strip() for s in a.split(",") if s.strip()]
        elif "scope_out_of_scope" in q:
            scope["out_of_scope"] = [s.strip() for s in a.split(",") if s.strip()]
        elif "scope_do_not_touch" in q:
            scope["do_not_touch"] = [s.strip() for s in a.split(",") if s.strip()]
        elif "scope_assumptions" in q:
            scope["assumptions"] = [s.strip() for s in a.split(",") if s.strip()]
        elif "req_functional" in q:
            requirements["functional"] = [s.strip() for s in a.split(",") if s.strip()]
        elif "req_performance" in q:
            requirements["non_functional"]["performance_targets"] = [
                s.strip() for s in a.split(",") if s.strip()
            ]
        elif "req_security" in q:
            requirements["non_functional"]["security"] = [
                s.strip() for s in a.split(",") if s.strip()
            ]
        elif "iface_api" in q:
            interfaces["api"] = [{"name": s.strip(), "notes": ""} for s in a.split(",") if s.strip()]
        elif "iface_data_models" in q:
            interfaces["data_models"] = [{"name": s.strip(), "fields": []} for s in a.split(",") if s.strip()]
        elif "stories_breakdown" in q:
            stories_raw = [s.strip() for s in a.split(",") if s.strip()]
        elif "stories_dependencies" in q:
            deps_raw = a
        elif "stories_verification" in q:
            verification_raw = a

    # Set title from goal or description
    title = overview.get("goal", state.description) or state.description
    overview.setdefault("title", title)
    overview.setdefault("goal", title)

    spec.overview = overview
    spec.scope = scope
    spec.requirements = requirements
    spec.interfaces = interfaces

    # Convert stories raw to SpecStory objects
    spec_stories: List[SpecStory] = []
    for idx, story_text in enumerate(stories_raw, 1):
        story_id = f"story-{idx:03d}"
        spec_stories.append(
            SpecStory(
                id=story_id,
                title=story_text,
                description=story_text,
                acceptance_criteria=[f"{story_text} is complete and tested"],
                verification={"commands": [], "manual_steps": []},
                context_estimate="medium",
            )
        )

    spec.stories = spec_stories
    spec.ensure_defaults()
    return spec


# ---------------------------------------------------------------------------
# Tool Registration
# ---------------------------------------------------------------------------


def register_spec_tools(mcp: Any, project_root: Path) -> None:
    """
    Register all spec-interview-related tools with the MCP server.

    Args:
        mcp: FastMCP server instance
        project_root: Root directory of the project
    """

    @mcp.tool()
    def spec_start(
        description: str,
        flow: Optional[str] = None,
        first_principles: bool = False,
        max_questions: Optional[int] = None,
        output_dir: Optional[str] = None,
    ) -> Dict[str, Any]:
        """
        Start a new spec interview session.

        Initializes a spec interview to gather requirements through structured
        questions. Creates .state/spec-interview.json to track progress and
        enable resumability.

        Args:
            description: Description of the feature/project to specify
            flow: Interview depth - "quick", "standard", or "full" (default: "standard")
            first_principles: If True, include first-principles reasoning questions
            max_questions: Maximum number of interview questions (default: 18)
            output_dir: Optional output directory for spec artifacts

        Returns:
            Initial set of interview questions with IDs for answering
        """
        resolved_dir = _resolve_output_dir(project_root, output_dir)
        flow_level = (flow or "standard").strip().lower()
        if flow_level not in ("quick", "standard", "full"):
            flow_level = "standard"

        effective_max = max_questions if max_questions and max_questions > 0 else 18

        # Check if an interview is already active
        existing_state = _load_state(resolved_dir)
        if existing_state and existing_state.status == "in_progress":
            return {
                "success": False,
                "error": "A spec interview is already active. Use spec_resume to continue or spec_cleanup to start fresh.",
            }

        # Create new interview state
        state = SpecInterviewState(
            status="in_progress",
            flow_level=flow_level,
            first_principles=first_principles,
            max_questions=effective_max,
            question_cursor=0,
            history=[],
            description=description,
        )

        # Persist state
        _save_state(state, resolved_dir)

        # Generate initial questions
        questions = _get_questions_for_flow(
            flow=flow_level,
            first_principles=first_principles,
            max_questions=effective_max,
            cursor=0,
        )

        return {
            "success": True,
            "message": f"Spec interview started ({flow_level} flow, max {effective_max} questions)",
            "questions": questions,
            "flow": flow_level,
            "state_file": str(_get_interview_state_path(resolved_dir)),
        }

    @mcp.tool()
    def spec_resume() -> Dict[str, Any]:
        """
        Resume an interrupted spec interview.

        Reads state from .state/spec-interview.json and returns the current
        question set and progress information.

        Returns:
            Current questions and progress information
        """
        resolved_dir = _resolve_output_dir(project_root, None)
        state = _load_state(resolved_dir)

        if not state:
            return {
                "success": False,
                "error": "No active spec interview found. Use spec_start to begin a new interview.",
            }

        if state.status == "finalized":
            return {
                "success": False,
                "error": "Interview already finalized. Use spec_cleanup to start a new one.",
            }

        # Get current questions from cursor position
        questions = _get_questions_for_flow(
            flow=state.flow_level,
            first_principles=state.first_principles,
            max_questions=state.max_questions,
            cursor=state.question_cursor,
        )

        total_pool = _get_questions_for_flow(
            flow=state.flow_level,
            first_principles=state.first_principles,
            max_questions=state.max_questions,
            cursor=0,
        )
        total_questions = len(total_pool)
        questions_asked = state.question_cursor
        completion_pct = int((questions_asked / total_questions) * 100) if total_questions > 0 else 0

        return {
            "success": True,
            "message": f"Interview resumed ({state.flow_level} flow)",
            "description": state.description,
            "questions": questions,
            "progress": {
                "questions_asked": questions_asked,
                "total_questions": total_questions,
                "completion_percentage": completion_pct,
            },
        }

    @mcp.tool()
    def spec_submit_answers(
        answers: Dict[str, str],
        compile: bool = False,
    ) -> Dict[str, Any]:
        """
        Submit answers to the current interview questions and advance the interview.

        Processes the submitted answers, records them in the interview state,
        and either returns the next batch of questions or finalizes the spec.

        Args:
            answers: Dict mapping question_id to answer string
            compile: If True, compile to prd.json upon completion

        Returns:
            Next questions or completion status with output file paths
        """
        resolved_dir = _resolve_output_dir(project_root, None)
        state = _load_state(resolved_dir)

        if not state:
            return {
                "success": False,
                "error": "No active spec interview found. Use spec_start to begin.",
            }

        if state.status == "finalized":
            return {
                "success": False,
                "error": "Interview already finalized.",
            }

        if not answers:
            return {
                "success": False,
                "error": "No answers provided. Please provide a dict of question_id -> answer.",
            }

        # Record answers in history
        for qid, answer in answers.items():
            state.history.append({
                "ts": utc_now_iso(),
                "question": qid,
                "answer": str(answer),
            })

        # Advance cursor by number of answers submitted
        state.question_cursor += len(answers)

        # Check if we have more questions
        remaining = _get_questions_for_flow(
            flow=state.flow_level,
            first_principles=state.first_principles,
            max_questions=state.max_questions,
            cursor=state.question_cursor,
        )

        result: Dict[str, Any] = {
            "success": True,
            "answers_recorded": len(answers),
        }

        if remaining:
            # More questions to ask
            result["message"] = f"Answers recorded. {len(remaining)} questions remaining."
            result["status"] = "in_progress"
            result["questions"] = remaining
        else:
            # Interview complete - finalize
            state.status = "finalized"
            result["message"] = "Interview complete. Spec finalized."
            result["status"] = "finalized"

            # Build spec from history
            spec = _build_spec_from_history(state)
            paths = get_spec_paths(resolved_dir)

            # Save spec.json and spec.md
            save_spec(spec, paths)
            spec_md = render_spec_md(spec)
            save_spec_md(spec_md, paths)

            result["spec_json_path"] = str(paths.spec_json_path)
            result["spec_md_path"] = str(paths.spec_md_path)

            # Compile to prd.json if requested
            if compile:
                prd = compile_spec_to_prd(
                    spec,
                    options=CompileOptions(
                        description=state.description,
                        flow_level=state.flow_level,
                    ),
                )
                write_json(paths.prd_json_path, prd)
                result["compiled"] = True
                result["prd_json_path"] = str(paths.prd_json_path)

        # Persist updated state
        _save_state(state, resolved_dir)

        return result

    @mcp.tool()
    def spec_get_status() -> Dict[str, Any]:
        """
        Get the current spec interview status.

        Returns structured status including whether an interview is active,
        questions asked/remaining, and completion percentage.

        Returns:
            Structured status dict with active state and progress
        """
        resolved_dir = _resolve_output_dir(project_root, None)
        state = _load_state(resolved_dir)

        if not state:
            return {
                "success": True,
                "active": False,
                "message": "No spec interview in progress.",
                "questions_asked": 0,
                "questions_remaining": 0,
                "completion_percentage": 0,
            }

        total_pool = _get_questions_for_flow(
            flow=state.flow_level,
            first_principles=state.first_principles,
            max_questions=state.max_questions,
            cursor=0,
        )
        total_questions = len(total_pool)
        questions_asked = state.question_cursor
        questions_remaining = max(0, total_questions - questions_asked)
        completion_pct = int((questions_asked / total_questions) * 100) if total_questions > 0 else 0

        is_active = state.status == "in_progress"

        return {
            "success": True,
            "active": is_active,
            "status": state.status,
            "flow_level": state.flow_level,
            "description": state.description,
            "questions_asked": questions_asked,
            "questions_remaining": questions_remaining,
            "total_questions": total_questions,
            "completion_percentage": completion_pct,
            "first_principles": state.first_principles,
            "max_questions": state.max_questions,
            "history_length": len(state.history),
            "message": f"Interview {'active' if is_active else 'finalized'} ({state.flow_level} flow)",
        }

    @mcp.tool()
    def spec_cleanup(
        remove_outputs: bool = False,
    ) -> Dict[str, Any]:
        """
        Clean up spec interview state files and optionally output files.

        Removes .state/spec-interview.json and optionally spec.json/spec.md.

        Args:
            remove_outputs: If True, also remove spec.json and spec.md output files

        Returns:
            Cleanup result with list of removed files
        """
        resolved_dir = _resolve_output_dir(project_root, None)
        removed_files: List[str] = []

        # Remove interview state
        state_path = _get_interview_state_path(resolved_dir)
        if state_path.exists():
            state_path.unlink()
            removed_files.append(str(state_path))

        # Optionally remove output files
        if remove_outputs:
            for filename in ("spec.json", "spec.md"):
                filepath = resolved_dir / filename
                if filepath.exists():
                    filepath.unlink()
                    removed_files.append(str(filepath))

        if removed_files:
            return {
                "success": True,
                "message": f"Cleaned up {len(removed_files)} file(s).",
                "removed_files": removed_files,
            }
        else:
            return {
                "success": True,
                "message": "No interview state files found to clean up.",
                "removed_files": [],
            }
