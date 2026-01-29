[中文版](Design-Plan-Cascade-Standalone_zh.md)

# Plan Cascade Standalone - Technical Design Document

**Version**: 4.0.0
**Date**: 2026-01-29
**Author**: Plan Cascade Team
**Status**: Implementation In Progress

---

## Implementation Status Overview

> **Current Progress**: ~98% core functionality implemented
> **Last Updated**: 2026-01-29

### Module Implementation Status

| Module | Status | File | Notes |
|--------|--------|------|-------|
| **Core Orchestration Layer** | | | |
| Intent Classifier | ✅ Complete | `core/intent_classifier.py` | Distinguishes TASK/QUERY/CHAT |
| Strategy Analyzer | ✅ Complete | `core/strategy_analyzer.py` | AI auto-determines execution strategy |
| PRD Generator | ✅ Complete | `core/prd_generator.py` | Generates PRD from requirements |
| Mega Generator | ✅ Complete | `core/mega_generator.py` | Large project multi-PRD cascade |
| Orchestrator | ✅ Complete | `core/orchestrator.py` | Batch dependency analysis and scheduling |
| Simple Mode Workflow | ✅ Complete | `core/simple_workflow.py` | One-click execution |
| Expert Mode Workflow | ✅ Complete | `core/expert_workflow.py` | Fine-grained control |
| Quality Gate | ✅ Complete | `core/quality_gate.py` | typecheck/test/lint |
| Retry Manager | ✅ Complete | `core/retry_manager.py` | Smart retry |
| Iteration Loop | ✅ Complete | `core/iteration_loop.py` | Iteration execution |
| **ReAct Execution Engine** | | | |
| ReAct Engine | ✅ Complete | `core/react_engine.py` | Standalone Think→Act→Observe engine |
| **Backend Abstraction Layer** | | | |
| Backend Base Class | ✅ Complete | `backends/base.py` | AgentBackend abstraction |
| Backend Factory | ✅ Complete | `backends/factory.py` | Dynamic backend creation |
| Built-in Backend | ✅ Complete | `backends/builtin.py` | ReAct + tool execution |
| Claude Code Backend | ✅ Complete | `backends/claude_code.py` | CLI integration |
| Agent Executor | ✅ Complete | `backends/agent_executor.py` | Multi-Agent collaboration |
| Phase Config | ✅ Complete | `backends/phase_config.py` | Phase/type Agent mapping |
| Claude Code GUI Backend | ⚠️ Pending | `backends/claude_code_gui.py` | P2 priority |
| **LLM Abstraction Layer** | | | |
| LLM Base Class | ✅ Complete | `llm/base.py` | LLMProvider abstraction |
| LLM Factory | ✅ Complete | `llm/factory.py` | Supports 5 Providers |
| Claude Provider | ✅ Complete | `llm/providers/claude.py` | Anthropic API |
| Claude Max Provider | ✅ Complete | `llm/providers/claude_max.py` | Get LLM via Claude Code |
| OpenAI Provider | ✅ Complete | `llm/providers/openai.py` | OpenAI API |
| DeepSeek Provider | ✅ Complete | `llm/providers/deepseek.py` | DeepSeek API |
| Ollama Provider | ✅ Complete | `llm/providers/ollama.py` | Local models |
| **Tool Execution Layer** | | | |
| Tool Registry | ✅ Complete | `tools/registry.py` | Tool management |
| File Tools | ✅ Complete | `tools/file_tools.py` | Read/Write/Edit |
| Search Tools | ✅ Complete | `tools/search_tools.py` | Glob/Grep |
| Shell Tools | ✅ Complete | `tools/shell_tools.py` | Bash execution |
| **Settings and State** | | | |
| Settings Models | ✅ Complete | `settings/models.py` | Configuration data structures |
| Settings Storage | ✅ Complete | `settings/storage.py` | YAML + Keyring |
| State Manager | ✅ Complete | `state/state_manager.py` | State tracking |
| Context Filter | ✅ Complete | `state/context_filter.py` | Context management |
| **CLI** | | | |
| CLI Main Entry | ✅ Complete | `cli/main.py` | run/config/status/chat |
| Interactive REPL | ✅ Complete | `cli/main.py` | chat command |
| Output Formatting | ✅ Complete | `cli/output.py` | Rich output |
| **Desktop Application** | | | |
| Tauri Desktop | ⏳ Planning | `desktop/` | Phase 2 target |

### Feature Implementation Status

| Feature | Status | Notes |
|---------|--------|-------|
| Simple Mode | ✅ Complete | One-click execution, AI auto-determines strategy |
| Expert Mode | ✅ Complete | PRD editing, strategy selection, Agent specification |
| Interactive REPL | ✅ Complete | `plan-cascade chat` command |
| Streaming Output | ✅ Complete | `--include-partial-messages` |
| Multi-LLM Backend | ✅ Complete | Claude Max/API, OpenAI, DeepSeek, Ollama |
| Multi-Agent Collaboration | ✅ Complete | Phase/type-based Agent selection |
| Quality Gates | ✅ Complete | typecheck/test/lint/custom |
| Git Worktree | ✅ Complete | Isolated development support |
| Claude Code GUI Mode | ⚠️ Partial | Basic functionality available, GUI-specific backend pending |
| Desktop Application | ⏳ Planning | Tauri implementation, Phase 2 target |

---

## 1. Design Goals

### 1.1 Core Objectives

1. **Complete Orchestration Capability**: Plan Cascade executes tools itself (Read/Write/Edit/Bash/Glob/Grep)
2. **Multi-LLM Support**: Claude Max (no API Key), Claude API, OpenAI, DeepSeek, Ollama
3. **Dual Working Modes**: Standalone orchestration mode (recommended) + Claude Code GUI mode
4. **Three Forms Unified**: CLI, Desktop (GUI version of CLI), Claude Code Plugin
5. **Preserve Core Philosophy**: Hierarchical decomposition, parallel execution, quality assurance, state tracking

### 1.2 Design Constraints

| Constraint | Description |
|------------|-------------|
| Zero API Key Option | Claude Max users can get LLM capability through Claude Code |
| Complete Tool Execution | Standalone orchestration mode executes all tools itself, no external Agent dependency |
| Progressive Disclosure | Simple mode hides complex concepts, expert mode fully exposed |
| Claude Code Compatible | GUI mode fully compatible with all Claude Code features |
| Cross-Platform | Supports Windows, macOS, Linux |

---

## 2. Dual-Mode Architecture

### 2.1 Mode Switching Design

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Plan Cascade                                     │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─────────────────────────┐     ┌─────────────────────────┐           │
│   │      Simple Mode         │     │      Expert Mode         │           │
│   │                         │     │                         │           │
│   │  User enters description │     │  User enters description │           │
│   │       ↓                 │     │       ↓                 │           │
│   │  AI auto-determines      │     │  Generate PRD (editable) │           │
│   │  strategy               │     │       ↓                 │           │
│   │       ↓                 │     │  User Review/Modify      │           │
│   │  Auto-generate PRD      │     │       ↓                 │           │
│   │       ↓                 │     │  Select Strategy/Agent   │           │
│   │  Auto-execute           │     │       ↓                 │           │
│   │       ↓                 │     │  Execute                │           │
│   │  Complete               │     │                         │           │
│   └─────────────────────────┘     └─────────────────────────┘           │
│                                                                          │
│                              Shared Core                                 │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │  Orchestrator │ PRDGenerator │ QualityGate │ AgentExecutor      │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Mode Implementation

```python
# src/plan_cascade/core/mode.py

from enum import Enum
from dataclasses import dataclass
from typing import Optional

class UserMode(Enum):
    """User operation mode"""
    SIMPLE = "simple"    # Simple mode: one-click completion
    EXPERT = "expert"    # Expert mode: fine-grained control

@dataclass
class ModeConfig:
    """Mode configuration"""
    mode: UserMode
    auto_execute: bool = True          # Auto-execute (simple mode)
    show_prd_editor: bool = False      # Show PRD editor
    allow_strategy_select: bool = False # Allow strategy selection
    allow_agent_select: bool = False   # Allow Agent specification
    show_detailed_logs: bool = False   # Show detailed logs

    @classmethod
    def simple(cls) -> "ModeConfig":
        return cls(
            mode=UserMode.SIMPLE,
            auto_execute=True,
            show_prd_editor=False,
            allow_strategy_select=False,
            allow_agent_select=False,
            show_detailed_logs=False,
        )

    @classmethod
    def expert(cls) -> "ModeConfig":
        return cls(
            mode=UserMode.EXPERT,
            auto_execute=False,
            show_prd_editor=True,
            allow_strategy_select=True,
            allow_agent_select=True,
            show_detailed_logs=True,
        )
```

---

## 3. AI Automatic Strategy Determination

### 3.1 Strategy Types

```python
# src/plan_cascade/core/strategy.py

from enum import Enum
from dataclasses import dataclass

class ExecutionStrategy(Enum):
    """Execution strategy"""
    DIRECT = "direct"           # Direct execution, no PRD needed (small tasks)
    HYBRID_AUTO = "hybrid_auto" # Auto-generate PRD (medium tasks)
    MEGA_PLAN = "mega_plan"     # Multi-PRD cascade (large projects)

@dataclass
class StrategyDecision:
    """Strategy decision result"""
    strategy: ExecutionStrategy
    use_worktree: bool
    estimated_stories: int
    confidence: float
    reasoning: str
```

### 3.2 Strategy Analyzer

```python
# src/plan_cascade/core/strategy_analyzer.py

from ..llm.base import LLMProvider
from .strategy import ExecutionStrategy, StrategyDecision

class StrategyAnalyzer:
    """
    AI-driven strategy analyzer

    Automatically determines the best execution strategy based on user requirements
    """

    ANALYSIS_PROMPT = """
Analyze the following development requirement and determine the most suitable execution strategy:

Requirement Description:
{description}

Project Context:
{context}

Please analyze and return JSON-formatted decision result:
{{
    "strategy": "direct" | "hybrid_auto" | "mega_plan",
    "use_worktree": true | false,
    "estimated_stories": <estimated task count>,
    "confidence": <0.0-1.0 confidence>,
    "reasoning": "<decision reasoning>"
}}

Decision Criteria:
- direct: Single simple task, like "add a button", "fix a typo"
- hybrid_auto: Medium feature development, like "implement user login", "add search functionality"
- mega_plan: Large project, like "develop complete e-commerce system", "refactor entire module"

use_worktree Decision Criteria:
- true: Requires isolated development, like "don't affect existing functionality", "experimental feature"
- false: Normal development
"""

    def __init__(self, llm: LLMProvider):
        self.llm = llm

    async def analyze(
        self,
        description: str,
        context: str = ""
    ) -> StrategyDecision:
        """Analyze requirements, return strategy decision"""
        prompt = self.ANALYSIS_PROMPT.format(
            description=description,
            context=context
        )

        response = await self.llm.complete([
            {"role": "user", "content": prompt}
        ])

        result = self._parse_response(response.content)
        return result

    def _parse_response(self, content: str) -> StrategyDecision:
        """Parse LLM response"""
        import json
        data = json.loads(content)

        return StrategyDecision(
            strategy=ExecutionStrategy(data["strategy"]),
            use_worktree=data.get("use_worktree", False),
            estimated_stories=data.get("estimated_stories", 1),
            confidence=data.get("confidence", 0.8),
            reasoning=data.get("reasoning", "")
        )
```

### 3.3 Simple Mode Workflow

```python
# src/plan_cascade/core/simple_workflow.py

class SimpleWorkflow:
    """
    Simple mode workflow

    User input description → AI analysis → Auto-select strategy → Auto-execute → Complete
    """

    def __init__(self, config: dict):
        self.backend = self._create_backend(config)
        self.strategy_analyzer = StrategyAnalyzer(self.backend.llm)
        self.orchestrator = None

    async def run(self, description: str, project_path: str):
        """One-click execution"""
        # 1. Analyze strategy
        context = await self._gather_context(project_path)
        decision = await self.strategy_analyzer.analyze(description, context)

        # 2. Execute based on strategy
        if decision.strategy == ExecutionStrategy.DIRECT:
            # Direct execution, no PRD needed
            return await self._execute_direct(description, context)

        elif decision.strategy == ExecutionStrategy.HYBRID_AUTO:
            # Auto-generate PRD and execute
            return await self._execute_hybrid(
                description,
                context,
                use_worktree=decision.use_worktree
            )

        elif decision.strategy == ExecutionStrategy.MEGA_PLAN:
            # Large project, multi-PRD cascade
            return await self._execute_mega(description, context)

    async def _execute_direct(self, description: str, context: str):
        """Direct execution of simple tasks"""
        result = await self.backend.execute(description, context)
        return result

    async def _execute_hybrid(
        self,
        description: str,
        context: str,
        use_worktree: bool = False
    ):
        """Hybrid mode execution"""
        from .prd_generator import PRDGenerator
        from .orchestrator import Orchestrator

        # Setup worktree (if needed)
        if use_worktree:
            await self._setup_worktree()

        # Generate PRD
        generator = PRDGenerator(self.backend.llm)
        prd = await generator.generate(description, context)

        # Execute
        self.orchestrator = Orchestrator(prd, self.backend)
        result = await self.orchestrator.auto_run()

        return result

    async def _execute_mega(self, description: str, context: str):
        """Mega Plan execution"""
        from .mega_generator import MegaGenerator

        generator = MegaGenerator(self.backend.llm)
        mega_plan = await generator.generate(description, context)

        # Execute each feature module sequentially
        for feature in mega_plan.features:
            await self._execute_hybrid(
                feature.description,
                context + f"\n\nCompleted features: {mega_plan.get_completed()}"
            )

        return mega_plan
```

---

## 4. Core Architecture

### 4.1 Dual Working Mode Architecture

**Core Philosophy: Plan Cascade = Brain (Orchestration), Execution Layer = Hands (Tool Execution)**

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Plan Cascade                                   │
│                    (Orchestration Layer - Shared by Both Modes)          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │                    Orchestration Engine (Shared)                  │   │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │   │
│   │  │ PRD Generator│  │ Dependency  │  │  Batch     │              │   │
│   │  │             │  │ Analyzer    │  │  Scheduler │              │   │
│   │  └─────────────┘  └─────────────┘  └─────────────┘              │   │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │   │
│   │  │ State       │  │ Quality    │  │  Retry     │              │   │
│   │  │ Manager     │  │ Gates      │  │  Manager   │              │   │
│   │  └─────────────┘  └─────────────┘  └─────────────┘              │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                              │                                           │
│                    ┌─────────┴─────────┐                                │
│                    │  Execution Layer   │                                │
│                    │  Selection         │                                │
│                    └─────────┬─────────┘                                │
│              ┌───────────────┴───────────────┐                          │
│              ▼                               ▼                          │
│   ┌─────────────────────────┐    ┌─────────────────────────┐           │
│   │  Standalone Orchestration│    │  Claude Code GUI Mode   │           │
│   │  Mode                    │    │                         │           │
│   ├─────────────────────────┤    ├─────────────────────────┤           │
│   │                         │    │                         │           │
│   │   Built-in Tool Engine  │    │   Claude Code CLI       │           │
│   │   ┌───────────────┐     │    │   ┌───────────────┐     │           │
│   │   │ Read/Write    │     │    │   │ Claude Code   │     │           │
│   │   │ Edit/Bash     │     │    │   │ Executes Tools│     │           │
│   │   │ Glob/Grep     │     │    │   │ (stream-json) │     │           │
│   │   └───────────────┘     │    │   └───────────────┘     │           │
│   │          │              │    │          │              │           │
│   │          ▼              │    │          ▼              │           │
│   │   ┌───────────────┐     │    │   ┌───────────────┐     │           │
│   │   │ LLM Abstraction│    │    │   │ Plan Cascade  │     │           │
│   │   │ Layer          │    │    │   │ Visual UI     │     │           │
│   │   │ (Multiple)    │     │    │   └───────────────┘     │           │
│   │   └───────────────┘     │    │                         │           │
│   │          │              │    │                         │           │
│   │   ┌──────┴──────┐       │    │                         │           │
│   │   ▼      ▼      ▼       │    │                         │           │
│   │ Claude Claude OpenAI    │    │                         │           │
│   │ Max    API    etc.      │    │                         │           │
│   │                         │    │                         │           │
│   └─────────────────────────┘    └─────────────────────────┘           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘

Both modes support: PRD-driven development, batch execution, quality gates, state tracking
```

### 4.2 Tool Execution Engine

In standalone orchestration mode, Plan Cascade executes all tools itself:

```python
# src/plan_cascade/tools/registry.py

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Any
from pathlib import Path

@dataclass
class ToolResult:
    """Tool execution result"""
    success: bool
    output: str
    error: str | None = None

class Tool(ABC):
    """Tool base class"""

    @property
    @abstractmethod
    def name(self) -> str:
        """Tool name"""
        pass

    @property
    @abstractmethod
    def description(self) -> str:
        """Tool description"""
        pass

    @property
    @abstractmethod
    def parameters(self) -> dict:
        """Parameter definition (JSON Schema)"""
        pass

    @abstractmethod
    async def execute(self, **kwargs) -> ToolResult:
        """Execute tool"""
        pass

class ToolRegistry:
    """Tool registry"""

    def __init__(self, project_root: Path):
        self.project_root = project_root
        self.tools: dict[str, Tool] = {}
        self._register_builtin_tools()

    def _register_builtin_tools(self):
        """Register built-in tools"""
        from .file_tools import ReadTool, WriteTool, EditTool, GlobTool, GrepTool
        from .shell_tools import BashTool

        for tool_class in [ReadTool, WriteTool, EditTool, GlobTool, GrepTool, BashTool]:
            tool = tool_class(self.project_root)
            self.tools[tool.name] = tool

    def get_definitions(self) -> list[dict]:
        """Get all tool definitions (for LLM)"""
        return [
            {
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
            }
            for tool in self.tools.values()
        ]

    async def execute(self, name: str, **kwargs) -> ToolResult:
        """Execute tool"""
        if name not in self.tools:
            return ToolResult(success=False, output="", error=f"Unknown tool: {name}")

        try:
            return await self.tools[name].execute(**kwargs)
        except Exception as e:
            return ToolResult(success=False, output="", error=str(e))
```

### 4.3 Built-in Tool Implementation

```python
# src/plan_cascade/tools/file_tools.py

from pathlib import Path
from .registry import Tool, ToolResult

class ReadTool(Tool):
    """Read file contents"""

    def __init__(self, project_root: Path):
        self.project_root = project_root

    @property
    def name(self) -> str:
        return "read"

    @property
    def description(self) -> str:
        return "Read file contents. Returns the content with line numbers."

    @property
    def parameters(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "Path to the file"},
                "offset": {"type": "integer", "description": "Start line (optional)"},
                "limit": {"type": "integer", "description": "Number of lines (optional)"},
            },
            "required": ["file_path"],
        }

    async def execute(self, file_path: str, offset: int = 0, limit: int = 2000) -> ToolResult:
        try:
            path = self._resolve_path(file_path)
            content = path.read_text(encoding="utf-8")
            lines = content.splitlines()

            # Apply offset and limit
            selected = lines[offset:offset + limit]

            # Add line numbers
            numbered = [f"{i + offset + 1:6d}->{line}" for i, line in enumerate(selected)]

            return ToolResult(success=True, output="\n".join(numbered))
        except Exception as e:
            return ToolResult(success=False, output="", error=str(e))

    def _resolve_path(self, file_path: str) -> Path:
        """Resolve path relative to project root"""
        path = Path(file_path)
        if not path.is_absolute():
            path = self.project_root / path
        return path


class WriteTool(Tool):
    """Write file contents"""

    def __init__(self, project_root: Path):
        self.project_root = project_root

    @property
    def name(self) -> str:
        return "write"

    @property
    def description(self) -> str:
        return "Write content to a file. Creates directories if needed."

    @property
    def parameters(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "Path to the file"},
                "content": {"type": "string", "description": "Content to write"},
            },
            "required": ["file_path", "content"],
        }

    async def execute(self, file_path: str, content: str) -> ToolResult:
        try:
            path = self._resolve_path(file_path)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
            return ToolResult(success=True, output=f"Wrote {len(content)} bytes to {path}")
        except Exception as e:
            return ToolResult(success=False, output="", error=str(e))

    def _resolve_path(self, file_path: str) -> Path:
        path = Path(file_path)
        if not path.is_absolute():
            path = self.project_root / path
        return path


class EditTool(Tool):
    """Edit file (find and replace)"""

    def __init__(self, project_root: Path):
        self.project_root = project_root

    @property
    def name(self) -> str:
        return "edit"

    @property
    def description(self) -> str:
        return "Edit a file by replacing old_string with new_string."

    @property
    def parameters(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "Path to the file"},
                "old_string": {"type": "string", "description": "String to replace"},
                "new_string": {"type": "string", "description": "Replacement string"},
                "replace_all": {"type": "boolean", "description": "Replace all occurrences"},
            },
            "required": ["file_path", "old_string", "new_string"],
        }

    async def execute(
        self,
        file_path: str,
        old_string: str,
        new_string: str,
        replace_all: bool = False
    ) -> ToolResult:
        try:
            path = self._resolve_path(file_path)
            content = path.read_text(encoding="utf-8")

            if old_string not in content:
                return ToolResult(success=False, output="", error="old_string not found")

            if replace_all:
                new_content = content.replace(old_string, new_string)
                count = content.count(old_string)
            else:
                # Ensure uniqueness
                if content.count(old_string) > 1:
                    return ToolResult(
                        success=False,
                        output="",
                        error="old_string is not unique. Use replace_all or provide more context."
                    )
                new_content = content.replace(old_string, new_string, 1)
                count = 1

            path.write_text(new_content, encoding="utf-8")
            return ToolResult(success=True, output=f"Replaced {count} occurrence(s)")
        except Exception as e:
            return ToolResult(success=False, output="", error=str(e))

    def _resolve_path(self, file_path: str) -> Path:
        path = Path(file_path)
        if not path.is_absolute():
            path = self.project_root / path
        return path
```

### 4.4 ReAct Execution Engine

```python
# src/plan_cascade/core/react_engine.py

from dataclasses import dataclass
from typing import Any
from ..tools.registry import ToolRegistry, ToolResult
from ..llm.base import LLMProvider

@dataclass
class ReActResult:
    """ReAct execution result"""
    success: bool
    output: str
    iterations: int
    tool_calls: list[dict]
    error: str | None = None

class ReActEngine:
    """
    ReAct Execution Engine

    Implements Think -> Act -> Observe loop:
    1. Think: LLM thinks about next action
    2. Act: Execute tool
    3. Observe: Observe result, return to Think
    """

    SYSTEM_PROMPT = """You are a professional software development Agent.

You have the following tools available:
{tools}

Working Principles:
1. First read relevant code, understand existing structure
2. Follow project's code style and conventions
3. Verify code works correctly after completion
4. When task is complete, reply "TASK_COMPLETE" and summarize completed work

Important: You can only perform operations by calling tools, do not output code directly for users to execute.
"""

    def __init__(
        self,
        llm: LLMProvider,
        tools: ToolRegistry,
        max_iterations: int = 50,
        on_tool_call: callable = None,
        on_text: callable = None,
    ):
        self.llm = llm
        self.tools = tools
        self.max_iterations = max_iterations
        self.on_tool_call = on_tool_call
        self.on_text = on_text

    async def execute(self, task: str, context: str = "") -> ReActResult:
        """Execute task"""
        # Build system prompt
        tools_desc = self._format_tools_description()
        system_prompt = self.SYSTEM_PROMPT.format(tools=tools_desc)

        messages = [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": f"{context}\n\nTask: {task}"},
        ]

        tool_calls_history = []

        for iteration in range(self.max_iterations):
            # Think: LLM decides next step
            response = await self.llm.complete(
                messages=messages,
                tools=self.tools.get_definitions(),
            )

            # Send text to callback
            if self.on_text and response.content:
                self.on_text(response.content)

            # Check if complete
            if "TASK_COMPLETE" in (response.content or ""):
                return ReActResult(
                    success=True,
                    output=response.content,
                    iterations=iteration + 1,
                    tool_calls=tool_calls_history,
                )

            # No tool calls, continue waiting
            if not response.tool_calls:
                messages.append({"role": "assistant", "content": response.content})
                messages.append({
                    "role": "user",
                    "content": "Please continue working. If task is complete, reply TASK_COMPLETE and summarize."
                })
                continue

            # Act: Execute tools
            tool_results = []
            for tool_call in response.tool_calls:
                result = await self.tools.execute(
                    tool_call.name,
                    **tool_call.arguments
                )

                tool_calls_history.append({
                    "name": tool_call.name,
                    "arguments": tool_call.arguments,
                    "result": result.output if result.success else result.error,
                    "success": result.success,
                })

                # Callback
                if self.on_tool_call:
                    self.on_tool_call(tool_calls_history[-1])

                tool_results.append({
                    "tool_call_id": tool_call.id,
                    "result": result.output if result.success else f"Error: {result.error}",
                })

            # Observe: Add results to message history
            messages.append({
                "role": "assistant",
                "content": response.content,
                "tool_calls": response.tool_calls,
            })
            messages.append({
                "role": "user",
                "content": self._format_tool_results(tool_results),
            })

        return ReActResult(
            success=False,
            output="",
            iterations=self.max_iterations,
            tool_calls=tool_calls_history,
            error="Max iterations reached",
        )

    def _format_tools_description(self) -> str:
        """Format tool descriptions"""
        lines = []
        for tool_def in self.tools.get_definitions():
            lines.append(f"- {tool_def['name']}: {tool_def['description']}")
        return "\n".join(lines)

    def _format_tool_results(self, results: list) -> str:
        """Format tool results"""
        parts = []
        for r in results:
            parts.append(f"Tool execution result:\n{r['result']}")
        return "\n\n".join(parts)
```

### 4.5 Claude Max LLM Backend

```python
# src/plan_cascade/llm/claude_max.py

import asyncio
import json
from .base import LLMProvider, LLMResponse

class ClaudeMaxLLM(LLMProvider):
    """
    Claude Max LLM Backend

    Get LLM capability through local Claude Code:
    - Send prompt to Claude Code
    - Disable Claude Code's tool execution (only thinking)
    - Parse response and return
    """

    def __init__(self, claude_path: str = "claude"):
        self.claude_path = claude_path

    async def complete(
        self,
        messages: list[dict],
        tools: list[dict] | None = None,
        on_text: callable = None,
    ) -> LLMResponse:
        """Call Claude Code to get LLM response"""

        # Build prompt
        prompt = self._build_prompt(messages, tools)

        # Call Claude Code (disable tool execution)
        process = await asyncio.create_subprocess_exec(
            self.claude_path,
            "--print",
            "--output-format", "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--no-tools",  # Key: disable tool execution
            prompt,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )

        content_parts = []
        tool_calls = []

        async for line in process.stdout:
            try:
                data = json.loads(line.decode())
                event_type = data.get("type", "")

                if event_type == "stream_event":
                    inner_event = data.get("event", {})
                    if inner_event.get("type") == "content_block_delta":
                        delta = inner_event.get("delta", {})
                        if delta.get("type") == "text_delta":
                            text = delta.get("text", "")
                            if text:
                                content_parts.append(text)
                                if on_text:
                                    on_text(text)

                elif event_type == "assistant":
                    # Parse tool call requests (if LLM returns)
                    message = data.get("message", {})
                    for block in message.get("content", []):
                        if block.get("type") == "tool_use":
                            tool_calls.append(ToolCall(
                                id=block.get("id"),
                                name=block.get("name"),
                                arguments=block.get("input", {}),
                            ))

            except json.JSONDecodeError:
                continue

        await process.wait()

        return LLMResponse(
            content="".join(content_parts),
            tool_calls=tool_calls if tool_calls else None,
            stop_reason="end_turn" if not tool_calls else "tool_use",
        )

    def _build_prompt(self, messages: list[dict], tools: list[dict] | None) -> str:
        """Build prompt to send to Claude Code"""
        parts = []

        for msg in messages:
            role = msg.get("role", "user")
            content = msg.get("content", "")
            parts.append(f"[{role}]\n{content}")

        # If there are tool definitions, add to prompt so LLM knows it can call them
        if tools:
            tools_desc = "\n".join(
                f"- {t['name']}: {t['description']}"
                for t in tools
            )
            parts.append(f"\nAvailable tools:\n{tools_desc}")
            parts.append("\nTo use a tool, reply in JSON format: {\"tool\": \"name\", \"args\": {...}}")

        return "\n\n".join(parts)
```

### 4.6 Multi-Agent Collaborative Execution

Plan Cascade supports multiple Agents working collaboratively, intelligently selecting the most suitable Agent based on execution phase and Story type.

```python
# src/plan_cascade/backends/agent_executor.py

class AgentExecutor:
    """
    Agent Execution Abstraction Layer, supports multiple Agent types:
    - task-tool: Built-in ReAct engine or Claude Code Task tool
    - cli: External CLI tools (codex, aider, amp-code, etc.)

    Features:
    - Auto-fallback: If CLI Agent unavailable, fallback to claude-code
    - Phase-based Agent selection
    - Story type smart matching
    - Process management and state tracking
    """

    def _resolve_agent(
        self,
        agent_name: str | None = None,
        phase: ExecutionPhase | None = None,
        story: dict | None = None,
        override: AgentOverrides | None = None
    ) -> tuple[str, dict]:
        """
        Resolve Agent with auto-fallback.

        Priority:
        1. agent_name parameter (explicit override)
        2. Phase-based resolution (PhaseAgentManager)
        3. story.agent metadata
        4. Story type override
        5. Phase default Agent
        6. Fallback chain
        7. claude-code (ultimate fallback)
        """
        pass

    def execute_story(
        self,
        story: dict,
        context: dict,
        phase: ExecutionPhase,
        task_callback: Callable | None = None
    ) -> dict:
        """Execute Story, auto-select appropriate Agent."""
        resolved_name, agent_config = self._resolve_agent(phase=phase, story=story)

        if agent_config.get("type") == "task-tool":
            return self._execute_via_task_tool(story, context, task_callback)
        elif agent_config.get("type") == "cli":
            return self._execute_via_cli(story, context, agent_config)
```

```python
# src/plan_cascade/backends/phase_config.py

class ExecutionPhase(Enum):
    """Execution phase"""
    PLANNING = "planning"
    IMPLEMENTATION = "implementation"
    RETRY = "retry"
    REFACTOR = "refactor"
    REVIEW = "review"

class StoryType(Enum):
    """Story type"""
    FEATURE = "feature"
    BUGFIX = "bugfix"
    REFACTOR = "refactor"
    TEST = "test"
    DOCUMENTATION = "documentation"
    INFRASTRUCTURE = "infrastructure"

class PhaseAgentManager:
    """
    Phase-based Agent Selection Manager.

    Default configuration:
    - Planning: codex -> claude-code
    - Implementation: claude-code -> codex -> aider
      - bugfix type override: codex
      - refactor type override: aider
    - Retry: claude-code -> aider
    - Refactor: aider -> claude-code
    - Review: claude-code -> codex
    """

    def get_agent_for_story(
        self,
        story: dict,
        phase: ExecutionPhase,
        override: AgentOverrides | None = None
    ) -> str:
        """Get Agent suitable for specified Story and phase."""
        pass

    def infer_story_type(self, story: dict) -> StoryType:
        """Infer Story type from title, description, tags."""
        pass
```

### 4.7 Backend Factory

```python
# src/plan_cascade/backends/factory.py

from typing import Optional
from .base import AgentBackend
from .claude_code import ClaudeCodeBackend
from .builtin import BuiltinBackend

class BackendFactory:
    """Backend Factory"""

    @staticmethod
    def create(config: dict) -> AgentBackend:
        """
        Create backend based on configuration

        config:
            backend: "claude-code" | "builtin" | "aider" | ...
            provider: "claude" | "openai" | ...  (needed for builtin mode)
            api_key: "..."  (needed for builtin mode)
            model: "..."  (optional)
        """
        backend_type = config.get("backend", "claude-code")

        if backend_type == "claude-code":
            return ClaudeCodeBackend(
                claude_path=config.get("claude_path", "claude")
            )

        elif backend_type == "builtin":
            return BuiltinBackend(
                provider=config.get("provider", "claude"),
                model=config.get("model"),
                api_key=config.get("api_key"),
                config=config
            )

        elif backend_type in ("aider", "codex", "amp"):
            from .external import ExternalCLIBackend
            return ExternalCLIBackend(backend_type, config)

        else:
            raise ValueError(f"Unknown backend: {backend_type}")

    @staticmethod
    def create_from_settings(settings) -> AgentBackend:
        """Create backend from settings"""
        return BackendFactory.create({
            "backend": settings.backend,
            "provider": settings.provider,
            "model": settings.model,
            "api_key": settings.get_api_key(),
        })
```

---

## 5. Settings Management

### 5.1 Settings Structure

```python
# src/plan_cascade/settings/models.py

from dataclasses import dataclass, field
from typing import Optional, List
from enum import Enum

class BackendType(Enum):
    CLAUDE_CODE = "claude-code"
    CLAUDE_API = "claude-api"
    OPENAI = "openai"
    DEEPSEEK = "deepseek"
    OLLAMA = "ollama"

@dataclass
class AgentConfig:
    """Execution Agent configuration"""
    name: str
    enabled: bool = True
    command: str = ""
    is_default: bool = False

@dataclass
class QualityGateConfig:
    """Quality gate configuration"""
    typecheck: bool = True
    test: bool = True
    lint: bool = True
    custom: bool = False
    custom_script: str = ""
    max_retries: int = 3

@dataclass
class Settings:
    """Global settings"""
    # Backend configuration
    backend: BackendType = BackendType.CLAUDE_CODE
    provider: str = "claude"
    model: str = ""
    # API Key stored in keyring, not in config file

    # Execution Agent list
    agents: List[AgentConfig] = field(default_factory=lambda: [
        AgentConfig(name="claude-code", enabled=True, command="claude", is_default=True),
        AgentConfig(name="aider", enabled=False, command="aider"),
        AgentConfig(name="codex", enabled=False, command="codex"),
    ])

    # Agent selection strategy
    agent_selection: str = "prefer_default"  # "smart" | "prefer_default" | "manual"
    default_agent: str = "claude-code"

    # Quality gates
    quality_gates: QualityGateConfig = field(default_factory=QualityGateConfig)

    # Execution configuration
    max_parallel_stories: int = 3
    max_iterations: int = 50
    timeout_seconds: int = 300

    # UI configuration
    default_mode: str = "simple"  # "simple" | "expert"
    theme: str = "system"  # "light" | "dark" | "system"
```

### 5.2 Settings Storage

```python
# src/plan_cascade/settings/storage.py

import yaml
import keyring
from pathlib import Path
from .models import Settings

class SettingsStorage:
    """Settings storage management"""

    KEYRING_SERVICE = "plan-cascade"

    def __init__(self, config_dir: Path = None):
        self.config_dir = config_dir or Path.home() / ".plan-cascade"
        self.config_file = self.config_dir / "config.yaml"

    def load(self) -> Settings:
        """Load settings"""
        if not self.config_file.exists():
            return Settings()

        with open(self.config_file) as f:
            data = yaml.safe_load(f) or {}

        return Settings(**data)

    def save(self, settings: Settings):
        """Save settings"""
        self.config_dir.mkdir(parents=True, exist_ok=True)

        with open(self.config_file, "w") as f:
            yaml.dump(settings.__dict__, f)

    def get_api_key(self, provider: str) -> str:
        """Get API Key (from system keychain)"""
        return keyring.get_password(self.KEYRING_SERVICE, provider) or ""

    def set_api_key(self, provider: str, api_key: str):
        """Save API Key (to system keychain)"""
        keyring.set_password(self.KEYRING_SERVICE, provider, api_key)

    def delete_api_key(self, provider: str):
        """Delete API Key"""
        try:
            keyring.delete_password(self.KEYRING_SERVICE, provider)
        except keyring.errors.PasswordDeleteError:
            pass
```

---

## 6. Development Roadmap

### Phase 1: Core Refactor + CLI Dual-Mode (2 weeks)

```
Goal: Independently runnable CLI, supporting simple/expert modes

Tasks:
├── [ ] Create new project structure
├── [ ] Implement Backend abstraction layer
│   ├── [ ] ClaudeCodeBackend
│   └── [ ] BuiltinBackend
├── [ ] Implement StrategyAnalyzer (AI auto-determination)
├── [ ] Implement SimpleWorkflow (simple mode)
├── [ ] Implement ExpertWorkflow (expert mode)
├── [ ] CLI command implementation
│   ├── [ ] plan-cascade run
│   ├── [ ] plan-cascade run --expert
│   ├── [ ] plan-cascade config
│   └── [ ] plan-cascade status
├── [ ] Settings management implementation
└── [ ] Basic tests

Deliverables:
- pip install plan-cascade available
- Supports simple/expert dual modes
```

### Phase 2: Desktop Application Alpha (2 weeks)

```
Goal: Graphical interface, dual-mode + Claude Code GUI

Tasks:
├── [ ] Tauri project setup
├── [ ] FastAPI Sidecar
├── [ ] Simple mode UI
├── [ ] Expert mode UI
├── [ ] Settings page
├── [ ] Claude Code GUI mode
│   ├── [ ] Chat view
│   ├── [ ] Tool call visualization
│   └── [ ] File change preview
└── [ ] Packaging tests

Deliverables:
- Windows/macOS/Linux installers
```

### Phase 3: Feature Completion (2 weeks)

```
Goal: Production ready

Tasks:
├── [ ] Complete PRD editor
├── [ ] Dependency visualization
├── [ ] More LLM backends
├── [ ] Auto-update
├── [ ] Complete documentation
└── [ ] Plugin compatibility verification

Deliverables:
- Stable release
```

### Phase 4: Advanced Features (Ongoing)

```
├── [ ] Multi-Agent collaboration
├── [ ] Git Worktree integration
├── [ ] Team collaboration
└── [ ] Plugin system
```

---

## 7. Appendix

### 7.1 Configuration File Example

```yaml
# ~/.plan-cascade/config.yaml

# Backend configuration
backend: claude-code  # claude-code | builtin
provider: claude      # claude | openai | deepseek | ollama
model: ""            # Leave empty for default

# Execution Agents
agents:
  - name: claude-code
    enabled: true
    command: claude
    is_default: true
  - name: aider
    enabled: true
    command: aider --model gpt-4o
  - name: codex
    enabled: false
    command: codex

# Agent selection strategy
agent_selection: prefer_default  # smart | prefer_default | manual
default_agent: claude-code

# Quality gates
quality_gates:
  typecheck: true
  test: true
  lint: true
  custom: false
  custom_script: ""
  max_retries: 3

# Execution configuration
max_parallel_stories: 3
max_iterations: 50
timeout_seconds: 300

# UI configuration
default_mode: simple  # simple | expert
theme: system        # light | dark | system
```

### 7.2 Glossary

| Term | Definition |
|------|------------|
| Standalone Orchestration Mode | Plan Cascade executes tools itself, LLM only provides thinking |
| Claude Code GUI Mode | Plan Cascade as graphical interface for Claude Code |
| Simple Mode | One-click completion, AI auto-handles everything |
| Expert Mode | Editable PRD, strategy selection, Agent specification |
| Claude Max LLM | Get LLM capability through Claude Code (no API Key) |
| ReAct Engine | Think→Act→Observe loop execution engine |
| Tool Execution Layer | Tools implemented by Plan Cascade itself (Read/Write/Edit/Bash/Glob/Grep) |
| Strategy | Execution strategy (Direct/Hybrid/Mega) |
| Intent Classification | Distinguish user intent: TASK/QUERY/CHAT |
| REPL | Interactive command line, supports continuous dialogue |

### 7.3 Two Working Modes Comparison

| Feature | Standalone Orchestration Mode | Claude Code GUI Mode |
|---------|------------------------------|----------------------|
| Orchestration Layer | Plan Cascade | Plan Cascade |
| Tool Execution | Plan Cascade executes itself | Claude Code CLI executes |
| LLM Source | Claude Max/API, OpenAI, DeepSeek, Ollama | Claude Code |
| PRD-Driven | ✅ Full support | ✅ Full support |
| Batch Execution | ✅ Full support | ✅ Full support |
| Offline Available | ✅ (using Ollama) | ❌ |
| Use Case | Need other LLMs or offline use | Have Claude Max/Code subscription |

**Core Philosophy: Plan Cascade = Brain (Orchestration), Execution Layer = Hands (Tool Execution)**

Both modes are controlled by Plan Cascade for the complete orchestration workflow:
- PRD generation and Story decomposition
- Dependency analysis and batch scheduling
- State tracking and quality gates
- Retry management

The only difference is who executes the tools during Story execution.
