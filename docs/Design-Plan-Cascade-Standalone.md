# Plan Cascade Standalone - 技术设计文档

**版本**: 4.0.0
**日期**: 2026-01-29
**作者**: Plan Cascade Team
**状态**: Implementation In Progress

---

## 实现状态总览

> **当前进度**: ~98% 核心功能已实现
> **最后更新**: 2026-01-29

### 模块实现状态

| 模块 | 状态 | 文件 | 说明 |
|------|------|------|------|
| **核心编排层** | | | |
| 意图分类器 | ✅ 已完成 | `core/intent_classifier.py` | 区分 TASK/QUERY/CHAT |
| 策略分析器 | ✅ 已完成 | `core/strategy_analyzer.py` | AI 自动判断执行策略 |
| PRD 生成器 | ✅ 已完成 | `core/prd_generator.py` | 从需求生成 PRD |
| Mega 生成器 | ✅ 已完成 | `core/mega_generator.py` | 大型项目多 PRD 级联 |
| 编排器 | ✅ 已完成 | `core/orchestrator.py` | 批次依赖分析和调度 |
| 简单模式工作流 | ✅ 已完成 | `core/simple_workflow.py` | 一键式执行 |
| 专家模式工作流 | ✅ 已完成 | `core/expert_workflow.py` | 精细控制 |
| 质量门控 | ✅ 已完成 | `core/quality_gate.py` | typecheck/test/lint |
| 重试管理 | ✅ 已完成 | `core/retry_manager.py` | 智能重试 |
| 迭代循环 | ✅ 已完成 | `core/iteration_loop.py` | 迭代执行 |
| **ReAct 执行引擎** | | | |
| ReAct 引擎 | ✅ 已完成 | `core/react_engine.py` | 独立 Think→Act→Observe 引擎 |
| **后端抽象层** | | | |
| 后端基类 | ✅ 已完成 | `backends/base.py` | AgentBackend 抽象 |
| 后端工厂 | ✅ 已完成 | `backends/factory.py` | 动态创建后端 |
| 内置后端 | ✅ 已完成 | `backends/builtin.py` | ReAct + 工具执行 |
| Claude Code 后端 | ✅ 已完成 | `backends/claude_code.py` | CLI 集成 |
| Agent 执行器 | ✅ 已完成 | `backends/agent_executor.py` | 多 Agent 协同 |
| 阶段配置 | ✅ 已完成 | `backends/phase_config.py` | 阶段/类型 Agent 映射 |
| Claude Code GUI 后端 | ⚠️ 待实现 | `backends/claude_code_gui.py` | P2 优先级 |
| **LLM 抽象层** | | | |
| LLM 基类 | ✅ 已完成 | `llm/base.py` | LLMProvider 抽象 |
| LLM 工厂 | ✅ 已完成 | `llm/factory.py` | 支持 5 种 Provider |
| Claude Provider | ✅ 已完成 | `llm/providers/claude.py` | Anthropic API |
| Claude Max Provider | ✅ 已完成 | `llm/providers/claude_max.py` | 通过 Claude Code 获取 LLM |
| OpenAI Provider | ✅ 已完成 | `llm/providers/openai.py` | OpenAI API |
| DeepSeek Provider | ✅ 已完成 | `llm/providers/deepseek.py` | DeepSeek API |
| Ollama Provider | ✅ 已完成 | `llm/providers/ollama.py` | 本地模型 |
| **工具执行层** | | | |
| 工具注册表 | ✅ 已完成 | `tools/registry.py` | 工具管理 |
| 文件工具 | ✅ 已完成 | `tools/file_tools.py` | Read/Write/Edit |
| 搜索工具 | ✅ 已完成 | `tools/search_tools.py` | Glob/Grep |
| Shell 工具 | ✅ 已完成 | `tools/shell_tools.py` | Bash 执行 |
| **设置与状态** | | | |
| 设置模型 | ✅ 已完成 | `settings/models.py` | 配置数据结构 |
| 设置存储 | ✅ 已完成 | `settings/storage.py` | YAML + Keyring |
| 状态管理 | ✅ 已完成 | `state/state_manager.py` | 状态追踪 |
| 上下文过滤 | ✅ 已完成 | `state/context_filter.py` | 上下文管理 |
| **CLI** | | | |
| CLI 主入口 | ✅ 已完成 | `cli/main.py` | run/config/status/chat |
| 交互式 REPL | ✅ 已完成 | `cli/main.py` | chat 命令 |
| 输出格式化 | ✅ 已完成 | `cli/output.py` | Rich 输出 |
| **桌面应用** | | | |
| Tauri Desktop | ⏳ 规划中 | `desktop/` | Phase 2 目标 |

### 功能实现状态

| 功能 | 状态 | 说明 |
|------|------|------|
| 简单模式 | ✅ 已完成 | 一键执行，AI 自动判断策略 |
| 专家模式 | ✅ 已完成 | PRD 编辑，策略选择，Agent 指定 |
| 交互式 REPL | ✅ 已完成 | `plan-cascade chat` 命令 |
| 流式输出 | ✅ 已完成 | `--include-partial-messages` |
| 多 LLM 后端 | ✅ 已完成 | Claude Max/API, OpenAI, DeepSeek, Ollama |
| 多 Agent 协同 | ✅ 已完成 | 基于阶段/类型的 Agent 选择 |
| 质量门控 | ✅ 已完成 | typecheck/test/lint/custom |
| Git Worktree | ✅ 已完成 | 隔离开发支持 |
| Claude Code GUI 模式 | ⚠️ 待完善 | 基础功能可用，GUI 专用后端待实现 |
| 桌面应用 | ⏳ 规划中 | Tauri 实现，Phase 2 目标 |

---

## 1. 设计目标

### 1.1 核心目标

1. **完整编排能力**: Plan Cascade 自己执行工具（Read/Write/Edit/Bash/Glob/Grep）
2. **多 LLM 支持**: Claude Max（无 API Key）、Claude API、OpenAI、DeepSeek、Ollama
3. **双工作模式**: 独立编排模式（推荐）+ Claude Code GUI 模式
4. **三形态统一**: CLI、Desktop（CLI 的 GUI 版）、Claude Code Plugin
5. **保留核心理念**: 层层分解、并行执行、质量保障、状态追踪

### 1.2 设计约束

| 约束 | 说明 |
|------|------|
| 零 API Key 选项 | Claude Max 用户可通过 Claude Code 获取 LLM 能力 |
| 完整工具执行 | 独立编排模式下自己执行所有工具，不依赖外部 Agent |
| 渐进式披露 | 简单模式隐藏复杂概念，专家模式完全开放 |
| Claude Code 兼容 | GUI 模式完整兼容 Claude Code 所有功能 |
| 跨平台 | 支持 Windows、macOS、Linux |

---

## 2. 双模式架构

### 2.1 模式切换设计

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Plan Cascade                                     │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─────────────────────────┐     ┌─────────────────────────┐           │
│   │      简单模式            │     │      专家模式            │           │
│   │                         │     │                         │           │
│   │  用户输入描述            │     │  用户输入描述            │           │
│   │       ↓                 │     │       ↓                 │           │
│   │  AI 自动判断策略         │     │  生成 PRD (可编辑)       │           │
│   │       ↓                 │     │       ↓                 │           │
│   │  自动生成 PRD           │     │  用户 Review/修改        │           │
│   │       ↓                 │     │       ↓                 │           │
│   │  自动执行               │     │  选择策略/Agent          │           │
│   │       ↓                 │     │       ↓                 │           │
│   │  完成                   │     │  执行                   │           │
│   └─────────────────────────┘     └─────────────────────────┘           │
│                                                                          │
│                              共享核心                                    │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │  Orchestrator │ PRDGenerator │ QualityGate │ AgentExecutor      │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.2 模式实现

```python
# src/plan_cascade/core/mode.py

from enum import Enum
from dataclasses import dataclass
from typing import Optional

class UserMode(Enum):
    """用户操作模式"""
    SIMPLE = "simple"    # 简单模式：一键完成
    EXPERT = "expert"    # 专家模式：精细控制

@dataclass
class ModeConfig:
    """模式配置"""
    mode: UserMode
    auto_execute: bool = True          # 自动执行（简单模式）
    show_prd_editor: bool = False      # 显示 PRD 编辑器
    allow_strategy_select: bool = False # 允许选择执行策略
    allow_agent_select: bool = False   # 允许指定 Agent
    show_detailed_logs: bool = False   # 显示详细日志

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

## 3. AI 自动策略判断

### 3.1 策略类型

```python
# src/plan_cascade/core/strategy.py

from enum import Enum
from dataclasses import dataclass

class ExecutionStrategy(Enum):
    """执行策略"""
    DIRECT = "direct"           # 直接执行，无需 PRD（小任务）
    HYBRID_AUTO = "hybrid_auto" # 自动生成 PRD（中等任务）
    MEGA_PLAN = "mega_plan"     # 多 PRD 级联（大型项目）

@dataclass
class StrategyDecision:
    """策略决策结果"""
    strategy: ExecutionStrategy
    use_worktree: bool
    estimated_stories: int
    confidence: float
    reasoning: str
```

### 3.2 策略判断器

```python
# src/plan_cascade/core/strategy_analyzer.py

from ..llm.base import LLMProvider
from .strategy import ExecutionStrategy, StrategyDecision

class StrategyAnalyzer:
    """
    AI 驱动的策略分析器

    根据用户需求自动判断最佳执行策略
    """

    ANALYSIS_PROMPT = """
分析以下开发需求，判断最适合的执行策略：

需求描述：
{description}

项目上下文：
{context}

请分析并返回 JSON 格式的判断结果：
{{
    "strategy": "direct" | "hybrid_auto" | "mega_plan",
    "use_worktree": true | false,
    "estimated_stories": <预估任务数>,
    "confidence": <0.0-1.0 置信度>,
    "reasoning": "<判断理由>"
}}

判断标准：
- direct: 单一简单任务，如"添加一个按钮"、"修复一个 typo"
- hybrid_auto: 中等功能开发，如"实现用户登录"、"添加搜索功能"
- mega_plan: 大型项目，如"开发完整电商系统"、"重构整个模块"

use_worktree 判断标准：
- true: 需要隔离开发，如"不要影响现有功能"、"实验性功能"
- false: 正常开发
"""

    def __init__(self, llm: LLMProvider):
        self.llm = llm

    async def analyze(
        self,
        description: str,
        context: str = ""
    ) -> StrategyDecision:
        """分析需求，返回策略决策"""
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
        """解析 LLM 响应"""
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

### 3.3 简单模式工作流

```python
# src/plan_cascade/core/simple_workflow.py

class SimpleWorkflow:
    """
    简单模式工作流

    用户输入描述 → AI 分析 → 自动选择策略 → 自动执行 → 完成
    """

    def __init__(self, config: dict):
        self.backend = self._create_backend(config)
        self.strategy_analyzer = StrategyAnalyzer(self.backend.llm)
        self.orchestrator = None

    async def run(self, description: str, project_path: str):
        """一键执行"""
        # 1. 分析策略
        context = await self._gather_context(project_path)
        decision = await self.strategy_analyzer.analyze(description, context)

        # 2. 根据策略执行
        if decision.strategy == ExecutionStrategy.DIRECT:
            # 直接执行，无需 PRD
            return await self._execute_direct(description, context)

        elif decision.strategy == ExecutionStrategy.HYBRID_AUTO:
            # 自动生成 PRD 并执行
            return await self._execute_hybrid(
                description,
                context,
                use_worktree=decision.use_worktree
            )

        elif decision.strategy == ExecutionStrategy.MEGA_PLAN:
            # 大型项目，多 PRD 级联
            return await self._execute_mega(description, context)

    async def _execute_direct(self, description: str, context: str):
        """直接执行简单任务"""
        result = await self.backend.execute(description, context)
        return result

    async def _execute_hybrid(
        self,
        description: str,
        context: str,
        use_worktree: bool = False
    ):
        """Hybrid 模式执行"""
        from .prd_generator import PRDGenerator
        from .orchestrator import Orchestrator

        # 设置 worktree（如果需要）
        if use_worktree:
            await self._setup_worktree()

        # 生成 PRD
        generator = PRDGenerator(self.backend.llm)
        prd = await generator.generate(description, context)

        # 执行
        self.orchestrator = Orchestrator(prd, self.backend)
        result = await self.orchestrator.auto_run()

        return result

    async def _execute_mega(self, description: str, context: str):
        """Mega Plan 执行"""
        from .mega_generator import MegaGenerator

        generator = MegaGenerator(self.backend.llm)
        mega_plan = await generator.generate(description, context)

        # 按顺序执行每个功能模块
        for feature in mega_plan.features:
            await self._execute_hybrid(
                feature.description,
                context + f"\n\n已完成的功能: {mega_plan.get_completed()}"
            )

        return mega_plan
```

---

## 4. 核心架构

### 4.1 双工作模式架构

**核心理念：Plan Cascade = 大脑（编排），执行层 = 手（工具执行）**

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Plan Cascade                                   │
│                    (编排层 - 两种模式共享)                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │                    编排引擎 (共享)                                │   │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │   │
│   │  │ PRD 生成器  │  │ 依赖分析器  │  │  批次调度器 │              │   │
│   │  └─────────────┘  └─────────────┘  └─────────────┘              │   │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │   │
│   │  │ 状态管理器  │  │ 质量门控    │  │  重试管理   │              │   │
│   │  └─────────────┘  └─────────────┘  └─────────────┘              │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                              │                                           │
│                    ┌─────────┴─────────┐                                │
│                    │  执行层选择        │                                │
│                    └─────────┬─────────┘                                │
│              ┌───────────────┴───────────────┐                          │
│              ▼                               ▼                          │
│   ┌─────────────────────────┐    ┌─────────────────────────┐           │
│   │    独立编排模式          │    │  Claude Code GUI 模式   │           │
│   ├─────────────────────────┤    ├─────────────────────────┤           │
│   │                         │    │                         │           │
│   │   内置工具执行引擎       │    │   Claude Code CLI       │           │
│   │   ┌───────────────┐     │    │   ┌───────────────┐     │           │
│   │   │ Read/Write    │     │    │   │ Claude Code   │     │           │
│   │   │ Edit/Bash     │     │    │   │ 执行工具      │     │           │
│   │   │ Glob/Grep     │     │    │   │ (stream-json) │     │           │
│   │   └───────────────┘     │    │   └───────────────┘     │           │
│   │          │              │    │          │              │           │
│   │          ▼              │    │          ▼              │           │
│   │   ┌───────────────┐     │    │   ┌───────────────┐     │           │
│   │   │ LLM 抽象层    │     │    │   │ Plan Cascade  │     │           │
│   │   │ (多种选择)    │     │    │   │ 可视化界面    │     │           │
│   │   └───────────────┘     │    │   └───────────────┘     │           │
│   │          │              │    │                         │           │
│   │   ┌──────┴──────┐       │    │                         │           │
│   │   ▼      ▼      ▼       │    │                         │           │
│   │ Claude Claude OpenAI    │    │                         │           │
│   │ Max    API    etc.      │    │                         │           │
│   │                         │    │                         │           │
│   └─────────────────────────┘    └─────────────────────────┘           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘

两种模式都支持：PRD 驱动开发、批次执行、质量门控、状态追踪
```

### 4.2 独立编排模式架构详解

```
┌─────────────────────────────────────────────────────────────────────────┐
│                       独立编排模式                                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─ 编排层 ─────────────────────────────────────────────────────────┐   │
│  │                                                                    │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                │   │
│  │  │ 意图分类器  │  │ 策略分析器  │  │  PRD 生成器 │                │   │
│  │  │ Intent     │  │ Strategy   │  │ PRDGenerator│                │   │
│  │  │ Classifier │  │ Analyzer   │  │             │                │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                │   │
│  │         │               │               │                          │   │
│  │         └───────────────┴───────────────┘                          │   │
│  │                         │                                          │   │
│  │                         ▼                                          │   │
│  │  ┌─────────────────────────────────────────────────────────────┐  │   │
│  │  │                   Orchestrator                               │  │   │
│  │  │  • 批次依赖分析                                              │  │   │
│  │  │  • 并行执行协调                                              │  │   │
│  │  │  • 质量门控检查                                              │  │   │
│  │  │  • 重试管理                                                  │  │   │
│  │  └─────────────────────────────────────────────────────────────┘  │   │
│  │                         │                                          │   │
│  └─────────────────────────┼──────────────────────────────────────────┘   │
│                            ▼                                              │
│  ┌─ 执行层 ─────────────────────────────────────────────────────────┐   │
│  │                                                                    │   │
│  │  ┌─────────────────────────────────────────────────────────────┐  │   │
│  │  │                   ReAct 执行引擎                             │  │   │
│  │  │                                                              │  │   │
│  │  │   ┌─────────┐     ┌─────────┐     ┌─────────┐               │  │   │
│  │  │   │  Think  │ ──→ │   Act   │ ──→ │ Observe │ ──→ (循环)    │  │   │
│  │  │   │  (LLM)  │     │ (工具)  │     │ (结果)  │               │  │   │
│  │  │   └─────────┘     └─────────┘     └─────────┘               │  │   │
│  │  │                                                              │  │   │
│  │  └─────────────────────────────────────────────────────────────┘  │   │
│  │                         │                                          │   │
│  │                         ▼                                          │   │
│  │  ┌─────────────────────────────────────────────────────────────┐  │   │
│  │  │                   工具执行引擎                               │  │   │
│  │  │                                                              │  │   │
│  │  │   ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐   │  │   │
│  │  │   │  Read  │ │ Write  │ │  Edit  │ │  Bash  │ │  Glob  │   │  │   │
│  │  │   └────────┘ └────────┘ └────────┘ └────────┘ └────────┘   │  │   │
│  │  │   ┌────────┐ ┌────────┐                                     │  │   │
│  │  │   │  Grep  │ │   LS   │                                     │  │   │
│  │  │   └────────┘ └────────┘                                     │  │   │
│  │  │                                                              │  │   │
│  │  └─────────────────────────────────────────────────────────────┘  │   │
│  │                                                                    │   │
│  └────────────────────────────────────────────────────────────────────┘   │
│                            │                                              │
│                            ▼                                              │
│  ┌─ LLM 层 ─────────────────────────────────────────────────────────┐   │
│  │                                                                    │   │
│  │  ┌─────────────────────────────────────────────────────────────┐  │   │
│  │  │                   LLM 抽象层                                 │  │   │
│  │  │              (只提供思考，不执行工具)                        │  │   │
│  │  └─────────────────────────────────────────────────────────────┘  │   │
│  │                         │                                          │   │
│  │       ┌─────────────────┼─────────────────┐                       │   │
│  │       ▼                 ▼                 ▼                       │   │
│  │  ┌─────────┐       ┌─────────┐       ┌─────────┐                 │   │
│  │  │ Claude  │       │ Claude  │       │ OpenAI  │                 │   │
│  │  │   Max   │       │   API   │       │ DeepSeek│                 │   │
│  │  │(via CC) │       │         │       │ Ollama  │                 │   │
│  │  └─────────┘       └─────────┘       └─────────┘                 │   │
│  │                                                                    │   │
│  └────────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 4.3 工具执行引擎

独立编排模式下，Plan Cascade 自己执行所有工具：

```python
# src/plan_cascade/tools/registry.py

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Any
from pathlib import Path

@dataclass
class ToolResult:
    """工具执行结果"""
    success: bool
    output: str
    error: str | None = None

class Tool(ABC):
    """工具基类"""

    @property
    @abstractmethod
    def name(self) -> str:
        """工具名称"""
        pass

    @property
    @abstractmethod
    def description(self) -> str:
        """工具描述"""
        pass

    @property
    @abstractmethod
    def parameters(self) -> dict:
        """参数定义（JSON Schema）"""
        pass

    @abstractmethod
    async def execute(self, **kwargs) -> ToolResult:
        """执行工具"""
        pass

class ToolRegistry:
    """工具注册表"""

    def __init__(self, project_root: Path):
        self.project_root = project_root
        self.tools: dict[str, Tool] = {}
        self._register_builtin_tools()

    def _register_builtin_tools(self):
        """注册内置工具"""
        from .file_tools import ReadTool, WriteTool, EditTool, GlobTool, GrepTool
        from .shell_tools import BashTool

        for tool_class in [ReadTool, WriteTool, EditTool, GlobTool, GrepTool, BashTool]:
            tool = tool_class(self.project_root)
            self.tools[tool.name] = tool

    def get_definitions(self) -> list[dict]:
        """获取所有工具定义（用于 LLM）"""
        return [
            {
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
            }
            for tool in self.tools.values()
        ]

    async def execute(self, name: str, **kwargs) -> ToolResult:
        """执行工具"""
        if name not in self.tools:
            return ToolResult(success=False, output="", error=f"Unknown tool: {name}")

        try:
            return await self.tools[name].execute(**kwargs)
        except Exception as e:
            return ToolResult(success=False, output="", error=str(e))
```

### 4.4 内置工具实现

```python
# src/plan_cascade/tools/file_tools.py

from pathlib import Path
from .registry import Tool, ToolResult

class ReadTool(Tool):
    """读取文件内容"""

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
            numbered = [f"{i + offset + 1:6d}→{line}" for i, line in enumerate(selected)]

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
    """写入文件内容"""

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
    """编辑文件（查找替换）"""

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

### 4.5 ReAct 执行引擎

```python
# src/plan_cascade/core/react_engine.py

from dataclasses import dataclass
from typing import Any
from ..tools.registry import ToolRegistry, ToolResult
from ..llm.base import LLMProvider

@dataclass
class ReActResult:
    """ReAct 执行结果"""
    success: bool
    output: str
    iterations: int
    tool_calls: list[dict]
    error: str | None = None

class ReActEngine:
    """
    ReAct 执行引擎

    实现 Think → Act → Observe 循环：
    1. Think: LLM 思考下一步行动
    2. Act: 执行工具
    3. Observe: 观察结果，回到 Think
    """

    SYSTEM_PROMPT = """你是一个专业的软件开发 Agent。

你有以下工具可用：
{tools}

工作原则：
1. 先阅读相关代码，理解现有结构
2. 遵循项目的代码风格和约定
3. 完成后验证代码可以正常运行
4. 当任务完成时，回复 "TASK_COMPLETE" 并总结完成的工作

重要：你只能通过调用工具来执行操作，不能直接输出代码让用户执行。
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
        """执行任务"""
        # 构建系统提示
        tools_desc = self._format_tools_description()
        system_prompt = self.SYSTEM_PROMPT.format(tools=tools_desc)

        messages = [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": f"{context}\n\n任务：{task}"},
        ]

        tool_calls_history = []

        for iteration in range(self.max_iterations):
            # Think: LLM 决定下一步
            response = await self.llm.complete(
                messages=messages,
                tools=self.tools.get_definitions(),
            )

            # 发送文本到回调
            if self.on_text and response.content:
                self.on_text(response.content)

            # 检查是否完成
            if "TASK_COMPLETE" in (response.content or ""):
                return ReActResult(
                    success=True,
                    output=response.content,
                    iterations=iteration + 1,
                    tool_calls=tool_calls_history,
                )

            # 没有工具调用则继续等待
            if not response.tool_calls:
                messages.append({"role": "assistant", "content": response.content})
                messages.append({
                    "role": "user",
                    "content": "请继续工作。如果任务已完成，请回复 TASK_COMPLETE 并总结。"
                })
                continue

            # Act: 执行工具
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

                # 回调
                if self.on_tool_call:
                    self.on_tool_call(tool_calls_history[-1])

                tool_results.append({
                    "tool_call_id": tool_call.id,
                    "result": result.output if result.success else f"Error: {result.error}",
                })

            # Observe: 添加结果到消息历史
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
        """格式化工具描述"""
        lines = []
        for tool_def in self.tools.get_definitions():
            lines.append(f"- {tool_def['name']}: {tool_def['description']}")
        return "\n".join(lines)

    def _format_tool_results(self, results: list) -> str:
        """格式化工具结果"""
        parts = []
        for r in results:
            parts.append(f"工具执行结果：\n{r['result']}")
        return "\n\n".join(parts)
```

### 4.6 Claude Max LLM 后端

```python
# src/plan_cascade/llm/claude_max.py

import asyncio
import json
from .base import LLMProvider, LLMResponse

class ClaudeMaxLLM(LLMProvider):
    """
    Claude Max LLM 后端

    通过本地 Claude Code 获取 LLM 能力：
    - 发送 prompt 给 Claude Code
    - 禁用 Claude Code 的工具执行（只要思考）
    - 解析响应返回
    """

    def __init__(self, claude_path: str = "claude"):
        self.claude_path = claude_path

    async def complete(
        self,
        messages: list[dict],
        tools: list[dict] | None = None,
        on_text: callable = None,
    ) -> LLMResponse:
        """调用 Claude Code 获取 LLM 响应"""

        # 构建 prompt
        prompt = self._build_prompt(messages, tools)

        # 调用 Claude Code（禁用工具执行）
        process = await asyncio.create_subprocess_exec(
            self.claude_path,
            "--print",
            "--output-format", "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--no-tools",  # 关键：禁用工具执行
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
                    # 解析工具调用请求（如果 LLM 返回）
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
        """构建发送给 Claude Code 的 prompt"""
        parts = []

        for msg in messages:
            role = msg.get("role", "user")
            content = msg.get("content", "")
            parts.append(f"[{role}]\n{content}")

        # 如果有工具定义，添加到 prompt 中让 LLM 知道可以调用
        if tools:
            tools_desc = "\n".join(
                f"- {t['name']}: {t['description']}"
                for t in tools
            )
            parts.append(f"\n可用工具：\n{tools_desc}")
            parts.append("\n如需使用工具，请以 JSON 格式回复：{\"tool\": \"name\", \"args\": {...}}")

        return "\n\n".join(parts)
```

### 4.7 Claude Code GUI 模式后端

Claude Code GUI 模式下，Plan Cascade 仍然控制完整的编排流程，只是 Story 执行由 Claude Code 完成。

```python
# src/plan_cascade/backends/claude_code_gui.py

import asyncio
import json
from .base import AgentBackend, ExecutionResult

class ClaudeCodeGUIBackend(AgentBackend):
    """
    Claude Code GUI 模式后端

    核心理念：Plan Cascade = 大脑，Claude Code = 手

    Plan Cascade 控制：
    - PRD 生成和 Story 分解
    - 依赖分析和批次调度
    - 状态追踪和质量门控
    - 重试管理

    Claude Code 负责：
    - 执行单个 Story 的工具调用
    - Read/Write/Edit/Bash 等操作
    """

    def __init__(self, claude_path: str = "claude"):
        self.claude_path = claude_path
        self.process = None
        self._session_id = None

    async def start_session(self, project_path: str):
        """启动 Claude Code 会话"""
        self.process = await asyncio.create_subprocess_exec(
            self.claude_path,
            "--output-format", "stream-json",
            "--verbose",
            "--include-partial-messages",
            cwd=project_path,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )

    async def execute(self, story: dict, context: str) -> ExecutionResult:
        """
        执行 Story（由 Claude Code 完成工具调用）

        这个方法由 Orchestrator 调用，Orchestrator 负责：
        - 根据依赖关系决定执行顺序
        - 过滤上下文（只提供相关的 findings）
        - 记录状态和进度
        """
        prompt = self._build_prompt(story, context)

        # 发送给 Claude Code
        self.process.stdin.write(f"{prompt}\n".encode())
        await self.process.stdin.drain()

        output_lines = []
        tool_calls = []

        async for line in self.process.stdout:
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
                                output_lines.append(text)
                                if self.on_text:
                                    self.on_text(text)

                elif event_type == "tool_use":
                    tool_calls.append(data)
                    if self.on_tool_call:
                        await self.on_tool_call(data)

                elif event_type == "result":
                    self._session_id = data.get("session_id")
                    break

            except json.JSONDecodeError:
                continue

        return ExecutionResult(
            success=True,
            output="".join(output_lines),
            iterations=len(tool_calls),
            tool_calls=tool_calls,
        )

    def get_name(self) -> str:
        return "claude-code-gui"

    # 回调（用于 UI 更新）
    on_tool_call = None
    on_text = None
```

**与独立编排模式的对比**：

| 组件 | 独立编排模式 | Claude Code GUI 模式 |
|------|--------------|----------------------|
| PRD 生成 | Plan Cascade (LLM) | Plan Cascade (Claude Code) |
| 依赖分析 | Plan Cascade | Plan Cascade |
| 批次调度 | Plan Cascade | Plan Cascade |
| Story 执行 | Plan Cascade (ReAct) | Claude Code CLI |
| 工具调用 | 内置工具引擎 | Claude Code |
| 状态追踪 | Plan Cascade | Plan Cascade |
| 质量门控 | Plan Cascade | Plan Cascade |

### 4.8 多 Agent 协同执行

Plan Cascade 支持多种 Agent 协同工作，可根据执行阶段、Story 类型智能选择最适合的 Agent。

```python
# src/plan_cascade/backends/agent_executor.py

class AgentExecutor:
    """
    Agent 执行抽象层，支持多种 Agent 类型：
    - task-tool: 内置 ReAct 引擎或 Claude Code Task tool
    - cli: 外部 CLI 工具 (codex, aider, amp-code, etc.)

    特性：
    - 自动回退：如果 CLI Agent 不可用，回退到 claude-code
    - 基于阶段的 Agent 选择
    - Story 类型智能匹配
    - 进程管理和状态追踪
    """

    def _resolve_agent(
        self,
        agent_name: str | None = None,
        phase: ExecutionPhase | None = None,
        story: dict | None = None,
        override: AgentOverrides | None = None
    ) -> tuple[str, dict]:
        """
        解析 Agent，带自动回退。

        优先级：
        1. agent_name 参数 (显式覆盖)
        2. 基于阶段的解析 (PhaseAgentManager)
        3. story.agent 元数据
        4. Story 类型覆盖
        5. 阶段默认 Agent
        6. 回退链
        7. claude-code (终极回退)
        """
        pass

    def execute_story(
        self,
        story: dict,
        context: dict,
        phase: ExecutionPhase,
        task_callback: Callable | None = None
    ) -> dict:
        """执行 Story，自动选择合适的 Agent。"""
        resolved_name, agent_config = self._resolve_agent(phase=phase, story=story)

        if agent_config.get("type") == "task-tool":
            return self._execute_via_task_tool(story, context, task_callback)
        elif agent_config.get("type") == "cli":
            return self._execute_via_cli(story, context, agent_config)
```

```python
# src/plan_cascade/backends/phase_config.py

class ExecutionPhase(Enum):
    """执行阶段"""
    PLANNING = "planning"
    IMPLEMENTATION = "implementation"
    RETRY = "retry"
    REFACTOR = "refactor"
    REVIEW = "review"

class StoryType(Enum):
    """Story 类型"""
    FEATURE = "feature"
    BUGFIX = "bugfix"
    REFACTOR = "refactor"
    TEST = "test"
    DOCUMENTATION = "documentation"
    INFRASTRUCTURE = "infrastructure"

class PhaseAgentManager:
    """
    基于阶段的 Agent 选择管理器。

    默认配置：
    - Planning: codex → claude-code
    - Implementation: claude-code → codex → aider
      - bugfix 类型覆盖: codex
      - refactor 类型覆盖: aider
    - Retry: claude-code → aider
    - Refactor: aider → claude-code
    - Review: claude-code → codex
    """

    def get_agent_for_story(
        self,
        story: dict,
        phase: ExecutionPhase,
        override: AgentOverrides | None = None
    ) -> str:
        """获取适合指定 Story 和阶段的 Agent。"""
        pass

    def infer_story_type(self, story: dict) -> StoryType:
        """从 title、description、tags 推断 Story 类型。"""
        pass
```

**两种模式下的多 Agent 支持**：

```
┌─────────────────────────────────────────────────────────────────────────┐
│                       多 Agent 协同架构                                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   Plan Cascade 编排层                                                    │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │  Orchestrator → AgentExecutor → PhaseAgentManager               │   │
│   │       │              │               │                           │   │
│   │       │              │               └─ 阶段/类型 → Agent 映射   │   │
│   │       │              └─ 解析最佳 Agent                           │   │
│   │       └─ 调度 Story 执行                                        │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                              │                                           │
│              ┌───────────────┴───────────────┐                          │
│              ▼                               ▼                          │
│   ┌─────────────────────────┐    ┌─────────────────────────┐           │
│   │    独立编排模式          │    │  Claude Code GUI 模式   │           │
│   │                         │    │                         │           │
│   │   默认 Agent:            │    │   默认 Agent:            │           │
│   │   内置 ReAct 引擎        │    │   Claude Code CLI       │           │
│   │                         │    │                         │           │
│   │   可选 CLI Agents:       │    │   可选 CLI Agents:       │           │
│   │   codex, aider, amp...  │    │   codex, aider, amp...  │           │
│   │                         │    │                         │           │
│   └─────────────────────────┘    └─────────────────────────┘           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 4.5 后端工厂

```python
# src/plan_cascade/backends/factory.py

from typing import Optional
from .base import AgentBackend
from .claude_code import ClaudeCodeBackend
from .builtin import BuiltinBackend

class BackendFactory:
    """后端工厂"""

    @staticmethod
    def create(config: dict) -> AgentBackend:
        """
        根据配置创建后端

        config:
            backend: "claude-code" | "builtin" | "aider" | ...
            provider: "claude" | "openai" | ...  (builtin 模式需要)
            api_key: "..."  (builtin 模式需要)
            model: "..."  (可选)
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
        """从设置创建后端"""
        return BackendFactory.create({
            "backend": settings.backend,
            "provider": settings.provider,
            "model": settings.model,
            "api_key": settings.get_api_key(),
        })
```

---

## 5. 设置管理

### 5.1 设置结构

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
    """执行 Agent 配置"""
    name: str
    enabled: bool = True
    command: str = ""
    is_default: bool = False

@dataclass
class QualityGateConfig:
    """质量门控配置"""
    typecheck: bool = True
    test: bool = True
    lint: bool = True
    custom: bool = False
    custom_script: str = ""
    max_retries: int = 3

@dataclass
class Settings:
    """全局设置"""
    # 后端配置
    backend: BackendType = BackendType.CLAUDE_CODE
    provider: str = "claude"
    model: str = ""
    # API Key 存储在 keyring 中，不在配置文件

    # 执行 Agent 列表
    agents: List[AgentConfig] = field(default_factory=lambda: [
        AgentConfig(name="claude-code", enabled=True, command="claude", is_default=True),
        AgentConfig(name="aider", enabled=False, command="aider"),
        AgentConfig(name="codex", enabled=False, command="codex"),
    ])

    # Agent 选择策略
    agent_selection: str = "prefer_default"  # "smart" | "prefer_default" | "manual"
    default_agent: str = "claude-code"

    # 质量门控
    quality_gates: QualityGateConfig = field(default_factory=QualityGateConfig)

    # 执行配置
    max_parallel_stories: int = 3
    max_iterations: int = 50
    timeout_seconds: int = 300

    # UI 配置
    default_mode: str = "simple"  # "simple" | "expert"
    theme: str = "system"  # "light" | "dark" | "system"
```

### 5.2 设置存储

```python
# src/plan_cascade/settings/storage.py

import yaml
import keyring
from pathlib import Path
from .models import Settings

class SettingsStorage:
    """设置存储管理"""

    KEYRING_SERVICE = "plan-cascade"

    def __init__(self, config_dir: Path = None):
        self.config_dir = config_dir or Path.home() / ".plan-cascade"
        self.config_file = self.config_dir / "config.yaml"

    def load(self) -> Settings:
        """加载设置"""
        if not self.config_file.exists():
            return Settings()

        with open(self.config_file) as f:
            data = yaml.safe_load(f) or {}

        return Settings(**data)

    def save(self, settings: Settings):
        """保存设置"""
        self.config_dir.mkdir(parents=True, exist_ok=True)

        with open(self.config_file, "w") as f:
            yaml.dump(settings.__dict__, f)

    def get_api_key(self, provider: str) -> str:
        """获取 API Key（从系统密钥库）"""
        return keyring.get_password(self.KEYRING_SERVICE, provider) or ""

    def set_api_key(self, provider: str, api_key: str):
        """保存 API Key（到系统密钥库）"""
        keyring.set_password(self.KEYRING_SERVICE, provider, api_key)

    def delete_api_key(self, provider: str):
        """删除 API Key"""
        try:
            keyring.delete_password(self.KEYRING_SERVICE, provider)
        except keyring.errors.PasswordDeleteError:
            pass
```

---

## 6. CLI 设计

### 6.1 双模式命令

```python
# src/plan_cascade/cli/main.py

import typer
from rich.console import Console
from pathlib import Path

app = typer.Typer(
    name="plan-cascade",
    help="让 AI 编程变得简单"
)
console = Console()

@app.command()
def run(
    description: str = typer.Argument(..., help="任务描述"),
    expert: bool = typer.Option(False, "--expert", "-e", help="专家模式"),
    backend: str = typer.Option(None, "--backend", "-b", help="后端选择"),
):
    """
    执行开发任务

    简单模式（默认）：自动完成全部流程
    专家模式（--expert）：可编辑 PRD、选择 Agent 等
    """
    import asyncio

    if expert:
        asyncio.run(_run_expert(description, backend))
    else:
        asyncio.run(_run_simple(description, backend))

async def _run_simple(description: str, backend: str = None):
    """简单模式执行"""
    from ..core.simple_workflow import SimpleWorkflow
    from ..settings.storage import SettingsStorage

    console.print(f"[blue]正在处理: {description}[/blue]")

    settings = SettingsStorage().load()
    config = _build_config(settings, backend)

    workflow = SimpleWorkflow(config)

    # 进度回调
    async def on_progress(event):
        if event["type"] == "strategy_decided":
            console.print(f"[dim]策略: {event['strategy']}[/dim]")
        elif event["type"] == "story_started":
            console.print(f"⟳ {event['story_title']}")
        elif event["type"] == "story_completed":
            console.print(f"✓ {event['story_title']}")
        elif event["type"] == "story_failed":
            console.print(f"✗ {event['story_title']}: {event['error']}")

    workflow.on_progress = on_progress

    result = await workflow.run(description, str(Path.cwd()))

    if result.success:
        console.print("[green]✓ 完成[/green]")
    else:
        console.print(f"[red]✗ 失败: {result.error}[/red]")

async def _run_expert(description: str, backend: str = None):
    """专家模式执行"""
    from ..core.prd_generator import PRDGenerator
    from ..settings.storage import SettingsStorage
    from ..backends.factory import BackendFactory
    from rich.prompt import Prompt, Confirm

    settings = SettingsStorage().load()
    config = _build_config(settings, backend)
    backend_instance = BackendFactory.create(config)

    # 1. 生成 PRD
    console.print("[blue]正在生成 PRD...[/blue]")
    generator = PRDGenerator(backend_instance.get_llm())
    prd = await generator.generate(description)

    console.print(f"[green]✓ 已生成 PRD ({len(prd.stories)} 个 Stories)[/green]")

    # 2. 交互式菜单
    while True:
        choice = Prompt.ask(
            "请选择操作",
            choices=["view", "edit", "agent", "run", "save", "quit"],
            default="view"
        )

        if choice == "view":
            _display_prd(prd)
        elif choice == "edit":
            prd = await _edit_prd(prd)
        elif choice == "agent":
            prd = await _assign_agents(prd, settings.agents)
        elif choice == "run":
            await _execute_prd(prd, backend_instance)
            break
        elif choice == "save":
            _save_prd(prd)
            console.print("[green]✓ 已保存[/green]")
        elif choice == "quit":
            break

def _build_config(settings, backend_override: str = None) -> dict:
    """构建配置"""
    storage = SettingsStorage()

    backend = backend_override or settings.backend.value

    return {
        "backend": backend,
        "provider": settings.provider,
        "model": settings.model,
        "api_key": storage.get_api_key(settings.provider),
    }

@app.command()
def config(
    show: bool = typer.Option(False, "--show", help="显示当前配置"),
    setup: bool = typer.Option(False, "--setup", help="运行配置向导"),
):
    """配置管理"""
    from ..settings.storage import SettingsStorage

    storage = SettingsStorage()

    if show:
        settings = storage.load()
        console.print(f"后端: {settings.backend.value}")
        console.print(f"Provider: {settings.provider}")
        console.print(f"Model: {settings.model or '(默认)'}")
        console.print(f"默认模式: {settings.default_mode}")
    elif setup:
        _run_setup_wizard(storage)
    else:
        console.print("使用 --show 查看配置或 --setup 运行向导")

def _run_setup_wizard(storage):
    """配置向导"""
    from rich.prompt import Prompt
    from ..settings.models import BackendType

    console.print("\n[bold]Plan Cascade 配置向导[/bold]\n")

    # 1. 选择后端
    console.print("选择后端:")
    console.print("  1. Claude Code (推荐，无需 API Key)")
    console.print("  2. Claude API")
    console.print("  3. OpenAI")
    console.print("  4. DeepSeek")
    console.print("  5. Ollama (本地)")

    choice = Prompt.ask("选择", choices=["1", "2", "3", "4", "5"], default="1")

    backend_map = {
        "1": BackendType.CLAUDE_CODE,
        "2": BackendType.CLAUDE_API,
        "3": BackendType.OPENAI,
        "4": BackendType.DEEPSEEK,
        "5": BackendType.OLLAMA,
    }

    settings = storage.load()
    settings.backend = backend_map[choice]

    # 2. 如果需要 API Key
    if choice != "1":
        provider = {
            "2": "claude",
            "3": "openai",
            "4": "deepseek",
            "5": "ollama",
        }[choice]

        settings.provider = provider

        if choice != "5":  # Ollama 不需要 API Key
            api_key = Prompt.ask(f"输入 {provider} API Key", password=True)
            storage.set_api_key(provider, api_key)

    storage.save(settings)
    console.print("\n[green]✓ 配置已保存[/green]")

@app.command()
def status():
    """查看执行状态"""
    from ..state.state_manager import StateManager

    state = StateManager()
    status = state.get_status()

    if not status:
        console.print("[dim]没有正在进行的任务[/dim]")
        return

    console.print(f"[bold]任务: {status['title']}[/bold]")
    console.print(f"进度: {status['completed']}/{status['total']}")

    for story in status["stories"]:
        icon = {"pending": "○", "in_progress": "⟳", "completed": "✓", "failed": "✗"}
        console.print(f"  {icon[story['status']]} {story['title']}")

def main():
    app()

if __name__ == "__main__":
    main()
```

---

## 7. 桌面应用设计

### 7.1 技术栈

| 组件 | 技术选择 | 理由 |
|------|----------|------|
| 框架 | Tauri | 轻量 (~10MB)，跨平台 |
| 前端 | React + TypeScript | 成熟生态 |
| 状态管理 | Zustand | 轻量，易用 |
| UI 组件 | Radix UI + Tailwind | 可访问性好 |
| 后端 | Python Sidecar (FastAPI) | 复用核心代码 |

### 7.2 组件结构

```
desktop/
├── src/
│   ├── components/
│   │   ├── ModeSwitch.tsx         # 简单/专家模式切换
│   │   ├── SimpleMode/
│   │   │   ├── InputBox.tsx       # 需求输入框
│   │   │   ├── ProgressView.tsx   # 简化进度视图
│   │   │   └── ResultView.tsx     # 结果展示
│   │   ├── ExpertMode/
│   │   │   ├── PRDEditor.tsx      # PRD 编辑器
│   │   │   ├── StrategySelect.tsx # 策略选择
│   │   │   ├── AgentSelect.tsx    # Agent 选择
│   │   │   ├── DependencyGraph.tsx# 依赖关系图
│   │   │   └── DetailedLogs.tsx   # 详细日志
│   │   ├── ClaudeCodeMode/
│   │   │   ├── ChatView.tsx       # 对话视图
│   │   │   ├── ToolCallViewer.tsx # 工具调用可视化
│   │   │   └── DiffPreview.tsx    # 文件变更预览
│   │   └── Settings/
│   │       ├── BackendConfig.tsx  # 后端配置
│   │       ├── AgentConfig.tsx    # Agent 配置
│   │       └── QualityConfig.tsx  # 质量门控配置
│   ├── store/
│   │   ├── mode.ts                # 模式状态
│   │   ├── execution.ts           # 执行状态
│   │   └── settings.ts            # 设置状态
│   └── App.tsx
```

### 7.3 主界面布局

```tsx
// desktop/src/App.tsx

import { ModeSwitch } from './components/ModeSwitch';
import { SimpleMode } from './components/SimpleMode';
import { ExpertMode } from './components/ExpertMode';
import { useModeStore } from './store/mode';

export function App() {
  const { mode, setMode } = useModeStore();

  return (
    <div className="h-screen flex flex-col">
      {/* 顶部栏 */}
      <header className="h-12 border-b flex items-center px-4 justify-between">
        <h1 className="font-bold">Plan Cascade</h1>
        <div className="flex items-center gap-4">
          <ModeSwitch mode={mode} onChange={setMode} />
          <SettingsButton />
        </div>
      </header>

      {/* 主内容 */}
      <main className="flex-1 overflow-hidden">
        {mode === 'simple' ? <SimpleMode /> : <ExpertMode />}
      </main>
    </div>
  );
}
```

### 7.4 简单模式 UI

```tsx
// desktop/src/components/SimpleMode/index.tsx

import { useState } from 'react';
import { InputBox } from './InputBox';
import { ProgressView } from './ProgressView';
import { ResultView } from './ResultView';
import { useExecutionStore } from '../../store/execution';

export function SimpleMode() {
  const { status, start } = useExecutionStore();
  const [description, setDescription] = useState('');

  const handleStart = async () => {
    await start(description);
  };

  return (
    <div className="h-full flex flex-col p-6">
      {/* 输入区域 */}
      <div className="max-w-2xl mx-auto w-full">
        <InputBox
          value={description}
          onChange={setDescription}
          onSubmit={handleStart}
          disabled={status === 'running'}
        />
      </div>

      {/* 进度/结果 */}
      <div className="flex-1 mt-8 overflow-auto">
        {status === 'running' && <ProgressView />}
        {status === 'completed' && <ResultView />}
      </div>
    </div>
  );
}
```

### 7.5 设置页面

```tsx
// desktop/src/components/Settings/BackendConfig.tsx

import { useSettingsStore } from '../../store/settings';

export function BackendConfig() {
  const { backend, setBackend, provider, apiKey, setApiKey } = useSettingsStore();

  return (
    <div className="space-y-6">
      <section>
        <h3 className="font-medium mb-3">主后端</h3>
        <div className="space-y-2">
          <RadioOption
            label="Claude Code（推荐）"
            description="作为 Claude Code 的图形界面，无需 API Key"
            value="claude-code"
            checked={backend === 'claude-code'}
            onChange={() => setBackend('claude-code')}
          />
          <RadioOption
            label="Claude API"
            description="直接调用 Anthropic API"
            value="claude-api"
            checked={backend === 'claude-api'}
            onChange={() => setBackend('claude-api')}
          />
          <RadioOption
            label="OpenAI"
            description="使用 GPT-4o 等模型"
            value="openai"
            checked={backend === 'openai'}
            onChange={() => setBackend('openai')}
          />
          {/* ... 更多选项 */}
        </div>
      </section>

      {/* API Key 配置（非 Claude Code 模式） */}
      {backend !== 'claude-code' && (
        <section>
          <h3 className="font-medium mb-3">API Key</h3>
          <input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            className="w-full border rounded px-3 py-2"
            placeholder={`输入 ${provider} API Key`}
          />
        </section>
      )}
    </div>
  );
}
```

---

## 8. 项目结构

```
plan-cascade/
│
├── src/plan_cascade/              # 核心 Python 包
│   ├── __init__.py
│   │
│   ├── core/                      # 核心逻辑
│   │   ├── mode.py                # 模式定义
│   │   ├── strategy.py            # 策略定义
│   │   ├── strategy_analyzer.py   # AI 策略分析
│   │   ├── intent_classifier.py   # 意图分类器
│   │   ├── simple_workflow.py     # 简单模式工作流
│   │   ├── expert_workflow.py     # 专家模式工作流
│   │   ├── orchestrator.py        # 批次编排器
│   │   ├── prd_generator.py       # PRD 生成
│   │   ├── mega_generator.py      # Mega Plan 生成
│   │   ├── react_engine.py        # ReAct 执行引擎
│   │   ├── quality_gate.py        # 质量门控
│   │   └── retry_manager.py       # 重试管理
│   │
│   ├── backends/                  # 后端抽象
│   │   ├── base.py                # 后端基类
│   │   ├── factory.py             # 后端工厂
│   │   ├── builtin.py             # 独立编排后端（执行工具）
│   │   └── claude_code_gui.py     # Claude Code GUI 后端
│   │
│   ├── llm/                       # LLM 抽象层
│   │   ├── base.py                # Provider 基类
│   │   ├── factory.py             # Provider 工厂
│   │   └── providers/
│   │       ├── claude_max.py      # Claude Max (通过 Claude Code)
│   │       ├── claude_api.py      # Claude API (直接调用)
│   │       ├── openai.py          # OpenAI API
│   │       ├── deepseek.py        # DeepSeek API
│   │       └── ollama.py          # Ollama (本地)
│   │
│   ├── tools/                     # 工具执行层
│   │   ├── registry.py            # 工具注册表
│   │   ├── base.py                # 工具基类
│   │   ├── file_tools.py          # Read/Write/Edit
│   │   ├── search_tools.py        # Glob/Grep
│   │   └── shell_tools.py         # Bash 执行
│   │
│   ├── settings/                  # 设置管理
│   │   ├── models.py              # 设置模型
│   │   └── storage.py             # 设置存储
│   │
│   ├── state/                     # 状态管理
│   │   ├── state_manager.py
│   │   └── context_filter.py
│   │
│   └── cli/                       # CLI 入口
│       ├── main.py                # CLI 主入口
│       ├── repl.py                # 交互式 REPL
│       └── output.py              # 输出格式化
│
├── server/                        # FastAPI 服务 (Desktop 后端)
│   └── plan_cascade_server/
│       ├── main.py
│       ├── routes/
│       └── websocket.py
│
├── desktop/                       # Desktop (CLI 的 GUI 版)
│   ├── src/
│   │   ├── components/
│   │   │   ├── ModeSwitch.tsx     # 简单/专家模式切换
│   │   │   ├── SimpleMode/        # 简单模式 UI
│   │   │   ├── ExpertMode/        # 专家模式 UI
│   │   │   ├── ClaudeCodeGUI/     # Claude Code GUI 模式
│   │   │   └── Settings/          # 设置页面
│   │   ├── store/
│   │   └── App.tsx
│   └── src-tauri/
│
├── plugin/                        # Claude Code Plugin (保持兼容)
│   ├── .claude-plugin/
│   ├── commands/
│   └── skills/
│
├── tests/
├── docs/
│   ├── PRD-Plan-Cascade-Standalone.md
│   └── Design-Plan-Cascade-Standalone.md
├── pyproject.toml
└── README.md
```

### 8.1 关键模块说明

| 模块 | 说明 |
|------|------|
| `core/react_engine.py` | ReAct 执行引擎，实现 Think→Act→Observe 循环 |
| `core/intent_classifier.py` | 意图分类器，区分 TASK/QUERY/CHAT |
| `tools/` | 工具执行层，Plan Cascade 自己执行的工具 |
| `llm/providers/claude_max.py` | 通过 Claude Code 获取 LLM 能力（无需 API Key） |
| `backends/builtin.py` | 独立编排后端，使用 ReAct + 工具执行 |
| `backends/claude_code_gui.py` | Claude Code GUI 后端，提供可视化 |

---

## 9. 开发路线图

### Phase 1: 核心重构 + CLI 双模式 (2 周)

```
目标: 可独立运行的 CLI，支持简单/专家模式

任务:
├── [ ] 创建新项目结构
├── [ ] 实现 Backend 抽象层
│   ├── [ ] ClaudeCodeBackend
│   └── [ ] BuiltinBackend
├── [ ] 实现 StrategyAnalyzer（AI 自动判断）
├── [ ] 实现 SimpleWorkflow（简单模式）
├── [ ] 实现 ExpertWorkflow（专家模式）
├── [ ] CLI 命令实现
│   ├── [ ] plan-cascade run
│   ├── [ ] plan-cascade run --expert
│   ├── [ ] plan-cascade config
│   └── [ ] plan-cascade status
├── [ ] 设置管理实现
└── [ ] 基础测试

交付物:
- pip install plan-cascade 可用
- 支持简单/专家两种模式
```

### Phase 2: 桌面应用 Alpha (2 周)

```
目标: 图形化界面，双模式 + Claude Code GUI

任务:
├── [ ] Tauri 项目搭建
├── [ ] FastAPI Sidecar
├── [ ] 简单模式 UI
├── [ ] 专家模式 UI
├── [ ] 设置页面
├── [ ] Claude Code GUI 模式
│   ├── [ ] 对话视图
│   ├── [ ] 工具调用可视化
│   └── [ ] 文件变更预览
└── [ ] 打包测试

交付物:
- Windows/macOS/Linux 安装包
```

### Phase 3: 功能完善 (2 周)

```
目标: 生产可用

任务:
├── [ ] 完整 PRD 编辑器
├── [ ] 依赖关系可视化
├── [ ] 更多 LLM 后端
├── [ ] 自动更新
├── [ ] 完善文档
└── [ ] Plugin 兼容性验证

交付物:
- 稳定版发布
```

### Phase 4: 高级功能 (持续)

```
├── [ ] 多 Agent 协作
├── [ ] Git Worktree 集成
├── [ ] 团队协作
└── [ ] 插件系统
```

---

## 10. 附录

### 10.1 配置文件示例

```yaml
# ~/.plan-cascade/config.yaml

# 后端配置
backend: claude-code  # claude-code | builtin
provider: claude      # claude | openai | deepseek | ollama
model: ""            # 留空使用默认

# 执行 Agent
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

# Agent 选择策略
agent_selection: prefer_default  # smart | prefer_default | manual
default_agent: claude-code

# 质量门控
quality_gates:
  typecheck: true
  test: true
  lint: true
  custom: false
  custom_script: ""
  max_retries: 3

# 执行配置
max_parallel_stories: 3
max_iterations: 50
timeout_seconds: 300

# UI 配置
default_mode: simple  # simple | expert
theme: system        # light | dark | system
```

### 10.2 术语表

| 术语 | 定义 |
|------|------|
| 独立编排模式 | Plan Cascade 自己执行工具，LLM 只提供思考 |
| Claude Code GUI 模式 | Plan Cascade 作为 Claude Code 的图形界面 |
| 简单模式 | 一键完成，AI 自动处理一切 |
| 专家模式 | 可编辑 PRD、选择策略、指定 Agent |
| Claude Max LLM | 通过 Claude Code 获取 LLM 能力（无需 API Key） |
| ReAct 引擎 | Think→Act→Observe 循环执行引擎 |
| 工具执行层 | Plan Cascade 自己实现的工具（Read/Write/Edit/Bash/Glob/Grep） |
| Strategy | 执行策略 (Direct/Hybrid/Mega) |
| 意图分类 | 区分用户意图：TASK/QUERY/CHAT |
| REPL | 交互式命令行，支持连续对话 |

### 10.3 两种工作模式对比

| 特性 | 独立编排模式 | Claude Code GUI 模式 |
|------|--------------|----------------------|
| 编排层 | Plan Cascade | Plan Cascade |
| 工具执行 | Plan Cascade 自己执行 | Claude Code CLI 执行 |
| LLM 来源 | Claude Max/API, OpenAI, DeepSeek, Ollama | Claude Code |
| PRD 驱动 | ✅ 完整支持 | ✅ 完整支持 |
| 批次执行 | ✅ 完整支持 | ✅ 完整支持 |
| 离线可用 | ✅ (使用 Ollama) | ❌ |
| 适用场景 | 需要其他 LLM 或离线使用 | 有 Claude Max/Code 订阅 |

**核心理念：Plan Cascade = 大脑（编排），执行层 = 手（工具执行）**

两种模式都由 Plan Cascade 控制完整的编排流程：
- PRD 生成和 Story 分解
- 依赖分析和批次调度
- 状态追踪和质量门控
- 重试管理

区别只在于 Story 执行时的工具调用由谁完成。
