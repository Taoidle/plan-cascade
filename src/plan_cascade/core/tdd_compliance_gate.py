"""
TDD Compliance Gate for Plan Cascade.

Quality gate that checks TDD compliance for story implementations.
Verifies that stories have appropriate test changes when TDD mode is enabled.
"""

from datetime import datetime
from pathlib import Path
from typing import Any

from .quality_gate import Gate, GateConfig, GateOutput, GateType
from .tdd_support import (
    TDDConfig,
    check_tdd_compliance,
    get_tdd_config_from_prd,
)


class TDDComplianceGate(Gate):
    """
    TDD Compliance gate that validates test coverage for story changes.

    This gate checks whether stories have appropriate test changes when
    TDD mode is enabled. It integrates with the TDD support module for
    configuration and compliance checking.
    """

    def execute(self, story_id: str, context: dict[str, Any]) -> GateOutput:
        """
        Execute TDD compliance check.

        Args:
            story_id: ID of the story being verified
            context: Additional context including:
                - story: The story dictionary
                - changed_files: List of files changed during implementation
                - prd: The PRD dictionary (for TDD config)
                - tdd_config: Optional explicit TDD configuration

        Returns:
            GateOutput with compliance check results
        """
        start_time = datetime.now()

        # Get story from context
        story = context.get("story", {})
        if not story:
            # Try to find story by ID from PRD
            prd = context.get("prd", {})
            stories = prd.get("stories", [])
            for s in stories:
                if s.get("id") == story_id:
                    story = s
                    break

        if not story:
            return GateOutput(
                gate_name=self.config.name,
                gate_type=GateType.TDD_COMPLIANCE,
                passed=True,  # Pass if no story found (can't validate)
                exit_code=0,
                stdout="No story found for TDD compliance check",
                stderr="",
                duration_seconds=0.0,
                command="tdd_compliance_check",
                error_summary=None,
            )

        # Get TDD configuration
        tdd_config = context.get("tdd_config")
        if tdd_config is None:
            prd = context.get("prd", {})
            tdd_config = get_tdd_config_from_prd(prd)

        # Get changed files
        changed_files = context.get("changed_files", [])

        # Get existing gate outputs for additional context
        gate_outputs = context.get("gate_outputs", {})

        # Run TDD compliance check
        result = check_tdd_compliance(
            story=story,
            changed_files=changed_files,
            config=tdd_config,
            gate_outputs=gate_outputs,
        )

        duration = (datetime.now() - start_time).total_seconds()

        # Build stdout/stderr from result
        stdout_parts = []
        if result.passed:
            stdout_parts.append(f"TDD compliance check passed for story {story_id}")
            if result.details.get("message"):
                stdout_parts.append(result.details["message"])
        else:
            stdout_parts.append(f"TDD compliance check failed for story {story_id}")

        if result.details.get("test_files_changed", 0) > 0:
            stdout_parts.append(f"Test files changed: {result.details['test_files_changed']}")
        if result.details.get("code_files_changed", 0) > 0:
            stdout_parts.append(f"Code files changed: {result.details['code_files_changed']}")

        stderr_parts = []
        for error in result.errors:
            stderr_parts.append(f"ERROR: {error}")
        for warning in result.warnings:
            stderr_parts.append(f"WARNING: {warning}")

        # Build error summary
        error_summary = None
        if not result.passed:
            if result.errors:
                error_summary = "; ".join(result.errors[:3])
            elif result.warnings:
                error_summary = "; ".join(result.warnings[:3])

        return GateOutput(
            gate_name=self.config.name,
            gate_type=GateType.TDD_COMPLIANCE,
            passed=result.passed,
            exit_code=0 if result.passed else 1,
            stdout="\n".join(stdout_parts),
            stderr="\n".join(stderr_parts),
            duration_seconds=duration,
            command="tdd_compliance_check",
            error_summary=error_summary,
            checked_files=changed_files if changed_files else None,
        )


def create_tdd_gate_config(
    name: str = "tdd_compliance",
    enabled: bool = True,
    required: bool = False,
) -> GateConfig:
    """
    Create a GateConfig for TDD compliance gate.

    Args:
        name: Name for the gate
        enabled: Whether the gate is enabled
        required: Whether gate failure blocks progression

    Returns:
        GateConfig for TDD compliance gate
    """
    return GateConfig(
        name=name,
        type=GateType.TDD_COMPLIANCE,
        enabled=enabled,
        required=required,
    )
