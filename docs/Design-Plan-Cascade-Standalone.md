# Plan Cascade Standalone - 技术设计文档

**版本**: 2.0.0
**日期**: 2026-01-28
**作者**: Plan Cascade Team
**状态**: Draft

---

## 1. 设计目标

### 1.1 核心目标

1. **易用性优先**: 简单模式一键完成，专家模式精细控制
2. **双后端支持**: Claude Code GUI 模式 + 独立 LLM 模式
3. **保留核心理念**: 层层分解、并行执行、多 Agent 协作、质量保障、状态追踪
4. **三形态统一**: 同一套代码支持 CLI、桌面应用、Claude Code Plugin

### 1.2 设计约束

| 约束 | 说明 |
|------|------|
| 默认零配置 | 选择 Claude Code 后端时无需 API Key |
| 渐进式披露 | 简单模式隐藏复杂概念，专家模式完全开放 |
| 保持兼容 | Claude Code Plugin 功能完整保留 |
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

## 4. 后端抽象层

### 4.1 双后端架构

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Plan Cascade                                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│                         Backend Abstraction                              │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │                     AgentBackend (ABC)                           │   │
│   │   + execute(story, context) -> Result                           │   │
│   │   + get_llm() -> LLMProvider                                    │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                              │                                           │
│              ┌───────────────┼───────────────┐                          │
│              ▼               ▼               ▼                          │
│   ┌─────────────────┐ ┌─────────────┐ ┌─────────────────┐              │
│   │ ClaudeCodeBackend│ │BuiltinBackend│ │ ExternalBackend │              │
│   │                 │ │             │ │                 │              │
│   │ Plan Cascade    │ │ 独立运行    │ │ aider/codex等  │              │
│   │ = Claude Code   │ │ 调用 LLM API│ │                 │              │
│   │   的 GUI        │ │             │ │                 │              │
│   └─────────────────┘ └─────────────┘ └─────────────────┘              │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 4.2 后端实现

```python
# src/plan_cascade/backends/base.py

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Optional

@dataclass
class ExecutionResult:
    """执行结果"""
    success: bool
    output: str
    iterations: int = 0
    error: Optional[str] = None

class AgentBackend(ABC):
    """Agent 后端抽象基类"""

    @abstractmethod
    async def execute(self, story: dict, context: str) -> ExecutionResult:
        """执行单个 Story"""
        pass

    @abstractmethod
    def get_llm(self):
        """获取 LLM Provider（用于 PRD 生成等）"""
        pass

    @abstractmethod
    def get_name(self) -> str:
        """后端名称"""
        pass
```

### 4.3 Claude Code 后端（GUI 模式）

```python
# src/plan_cascade/backends/claude_code.py

import asyncio
import json
from .base import AgentBackend, ExecutionResult

class ClaudeCodeBackend(AgentBackend):
    """
    Claude Code 后端

    Plan Cascade 作为 Claude Code 的 GUI：
    - 通过子进程与 Claude Code 通信
    - 解析 Claude Code 的输出用于可视化
    - 不需要用户配置 API Key
    """

    def __init__(self, claude_path: str = "claude"):
        self.claude_path = claude_path
        self.process = None
        self._llm = None  # 延迟初始化

    async def start_session(self, project_path: str):
        """启动 Claude Code 会话"""
        self.process = await asyncio.create_subprocess_exec(
            self.claude_path,
            "--output-format", "stream-json",
            "--print", "tools",  # 输出工具调用信息
            cwd=project_path,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )

    async def execute(self, story: dict, context: str) -> ExecutionResult:
        """执行 Story"""
        prompt = self._build_prompt(story, context)

        # 发送给 Claude Code
        self.process.stdin.write(f"{prompt}\n".encode())
        await self.process.stdin.drain()

        # 收集输出
        output_lines = []
        tool_calls = []

        async for line in self.process.stdout:
            data = json.loads(line.decode())

            if data.get("type") == "tool_use":
                tool_calls.append(data)
                # 触发回调用于 UI 更新
                if self.on_tool_call:
                    await self.on_tool_call(data)

            elif data.get("type") == "text":
                output_lines.append(data.get("content", ""))

            elif data.get("type") == "end":
                break

        return ExecutionResult(
            success=True,
            output="\n".join(output_lines),
            iterations=len(tool_calls)
        )

    def get_llm(self):
        """获取 LLM（用于 PRD 生成）"""
        # Claude Code 模式下，PRD 生成也通过 Claude Code
        if self._llm is None:
            from .claude_code_llm import ClaudeCodeLLM
            self._llm = ClaudeCodeLLM(self)
        return self._llm

    def get_name(self) -> str:
        return "claude-code"

    def _build_prompt(self, story: dict, context: str) -> str:
        """构建 prompt"""
        return f"""
请完成以下开发任务：

## Story: {story.get('title', story.get('id'))}
{story.get('description', '')}

## 验收标准
{self._format_acceptance_criteria(story)}

## 上下文
{context}

完成后请告诉我结果。
"""

    def _format_acceptance_criteria(self, story: dict) -> str:
        ac = story.get("acceptance_criteria", [])
        if isinstance(ac, list):
            return "\n".join(f"- {item}" for item in ac)
        return str(ac)

    # 回调函数，用于 UI 更新
    on_tool_call = None
    on_text = None
```

### 4.4 内置后端（独立 LLM 模式）

```python
# src/plan_cascade/backends/builtin.py

from .base import AgentBackend, ExecutionResult
from ..llm.base import LLMProvider
from ..llm.factory import LLMFactory
from ..tools.registry import ToolRegistry

class BuiltinBackend(AgentBackend):
    """
    内置后端

    使用用户配置的 LLM API 独立运行
    - 需要用户配置 API Key
    - 自己实现工具调用循环
    """

    SYSTEM_PROMPT = """你是一个专业的软件开发 Agent。

你有以下工具可用：
- read_file: 读取文件内容
- write_file: 创建或覆盖文件
- edit_file: 编辑文件的特定部分
- run_command: 执行 Shell 命令
- search_files: 搜索文件
- grep: 在文件中搜索内容

工作原则：
1. 先阅读相关代码，理解现有结构
2. 遵循项目的代码风格和约定
3. 完成后验证代码可以正常运行
"""

    def __init__(
        self,
        provider: str = "claude",
        model: str = None,
        api_key: str = None,
        config: dict = None
    ):
        config = config or {}
        self.llm = LLMFactory.create(
            provider=provider or config.get("provider", "claude"),
            model=model or config.get("model"),
            api_key=api_key or config.get("api_key")
        )
        self.tools = ToolRegistry()
        self.max_iterations = config.get("max_iterations", 50)

    async def execute(self, story: dict, context: str) -> ExecutionResult:
        """执行 Story（ReAct 循环）"""
        messages = [
            {"role": "system", "content": self.SYSTEM_PROMPT},
            {"role": "user", "content": self._build_prompt(story, context)}
        ]

        tool_calls_count = 0

        for iteration in range(self.max_iterations):
            response = await self.llm.complete(
                messages=messages,
                tools=self.tools.get_definitions()
            )

            # 检查是否完成
            if response.stop_reason == "end_turn" or not response.tool_calls:
                return ExecutionResult(
                    success=True,
                    output=response.content,
                    iterations=iteration + 1
                )

            # 执行工具调用
            tool_results = []
            for tool_call in response.tool_calls:
                tool_calls_count += 1
                result = await self.tools.execute(
                    tool_call.name,
                    **tool_call.arguments
                )
                tool_results.append({
                    "tool_call_id": tool_call.id,
                    "result": result
                })

                # 触发回调
                if self.on_tool_call:
                    await self.on_tool_call({
                        "name": tool_call.name,
                        "arguments": tool_call.arguments,
                        "result": result
                    })

            # 添加到消息历史
            messages.append({
                "role": "assistant",
                "content": response.content,
                "tool_calls": response.tool_calls
            })
            messages.append({
                "role": "user",
                "content": self._format_tool_results(tool_results)
            })

        return ExecutionResult(
            success=False,
            output="",
            iterations=self.max_iterations,
            error="Max iterations reached"
        )

    def get_llm(self):
        return self.llm

    def get_name(self) -> str:
        return "builtin"

    def _build_prompt(self, story: dict, context: str) -> str:
        """构建 prompt"""
        ac = story.get("acceptance_criteria", [])
        ac_text = "\n".join(f"- {item}" for item in ac) if isinstance(ac, list) else str(ac)

        return f"""
请完成以下开发任务：

## Story: {story.get('title', story.get('id'))}
{story.get('description', '')}

## 验收标准
{ac_text}

## 项目上下文
{context}

请开始工作。
"""

    def _format_tool_results(self, results: list) -> str:
        return "\n\n".join(f"Tool result:\n{r['result']}" for r in results)

    # 回调
    on_tool_call = None
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
│   │   ├── simple_workflow.py     # 简单模式工作流
│   │   ├── expert_workflow.py     # 专家模式工作流
│   │   ├── orchestrator.py        # 批次编排器
│   │   ├── prd_generator.py       # PRD 生成
│   │   ├── mega_generator.py      # Mega Plan 生成
│   │   ├── quality_gate.py        # 质量门控
│   │   └── retry_manager.py       # 重试管理
│   │
│   ├── backends/                  # 后端抽象
│   │   ├── base.py                # 后端基类
│   │   ├── factory.py             # 后端工厂
│   │   ├── claude_code.py         # Claude Code 后端
│   │   ├── builtin.py             # 内置后端
│   │   └── external.py            # 外部 CLI 后端
│   │
│   ├── llm/                       # LLM 抽象层
│   │   ├── base.py                # Provider 基类
│   │   ├── factory.py             # Provider 工厂
│   │   └── providers/
│   │       ├── claude.py
│   │       ├── openai.py
│   │       └── ollama.py
│   │
│   ├── tools/                     # 工具层
│   │   ├── registry.py            # 工具注册
│   │   ├── file_tools.py          # 文件操作
│   │   └── shell_tools.py         # Shell 执行
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
│       └── main.py
│
├── server/                        # FastAPI 服务
│   └── plan_cascade_server/
│       ├── main.py
│       ├── routes/
│       └── websocket.py
│
├── desktop/                       # Tauri 桌面应用
│   ├── src/
│   │   ├── components/
│   │   ├── store/
│   │   └── App.tsx
│   └── src-tauri/
│
├── plugin/                        # Claude Code Plugin
│   ├── .claude-plugin/
│   ├── commands/
│   └── skills/
│
├── tests/
├── docs/
├── pyproject.toml
└── README.md
```

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
| 简单模式 | 一键完成，AI 自动处理一切 |
| 专家模式 | 可编辑 PRD、选择策略、指定 Agent |
| Claude Code 后端 | Plan Cascade 作为 Claude Code 的 GUI |
| Builtin 后端 | 使用 LLM API 独立运行 |
| Strategy | 执行策略 (Direct/Hybrid/Mega) |
