#!/usr/bin/env python3
"""
Memory Doctor — Decision Conflict Detection CLI

Diagnoses decision conflicts, duplicates, and superseded entries
across all design documents in a Plan Cascade project.

Usage:
  python memory-doctor.py --mode full [--project-root DIR]
  python memory-doctor.py --mode passive --new-decisions FILE [--project-root DIR]
"""

import argparse
import asyncio
import json
import os
import sys
from pathlib import Path


def _force_utf8_stdio() -> None:
    """Force UTF-8 for stdout/stderr so box-drawing characters render correctly."""
    for stream_name in ("stdout", "stderr"):
        stream = getattr(sys, stream_name, None)
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except Exception:
            pass


_force_utf8_stdio()

# Box drawing characters (matching unified-review.py)
BOX_TL = "\u250c"  # ┌
BOX_TR = "\u2510"  # ┐
BOX_BL = "\u2514"  # └
BOX_BR = "\u2518"  # ┘
BOX_H = "\u2500"   # ─
BOX_V = "\u2502"   # │
BOX_ML = "\u251c"  # ├
BOX_MR = "\u2524"  # ┤
DOUBLE_H = "\u2550"  # ═


def setup_path():
    """Add src/ to sys.path for plan_cascade imports."""
    src_dir = Path(__file__).parent.parent.parent.parent / "src"
    if src_dir.exists() and str(src_dir) not in sys.path:
        sys.path.insert(0, str(src_dir))


def load_json_file(filepath: Path) -> dict | None:
    """Load a JSON file if it exists."""
    if not filepath.exists():
        return None
    try:
        with open(filepath, encoding="utf-8-sig") as f:
            return json.load(f)
    except (json.JSONDecodeError, OSError):
        return None


def create_llm_provider():
    """Create an LLM provider from environment or settings."""
    try:
        from plan_cascade.llm import LLMFactory

        # Try environment-based provider detection
        # Priority: ANTHROPIC_API_KEY > OPENAI_API_KEY > DEEPSEEK_API_KEY
        providers = [
            ("claude", "ANTHROPIC_API_KEY"),
            ("openai", "OPENAI_API_KEY"),
            ("deepseek", "DEEPSEEK_API_KEY"),
        ]

        for provider_name, env_key in providers:
            api_key = os.environ.get(env_key)
            if api_key:
                try:
                    return LLMFactory.create(provider_name, api_key=api_key)
                except Exception:
                    continue

        # Try settings-based
        try:
            from plan_cascade.settings import SettingsStorage
            storage = SettingsStorage()
            settings = storage.load()
            return LLMFactory.create_from_settings(settings)
        except Exception:
            pass

        return None
    except ImportError:
        return None


def print_no_llm_warning():
    """Print warning when no LLM provider is available."""
    w = 62
    print(f"{BOX_TL}{BOX_H * w}{BOX_TR}")
    print(f"{BOX_V}  MEMORY DOCTOR — No LLM Available{' ' * (w - 34)}{BOX_V}")
    print(f"{BOX_ML}{BOX_H * w}{BOX_MR}")
    msg = "  Cannot perform semantic analysis without an LLM provider."
    print(f"{BOX_V}{msg:<{w}}{BOX_V}")
    msg2 = "  Set ANTHROPIC_API_KEY, OPENAI_API_KEY, or DEEPSEEK_API_KEY."
    print(f"{BOX_V}{msg2:<{w}}{BOX_V}")
    print(f"{BOX_BL}{BOX_H * w}{BOX_BR}")


def run_full_diagnosis(project_root: Path):
    """Run full diagnosis on all design documents."""
    setup_path()

    llm = create_llm_provider()
    if not llm:
        print_no_llm_warning()
        sys.exit(1)

    from plan_cascade.core.memory_doctor import MemoryDoctor

    doctor = MemoryDoctor(project_root, llm_provider=llm)
    all_decisions = doctor.collect_all_decisions()

    if not all_decisions:
        w = 62
        print(f"{BOX_TL}{BOX_H * w}{BOX_TR}")
        print(f"{BOX_V}  MEMORY DOCTOR — No Decisions Found{' ' * (w - 36)}{BOX_V}")
        print(f"{BOX_BL}{BOX_H * w}{BOX_BR}")
        print("\n  No design_doc.json files with decisions found in project.")
        return

    # Count unique sources
    sources = {d.get("_source", "unknown") for d in all_decisions}

    diagnoses = asyncio.run(doctor.full_diagnosis(all_decisions))
    report = doctor.format_report(diagnoses, total_scanned=len(all_decisions), source_count=len(sources))
    print(report)

    # Output as JSON for programmatic consumption
    if diagnoses:
        json_output = json.dumps(
            [d.to_dict() for d in diagnoses],
            indent=2,
            ensure_ascii=False,
        )
        # Write to stderr for machine parsing (stdout has the human report)
        print(json_output, file=sys.stderr)


def run_passive_diagnosis(project_root: Path, new_decisions_path: Path):
    """Run passive diagnosis comparing new decisions against existing ones."""
    setup_path()

    # Load new decisions
    new_doc = load_json_file(new_decisions_path)
    if not new_doc:
        print(f"Error: Cannot load {new_decisions_path}", file=sys.stderr)
        sys.exit(1)

    new_decisions = new_doc.get("decisions", [])
    if not new_decisions:
        # No decisions in new doc, nothing to check
        sys.exit(0)

    llm = create_llm_provider()
    if not llm:
        print_no_llm_warning()
        sys.exit(1)

    from plan_cascade.core.memory_doctor import MemoryDoctor

    doctor = MemoryDoctor(project_root, llm_provider=llm)

    # Collect existing decisions (excluding the new doc itself)
    all_existing = doctor.collect_all_decisions()
    new_source = str(new_decisions_path.resolve())
    existing = [d for d in all_existing if d.get("_source") != new_source]

    if not existing:
        # No existing decisions to compare against
        sys.exit(0)

    # Tag new decisions with source
    for d in new_decisions:
        d["_source"] = new_source

    diagnoses = asyncio.run(
        doctor.diagnose_new_decisions(
            new_decisions,
            existing,
            source_label=str(new_decisions_path),
        )
    )

    if not diagnoses:
        sys.exit(0)

    sources = {d.get("_source", "unknown") for d in existing + new_decisions}
    report = doctor.format_report(
        diagnoses,
        total_scanned=len(existing) + len(new_decisions),
        source_count=len(sources),
    )
    print(report)

    # JSON on stderr
    json_output = json.dumps(
        [d.to_dict() for d in diagnoses],
        indent=2,
        ensure_ascii=False,
    )
    print(json_output, file=sys.stderr)

    # Exit with code 1 to signal issues found
    sys.exit(1)


def main():
    parser = argparse.ArgumentParser(
        description="Memory Doctor — Decision Conflict Detection",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--mode",
        choices=["passive", "full"],
        default="full",
        help="Diagnosis mode: 'full' scans all decisions, 'passive' compares new vs existing",
    )
    parser.add_argument(
        "--new-decisions",
        type=Path,
        help="Path to design_doc.json with new decisions (required for passive mode)",
    )
    parser.add_argument(
        "--project-root",
        type=Path,
        default=Path.cwd(),
        help="Project root directory (default: current directory)",
    )

    args = parser.parse_args()

    if args.mode == "passive":
        if not args.new_decisions:
            parser.error("--new-decisions is required for passive mode")
        run_passive_diagnosis(args.project_root, args.new_decisions)
    else:
        run_full_diagnosis(args.project_root)


if __name__ == "__main__":
    main()
