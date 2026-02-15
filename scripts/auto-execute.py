#!/usr/bin/env python3
"""
Auto-Execute Script for Plan Cascade Plugin Mode

Provides automatic story execution with quality gates and retry management.
Called by approve.md to enable hands-off execution with auto-retry on failures.

Usage:
    uv run python scripts/auto-execute.py [options]

Options:
    --prd PATH              Path to prd.json (default: prd.json)
    --flow LEVEL            Flow level: quick, standard, full (default: standard)
    --tdd MODE              TDD mode: off, on, auto (default: auto)
    --max-retries N         Maximum retry attempts per story (default: 3)
    --no-retry              Disable automatic retry
    --gate-mode MODE        Gate mode: soft, hard (default from flow)
    --parallel              Enable parallel execution within batches
    --max-concurrency N     Max parallel stories (default: CPU count)
    --batch N               Execute only batch N (1-indexed)
    --dry-run               Show what would be executed without running
    --output-format FMT     Output format: text, json (default: text)
    --state-file PATH       Path to state file (default: .iteration-state.json)
    --agent NAME            Global agent override
    --impl-agent NAME       Implementation agent override
    --retry-agent NAME      Retry agent override
    --no-fallback           Disable fallback when selected agent is unavailable
"""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional


def setup_path():
    """Add src to Python path for imports."""
    script_dir = Path(__file__).parent
    src_dir = script_dir.parent / "src"
    if src_dir.exists() and str(src_dir) not in sys.path:
        sys.path.insert(0, str(src_dir))


setup_path()


def parse_args() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(
        description="Auto-execute PRD stories with quality gates and retry management"
    )

    parser.add_argument(
        "--prd",
        type=Path,
        default=Path("prd.json"),
        help="Path to prd.json (default: prd.json)",
    )
    parser.add_argument(
        "--flow",
        choices=["quick", "standard", "full"],
        default="standard",
        help="Flow level (default: standard)",
    )
    parser.add_argument(
        "--tdd",
        choices=["off", "on", "auto"],
        default="auto",
        help="TDD mode (default: auto)",
    )
    parser.add_argument(
        "--max-retries",
        type=int,
        default=3,
        help="Maximum retry attempts per story (default: 3)",
    )
    parser.add_argument(
        "--no-retry",
        action="store_true",
        help="Disable automatic retry",
    )
    parser.add_argument(
        "--gate-mode",
        choices=["soft", "hard"],
        help="Gate mode (default: derived from flow)",
    )
    parser.add_argument(
        "--parallel",
        action="store_true",
        help="Enable parallel execution within batches",
    )
    parser.add_argument(
        "--max-concurrency",
        type=int,
        help="Max parallel stories (default: CPU count)",
    )
    parser.add_argument(
        "--batch",
        type=int,
        help="Execute only specific batch (1-indexed)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be executed without running",
    )
    parser.add_argument(
        "--output-format",
        choices=["text", "json"],
        default="text",
        help="Output format (default: text)",
    )
    parser.add_argument(
        "--state-file",
        type=Path,
        help="Path to state file",
    )
    parser.add_argument(
        "--project-root",
        type=Path,
        default=Path.cwd(),
        help="Project root directory",
    )
    parser.add_argument(
        "--agent",
        type=str,
        help="Global agent override for all phases",
    )
    parser.add_argument(
        "--impl-agent",
        type=str,
        help="Agent override for implementation phase",
    )
    parser.add_argument(
        "--retry-agent",
        type=str,
        help="Agent override for retry phase",
    )
    parser.add_argument(
        "--no-fallback",
        action="store_true",
        help="Disable fallback to alternate agents when selected agent is unavailable",
    )

    return parser.parse_args()


def load_prd(prd_path: Path) -> dict[str, Any]:
    """Load PRD from JSON file."""
    if not prd_path.exists():
        print(f"Error: PRD file not found: {prd_path}", file=sys.stderr)
        sys.exit(1)

    with open(prd_path) as f:
        return json.load(f)


def get_gate_mode(flow: str, explicit_mode: Optional[str]) -> str:
    """Determine gate mode from flow level."""
    if explicit_mode:
        return explicit_mode

    if flow == "full":
        return "hard"
    return "soft"


def calculate_batches(stories: list[dict]) -> list[list[dict]]:
    """Calculate execution batches based on dependencies."""
    from collections import defaultdict

    # Build dependency graph
    story_map = {s["id"]: s for s in stories}
    pending = set(story_map.keys())
    completed = set()
    batches = []

    while pending:
        # Find stories with all dependencies satisfied
        ready = []
        for story_id in pending:
            story = story_map[story_id]
            deps = set(story.get("dependencies", []))
            if deps.issubset(completed):
                ready.append(story)

        if not ready:
            # Circular dependency or missing dependency
            print(f"Warning: Cannot resolve dependencies for: {pending}", file=sys.stderr)
            # Add remaining as final batch
            ready = [story_map[sid] for sid in pending]

        batches.append(ready)
        for story in ready:
            pending.discard(story["id"])
            completed.add(story["id"])

    return batches


def print_execution_plan(
    prd: dict,
    batches: list[list[dict]],
    args: argparse.Namespace,
    gate_mode: str,
):
    """Print the execution plan."""
    print("=" * 60)
    print("AUTO-EXECUTE PLAN")
    print("=" * 60)
    print()
    print(f"PRD: {args.prd}")
    print(f"Goal: {prd.get('goal', 'N/A')}")
    print()
    print("Configuration:")
    print(f"  Flow Level: {args.flow}")
    print(f"  Gate Mode: {gate_mode}")
    print(f"  TDD Mode: {args.tdd}")
    print(f"  Auto Retry: {not args.no_retry}")
    if not args.no_retry:
        print(f"  Max Retries: {args.max_retries}")
    print(f"  Parallel: {args.parallel}")
    if args.parallel and args.max_concurrency:
        print(f"  Max Concurrency: {args.max_concurrency}")
    if args.agent:
        print(f"  Agent Override: {args.agent}")
    if args.impl_agent:
        print(f"  Impl Agent Override: {args.impl_agent}")
    if args.retry_agent:
        print(f"  Retry Agent Override: {args.retry_agent}")
    if args.no_fallback:
        print("  Fallback: disabled")
    print()
    print(f"Total Stories: {sum(len(b) for b in batches)}")
    print(f"Total Batches: {len(batches)}")
    print()

    for i, batch in enumerate(batches, 1):
        if args.batch and i != args.batch:
            continue
        print(f"Batch {i}:")
        for story in batch:
            deps = story.get("dependencies", [])
            dep_str = f" (deps: {', '.join(deps)})" if deps else ""
            print(f"  - {story['id']}: {story.get('title', 'Untitled')}{dep_str}")
    print()


def create_orchestrator(project_root: Path, prd: dict, args: argparse.Namespace):
    """Create orchestrator for story execution."""
    try:
        from plan_cascade.backends.agent_executor import AgentExecutor
        from plan_cascade.backends.phase_config import AgentOverrides
        from plan_cascade.core.orchestrator import Orchestrator
        from plan_cascade.state.path_resolver import PathResolver
        from plan_cascade.state.state_manager import StateManager

        # Create path resolver (legacy mode for plugin compatibility)
        path_resolver = PathResolver(project_root, legacy_mode=True)

        # Create state manager and write PRD to ensure orchestrator can read it
        state_manager = StateManager(project_root, path_resolver=path_resolver)
        state_manager.write_prd(prd)

        agent_override = AgentOverrides(
            global_agent=args.agent,
            impl_agent=args.impl_agent,
            retry_agent=args.retry_agent,
            no_fallback=args.no_fallback,
        )
        agent_executor = AgentExecutor(
            project_root=project_root,
            path_resolver=path_resolver,
            legacy_mode=True,
        )

        # Create orchestrator with correct signature
        orchestrator = Orchestrator(
            project_root=project_root,
            state_manager=state_manager,
            path_resolver=path_resolver,
            legacy_mode=True,
            agent_executor=agent_executor,
            agent_override=agent_override,
        )

        return orchestrator
    except ImportError as e:
        print(f"Warning: Could not import orchestrator: {e}", file=sys.stderr)
        return None


def create_quality_gate(project_root: Path, gate_mode: str, tdd_mode: str, flow: str):
    """Create quality gate for verification."""
    try:
        from plan_cascade.core.quality_gate import QualityGate, GateConfig, GateType

        # Build list of gates based on flow level
        gates: List[GateConfig] = []

        # Always include basic validation gates
        gates.append(GateConfig(
            name="typecheck",
            type=GateType.TYPECHECK,
            enabled=True,
            required=(gate_mode == "hard"),
        ))

        gates.append(GateConfig(
            name="lint",
            type=GateType.LINT,
            enabled=True,
            required=(gate_mode == "hard"),
        ))

        gates.append(GateConfig(
            name="test",
            type=GateType.TEST,
            enabled=True,
            required=(gate_mode == "hard"),
        ))

        # Add AI verification for standard and full flows
        if flow != "quick":
            gates.append(GateConfig(
                name="implementation_verify",
                type=GateType.IMPLEMENTATION_VERIFY,
                enabled=True,
                required=False,  # AI verification is advisory
            ))

        # Add code review and TDD compliance for full flow
        if flow == "full":
            gates.append(GateConfig(
                name="code_review",
                type=GateType.CODE_REVIEW,
                enabled=True,
                required=True,
            ))

            if tdd_mode != "off":
                gates.append(GateConfig(
                    name="tdd_compliance",
                    type=GateType.TDD_COMPLIANCE,
                    enabled=True,
                    required=(tdd_mode == "on"),  # Required only when TDD is explicitly on
                ))

        return QualityGate(
            project_root=project_root,
            gates=gates,
            fail_fast=(gate_mode == "hard"),
        )
    except ImportError as e:
        print(f"Warning: Could not import quality gate: {e}", file=sys.stderr)
        return None


def create_retry_manager(project_root: Path, max_retries: int, enabled: bool):
    """Create retry manager for automatic retries."""
    if not enabled:
        return None

    try:
        from plan_cascade.core.retry_manager import RetryManager, RetryConfig

        config = RetryConfig(
            max_retries=max_retries,
            exponential_backoff=True,
            base_delay_seconds=5.0,
            max_delay_seconds=60.0,
            inject_failure_context=True,
            switch_agent_on_retry=True,
        )

        return RetryManager(project_root, config)
    except ImportError as e:
        print(f"Warning: Could not import retry manager: {e}", file=sys.stderr)
        return None


def run_sequential(
    project_root: Path,
    prd: dict,
    batches: list[list[dict]],
    args: argparse.Namespace,
    gate_mode: str,
) -> dict[str, Any]:
    """Run execution sequentially using IterationLoop."""
    from plan_cascade.core.iteration_loop import (
        IterationLoop,
        IterationConfig,
        IterationMode,
        IterationCallbacks,
    )

    # Create components
    orchestrator = create_orchestrator(project_root, prd, args)
    quality_gate = create_quality_gate(project_root, gate_mode, args.tdd, args.flow)
    retry_manager = create_retry_manager(project_root, args.max_retries, not args.no_retry)

    # Create config
    config = IterationConfig(
        mode=IterationMode.UNTIL_COMPLETE,
        max_iterations=50,
        poll_interval_seconds=10,
        batch_timeout_seconds=3600,
        quality_gates_enabled=quality_gate is not None,
        auto_retry_enabled=retry_manager is not None,
        stop_on_first_failure=gate_mode == "hard",
        dod_gates_enabled=True,
        dod_level="full" if args.flow == "full" else "standard",
    )

    # Create iteration loop
    loop = IterationLoop(
        project_root=project_root,
        config=config,
        orchestrator=orchestrator,
        quality_gate=quality_gate,
        retry_manager=retry_manager,
        state_file=args.state_file,
    )

    # Set up callbacks
    callbacks = IterationCallbacks()
    callbacks.on_batch_start = lambda batch_num, stories: print(f"\n=== Starting Batch {batch_num} ({len(stories)} stories) ===")
    callbacks.on_story_complete = lambda story_id, success: print(f"  [{story_id}] {'✓ Complete' if success else '✗ Failed'}")
    callbacks.on_quality_gate_run = lambda story_id, results: print(f"  [{story_id}] Quality gates checked")
    callbacks.on_story_retry = lambda story_id, attempt: print(f"  [{story_id}] Retrying (attempt {attempt})...")
    callbacks.on_batch_complete = lambda result: print(f"=== Batch {result.batch_num} complete: {result.stories_completed}/{result.stories_launched} succeeded ===\n")
    callbacks.on_error = lambda msg, exc: print(f"Error: {msg} - {exc}", file=sys.stderr)

    # Run
    print("\n" + "=" * 60)
    print("STARTING EXECUTION")
    print("=" * 60)

    try:
        final_state = loop.start(callbacks=callbacks, dry_run=args.dry_run)
        return {
            "success": final_state.status.value == "completed",
            "status": final_state.status.value,
            "total_stories": final_state.total_stories,
            "completed_stories": final_state.completed_stories,
            "failed_stories": final_state.failed_stories,
            "batches_completed": len(final_state.batch_results),
            "iterations": final_state.current_iteration,
        }
    except KeyboardInterrupt:
        print("\nInterrupted by user")
        loop.pause("User interruption")
        return {"success": False, "status": "paused", "message": "User interruption"}
    except Exception as e:
        print(f"\nExecution failed: {e}", file=sys.stderr)
        return {"success": False, "status": "failed", "error": str(e)}


async def run_parallel(
    project_root: Path,
    prd: dict,
    batches: list[list[dict]],
    args: argparse.Namespace,
    gate_mode: str,
) -> dict[str, Any]:
    """Run execution in parallel using ParallelExecutor."""
    from plan_cascade.core.parallel_executor import (
        ParallelExecutor,
        ParallelExecutionConfig,
    )
    from plan_cascade.state.state_manager import StateManager

    # Create components
    orchestrator = create_orchestrator(project_root, prd, args)
    quality_gate = create_quality_gate(project_root, gate_mode, args.tdd, args.flow)
    retry_manager = create_retry_manager(project_root, args.max_retries, not args.no_retry)
    state_manager = StateManager(project_root)

    # Create config
    config = ParallelExecutionConfig(
        max_concurrency=args.max_concurrency,
        poll_interval_seconds=1.0,
        timeout_seconds=3600,
        persist_progress=True,
        quality_gates_enabled=quality_gate is not None,
        auto_retry_enabled=retry_manager is not None,
        dod_gates_enabled=True,
        dod_level="full" if args.flow == "full" else "standard",
    )

    # Create executor
    executor = ParallelExecutor(
        project_root=project_root,
        config=config,
        orchestrator=orchestrator,
        state_manager=state_manager,
        quality_gate=quality_gate,
        retry_manager=retry_manager,
    )

    print("\n" + "=" * 60)
    print("STARTING PARALLEL EXECUTION")
    print("=" * 60)

    total_completed = 0
    total_failed = 0
    all_results = []

    try:
        for batch_num, batch in enumerate(batches, 1):
            if args.batch and batch_num != args.batch:
                continue

            print(f"\n=== Starting Batch {batch_num} ({len(batch)} stories, parallel) ===")

            if args.dry_run:
                print("  [DRY RUN] Would execute stories:")
                for story in batch:
                    print(f"    - {story['id']}: {story.get('title', 'Untitled')}")
                continue

            # Execute batch (stories first, batch_num second)
            result = await executor.execute_batch(batch, batch_num)
            all_results.append(result)

            total_completed += result.stories_completed
            total_failed += result.stories_failed

            print(f"=== Batch {batch_num} complete: {result.stories_completed}/{len(batch)} succeeded ===")

            if result.stories_failed > 0 and gate_mode == "hard":
                print(f"\nHARD GATE: Stopping due to failures in batch {batch_num}")
                break

        return {
            "success": total_failed == 0,
            "status": "completed" if total_failed == 0 else "failed",
            "total_stories": sum(len(b) for b in batches),
            "completed_stories": total_completed,
            "failed_stories": total_failed,
            "batches_completed": len(all_results),
        }

    except KeyboardInterrupt:
        print("\nInterrupted by user")
        return {"success": False, "status": "paused", "message": "User interruption"}
    except Exception as e:
        print(f"\nExecution failed: {e}", file=sys.stderr)
        return {"success": False, "status": "failed", "error": str(e)}


def print_results(results: dict[str, Any], output_format: str):
    """Print execution results."""
    if output_format == "json":
        print(json.dumps(results, indent=2))
        return

    print()
    print("=" * 60)
    print("EXECUTION RESULTS")
    print("=" * 60)
    print(f"Status: {results.get('status', 'unknown')}")
    print(f"Success: {results.get('success', False)}")
    print()
    print(f"Stories Completed: {results.get('completed_stories', 0)}/{results.get('total_stories', 0)}")
    print(f"Stories Failed: {results.get('failed_stories', 0)}")
    if "batches_completed" in results:
        print(f"Batches Completed: {results['batches_completed']}")
    if "iterations" in results:
        print(f"Iterations: {results['iterations']}")
    if "error" in results:
        print(f"Error: {results['error']}")
    print("=" * 60)


def run_dor_check(prd: dict, gate_mode: str, flow_level: str, project_root: Path) -> bool:
    """
    Run Definition of Readiness (DoR) check before execution.

    Args:
        prd: PRD dictionary
        gate_mode: Gate mode ('soft' or 'hard')
        flow_level: Execution flow level ('quick', 'standard', 'full')
        project_root: Project root directory

    Returns:
        True if DoR check passed (or soft mode), False if blocked
    """
    try:
        from plan_cascade.core.readiness_gate import ReadinessGate
    except ImportError:
        print("Warning: ReadinessGate not available, skipping DoR check", file=sys.stderr)
        return True

    print("\n" + "=" * 60)
    print("Running DoR (Definition of Readiness) Check...")
    print("=" * 60 + "\n")

    dor_gate = ReadinessGate.from_flow(flow_level)
    result = dor_gate.check_prd(prd)

    # Display results
    if result.errors:
        print(f"  Errors ({len(result.errors)}):")
        for error in result.errors:
            print(f"    [FAIL] {error}")

    if result.warnings:
        print(f"  Warnings ({len(result.warnings)}):")
        for warning in result.warnings:
            print(f"    [WARN] {warning}")

    if result.suggestions:
        print(f"  Suggestions ({len(result.suggestions)}):")
        for suggestion in result.suggestions:
            print(f"    [INFO] {suggestion}")

    passed_count = len(result.details.get("checks", [])) - len(result.errors)
    print(f"\nDoR Result: {'PASSED' if result.passed else 'FAILED'}")
    print(f"  Passed: {passed_count}, Warnings: {len(result.warnings)}, Failed: {len(result.errors)}")

    # Write DoR result to progress.txt
    progress_path = project_root / "progress.txt"
    try:
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        with open(progress_path, "a", encoding="utf-8") as f:
            if result.passed:
                f.write(f"[{timestamp}] [DoR_PASSED] Readiness check passed\n")
            else:
                summary = "; ".join(result.errors[:3]) if result.errors else "Check failed"
                f.write(f"[{timestamp}] [DoR_FAILED] Readiness check failed: {summary}\n")
    except IOError:
        pass  # Non-critical

    # In HARD mode, block execution on failure
    if not result.passed and gate_mode == "hard":
        print("\n[ERROR] DoR check failed in HARD gate mode. Execution blocked.")
        print("Fix the errors above or use --gate-mode soft to continue with warnings.")
        return False

    return True


def main():
    """Main entry point."""
    args = parse_args()

    # Load PRD
    prd = load_prd(args.prd)

    # Get stories
    stories = prd.get("stories", [])
    if not stories:
        print("Error: No stories found in PRD", file=sys.stderr)
        sys.exit(1)

    # Filter to pending stories
    pending_stories = [s for s in stories if s.get("status", "pending") == "pending"]
    if not pending_stories:
        print("All stories are already complete!")
        sys.exit(0)

    # Calculate batches
    batches = calculate_batches(pending_stories)

    # Determine gate mode
    gate_mode = get_gate_mode(args.flow, args.gate_mode)

    # Print execution plan
    print_execution_plan(prd, batches, args, gate_mode)

    # Run DoR check before execution
    if not run_dor_check(prd, gate_mode, args.flow, args.project_root):
        sys.exit(1)

    if args.dry_run:
        print("[DRY RUN] Execution plan shown above. No changes made.")
        sys.exit(0)

    # Run execution
    try:
        if args.parallel:
            results = asyncio.run(run_parallel(
                args.project_root,
                prd,
                batches,
                args,
                gate_mode,
            ))
        else:
            results = run_sequential(
                args.project_root,
                prd,
                batches,
                args,
                gate_mode,
            )

        # Print results
        print_results(results, args.output_format)

        # Exit with appropriate code
        sys.exit(0 if results.get("success", False) else 1)

    except ImportError as e:
        print(f"Error: Required modules not available: {e}", file=sys.stderr)
        print("Make sure plan-cascade is installed: pip install -e .", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
