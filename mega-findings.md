# Mega Plan Findings

Project: 将 Plan Cascade 从 Claude Code Plugin 重构为独立的多形态客户端
Created: 2026-01-28T10:30:00Z

This file contains shared findings across all features.
Feature-specific findings should be in their respective worktrees.

---

## Project-Wide Decisions

### 1. 保持向后兼容性

现有 Claude Code Plugin 功能必须完整保留：
- `skills/` 目录结构保持不变
- `commands/` 中的命令继续可用
- `mcp_server/` 保持独立运行能力

新核心包 `src/plan_cascade/` 将：
- 提取并重构现有 `skills/hybrid-ralph/core/` 和 `skills/mega-plan/core/` 的代码
- 通过重新导出或符号链接保持原有导入路径可用

### 2. 渐进式迁移策略

采用渐进式迁移而非一次性重写：
- Phase 1: 创建新包结构，复制并重构核心代码
- Phase 2: 更新 Plugin 使用新包（可选）
- Phase 3: 逐步废弃旧代码路径

### 3. 测试策略

- 为新代码编写单元测试
- 确保现有 Plugin 功能通过集成测试验证

---

## Shared Patterns

### 1. 异步优先

所有 I/O 操作使用 async/await：
```python
async def execute(self, story: dict, context: str) -> ExecutionResult:
    ...
```

### 2. 数据类与类型提示

使用 dataclasses 和完整类型提示：
```python
from dataclasses import dataclass
from typing import Optional, List

@dataclass
class ExecutionResult:
    success: bool
    output: str
    iterations: int = 0
    error: Optional[str] = None
```

### 3. 回调模式

使用回调函数实现 UI 解耦：
```python
class Backend:
    on_tool_call: Callable[[dict], Awaitable[None]] = None
    on_progress: Callable[[dict], Awaitable[None]] = None
```

### 4. 工厂模式

使用工厂方法创建实例：
```python
class BackendFactory:
    @staticmethod
    def create(config: dict) -> AgentBackend:
        ...
```

---

## Integration Notes

### 1. 现有代码资产

**可直接复用的模块** (来自 skills/hybrid-ralph/core/):
- `state_manager.py` - 线程安全文件操作，已实现跨平台锁
- `context_filter.py` - 上下文过滤，设计良好
- `quality_gate.py` - 质量门控，已实现多类型检查
- `retry_manager.py` - 重试管理，已实现指数退避
- `phase_config.py` - 阶段配置，已定义执行阶段枚举
- `cross_platform_detector.py` - 跨平台 Agent 检测

**需要重构的模块**:
- `orchestrator.py` - 需要适配新的 Backend 抽象
- `prd_generator.py` - 需要适配 LLM Provider 抽象
- `agent_executor.py` - 部分功能移至 Backend 实现
- `iteration_loop.py` - 需要整合到工作流中

### 2. 外部依赖

**新增依赖**:
- `typer` - CLI 框架
- `rich` - 终端美化
- `keyring` - 安全密钥存储
- `pyyaml` - 配置文件
- `aiohttp` / `httpx` - 异步 HTTP (用于 LLM API)

**桌面应用依赖** (Phase 2+):
- `fastapi` + `uvicorn` - API 服务
- `websockets` - 实时通信
- Tauri (Rust)
- React + TypeScript

### 3. 目录结构映射

```
现有结构                           新结构
---------                         -------
skills/hybrid-ralph/core/    →    src/plan_cascade/core/
skills/mega-plan/core/       →    src/plan_cascade/core/ (mega_*)
mcp_server/tools/            →    保持不变，导入新包
(新建)                        →    src/plan_cascade/backends/
(新建)                        →    src/plan_cascade/llm/
(新建)                        →    src/plan_cascade/settings/
(新建)                        →    src/plan_cascade/cli/
(新建)                        →    server/plan_cascade_server/
(新建)                        →    desktop/
```

### 4. 入口点配置

pyproject.toml 中配置：
```toml
[project.scripts]
plan-cascade = "plan_cascade.cli.main:main"

[project.gui-scripts]
plan-cascade-gui = "plan_cascade_server.main:start_with_gui"
```

---

## Risk Assessment

### 高风险项

1. **状态管理兼容性**: 确保新旧代码使用相同的状态文件格式
2. **Claude Code 进程通信**: 需要正确解析 JSON 流输出

### 中风险项

1. **LLM API 集成**: 不同 Provider 的响应格式差异
2. **跨平台兼容**: Windows/macOS/Linux 路径和进程管理差异

### 低风险项

1. **CLI 实现**: Typer 框架成熟稳定
2. **设置存储**: YAML + keyring 是标准方案
