#!/usr/bin/env python3
"""
Stage State Machine for Plan Cascade

Provides unified stage tracking for all execution strategies (DIRECT, HYBRID_AUTO,
HYBRID_WORKTREE, MEGA_PLAN). Implements the 8-stage execution model:
Intake → Analyze → Plan → Design → ReadyCheck → Execute → Verify & Review → WrapUp

This module follows ADR-002: Stage state persisted to independent .state/stage-state.json file.
"""

from __future__ import annotations

import json
import time
from dataclasses import dataclass, field, asdict
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any, TYPE_CHECKING, Callable

if TYPE_CHECKING:
    from ..state.path_resolver import PathResolver


class ExecutionStage(str, Enum):
    """
    Execution stages for Plan Cascade workflows.

    The 8 stages represent distinct phases in the execution lifecycle:
    - INTAKE: Collect input and context
    - ANALYZE: Analyze complexity, select strategy and Flow
    - PLAN: Generate plan (PRD/Mega-Plan/minimal plan)
    - DESIGN: Generate/validate design document
    - READY_CHECK: DoR gate check
    - EXECUTE: Batch parallel execution
    - VERIFY_REVIEW: Quality gate + AI review
    - WRAP_UP: Summary and completion
    """
    INTAKE = "intake"
    ANALYZE = "analyze"
    PLAN = "plan"
    DESIGN = "design"
    READY_CHECK = "ready_check"
    EXECUTE = "execute"
    VERIFY_REVIEW = "verify_review"
    WRAP_UP = "wrap_up"

    @classmethod
    def get_order(cls) -> list["ExecutionStage"]:
        """Get stages in execution order."""
        return [
            cls.INTAKE,
            cls.ANALYZE,
            cls.PLAN,
            cls.DESIGN,
            cls.READY_CHECK,
            cls.EXECUTE,
            cls.VERIFY_REVIEW,
            cls.WRAP_UP,
        ]

    @classmethod
    def get_index(cls, stage: "ExecutionStage") -> int:
        """Get the index of a stage in execution order."""
        return cls.get_order().index(stage)

    def next_stage(self) -> "ExecutionStage | None":
        """Get the next stage in sequence, or None if at end."""
        order = ExecutionStage.get_order()
        idx = order.index(self)
        if idx < len(order) - 1:
            return order[idx + 1]
        return None

    def previous_stage(self) -> "ExecutionStage | None":
        """Get the previous stage in sequence, or None if at start."""
        order = ExecutionStage.get_order()
        idx = order.index(self)
        if idx > 0:
            return order[idx - 1]
        return None


class StageStatus(str, Enum):
    """
    Status of a stage in the execution lifecycle.

    - PENDING: Stage has not started
    - IN_PROGRESS: Stage is currently executing
    - COMPLETED: Stage finished successfully
    - FAILED: Stage failed with errors
    - SKIPPED: Stage was skipped (e.g., Quick Flow skipping Design)
    """
    PENDING = "pending"
    IN_PROGRESS = "in_progress"
    COMPLETED = "completed"
    FAILED = "failed"
    SKIPPED = "skipped"


@dataclass
class StageState:
    """
    State of a single execution stage.

    Attributes:
        stage: The execution stage
        status: Current status of the stage
        started_at: ISO-8601 timestamp when stage started
        completed_at: ISO-8601 timestamp when stage completed (None if not complete)
        outputs: Dictionary of stage outputs (varies by stage)
        errors: List of error messages if stage failed
    """
    stage: ExecutionStage
    status: StageStatus = StageStatus.PENDING
    started_at: str | None = None
    completed_at: str | None = None
    outputs: dict[str, Any] = field(default_factory=dict)
    errors: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "stage": self.stage.value,
            "status": self.status.value,
            "started_at": self.started_at,
            "completed_at": self.completed_at,
            "outputs": self.outputs,
            "errors": self.errors,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "StageState":
        """Create from dictionary (JSON deserialization)."""
        return cls(
            stage=ExecutionStage(data["stage"]),
            status=StageStatus(data["status"]),
            started_at=data.get("started_at"),
            completed_at=data.get("completed_at"),
            outputs=data.get("outputs", {}),
            errors=data.get("errors", []),
        )

    def is_terminal(self) -> bool:
        """Check if stage is in a terminal state (completed, failed, or skipped)."""
        return self.status in (StageStatus.COMPLETED, StageStatus.FAILED, StageStatus.SKIPPED)

    def can_start(self) -> bool:
        """Check if stage can be started."""
        return self.status == StageStatus.PENDING

    def elapsed_time(self) -> float | None:
        """Get elapsed time in seconds, or None if not applicable."""
        if not self.started_at:
            return None

        start = datetime.fromisoformat(self.started_at.replace("Z", "+00:00"))

        if self.completed_at:
            end = datetime.fromisoformat(self.completed_at.replace("Z", "+00:00"))
        else:
            end = datetime.now(start.tzinfo)

        return (end - start).total_seconds()


@dataclass
class StageInput:
    """
    Defines the required inputs for a stage.

    Attributes:
        name: Name of the input
        description: Human-readable description
        required: Whether this input is mandatory
        source_stage: Which stage produces this input (None for external)
    """
    name: str
    description: str
    required: bool = True
    source_stage: ExecutionStage | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "name": self.name,
            "description": self.description,
            "required": self.required,
            "source_stage": self.source_stage.value if self.source_stage else None,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "StageInput":
        """Create from dictionary (JSON deserialization)."""
        source = data.get("source_stage")
        return cls(
            name=data["name"],
            description=data["description"],
            required=data.get("required", True),
            source_stage=ExecutionStage(source) if source else None,
        )


@dataclass
class StageOutput:
    """
    Defines the expected outputs from a stage.

    Attributes:
        name: Name of the output
        description: Human-readable description
        required: Whether this output is mandatory for stage completion
    """
    name: str
    description: str
    required: bool = True

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "name": self.name,
            "description": self.description,
            "required": self.required,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "StageOutput":
        """Create from dictionary (JSON deserialization)."""
        return cls(
            name=data["name"],
            description=data["description"],
            required=data.get("required", True),
        )


# Type alias for acceptance check functions
AcceptanceCheck = Callable[[dict[str, Any]], tuple[bool, list[str]]]


@dataclass
class StageContract:
    """
    Contract defining inputs, outputs, and acceptance criteria for a stage.

    Attributes:
        stage: The execution stage this contract is for
        required_inputs: List of required inputs for this stage
        expected_outputs: List of expected outputs from this stage
        acceptance_check: Function to validate stage completion
        skippable: Whether this stage can be skipped (e.g., in Quick Flow)
        skip_conditions: Description of when this stage can be skipped
    """
    stage: ExecutionStage
    required_inputs: list[StageInput] = field(default_factory=list)
    expected_outputs: list[StageOutput] = field(default_factory=list)
    acceptance_check: AcceptanceCheck | None = None
    skippable: bool = False
    skip_conditions: str = ""

    def validate_inputs(self, inputs: dict[str, Any]) -> tuple[bool, list[str]]:
        """
        Validate that all required inputs are present.

        Args:
            inputs: Dictionary of input values

        Returns:
            Tuple of (is_valid, list of missing input names)
        """
        missing = []
        for inp in self.required_inputs:
            if inp.required and inp.name not in inputs:
                missing.append(inp.name)
        return len(missing) == 0, missing

    def validate_outputs(self, outputs: dict[str, Any]) -> tuple[bool, list[str]]:
        """
        Validate that all required outputs are present.

        Args:
            outputs: Dictionary of output values

        Returns:
            Tuple of (is_valid, list of missing output names)
        """
        missing = []
        for out in self.expected_outputs:
            if out.required and out.name not in outputs:
                missing.append(out.name)
        return len(missing) == 0, missing

    def check_acceptance(self, outputs: dict[str, Any]) -> tuple[bool, list[str]]:
        """
        Run acceptance check if defined.

        Args:
            outputs: Stage outputs to validate

        Returns:
            Tuple of (passed, list of failure messages)
        """
        if self.acceptance_check is None:
            return True, []
        return self.acceptance_check(outputs)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization (excluding callable)."""
        return {
            "stage": self.stage.value,
            "required_inputs": [i.to_dict() for i in self.required_inputs],
            "expected_outputs": [o.to_dict() for o in self.expected_outputs],
            "skippable": self.skippable,
            "skip_conditions": self.skip_conditions,
        }


class StageContractRegistry:
    """
    Registry of stage contracts for all execution stages.

    Provides centralized access to stage contracts with predefined
    inputs, outputs, and acceptance criteria for each stage.
    """

    def __init__(self) -> None:
        """Initialize the registry with default contracts."""
        self._contracts: dict[ExecutionStage, StageContract] = {}
        self._register_default_contracts()

    def _register_default_contracts(self) -> None:
        """Register default contracts for all stages."""
        # INTAKE stage
        self._contracts[ExecutionStage.INTAKE] = StageContract(
            stage=ExecutionStage.INTAKE,
            required_inputs=[
                StageInput("task_description", "User's task description"),
            ],
            expected_outputs=[
                StageOutput("context", "Collected context from codebase"),
                StageOutput("task_normalized", "Normalized task description"),
            ],
            skippable=False,
        )

        # ANALYZE stage
        self._contracts[ExecutionStage.ANALYZE] = StageContract(
            stage=ExecutionStage.ANALYZE,
            required_inputs=[
                StageInput("context", "Collected context", source_stage=ExecutionStage.INTAKE),
                StageInput("task_normalized", "Normalized task", source_stage=ExecutionStage.INTAKE),
            ],
            expected_outputs=[
                StageOutput("strategy", "Selected execution strategy"),
                StageOutput("flow", "Selected execution flow (Quick/Standard/Full)"),
                StageOutput("confidence", "Strategy confidence score"),
                StageOutput("reasoning", "Strategy selection reasoning"),
            ],
            skippable=False,
        )

        # PLAN stage
        self._contracts[ExecutionStage.PLAN] = StageContract(
            stage=ExecutionStage.PLAN,
            required_inputs=[
                StageInput("strategy", "Selected strategy", source_stage=ExecutionStage.ANALYZE),
                StageInput("context", "Collected context", source_stage=ExecutionStage.INTAKE),
            ],
            expected_outputs=[
                StageOutput("plan_type", "Type of plan generated (prd/mega-plan/minimal)"),
                StageOutput("plan_path", "Path to generated plan file"),
                StageOutput("stories_count", "Number of stories/features in plan", required=False),
            ],
            skippable=False,
        )

        # DESIGN stage
        self._contracts[ExecutionStage.DESIGN] = StageContract(
            stage=ExecutionStage.DESIGN,
            required_inputs=[
                StageInput("plan_path", "Path to plan file", source_stage=ExecutionStage.PLAN),
                StageInput("strategy", "Selected strategy", source_stage=ExecutionStage.ANALYZE),
            ],
            expected_outputs=[
                StageOutput("design_path", "Path to design document"),
                StageOutput("components", "List of components defined", required=False),
            ],
            skippable=True,
            skip_conditions="Quick Flow, DIRECT strategy with low complexity",
        )

        # READY_CHECK stage
        self._contracts[ExecutionStage.READY_CHECK] = StageContract(
            stage=ExecutionStage.READY_CHECK,
            required_inputs=[
                StageInput("plan_path", "Path to plan file", source_stage=ExecutionStage.PLAN),
                StageInput("flow", "Execution flow", source_stage=ExecutionStage.ANALYZE),
            ],
            expected_outputs=[
                StageOutput("dor_passed", "Whether DoR checks passed"),
                StageOutput("warnings", "List of warnings", required=False),
                StageOutput("blockers", "List of blockers (if failed)", required=False),
            ],
            skippable=True,
            skip_conditions="Quick Flow uses soft mode (warnings only)",
        )

        # EXECUTE stage
        self._contracts[ExecutionStage.EXECUTE] = StageContract(
            stage=ExecutionStage.EXECUTE,
            required_inputs=[
                StageInput("plan_path", "Path to plan file", source_stage=ExecutionStage.PLAN),
                StageInput("dor_passed", "DoR check result", source_stage=ExecutionStage.READY_CHECK),
            ],
            expected_outputs=[
                StageOutput("completed_stories", "List of completed story IDs"),
                StageOutput("failed_stories", "List of failed story IDs", required=False),
                StageOutput("batches_executed", "Number of batches executed"),
            ],
            skippable=False,
        )

        # VERIFY_REVIEW stage
        self._contracts[ExecutionStage.VERIFY_REVIEW] = StageContract(
            stage=ExecutionStage.VERIFY_REVIEW,
            required_inputs=[
                StageInput("completed_stories", "Completed stories", source_stage=ExecutionStage.EXECUTE),
                StageInput("flow", "Execution flow", source_stage=ExecutionStage.ANALYZE),
            ],
            expected_outputs=[
                StageOutput("quality_gate_passed", "Whether quality gate passed"),
                StageOutput("verification_passed", "Whether AI verification passed"),
                StageOutput("review_required", "Whether human review is required", required=False),
            ],
            skippable=False,
        )

        # WRAP_UP stage
        self._contracts[ExecutionStage.WRAP_UP] = StageContract(
            stage=ExecutionStage.WRAP_UP,
            required_inputs=[
                StageInput("completed_stories", "Completed stories", source_stage=ExecutionStage.EXECUTE),
                StageInput("quality_gate_passed", "Quality gate result", source_stage=ExecutionStage.VERIFY_REVIEW),
            ],
            expected_outputs=[
                StageOutput("summary", "Execution summary"),
                StageOutput("change_log", "List of changes made", required=False),
            ],
            skippable=False,
        )

    def get_contract(self, stage: ExecutionStage) -> StageContract:
        """
        Get the contract for a stage.

        Args:
            stage: The execution stage

        Returns:
            StageContract for the stage

        Raises:
            KeyError: If no contract is registered for the stage
        """
        if stage not in self._contracts:
            raise KeyError(f"No contract registered for stage: {stage}")
        return self._contracts[stage]

    def register_contract(self, contract: StageContract) -> None:
        """
        Register or override a stage contract.

        Args:
            contract: The contract to register
        """
        self._contracts[contract.stage] = contract

    def get_all_contracts(self) -> dict[ExecutionStage, StageContract]:
        """Get all registered contracts."""
        return self._contracts.copy()


# Global registry instance
_default_registry: StageContractRegistry | None = None


def get_contract_registry() -> StageContractRegistry:
    """Get the default contract registry (singleton)."""
    global _default_registry
    if _default_registry is None:
        _default_registry = StageContractRegistry()
    return _default_registry


class StageStateMachine:
    """
    State machine for tracking execution stages.

    Manages stage transitions, validates contracts, and provides
    persistence and recovery capabilities.

    Attributes:
        execution_id: Unique identifier for this execution
        strategy: Selected execution strategy
        flow: Selected execution flow
        stages: Dictionary of stage states
        stages_history: List of all state transitions
    """

    VERSION = "1.0.0"
    STATE_FILE_NAME = "stage-state.json"

    def __init__(
        self,
        execution_id: str,
        strategy: str | None = None,
        flow: str | None = None,
        skippable_stages: list[ExecutionStage] | None = None,
        contract_registry: StageContractRegistry | None = None,
    ) -> None:
        """
        Initialize the stage state machine.

        Args:
            execution_id: Unique identifier for this execution
            strategy: Selected execution strategy
            flow: Selected execution flow (Quick/Standard/Full)
            skippable_stages: List of stages that should be skipped
            contract_registry: Custom contract registry (uses default if None)
        """
        self.execution_id = execution_id
        self.strategy = strategy
        self.flow = flow
        self._skippable_stages = set(skippable_stages or [])
        self._registry = contract_registry or get_contract_registry()

        # Initialize all stages
        self._stages: dict[ExecutionStage, StageState] = {}
        for stage in ExecutionStage.get_order():
            self._stages[stage] = StageState(stage=stage)

        # History of all transitions
        self._history: list[dict[str, Any]] = []

        # Timestamps
        self._created_at = time.strftime("%Y-%m-%dT%H:%M:%SZ")
        self._updated_at = self._created_at

    @property
    def current_stage(self) -> ExecutionStage | None:
        """
        Get the current active stage.

        Returns:
            The stage currently in progress, or None if no stage is active
        """
        for stage in ExecutionStage.get_order():
            state = self._stages[stage]
            if state.status == StageStatus.IN_PROGRESS:
                return stage
        return None

    @property
    def last_completed_stage(self) -> ExecutionStage | None:
        """
        Get the last completed stage.

        Returns:
            The most recent completed stage, or None if none completed
        """
        for stage in reversed(ExecutionStage.get_order()):
            state = self._stages[stage]
            if state.status in (StageStatus.COMPLETED, StageStatus.SKIPPED):
                return stage
        return None

    @property
    def next_pending_stage(self) -> ExecutionStage | None:
        """
        Get the next stage that hasn't started yet.

        Returns:
            The next pending stage, or None if all stages are complete
        """
        for stage in ExecutionStage.get_order():
            state = self._stages[stage]
            if state.status == StageStatus.PENDING:
                return stage
        return None

    @property
    def stages(self) -> dict[ExecutionStage, StageState]:
        """Get all stage states."""
        return self._stages.copy()

    @property
    def stages_history(self) -> list[dict[str, Any]]:
        """Get the history of all state transitions."""
        return self._history.copy()

    def get_stage_state(self, stage: ExecutionStage) -> StageState:
        """
        Get the state of a specific stage.

        Args:
            stage: The stage to query

        Returns:
            StageState for the stage
        """
        return self._stages[stage]

    def can_transition_to(self, stage: ExecutionStage) -> tuple[bool, str]:
        """
        Check if transition to a stage is valid.

        Args:
            stage: Target stage

        Returns:
            Tuple of (can_transition, reason if not)
        """
        current = self.current_stage
        target_state = self._stages[stage]

        # Can't transition to already completed/failed stage
        if target_state.is_terminal():
            return False, f"Stage {stage.value} is already {target_state.status.value}"

        # Can't start a new stage while another is in progress
        if current is not None and current != stage:
            return False, f"Stage {current.value} is currently in progress"

        # Check stage order (can't skip required stages)
        target_idx = ExecutionStage.get_index(stage)
        for prev_stage in ExecutionStage.get_order()[:target_idx]:
            prev_state = self._stages[prev_stage]
            if prev_state.status == StageStatus.PENDING:
                # Check if previous stage can be skipped
                if prev_stage in self._skippable_stages:
                    continue
                contract = self._registry.get_contract(prev_stage)
                if contract.skippable:
                    continue
                return False, f"Previous stage {prev_stage.value} is not complete"

        return True, ""

    def start_stage(self, stage: ExecutionStage, inputs: dict[str, Any] | None = None) -> bool:
        """
        Start a stage.

        Args:
            stage: Stage to start
            inputs: Optional inputs to validate against contract

        Returns:
            True if stage was started successfully

        Raises:
            ValueError: If transition is not valid or inputs are missing
        """
        can_start, reason = self.can_transition_to(stage)
        if not can_start:
            raise ValueError(f"Cannot start stage {stage.value}: {reason}")

        # Validate inputs if provided
        if inputs:
            contract = self._registry.get_contract(stage)
            valid, missing = contract.validate_inputs(inputs)
            if not valid:
                raise ValueError(f"Missing required inputs for {stage.value}: {missing}")

        # Mark any skippable previous stages as skipped
        target_idx = ExecutionStage.get_index(stage)
        for prev_stage in ExecutionStage.get_order()[:target_idx]:
            prev_state = self._stages[prev_stage]
            if prev_state.status == StageStatus.PENDING:
                if prev_stage in self._skippable_stages:
                    self._skip_stage(prev_stage, "Configured as skippable")

        # Start the stage
        state = self._stages[stage]
        state.status = StageStatus.IN_PROGRESS
        state.started_at = time.strftime("%Y-%m-%dT%H:%M:%SZ")

        self._record_transition(stage, StageStatus.IN_PROGRESS)
        return True

    def complete_stage(
        self,
        stage: ExecutionStage,
        outputs: dict[str, Any] | None = None,
    ) -> bool:
        """
        Complete a stage.

        Args:
            stage: Stage to complete
            outputs: Stage outputs to validate and store

        Returns:
            True if stage was completed successfully

        Raises:
            ValueError: If stage is not in progress or outputs are missing
        """
        state = self._stages[stage]

        if state.status != StageStatus.IN_PROGRESS:
            raise ValueError(f"Stage {stage.value} is not in progress (status: {state.status.value})")

        # Validate outputs if provided
        if outputs:
            contract = self._registry.get_contract(stage)
            valid, missing = contract.validate_outputs(outputs)
            if not valid:
                raise ValueError(f"Missing required outputs for {stage.value}: {missing}")

            # Run acceptance check
            passed, failures = contract.check_acceptance(outputs)
            if not passed:
                raise ValueError(f"Acceptance check failed for {stage.value}: {failures}")

            state.outputs = outputs

        state.status = StageStatus.COMPLETED
        state.completed_at = time.strftime("%Y-%m-%dT%H:%M:%SZ")

        self._record_transition(stage, StageStatus.COMPLETED)
        return True

    def fail_stage(self, stage: ExecutionStage, errors: list[str]) -> bool:
        """
        Mark a stage as failed.

        Args:
            stage: Stage that failed
            errors: List of error messages

        Returns:
            True if stage was marked as failed

        Raises:
            ValueError: If stage is not in progress
        """
        state = self._stages[stage]

        if state.status != StageStatus.IN_PROGRESS:
            raise ValueError(f"Stage {stage.value} is not in progress (status: {state.status.value})")

        state.status = StageStatus.FAILED
        state.completed_at = time.strftime("%Y-%m-%dT%H:%M:%SZ")
        state.errors = errors

        self._record_transition(stage, StageStatus.FAILED, errors=errors)
        return True

    def _skip_stage(self, stage: ExecutionStage, reason: str) -> None:
        """Skip a stage with a reason."""
        state = self._stages[stage]
        state.status = StageStatus.SKIPPED
        state.completed_at = time.strftime("%Y-%m-%dT%H:%M:%SZ")
        state.outputs["skip_reason"] = reason

        self._record_transition(stage, StageStatus.SKIPPED)

    def skip_stage(self, stage: ExecutionStage, reason: str = "User requested") -> bool:
        """
        Explicitly skip a stage.

        Args:
            stage: Stage to skip
            reason: Reason for skipping

        Returns:
            True if stage was skipped

        Raises:
            ValueError: If stage cannot be skipped
        """
        state = self._stages[stage]

        if state.status != StageStatus.PENDING:
            raise ValueError(f"Stage {stage.value} is not pending (status: {state.status.value})")

        contract = self._registry.get_contract(stage)
        if not contract.skippable and stage not in self._skippable_stages:
            raise ValueError(f"Stage {stage.value} is not skippable")

        self._skip_stage(stage, reason)
        return True

    def _record_transition(
        self,
        stage: ExecutionStage,
        new_status: StageStatus,
        errors: list[str] | None = None,
    ) -> None:
        """Record a state transition in history."""
        self._updated_at = time.strftime("%Y-%m-%dT%H:%M:%SZ")

        entry = {
            "timestamp": self._updated_at,
            "stage": stage.value,
            "new_status": new_status.value,
        }
        if errors:
            entry["errors"] = errors

        self._history.append(entry)

    def get_resume_point(self) -> ExecutionStage | None:
        """
        Get the suggested stage to resume from.

        Returns:
            The stage to resume from, or None if execution is complete
        """
        # First, check for failed stages
        for stage in ExecutionStage.get_order():
            state = self._stages[stage]
            if state.status == StageStatus.FAILED:
                return stage

        # Then, check for in-progress stages
        current = self.current_stage
        if current is not None:
            return current

        # Finally, return next pending stage
        return self.next_pending_stage

    def is_resumable(self) -> bool:
        """
        Check if there is an incomplete execution that can be resumed.

        Returns:
            True if there are incomplete stages
        """
        return self.get_resume_point() is not None

    def is_complete(self) -> bool:
        """
        Check if all stages are complete.

        Returns:
            True if all stages are completed or skipped
        """
        for state in self._stages.values():
            if state.status not in (StageStatus.COMPLETED, StageStatus.SKIPPED):
                return False
        return True

    def is_failed(self) -> bool:
        """
        Check if any stage has failed.

        Returns:
            True if any stage is in FAILED status
        """
        return any(s.status == StageStatus.FAILED for s in self._stages.values())

    def reset_stage(self, stage: ExecutionStage) -> bool:
        """
        Reset a stage to allow retry.

        Args:
            stage: Stage to reset

        Returns:
            True if stage was reset

        Raises:
            ValueError: If stage cannot be reset
        """
        state = self._stages[stage]

        if state.status == StageStatus.PENDING:
            return True  # Already reset

        if state.status == StageStatus.IN_PROGRESS:
            raise ValueError(f"Cannot reset stage {stage.value} while it is in progress")

        # Reset the stage
        state.status = StageStatus.PENDING
        state.started_at = None
        state.completed_at = None
        state.outputs = {}
        state.errors = []

        self._record_transition(stage, StageStatus.PENDING)
        return True

    def resume_from(self, stage: ExecutionStage) -> bool:
        """
        Set up state machine to resume from a specific stage.

        This resets the target stage and all subsequent stages.

        Args:
            stage: Stage to resume from

        Returns:
            True if state machine was set up for resume
        """
        target_idx = ExecutionStage.get_index(stage)

        # Reset target and all subsequent stages
        for s in ExecutionStage.get_order()[target_idx:]:
            state = self._stages[s]
            if state.status != StageStatus.PENDING:
                self.reset_stage(s)

        return True

    def get_stage_outputs(self, stage: ExecutionStage) -> dict[str, Any]:
        """
        Get the outputs from a completed stage.

        Args:
            stage: Stage to get outputs from

        Returns:
            Dictionary of stage outputs (empty if stage not complete)
        """
        return self._stages[stage].outputs.copy()

    def get_all_outputs(self) -> dict[str, Any]:
        """
        Get combined outputs from all completed stages.

        Returns:
            Dictionary with all outputs (later stages override earlier)
        """
        outputs = {}
        for stage in ExecutionStage.get_order():
            state = self._stages[stage]
            if state.status == StageStatus.COMPLETED:
                outputs.update(state.outputs)
        return outputs

    def to_dict(self) -> dict[str, Any]:
        """Convert state machine to dictionary for persistence."""
        return {
            "version": self.VERSION,
            "execution_id": self.execution_id,
            "strategy": self.strategy,
            "flow": self.flow,
            "current_stage": self.current_stage.value if self.current_stage else None,
            "stages": {
                stage.value: state.to_dict()
                for stage, state in self._stages.items()
            },
            "history": self._history,
            "created_at": self._created_at,
            "updated_at": self._updated_at,
        }

    @classmethod
    def from_dict(
        cls,
        data: dict[str, Any],
        contract_registry: StageContractRegistry | None = None,
    ) -> "StageStateMachine":
        """
        Create state machine from dictionary.

        Args:
            data: Dictionary from to_dict() or JSON load
            contract_registry: Optional custom contract registry

        Returns:
            Restored StageStateMachine instance
        """
        machine = cls(
            execution_id=data["execution_id"],
            strategy=data.get("strategy"),
            flow=data.get("flow"),
            contract_registry=contract_registry,
        )

        # Restore stages
        for stage_value, stage_data in data.get("stages", {}).items():
            stage = ExecutionStage(stage_value)
            machine._stages[stage] = StageState.from_dict(stage_data)

        # Restore history
        machine._history = data.get("history", [])

        # Restore timestamps
        machine._created_at = data.get("created_at", machine._created_at)
        machine._updated_at = data.get("updated_at", machine._updated_at)

        return machine

    def save_state(self, path_resolver: "PathResolver") -> Path:
        """
        Save state to file.

        Args:
            path_resolver: PathResolver for determining file path

        Returns:
            Path to the saved state file
        """
        from ..state.state_manager import FileLock

        # Get state file path
        state_path = path_resolver.get_state_file_path(self.STATE_FILE_NAME)
        state_path.parent.mkdir(parents=True, exist_ok=True)

        # Write with lock
        lock_path = path_resolver.get_locks_dir() / f"{self.STATE_FILE_NAME}.lock"
        lock_path.parent.mkdir(parents=True, exist_ok=True)

        with FileLock(lock_path):
            with open(state_path, "w", encoding="utf-8") as f:
                json.dump(self.to_dict(), f, indent=2)

        return state_path

    @classmethod
    def load_state(
        cls,
        path_resolver: "PathResolver",
        contract_registry: StageContractRegistry | None = None,
    ) -> "StageStateMachine | None":
        """
        Load state from file.

        Args:
            path_resolver: PathResolver for determining file path
            contract_registry: Optional custom contract registry

        Returns:
            Restored StageStateMachine or None if file doesn't exist
        """
        from ..state.state_manager import FileLock

        state_path = path_resolver.get_state_file_path(cls.STATE_FILE_NAME)
        if not state_path.exists():
            return None

        lock_path = path_resolver.get_locks_dir() / f"{cls.STATE_FILE_NAME}.lock"
        lock_path.parent.mkdir(parents=True, exist_ok=True)

        with FileLock(lock_path):
            try:
                with open(state_path, encoding="utf-8") as f:
                    data = json.load(f)
                return cls.from_dict(data, contract_registry)
            except (OSError, json.JSONDecodeError):
                return None

    def get_progress_summary(self) -> dict[str, Any]:
        """
        Get a summary of execution progress.

        Returns:
            Dictionary with progress information
        """
        total = len(ExecutionStage.get_order())
        completed = sum(
            1 for s in self._stages.values()
            if s.status in (StageStatus.COMPLETED, StageStatus.SKIPPED)
        )
        failed = sum(
            1 for s in self._stages.values()
            if s.status == StageStatus.FAILED
        )

        return {
            "execution_id": self.execution_id,
            "strategy": self.strategy,
            "flow": self.flow,
            "current_stage": self.current_stage.value if self.current_stage else None,
            "total_stages": total,
            "completed_stages": completed,
            "failed_stages": failed,
            "progress_percent": int((completed / total) * 100),
            "is_complete": self.is_complete(),
            "is_failed": self.is_failed(),
            "is_resumable": self.is_resumable(),
            "resume_point": self.get_resume_point().value if self.get_resume_point() else None,
        }
