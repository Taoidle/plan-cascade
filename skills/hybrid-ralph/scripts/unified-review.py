#!/usr/bin/env python3
"""
Unified Review Display for Plan Cascade

Displays a combined view of:
- PRD (prd.json) + Design Document (design_doc.json) for hybrid modes
- Mega-Plan (mega-plan.json) + Design Document (design_doc.json) for mega mode

Usage:
  python unified-review.py                    # Auto-detect mode
  python unified-review.py --mode hybrid      # Force hybrid mode
  python unified-review.py --mode mega        # Force mega mode
"""

import argparse
import json
import sys
from pathlib import Path



def _force_utf8_stdio() -> None:
    """
    Force UTF-8 for stdout/stderr so box-drawing characters render correctly
    when the caller expects UTF-8 (common for plugin log capture).

    On some Windows setups, Python defaults to a legacy encoding (e.g. GBK),
    which causes mojibake like '�T�T�T' when the consumer decodes as UTF-8.
    """
    for stream_name in ("stdout", "stderr"):
        stream = getattr(sys, stream_name, None)
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except Exception:
            # Best-effort: if reconfigure isn't available, keep default behavior.
            pass


_force_utf8_stdio()
# Box drawing characters
BOX_TL = "\u250c"  # ┌
BOX_TR = "\u2510"  # ┐
BOX_BL = "\u2514"  # └
BOX_BR = "\u2518"  # ┘
BOX_H = "\u2500"   # ─
BOX_V = "\u2502"   # │
BOX_ML = "\u251c"  # ├
BOX_MR = "\u2524"  # ┤
BOX_MT = "\u252c"  # ┬
BOX_MB = "\u2534"  # ┴
BOX_CROSS = "\u253c"  # ┼
DOUBLE_H = "\u2550"  # ═
ARROW_DOWN = "\u2193"  # ↓


def get_path_resolver():
    """Get PathResolver if available, otherwise return None."""
    try:
        sys.path.insert(0, str(Path(__file__).parent.parent.parent.parent / "src"))
        from plan_cascade.state.path_resolver import PathResolver
        return PathResolver(Path.cwd())
    except ImportError:
        return None


def load_json_file(filepath: Path) -> dict | None:
    """Load a JSON file if it exists."""
    if not filepath.exists():
        return None
    try:
        # Use utf-8-sig to tolerate BOM (common on Windows when files are written via PowerShell)
        with open(filepath, "r", encoding="utf-8-sig") as f:
            return json.load(f)
    except (json.JSONDecodeError, OSError):
        return None


def detect_mode() -> str:
    """Detect whether we're in hybrid or mega mode based on available files."""
    resolver = get_path_resolver()

    # Check for mega-plan.json first (project level)
    if resolver:
        mega_path = resolver.get_mega_plan_path()
        prd_path = resolver.get_prd_path()
    else:
        mega_path = Path.cwd() / "mega-plan.json"
        prd_path = Path.cwd() / "prd.json"

    # Also check current directory
    local_mega = Path.cwd() / "mega-plan.json"
    local_prd = Path.cwd() / "prd.json"

    has_mega = mega_path.exists() or local_mega.exists()
    has_prd = prd_path.exists() or local_prd.exists()

    # Prefer prd if both exist (likely in a worktree)
    if has_prd:
        return "hybrid"
    elif has_mega:
        return "mega"
    else:
        return "hybrid"  # Default


def load_documents(mode: str) -> tuple[dict | None, dict | None]:
    """Load the appropriate documents based on mode."""
    resolver = get_path_resolver()

    # Load design_doc.json (always in current directory - user-visible file)
    design_doc = load_json_file(Path.cwd() / "design_doc.json")

    if mode == "mega":
        # Load mega-plan.json
        if resolver:
            plan = load_json_file(resolver.get_mega_plan_path())
        else:
            plan = load_json_file(Path.cwd() / "mega-plan.json")
        if not plan:
            plan = load_json_file(Path.cwd() / "mega-plan.json")
    else:
        # Load prd.json
        if resolver:
            plan = load_json_file(resolver.get_prd_path())
        else:
            plan = load_json_file(Path.cwd() / "prd.json")
        if not plan:
            plan = load_json_file(Path.cwd() / "prd.json")

    return plan, design_doc


def calculate_batches(items: list, id_key: str = "id") -> list[list]:
    """Calculate execution batches based on dependencies."""
    if not items:
        return []

    item_map = {item[id_key]: item for item in items}
    completed = set()
    batches = []

    while len(completed) < len(items):
        ready = []

        for item in items:
            item_id = item[id_key]

            if item_id in completed:
                continue

            # Check if all dependencies are complete
            deps = item.get("dependencies", [])
            if all(dep in completed for dep in deps):
                ready.append(item)

        if not ready:
            # Circular dependency or error - add remaining
            ready = [item for item in items if item[id_key] not in completed]

        # Sort by priority
        priority_order = {"high": 0, "medium": 1, "low": 2}
        ready.sort(key=lambda x: priority_order.get(x.get("priority", "medium"), 1))

        batches.append(ready)
        completed.update(item[id_key] for item in ready)

    return batches


def truncate_text(text: str, max_len: int) -> str:
    """Truncate text to max length with ellipsis."""
    if len(text) <= max_len:
        return text
    return text[:max_len - 3] + "..."


def print_header(title: str, width: int = 80):
    """Print a double-line header."""
    print(DOUBLE_H * width)
    padding = (width - len(title)) // 2
    print(" " * padding + title)
    print(DOUBLE_H * width)
    print()


def print_box(lines: list[str], width: int = 77):
    """Print content in a box."""
    print(BOX_TL + BOX_H * width + BOX_TR)
    for line in lines:
        # Ensure line fits
        if len(line) > width - 2:
            line = line[:width - 5] + "..."
        padding = width - 2 - len(line)
        print(f"{BOX_V}  {line}{' ' * padding}{BOX_V}")
    print(BOX_BL + BOX_H * width + BOX_BR)


def print_section_header(title: str):
    """Print a section header."""
    print(f"## {title}")
    print()


def print_table(headers: list[str], rows: list[list[str]], col_widths: list[int]):
    """Print a formatted table."""
    # Top border
    border = BOX_TL
    for i, w in enumerate(col_widths):
        border += BOX_H * (w + 2)
        border += BOX_MT if i < len(col_widths) - 1 else BOX_TR
    print(border)

    # Header row
    header_row = BOX_V
    for i, (h, w) in enumerate(zip(headers, col_widths)):
        header_row += f" {h:<{w}} " + BOX_V
    print(header_row)

    # Header separator
    sep = BOX_ML
    for i, w in enumerate(col_widths):
        sep += BOX_H * (w + 2)
        sep += BOX_CROSS if i < len(col_widths) - 1 else BOX_MR
    print(sep)

    # Data rows
    for row in rows:
        row_str = BOX_V
        for i, (cell, w) in enumerate(zip(row, col_widths)):
            cell = truncate_text(str(cell), w)
            row_str += f" {cell:<{w}} " + BOX_V
        print(row_str)

    # Bottom border
    border = BOX_BL
    for i, w in enumerate(col_widths):
        border += BOX_H * (w + 2)
        border += BOX_MB if i < len(col_widths) - 1 else BOX_BR
    print(border)


def display_prd_section(prd: dict):
    """Display PRD section."""
    stories = prd.get("stories", [])
    batches = calculate_batches(stories)

    # Count by priority
    high = len([s for s in stories if s.get("priority") == "high"])
    medium = len([s for s in stories if s.get("priority") == "medium"])
    low = len([s for s in stories if s.get("priority") == "low"])

    # Overview box
    print_box([
        "  PRODUCT REQUIREMENTS",
        BOX_H * 75,
        f"  Goal: {truncate_text(prd.get('goal', 'N/A'), 65)}",
        f"  Stories: {len(stories)} total | High: {high} | Medium: {medium} | Low: {low}",
        f"  Batches: {len(batches)} (estimated parallel execution)",
    ])
    print()

    # Stories table
    print_section_header("User Stories")

    if stories:
        headers = ["ID", "Title", "Priority", "Dependencies"]
        col_widths = [12, 38, 8, 15]
        rows = []
        for s in stories:
            deps = s.get("dependencies", [])
            dep_str = ", ".join(deps) if deps else "-"
            rows.append([
                s.get("id", "N/A"),
                truncate_text(s.get("title", "N/A"), 38),
                s.get("priority", "N/A"),
                dep_str
            ])
        print_table(headers, rows, col_widths)
    else:
        print("  (No stories defined)")
    print()

    # Execution batches
    print_section_header("Execution Batches")
    for i, batch in enumerate(batches, 1):
        ids = ", ".join(s["id"] for s in batch)
        parallel = " (Parallel)" if len(batch) > 1 else ""
        print(f"  Batch {i}{parallel}: {ids}")
        if i < len(batches):
            print(f"      {ARROW_DOWN}")
    print()


def display_mega_section(mega: dict):
    """Display mega-plan section."""
    features = mega.get("features", [])
    batches = calculate_batches(features)

    # Overview box
    print_box([
        "  PROJECT OVERVIEW",
        BOX_H * 75,
        f"  Goal: {truncate_text(mega.get('goal', 'N/A'), 65)}",
        f"  Features: {len(features)} total | Batches: {len(batches)}",
        f"  Mode: {mega.get('execution_mode', 'auto')} | Target: {mega.get('target_branch', 'main')}",
    ])
    print()

    # Features table
    print_section_header("Features")

    if features:
        headers = ["ID", "Title", "Priority", "Dependencies"]
        col_widths = [12, 38, 8, 15]
        rows = []
        for f in features:
            deps = f.get("dependencies", [])
            dep_str = ", ".join(deps) if deps else "-"
            rows.append([
                f.get("id", "N/A"),
                truncate_text(f.get("title", "N/A"), 38),
                f.get("priority", "N/A"),
                dep_str
            ])
        print_table(headers, rows, col_widths)
    else:
        print("  (No features defined)")
    print()

    # Feature batches
    print_section_header("Feature Batches")
    for i, batch in enumerate(batches, 1):
        ids = ", ".join(f["id"] for f in batch)
        parallel = " (Parallel)" if len(batch) > 1 else ""
        print(f"  Batch {i}{parallel}: {ids}")
        if i < len(batches):
            print(f"      {ARROW_DOWN} (merge to target branch)")
    print()


def display_design_section(design: dict, mode: str):
    """Display design document section."""
    overview = design.get("overview", {})
    arch = design.get("architecture", {})
    decisions = design.get("decisions", [])
    interfaces = design.get("interfaces", {})

    components = arch.get("components", [])
    patterns = arch.get("patterns", [])

    # Determine level
    level = design.get("metadata", {}).get("level", "feature")

    # Overview box
    title = "PROJECT DESIGN" if mode == "mega" else "TECHNICAL DESIGN"
    print_box([
        f"  {title}",
        BOX_H * 75,
        f"  Level: {level} | Components: {len(components)} | Patterns: {len(patterns)} | ADRs: {len(decisions)}",
    ])
    print()

    # System architecture (mega only)
    if mode == "mega" and arch.get("system_overview"):
        print_section_header("System Architecture")
        print(f"  {truncate_text(arch['system_overview'], 70)}")
        print()

    # Components
    print_section_header("Components")
    if components:
        for i, comp in enumerate(components, 1):
            name = comp.get("name", f"Component {i}")
            desc = truncate_text(comp.get("description", ""), 60)
            print(f"  [{i}] {name} - {desc}")
    else:
        print("  (No components defined)")
    print()

    # Patterns
    if patterns:
        print_section_header("Patterns")
        for p in patterns:
            name = p.get("name", "Pattern")
            rationale = truncate_text(p.get("rationale", ""), 50)
            print(f"  * {name} - {rationale}")
        print()

    # Decisions (ADRs)
    if decisions:
        print_section_header("Decisions (ADRs)")
        for d in decisions:
            adr_id = d.get("id", "ADR-???")
            title = truncate_text(d.get("title", "Untitled"), 45)
            status = d.get("status", "proposed")
            print(f"  [{adr_id}] {title} ({status})")
        print()


def display_mappings(plan: dict, design: dict, mode: str):
    """Display story/feature to design element mappings."""
    if mode == "mega":
        items = plan.get("features", [])
        mappings = design.get("feature_mappings", {})
        item_label = "Feature"
        id_key = "id"
    else:
        items = plan.get("stories", [])
        mappings = design.get("story_mappings", {})
        item_label = "Story"
        id_key = "id"

    if not items:
        return

    print_section_header(f"{item_label} " + ARROW_DOWN + " Design Mappings")

    headers = [item_label, "Components", "Decisions", "Interfaces"]
    col_widths = [12, 22, 15, 20]
    rows = []
    unmapped = []

    for item in items:
        item_id = item.get(id_key, "?")
        mapping = mappings.get(item_id, {})

        if not mapping:
            unmapped.append(item_id)
            rows.append([item_id, "! Not mapped", "", ""])
        else:
            comps = mapping.get("components", [])
            decs = mapping.get("decisions", [])
            intfs = mapping.get("interfaces", [])

            comp_str = ", ".join(comps[:3]) if comps else "-"
            dec_str = ", ".join(decs[:2]) if decs else "-"
            intf_str = ", ".join(intfs[:2]) if intfs else "-"

            rows.append([item_id, comp_str, dec_str, intf_str])

    print_table(headers, rows, col_widths)
    print()

    return unmapped


def display_summary(plan: dict, design: dict, mode: str, unmapped: list):
    """Display summary section."""
    print(DOUBLE_H * 80)

    if mode == "mega":
        features = plan.get("features", [])
        plan_summary = f"Mega-plan: {len(features)} features"
    else:
        stories = plan.get("stories", [])
        plan_summary = f"PRD: {len(stories)} stories"

    components = design.get("architecture", {}).get("components", [])
    design_summary = f"Design: {len(components)} components"

    if unmapped:
        unmapped_str = f"! Unmapped: {len(unmapped)} ({', '.join(unmapped[:3])}{'...' if len(unmapped) > 3 else ''})"
        print(f"SUMMARY: + {plan_summary} | + {design_summary} | {unmapped_str}")
    else:
        print(f"SUMMARY: + {plan_summary} | + {design_summary} | + All mapped")

    print(DOUBLE_H * 80)
    print()


def display_next_steps(mode: str):
    """Display available next steps."""
    print("Next steps:")
    if mode == "mega":
        print("  - /plan-cascade:mega-approve      Approve and start execution")
        print("  - /plan-cascade:mega-edit         Modify mega-plan")
    else:
        print("  - /plan-cascade:approve           Approve and start execution")
        print("  - /plan-cascade:edit              Modify PRD")
    print("  - /plan-cascade:design-review     Edit design document")
    print("  - /plan-cascade:show-dependencies View dependency graph")
    print()


def display_unified_review(plan: dict, design: dict, mode: str):
    """Display the complete unified review."""
    # Header
    if mode == "mega":
        title = "UNIFIED REVIEW: MEGA-PLAN + DESIGN DOCUMENT"
    else:
        title = "UNIFIED REVIEW: PRD + DESIGN DOCUMENT"

    print()
    print_header(title)

    # Plan section (PRD or Mega-Plan)
    if mode == "mega":
        display_mega_section(plan)
    else:
        display_prd_section(plan)

    # Design section
    display_design_section(design, mode)

    # Mappings
    unmapped = display_mappings(plan, design, mode) or []

    # Summary
    display_summary(plan, design, mode, unmapped)

    # Next steps
    display_next_steps(mode)


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(description="Unified Review Display for Plan Cascade")
    parser.add_argument(
        "--mode",
        choices=["hybrid", "mega", "auto"],
        default="auto",
        help="Display mode: hybrid (PRD+design), mega (mega-plan+design), or auto-detect"
    )
    args = parser.parse_args()

    # Detect or use specified mode
    mode = args.mode if args.mode != "auto" else detect_mode()

    # Load documents
    plan, design = load_documents(mode)

    # Check for missing documents
    if not plan:
        if mode == "mega":
            print("Error: mega-plan.json not found")
        else:
            print("Error: prd.json not found")
        print()
        print("Generate planning documents first:")
        if mode == "mega":
            print("  /plan-cascade:mega-plan \"project description\"")
        else:
            print("  /plan-cascade:hybrid-auto \"task description\"")
        sys.exit(1)

    if not design:
        print("Error: design_doc.json not found")
        print()
        print("Generate design document:")
        print("  /plan-cascade:design-generate")
        sys.exit(1)

    # Display unified review
    display_unified_review(plan, design, mode)


if __name__ == "__main__":
    main()
