"""
Memory Doctor — Decision Conflict Detection and Cleanup

Provides conflict-driven deprecation of architecture decisions (ADRs) across
design documents. Detects conflicts, superseded entries, and semantic duplicates.

Two trigger modes:
- Passive: Scans new decisions against existing ones after design_doc.json generation
- Active: Full diagnosis via /plan-cascade:memory-doctor command
"""

import asyncio
import json
import logging
from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


class DiagnosisType(Enum):
    """Type of decision health issue."""

    CONFLICT = "conflict"  # Contradictory decisions on same concern
    SUPERSEDED = "superseded"  # New decision covers old decision's scope
    DUPLICATE = "duplicate"  # Semantically identical, different wording


@dataclass
class Diagnosis:
    """A single diagnosis finding between two decisions."""

    type: DiagnosisType
    decision_a: dict  # Older/existing decision
    decision_b: dict  # Newer decision
    explanation: str  # LLM-generated conflict explanation
    suggestion: str  # Recommended action (deprecate/merge/skip)
    source_a: str  # Source file path
    source_b: str  # Source file path

    def to_dict(self) -> dict:
        return {
            "type": self.type.value,
            "decision_a_id": self.decision_a.get("id", "unknown"),
            "decision_b_id": self.decision_b.get("id", "unknown"),
            "explanation": self.explanation,
            "suggestion": self.suggestion,
            "source_a": self.source_a,
            "source_b": self.source_b,
        }


# Prompt template for LLM-based decision comparison
_DIAGNOSIS_PROMPT = """\
You are an architecture decision reviewer. Analyze the following Architecture Decision Records (ADRs) \
and identify any issues.

For each pair of decisions that has a problem, classify it as one of:
- "conflict": Two decisions contradict each other on the same topic
- "superseded": A newer decision replaces or covers the scope of an older one
- "duplicate": Two decisions say the same thing in different words

Only report genuine issues. If decisions are about different topics or are compatible, do NOT report them.

Decisions to analyze:
{decisions_json}

Respond with a JSON array (no markdown fencing). Each element:
{{
  "type": "conflict" | "superseded" | "duplicate",
  "decision_a_id": "<id of older/first decision>",
  "decision_b_id": "<id of newer/second decision>",
  "explanation": "<brief explanation of the issue>",
  "suggestion": "<recommended action: deprecate A / merge into A / skip>"
}}

If no issues found, respond with an empty array: []
"""


class MemoryDoctor:
    """
    Diagnoses decision health across design documents.

    Detects conflicts, superseded entries, and semantic duplicates
    among Architecture Decision Records (ADRs).
    """

    # Max decisions for single-call full diagnosis
    BATCH_THRESHOLD = 30

    def __init__(
        self,
        project_root: Path,
        llm_provider: Any | None = None,
    ):
        """
        Initialize MemoryDoctor.

        Args:
            project_root: Root directory of the project
            llm_provider: LLMProvider instance. If None, LLM-based diagnosis is skipped.
        """
        self.project_root = Path(project_root).resolve()
        self.llm_provider = llm_provider

        # Lazy-import PathResolver to avoid circular imports
        from plan_cascade.state.path_resolver import PathResolver

        self.path_resolver = PathResolver(self.project_root)

    async def diagnose_new_decisions(
        self,
        new_decisions: list[dict],
        existing_decisions: list[dict],
        source_label: str = "",
    ) -> list[Diagnosis]:
        """
        Passive trigger: compare new decisions against existing ones.

        Args:
            new_decisions: Newly generated decisions (from current design_doc.json)
            existing_decisions: All previously existing decisions
            source_label: Label for the source of new decisions

        Returns:
            List of Diagnosis findings
        """
        if not new_decisions or not existing_decisions:
            return []

        if not self.llm_provider:
            logger.warning("No LLM provider available — skipping decision diagnosis")
            return []

        # Tag decisions for the LLM
        tagged = []
        for d in existing_decisions:
            entry = {**d, "_role": "existing"}
            tagged.append(entry)
        for d in new_decisions:
            entry = {**d, "_role": "new", "_source_label": source_label}
            tagged.append(entry)

        return await self._run_diagnosis(tagged)

    async def full_diagnosis(
        self,
        all_decisions: list[dict] | None = None,
    ) -> list[Diagnosis]:
        """
        Active trigger: full pairwise scan of all decisions.

        Args:
            all_decisions: All decisions to analyze. If None, collects from project.

        Returns:
            List of Diagnosis findings
        """
        if all_decisions is None:
            all_decisions = self.collect_all_decisions()

        if len(all_decisions) < 2:
            return []

        if not self.llm_provider:
            logger.warning("No LLM provider available — skipping decision diagnosis")
            return []

        # If under threshold, single call; otherwise batch by category
        if len(all_decisions) <= self.BATCH_THRESHOLD:
            return await self._run_diagnosis(all_decisions)

        return await self._run_batched_diagnosis(all_decisions)

    def collect_all_decisions(self) -> list[dict]:
        """
        Collect all decisions from project root and worktree design documents.

        Returns:
            List of decision dicts, each annotated with '_source' field
        """
        results: list[dict] = []

        # 1. Project root design_doc.json
        root_doc = self.project_root / "design_doc.json"
        results.extend(self._load_decisions_from(root_doc))

        # 2. PathResolver project dir design_doc.json
        try:
            project_dir = self.path_resolver.get_project_dir()
            proj_doc = project_dir / "design_doc.json"
            if proj_doc != root_doc:
                results.extend(self._load_decisions_from(proj_doc))
        except Exception:
            pass

        # 3. Worktree directories
        try:
            worktree_dir = self.path_resolver.get_worktree_dir()
            if worktree_dir.exists():
                for wt in worktree_dir.iterdir():
                    if wt.is_dir():
                        wt_doc = wt / "design_doc.json"
                        results.extend(self._load_decisions_from(wt_doc))
        except Exception:
            pass

        # 4. Subdirectories that might contain feature design docs
        for child in self.project_root.iterdir():
            if child.is_dir() and not child.name.startswith("."):
                child_doc = child / "design_doc.json"
                if child_doc.exists() and child_doc != root_doc:
                    results.extend(self._load_decisions_from(child_doc))

        return results

    def _load_decisions_from(self, filepath: Path) -> list[dict]:
        """Load decisions from a design_doc.json file."""
        if not filepath.exists():
            return []
        try:
            with open(filepath, encoding="utf-8-sig") as f:
                data = json.load(f)
            decisions = data.get("decisions", [])
            for d in decisions:
                d["_source"] = str(filepath)
            return decisions
        except (json.JSONDecodeError, OSError) as e:
            logger.warning("Failed to load %s: %s", filepath, e)
            return []

    def format_report(self, diagnoses: list[Diagnosis], total_scanned: int = 0, source_count: int = 0) -> str:
        """
        Format diagnoses into a Markdown-compatible report with box-drawing.

        Args:
            diagnoses: List of Diagnosis findings
            total_scanned: Total number of decisions scanned
            source_count: Number of source files scanned

        Returns:
            Formatted report string
        """
        lines: list[str] = []
        w = 62  # inner width

        # Header box
        h_line = "\u2500" * w
        lines.append(f"\u250c{h_line}\u2510")
        lines.append(f"\u2502  MEMORY DOCTOR \u2014 \u51b3\u7b56\u8bca\u65ad\u62a5\u544a{' ' * (w - 27)}\u2502")
        lines.append(f"\u251c{h_line}\u2524")
        summary = f"  Scanned: {total_scanned} decisions from {source_count} sources | Found: {len(diagnoses)} issues"
        lines.append(f"\u2502{summary:<{w}}\u2502")
        lines.append(f"\u2514{h_line}\u2518")
        lines.append("")

        if not diagnoses:
            lines.append("  \u2705 No issues found. All decisions are healthy.")
            lines.append("")
            lines.append("\u2550" * (w + 2))
            return "\n".join(lines)

        # Group by type
        conflicts = [d for d in diagnoses if d.type == DiagnosisType.CONFLICT]
        superseded = [d for d in diagnoses if d.type == DiagnosisType.SUPERSEDED]
        duplicates = [d for d in diagnoses if d.type == DiagnosisType.DUPLICATE]

        if conflicts:
            lines.append(f"  \U0001f534 CONFLICT ({len(conflicts)})")
            lines.append("")
            for d in conflicts:
                lines.extend(self._format_diagnosis_entry(d, "vs"))
            lines.append("")

        if superseded:
            lines.append(f"  \U0001f7e0 SUPERSEDED ({len(superseded)})")
            lines.append("")
            for d in superseded:
                lines.extend(self._format_diagnosis_entry(d, "\u2192"))
            lines.append("")

        if duplicates:
            lines.append(f"  \U0001f7e1 DUPLICATE ({len(duplicates)})")
            lines.append("")
            for d in duplicates:
                lines.extend(self._format_diagnosis_entry(d, "\u2248"))
            lines.append("")

        lines.append("\u2550" * (w + 2))
        return "\n".join(lines)

    def _format_diagnosis_entry(self, d: Diagnosis, separator: str) -> list[str]:
        """Format a single diagnosis entry."""
        id_a = d.decision_a.get("id", "?")
        id_b = d.decision_b.get("id", "?")
        title_a = d.decision_a.get("title", d.decision_a.get("decision", ""))
        title_b = d.decision_b.get("title", d.decision_b.get("decision", ""))

        lines = [
            f"  {id_a} {separator} {id_b}:",
            f"    \u65e7: \"{title_a}\" ({d.source_a})",
            f"    \u65b0: \"{title_b}\" ({d.source_b})",
            f"    \u8bf4\u660e: {d.explanation}",
            f"    \u5efa\u8bae: {d.suggestion}",
            "",
        ]
        return lines

    def apply_action(self, diagnosis: Diagnosis, action: str) -> None:
        """
        Apply a user-chosen action to resolve a diagnosis.

        Args:
            diagnosis: The diagnosis to resolve
            action: One of "deprecate", "merge", "skip"
        """
        if action == "skip":
            return

        if action == "deprecate":
            self._deprecate_decision(diagnosis)
        elif action == "merge":
            self._merge_decisions(diagnosis)
        else:
            logger.warning("Unknown action: %s", action)

    def _deprecate_decision(self, diagnosis: Diagnosis) -> None:
        """Mark decision_a as deprecated by decision_b."""
        source_path = Path(diagnosis.source_a)
        if not source_path.exists():
            logger.error("Source file not found: %s", source_path)
            return

        try:
            with open(source_path, encoding="utf-8-sig") as f:
                data = json.load(f)
        except (json.JSONDecodeError, OSError) as e:
            logger.error("Failed to read %s: %s", source_path, e)
            return

        target_id = diagnosis.decision_a.get("id")
        deprecator_id = diagnosis.decision_b.get("id", "unknown")
        now = datetime.now(timezone.utc).isoformat()

        for dec in data.get("decisions", []):
            if dec.get("id") == target_id:
                dec["status"] = "deprecated"
                dec["deprecated_by"] = deprecator_id
                dec["deprecated_at"] = now
                break

        with open(source_path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)

        logger.info("Deprecated %s (by %s) in %s", target_id, deprecator_id, source_path)

    def _merge_decisions(self, diagnosis: Diagnosis) -> None:
        """Merge decision_b into decision_a, removing decision_b."""
        # Update decision_a's rationale with merge note
        source_a = Path(diagnosis.source_a)
        source_b = Path(diagnosis.source_b)

        if not source_a.exists() or not source_b.exists():
            logger.error("Source file(s) not found for merge")
            return

        # Update A: append merge note
        try:
            with open(source_a, encoding="utf-8-sig") as f:
                data_a = json.load(f)
        except (json.JSONDecodeError, OSError):
            return

        target_id_a = diagnosis.decision_a.get("id")
        target_id_b = diagnosis.decision_b.get("id")
        now = datetime.now(timezone.utc).isoformat()

        for dec in data_a.get("decisions", []):
            if dec.get("id") == target_id_a:
                merge_note = f"\n[Merged from {target_id_b} on {now}]"
                dec["rationale"] = dec.get("rationale", "") + merge_note
                break

        with open(source_a, "w", encoding="utf-8") as f:
            json.dump(data_a, f, indent=2, ensure_ascii=False)

        # Remove B from its source file
        try:
            with open(source_b, encoding="utf-8-sig") as f:
                data_b = json.load(f)
        except (json.JSONDecodeError, OSError):
            return

        data_b["decisions"] = [
            d for d in data_b.get("decisions", []) if d.get("id") != target_id_b
        ]

        with open(source_b, "w", encoding="utf-8") as f:
            json.dump(data_b, f, indent=2, ensure_ascii=False)

        logger.info("Merged %s into %s", target_id_b, target_id_a)

    async def _run_diagnosis(self, decisions: list[dict]) -> list[Diagnosis]:
        """Run LLM diagnosis on a list of decisions."""
        # Strip internal fields for the prompt
        clean = []
        for d in decisions:
            entry = {k: v for k, v in d.items() if not k.startswith("_")}
            # Keep _source and _role as context for the LLM
            if "_source" in d:
                entry["source_file"] = d["_source"]
            if "_role" in d:
                entry["role"] = d["_role"]
            clean.append(entry)

        prompt = _DIAGNOSIS_PROMPT.format(decisions_json=json.dumps(clean, indent=2, ensure_ascii=False))

        try:
            response = await self.llm_provider.complete(
                messages=[
                    {"role": "user", "content": prompt},
                ],
                temperature=0.2,
                max_tokens=4096,
            )
        except Exception as e:
            logger.error("LLM diagnosis failed: %s", e)
            return []

        return self._parse_llm_response(response.content, decisions)

    async def _run_batched_diagnosis(self, all_decisions: list[dict]) -> list[Diagnosis]:
        """Run diagnosis in batches grouped by decision category/topic."""
        # Group by applies_to or by source file as a proxy for topic
        groups: dict[str, list[dict]] = {}
        for d in all_decisions:
            # Use first applies_to value, or source file as group key
            applies = d.get("applies_to", [])
            if applies:
                key = applies[0] if isinstance(applies, list) else str(applies)
            else:
                key = d.get("_source", "default")
            groups.setdefault(key, []).append(d)

        # Also do cross-group comparison for decisions from different sources
        # by collecting one representative batch
        all_results: list[Diagnosis] = []

        for group_key, group_decisions in groups.items():
            if len(group_decisions) >= 2:
                results = await self._run_diagnosis(group_decisions)
                all_results.extend(results)

        # Cross-group: compare decisions from different sources
        sources = list(groups.keys())
        if len(sources) >= 2:
            # Take up to BATCH_THRESHOLD decisions across all groups
            cross_batch: list[dict] = []
            for group_decisions in groups.values():
                cross_batch.extend(group_decisions[:5])  # Up to 5 per group
                if len(cross_batch) >= self.BATCH_THRESHOLD:
                    break
            if len(cross_batch) >= 2:
                cross_results = await self._run_diagnosis(cross_batch)
                # Deduplicate against existing results
                existing_pairs = {
                    (r.decision_a.get("id"), r.decision_b.get("id")) for r in all_results
                }
                for r in cross_results:
                    pair = (r.decision_a.get("id"), r.decision_b.get("id"))
                    if pair not in existing_pairs:
                        all_results.append(r)

        return all_results

    def _parse_llm_response(self, content: str, decisions: list[dict]) -> list[Diagnosis]:
        """Parse LLM response into Diagnosis objects."""
        # Build lookup by ID
        by_id: dict[str, dict] = {}
        for d in decisions:
            did = d.get("id")
            if did:
                by_id[did] = d

        # Extract JSON from response (handle markdown fencing)
        text = content.strip()
        if text.startswith("```"):
            # Remove markdown code fencing
            lines = text.split("\n")
            # Remove first and last lines (``` markers)
            lines = [line for line in lines if not line.strip().startswith("```")]
            text = "\n".join(lines)

        try:
            findings = json.loads(text)
        except json.JSONDecodeError:
            logger.warning("Failed to parse LLM diagnosis response as JSON")
            return []

        if not isinstance(findings, list):
            return []

        results: list[Diagnosis] = []
        for f in findings:
            dtype_str = f.get("type", "")
            try:
                dtype = DiagnosisType(dtype_str)
            except ValueError:
                continue

            id_a = f.get("decision_a_id", "")
            id_b = f.get("decision_b_id", "")
            dec_a = by_id.get(id_a, {"id": id_a})
            dec_b = by_id.get(id_b, {"id": id_b})

            results.append(Diagnosis(
                type=dtype,
                decision_a=dec_a,
                decision_b=dec_b,
                explanation=f.get("explanation", ""),
                suggestion=f.get("suggestion", ""),
                source_a=dec_a.get("_source", "unknown"),
                source_b=dec_b.get("_source", "unknown"),
            ))

        return results
