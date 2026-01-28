# Plan Cascade MCP 集成增强方案

> 版本: 1.0.0
> 日期: 2026-01-28
> 基于: Plan Cascade v2.8.0

## 目录

- [项目概述](#项目概述)
- [当前架构分析](#当前架构分析)
- [MCP 集成机会](#mcp-集成机会)
  - [需求获取层](#1-需求获取层-mcp-服务器)
  - [代码智能分析层](#2-代码智能分析层-mcp-服务器)
  - [执行监控层](#3-执行监控层-mcp-服务器)
  - [知识管理层](#4-知识管理层-mcp-服务器)
  - [协作通知层](#5-协作通知层-mcp-服务器)
  - [PRD智能增强层](#6-prd-智能增强层-mcp-服务器)
- [架构集成设计](#架构集成设计)
- [实现路线图](#实现路线图)
- [附录](#附录)

---

## 项目概述

Plan Cascade 是一个三层级联并行开发框架，用于编排复杂的多任务项目开发：

```
┌─────────────────────────────────────────────────────────────────┐
│  第一层: MEGA PLAN (项目编排层)                                   │
│  ─────────────────────────────────────────────────────────────  │
│  • 将大型项目分解为多个功能模块 (Features)                         │
│  • 管理功能间依赖关系，计算执行批次                                 │
│  • 文件: mega-plan.json, mega-findings.md, .mega-status.json    │
│  • 命令: /mega:plan, /mega:approve, /mega:status, /mega:complete│
├─────────────────────────────────────────────────────────────────┤
│  第二层: HYBRID RALPH (功能开发层)                                │
│  ─────────────────────────────────────────────────────────────  │
│  • 每个功能在独立的 Git Worktree 中开发                           │
│  • 自动生成 PRD，将功能拆分为用户故事 (Stories)                    │
│  • 文件: prd.json, findings.md, progress.txt                    │
│  • 命令: /hybrid-worktree, /approve, /status, /hybrid-complete  │
├─────────────────────────────────────────────────────────────────┤
│  第三层: STORIES (任务执行层)                                     │
│  ─────────────────────────────────────────────────────────────  │
│  • 独立故事以 Task Agent 形式并行执行                              │
│  • 依赖驱动的批次进展 (Batch Progression)                         │
│  • 上下文过滤: 每个 Agent 只接收相关上下文                         │
│  • 执行模式: Auto (自动推进) / Manual (手动确认)                   │
└─────────────────────────────────────────────────────────────────┘
```

### 核心特性

| 特性 | 描述 |
|------|------|
| **依赖解析** | 自动分析功能/故事间依赖，计算最优执行批次 |
| **并行执行** | 无依赖的任务同时执行，最大化开发效率 |
| **上下文隔离** | 每个 Agent 只接收与其任务相关的上下文 |
| **状态持久化** | 基于文件的状态管理，支持会话恢复 |
| **Git Worktree** | 每个功能在独立分支/目录中开发，互不干扰 |
| **Hooks 系统** | PreToolUse/PostToolUse/Stop 钩子注入上下文 |

---

## 当前架构分析

### 核心模块结构

```
skills/
├── planning-with-files/          # 基础规划层 (v2.7.4)
│   ├── core/                     # Python 核心模块
│   ├── scripts/                  # 会话管理、worktree 脚本
│   └── templates/                # 任务计划模板
│
├── hybrid-ralph/                 # 功能开发层 (v2.8.0)
│   ├── core/
│   │   ├── prd_generator.py      # PRD 自动生成
│   │   ├── state_manager.py      # 状态管理 + 文件锁
│   │   ├── context_filter.py     # 上下文过滤
│   │   └── orchestrator.py       # 批次执行编排
│   ├── commands/                 # 8 个命令定义
│   └── scripts/                  # 辅助脚本
│
└── mega-plan/                    # 项目编排层 (v1.0.0)
    ├── core/
    │   ├── mega_generator.py     # Mega Plan 生成
    │   ├── mega_state.py         # 项目状态管理
    │   ├── feature_orchestrator.py # 功能编排
    │   └── merge_coordinator.py  # 合并协调
    ├── commands/                 # 5 个命令定义
    └── scripts/                  # 状态同步脚本
```

### 当前 MCP 集成状态

**结论: 项目当前没有任何 MCP 集成**

- ✗ 无 `.mcp.json` 配置文件
- ✗ 无 MCP 服务器实现
- ✗ 无外部服务 API 调用
- ✗ 所有功能均为本地文件操作

### 架构优势 (利于 MCP 集成)

1. **高度模块化**: 清晰的类和函数边界，易于注入 MCP 调用
2. **JSON 状态文件**: 结构化数据，易于序列化/反序列化
3. **明确的扩展点**: Generator、StateManager、Orchestrator 职责分明
4. **工具无关性**: Task Agent 通过命令执行，不依赖特定工具

---

## MCP 集成机会

### 1. 需求获取层 MCP 服务器

**目标**: 从外部项目管理工具自动获取需求，减少手动录入

#### 1.1 Jira MCP Server

| 属性 | 描述 |
|------|------|
| **功能** | 读取 Jira Epic/Story/Task，同步状态 |
| **集成点** | `mega_generator.py`, `prd_generator.py` |
| **价值** | 需求双向同步，避免重复录入 |

**使用场景**:
```bash
# 从 Jira Epic 生成 Mega Plan
/mega:plan --from-jira PROJ-123

# 从 Jira Sprint 生成 PRD
/hybrid-worktree auth main --from-jira PROJ-456
```

**MCP 工具定义**:
```json
{
  "name": "jira",
  "tools": [
    {
      "name": "get_epic",
      "description": "获取 Epic 及其子任务",
      "inputSchema": {
        "type": "object",
        "properties": {
          "epic_key": { "type": "string" }
        }
      }
    },
    {
      "name": "sync_status",
      "description": "同步故事状态到 Jira",
      "inputSchema": {
        "type": "object",
        "properties": {
          "issue_key": { "type": "string" },
          "status": { "type": "string" }
        }
      }
    }
  ]
}
```

#### 1.2 Linear MCP Server

| 属性 | 描述 |
|------|------|
| **功能** | 读取 Linear Projects/Issues/Cycles |
| **集成点** | `mega_generator.py`, `prd_generator.py` |
| **价值** | 适用于使用 Linear 的团队 |

#### 1.3 GitHub Issues MCP Server

| 属性 | 描述 |
|------|------|
| **功能** | 读取 Issues/Milestones，自动转化为 Stories |
| **集成点** | `prd_generator.py` |
| **价值** | 开源项目友好，Issue 状态双向同步 |

**使用场景**:
```bash
# 从 GitHub Milestone 生成 PRD
/hybrid-worktree v2.0 main --from-github-milestone 5

# Issue 完成后自动关闭
story 完成 → MCP 调用 → GitHub Issue 状态更新为 Closed
```

#### 1.4 Notion/Confluence MCP Server

| 属性 | 描述 |
|------|------|
| **功能** | 读取设计文档、技术规范 |
| **集成点** | `context_filter.py` |
| **价值** | 为 PRD 生成提供背景上下文 |

#### 1.5 Figma MCP Server

| 属性 | 描述 |
|------|------|
| **功能** | 解析设计稿，提取 UI 组件需求 |
| **集成点** | `prd_generator.py` |
| **价值** | 前端功能自动识别 UI 实现任务 |

---

### 2. 代码智能分析层 MCP 服务器

**目标**: 利用代码分析增强上下文理解和依赖检测

#### 2.1 Language Server MCP

| 属性 | 描述 |
|------|------|
| **功能** | AST 分析、符号查找、引用追踪 |
| **集成点** | `context_filter.py`, `prd_generator.py` |
| **价值** | 自动发现 story 间代码依赖 |

**使用场景**:
```python
# context_filter.py 增强
def get_story_context(story_id: str) -> dict:
    # 现有逻辑: 从 findings.md 过滤标签
    findings_context = filter_findings_by_tag(story_id)

    # 新增: MCP 调用获取代码依赖
    code_deps = mcp_call("language-server", {
        "action": "analyze_dependencies",
        "files": story.related_files,
        "depth": 2  # 追踪两层依赖
    })

    return {
        "findings": findings_context,
        "code_dependencies": code_deps,
        "affected_files": code_deps.affected_files
    }
```

**自动依赖检测**:
```
story-001: 修改 UserService.ts
story-002: 修改 AuthController.ts (imports UserService)

→ MCP 分析发现 AuthController 依赖 UserService
→ 自动设置 story-002.dependencies = ["story-001"]
```

#### 2.2 Git History MCP

| 属性 | 描述 |
|------|------|
| **功能** | 获取文件变更历史、blame、相关 commit |
| **集成点** | `context_filter.py` |
| **价值** | 提供代码演变上下文 |

**使用场景**:
```
Agent 修改 auth.ts 时:
→ MCP 获取最近 10 次 commit
→ 提供 "最近修改者"、"修改原因" 上下文
→ 避免重复修改或冲突
```

#### 2.3 Semantic Code Search MCP

| 属性 | 描述 |
|------|------|
| **功能** | 基于向量相似度搜索代码 |
| **集成点** | `context_filter.py`, Agent 执行 |
| **价值** | 找到语义相关的代码片段 |

**使用场景**:
```
story: "实现用户登录验证"
→ MCP 搜索: "用户验证", "登录逻辑", "认证"
→ 返回相关代码片段作为参考
→ Agent 可以复用已有模式
```

#### 2.4 Test Coverage MCP

| 属性 | 描述 |
|------|------|
| **功能** | 获取测试覆盖率数据 |
| **集成点** | `prd_generator.py` |
| **价值** | 识别低覆盖区域，优先安排测试 |

---

### 3. 执行监控层 MCP 服务器

**目标**: 实时监控执行状态，自动化 CI/CD 集成

#### 3.1 GitHub Actions MCP

| 属性 | 描述 |
|------|------|
| **功能** | 触发/监控 GitHub Actions workflow |
| **集成点** | `orchestrator.py`, `feature_orchestrator.py` |
| **价值** | CI 反馈集成，自动重试失败任务 |

**使用场景**:
```
Story 完成 → 提交代码 → 触发 CI
→ MCP 监控 GitHub Actions
→ 测试通过 → 自动标记 [COMPLETE]
→ 测试失败 → 保持 [IN_PROGRESS]，记录失败原因
```

**MCP 工具定义**:
```json
{
  "name": "github-actions",
  "tools": [
    {
      "name": "trigger_workflow",
      "description": "触发 GitHub Actions workflow",
      "inputSchema": {
        "type": "object",
        "properties": {
          "workflow_id": { "type": "string" },
          "ref": { "type": "string" },
          "inputs": { "type": "object" }
        }
      }
    },
    {
      "name": "get_run_status",
      "description": "获取 workflow run 状态",
      "inputSchema": {
        "type": "object",
        "properties": {
          "run_id": { "type": "integer" }
        }
      }
    },
    {
      "name": "get_run_logs",
      "description": "获取 workflow run 日志",
      "inputSchema": {
        "type": "object",
        "properties": {
          "run_id": { "type": "integer" }
        }
      }
    }
  ]
}
```

#### 3.2 Docker MCP

| 属性 | 描述 |
|------|------|
| **功能** | 管理容器环境 |
| **集成点** | `feature_orchestrator.py` |
| **价值** | 为每个 feature 提供隔离环境 |

**使用场景**:
```
/mega:approve
→ 为每个 feature worktree 启动独立容器
→ 容器内运行测试和构建
→ 避免环境污染
```

#### 3.3 Test Runner MCP

| 属性 | 描述 |
|------|------|
| **功能** | 执行单元测试/集成测试 |
| **集成点** | `orchestrator.py` |
| **价值** | 自动验证 story 完成质量 |

**使用场景**:
```python
# orchestrator.py 增强
def verify_story_completion(story_id: str) -> bool:
    # 现有: 检查 progress.txt 中的 [COMPLETE] 标记
    if not check_progress_marker(story_id):
        return False

    # 新增: MCP 运行相关测试
    test_result = mcp_call("test-runner", {
        "action": "run_tests",
        "pattern": f"**/test_{story_id}*.py",
        "coverage_threshold": 80
    })

    if not test_result.passed:
        # 记录失败原因到 findings.md
        append_to_findings(story_id, test_result.failure_reason)
        return False

    return True
```

#### 3.4 Deployment MCP

| 属性 | 描述 |
|------|------|
| **功能** | 部署到 staging 环境 |
| **集成点** | `merge_coordinator.py` |
| **价值** | 合并后自动部署验证 |

---

### 4. 知识管理层 MCP 服务器

**目标**: 构建可复用的知识库，增强上下文管理

#### 4.1 Vector Database MCP

| 属性 | 描述 |
|------|------|
| **功能** | 存储/检索向量化的知识 |
| **集成点** | `state_manager.py`, `context_filter.py` |
| **价值** | 跨项目知识复用，语义搜索 findings |

**使用场景**:
```python
# state_manager.py 增强
def append_finding(finding: str, tags: List[str]):
    # 现有: 写入 findings.md
    write_to_findings_file(finding, tags)

    # 新增: 向量化并存储
    mcp_call("vector-db", {
        "action": "upsert",
        "collection": "project_findings",
        "document": finding,
        "metadata": {
            "project": self.project_name,
            "tags": tags,
            "timestamp": datetime.now().isoformat()
        }
    })

# context_filter.py 增强
def get_relevant_findings(query: str, limit: int = 5) -> List[str]:
    # 新增: 语义搜索相关 findings
    results = mcp_call("vector-db", {
        "action": "search",
        "collection": "project_findings",
        "query": query,
        "limit": limit
    })
    return [r.document for r in results]
```

**跨项目知识复用**:
```
当前任务: "实现 OAuth2 登录"
→ MCP 搜索历史项目 findings
→ 找到: "项目A: OAuth2 需要处理 token 刷新边界情况"
→ Agent 获得经验教训，避免重复踩坑
```

#### 4.2 Documentation MCP

| 属性 | 描述 |
|------|------|
| **功能** | 获取框架/库官方文档 |
| **集成点** | `context_filter.py` |
| **价值** | 自动引用相关文档作为实现参考 |

**使用场景**:
```
story: "使用 NextAuth 实现认证"
→ MCP 获取 NextAuth 官方文档
→ 提取相关配置示例
→ Agent 获得正确的实现模式
```

#### 4.3 Code Comment Generator MCP

| 属性 | 描述 |
|------|------|
| **功能** | 生成 JSDoc/docstring 注释 |
| **集成点** | Agent 执行后 |
| **价值** | 自动化文档生成 |

#### 4.4 Knowledge Graph MCP

| 属性 | 描述 |
|------|------|
| **功能** | 构建代码/功能依赖图谱 |
| **集成点** | `show-dependencies` 命令 |
| **价值** | 可视化复杂依赖关系 |

---

### 5. 协作通知层 MCP 服务器

**目标**: 实时通知团队成员，增强协作效率

#### 5.1 Slack MCP

| 属性 | 描述 |
|------|------|
| **功能** | 发送通知到 Slack channel |
| **集成点** | `orchestrator.py`, `feature_orchestrator.py` |
| **价值** | 实时进度通知 |

**通知时机**:
```
┌─────────────────────────────────────────────────────┐
│ Batch 开始   │ 🚀 Batch 2 开始执行 (3 stories)      │
│ Story 完成   │ ✅ story-003: 用户认证 已完成         │
│ Story 失败   │ ⚠️ story-004 执行失败: [错误摘要]     │
│ Batch 完成   │ 🎉 Batch 2 (3/3 stories) 全部完成    │
│ Feature 完成 │ 🏆 Feature: 用户认证 开发完成         │
│ 合并完成     │ 🔀 已合并到 main 分支                 │
└─────────────────────────────────────────────────────┘
```

**MCP 工具定义**:
```json
{
  "name": "slack",
  "tools": [
    {
      "name": "send_message",
      "description": "发送消息到 Slack channel",
      "inputSchema": {
        "type": "object",
        "properties": {
          "channel": { "type": "string" },
          "text": { "type": "string" },
          "blocks": { "type": "array" }
        }
      }
    },
    {
      "name": "update_message",
      "description": "更新已发送的消息",
      "inputSchema": {
        "type": "object",
        "properties": {
          "channel": { "type": "string" },
          "ts": { "type": "string" },
          "text": { "type": "string" }
        }
      }
    }
  ]
}
```

#### 5.2 GitHub PR MCP

| 属性 | 描述 |
|------|------|
| **功能** | 自动创建/更新 Pull Request |
| **集成点** | `merge_coordinator.py`, `/complete` 命令 |
| **价值** | 标准化 PR 流程 |

**使用场景**:
```
/hybrid-complete
→ 验证所有 stories 完成
→ MCP 创建 PR:
  - Title: "Feature: 用户认证系统"
  - Body: 自动生成的变更摘要
  - Labels: auto-generated, feature
  - Reviewers: 从 CODEOWNERS 读取
```

#### 5.3 Teams MCP

| 属性 | 描述 |
|------|------|
| **功能** | 发送通知到 Microsoft Teams |
| **集成点** | 同 Slack MCP |
| **价值** | 适用于使用 Teams 的团队 |

#### 5.4 Email MCP

| 属性 | 描述 |
|------|------|
| **功能** | 发送邮件通知/日报 |
| **集成点** | 定时任务 |
| **价值** | 每日开发进度报告 |

---

### 6. PRD 智能增强层 MCP 服务器

**目标**: 利用 AI 能力提升 PRD 质量和估算准确性

#### 6.1 AI PRD Review MCP

| 属性 | 描述 |
|------|------|
| **功能** | AI 审查 PRD 完整性、可行性 |
| **集成点** | `prd_generator.py`, `/edit` 命令 |
| **价值** | 自动发现 PRD 问题 |

**审查维度**:
```
┌─────────────────────────────────────────────────────┐
│ 完整性检查   │ 是否缺少必要的 acceptance criteria   │
│ 依赖合理性   │ 依赖关系是否存在循环或遗漏            │
│ 粒度评估     │ story 是否过大需要拆分               │
│ 可测试性     │ acceptance criteria 是否可验证        │
│ 技术风险     │ 是否存在未识别的技术风险              │
└─────────────────────────────────────────────────────┘
```

**使用场景**:
```
PRD 生成后 → MCP 调用 AI Review
→ 发现: "story-003 缺少错误处理相关的 acceptance criteria"
→ 建议: "添加: '当认证失败时，应返回 401 状态码'"
→ 用户确认后自动更新 PRD
```

#### 6.2 Estimation MCP

| 属性 | 描述 |
|------|------|
| **功能** | 基于历史数据估算工作量 |
| **集成点** | `prd_generator.py` |
| **价值** | 提高 `context_estimate` 准确性 |

**估算逻辑**:
```python
def estimate_story_complexity(story: dict) -> str:
    # 获取历史相似 story 数据
    similar_stories = mcp_call("estimation", {
        "action": "find_similar",
        "description": story["description"],
        "limit": 5
    })

    # 基于历史数据估算
    avg_complexity = calculate_average(similar_stories)

    return avg_complexity  # "low" | "medium" | "high"
```

#### 6.3 Dependency Detector MCP

| 属性 | 描述 |
|------|------|
| **功能** | 分析代码库自动推断 story 依赖 |
| **集成点** | `prd_generator.py` |
| **价值** | 减少手动依赖标注 |

**自动检测逻辑**:
```
story-001: "创建 User 数据模型"
story-002: "实现用户注册 API"
story-003: "实现用户登录 API"

→ MCP 分析: 注册/登录都需要 User 模型
→ 自动设置:
   story-002.dependencies = ["story-001"]
   story-003.dependencies = ["story-001"]
```

#### 6.4 Acceptance Criteria Generator MCP

| 属性 | 描述 |
|------|------|
| **功能** | 根据描述自动生成验收标准 |
| **集成点** | `prd_generator.py` |
| **价值** | 标准化、完整的 AC |

---

## 架构集成设计

### MCP 配置文件结构

```
.mcp.json (项目根目录)
├── mcpServers
│   ├── jira          # 需求获取
│   ├── github-issues
│   ├── language-server  # 代码分析
│   ├── git-history
│   ├── github-actions   # 执行监控
│   ├── test-runner
│   ├── vector-db        # 知识管理
│   ├── slack            # 协作通知
│   └── ai-review        # PRD 增强
```

### 核心模块集成点

```python
# skills/hybrid-ralph/core/mcp_integration.py (新增)

from typing import Any, Dict, Optional
import json

class MCPClient:
    """MCP 服务调用客户端"""

    def __init__(self, config_path: str = ".mcp.json"):
        self.config = self._load_config(config_path)
        self.enabled_servers = self._get_enabled_servers()

    def call(self, server: str, tool: str, params: Dict[str, Any]) -> Any:
        """调用 MCP 服务器工具"""
        if server not in self.enabled_servers:
            return None  # 优雅降级

        # 实际 MCP 调用逻辑
        ...

    def is_enabled(self, server: str) -> bool:
        """检查服务器是否启用"""
        return server in self.enabled_servers


# 集成到现有模块
class EnhancedPRDGenerator(PRDGenerator):
    """增强的 PRD 生成器，支持 MCP"""

    def __init__(self, project_root: str):
        super().__init__(project_root)
        self.mcp = MCPClient()

    def generate_from_jira(self, epic_key: str) -> dict:
        """从 Jira Epic 生成 PRD"""
        if not self.mcp.is_enabled("jira"):
            raise ValueError("Jira MCP 未启用")

        epic_data = self.mcp.call("jira", "get_epic", {
            "epic_key": epic_key
        })

        return self._convert_jira_to_prd(epic_data)

    def auto_detect_dependencies(self, stories: List[dict]) -> List[dict]:
        """自动检测 story 依赖"""
        if not self.mcp.is_enabled("language-server"):
            return stories  # 降级: 返回原始 stories

        for story in stories:
            deps = self.mcp.call("language-server", "analyze_dependencies", {
                "files": story.get("related_files", [])
            })
            story["dependencies"] = self._merge_dependencies(
                story.get("dependencies", []),
                deps
            )

        return stories
```

### 渐进式集成策略

```
阶段 1: 可选集成
─────────────────
• MCP 服务作为可选增强
• 不影响核心功能
• 优雅降级机制

阶段 2: 深度集成
─────────────────
• 增强的上下文过滤
• 自动依赖检测
• CI/CD 反馈循环

阶段 3: 智能自动化
─────────────────
• AI PRD 审查
• 自动估算校准
• 知识图谱构建
```

---

## 实现路线图

### Phase 1: 高优先级 (MVP)

| MCP 服务器 | 价值 | 工作量 | 优先级 |
|-----------|------|--------|--------|
| **GitHub Issues** | 需求同步 | 中 | ⭐⭐⭐ |
| **GitHub Actions** | CI 反馈 | 中 | ⭐⭐⭐ |
| **Vector DB** | 知识复用 | 高 | ⭐⭐⭐ |
| **Slack** | 进度通知 | 低 | ⭐⭐⭐ |

**预期成果**:
- 从 GitHub Issues 自动生成 stories
- CI 测试结果自动反馈到 progress.txt
- findings.md 语义搜索
- 批次完成实时通知

### Phase 2: 中优先级 (增强体验)

| MCP 服务器 | 价值 | 工作量 | 优先级 |
|-----------|------|--------|--------|
| **Language Server** | 依赖检测 | 高 | ⭐⭐ |
| **Test Runner** | 质量验证 | 中 | ⭐⭐ |
| **GitHub PR** | PR 自动化 | 中 | ⭐⭐ |
| **Documentation** | 上下文增强 | 中 | ⭐⭐ |

**预期成果**:
- 自动检测 story 间代码依赖
- story 完成后自动运行测试
- /complete 时自动创建 PR
- 自动引用相关框架文档

### Phase 3: 低优先级 (锦上添花)

| MCP 服务器 | 价值 | 工作量 | 优先级 |
|-----------|------|--------|--------|
| **Jira/Linear** | 企业集成 | 高 | ⭐ |
| **AI PRD Review** | 质量提升 | 高 | ⭐ |
| **Figma** | 设计协作 | 高 | ⭐ |
| **Knowledge Graph** | 可视化 | 高 | ⭐ |

---

## 附录

### A. MCP 服务器模板

```typescript
// mcp-servers/template/index.ts

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";

const server = new Server(
  { name: "template-server", version: "1.0.0" },
  { capabilities: { tools: {} } }
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: "example_tool",
      description: "示例工具",
      inputSchema: {
        type: "object",
        properties: {
          param: { type: "string", description: "参数描述" }
        },
        required: ["param"]
      }
    }
  ]
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  if (request.params.name === "example_tool") {
    const { param } = request.params.arguments;
    return {
      content: [{ type: "text", text: `处理结果: ${param}` }]
    };
  }
  throw new Error("Unknown tool");
});

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch(console.error);
```

### B. .mcp.json 完整配置示例

```json
{
  "mcpServers": {
    "github-issues": {
      "command": "node",
      "args": ["mcp-servers/github-issues/dist/index.js"],
      "env": {
        "GITHUB_TOKEN": "${GITHUB_TOKEN}"
      }
    },
    "github-actions": {
      "command": "node",
      "args": ["mcp-servers/github-actions/dist/index.js"],
      "env": {
        "GITHUB_TOKEN": "${GITHUB_TOKEN}"
      }
    },
    "vector-db": {
      "command": "node",
      "args": ["mcp-servers/vector-db/dist/index.js"],
      "env": {
        "VECTOR_DB_URL": "${VECTOR_DB_URL}"
      }
    },
    "slack": {
      "command": "node",
      "args": ["mcp-servers/slack/dist/index.js"],
      "env": {
        "SLACK_BOT_TOKEN": "${SLACK_BOT_TOKEN}",
        "SLACK_CHANNEL": "${SLACK_CHANNEL}"
      }
    },
    "language-server": {
      "command": "node",
      "args": ["mcp-servers/language-server/dist/index.js"]
    }
  }
}
```

### C. 降级策略

```python
# 所有 MCP 调用应实现优雅降级

def get_context_with_fallback(story_id: str) -> dict:
    context = {}

    # 尝试 MCP 增强
    try:
        if mcp.is_enabled("language-server"):
            context["code_deps"] = mcp.call("language-server", "analyze", {...})
    except MCPError as e:
        logger.warning(f"Language Server MCP 失败: {e}")
        # 降级: 跳过代码依赖分析

    # 核心功能始终可用
    context["findings"] = filter_findings_by_tag(story_id)
    context["story"] = get_story(story_id)

    return context
```

---

## 总结

Plan Cascade 的模块化架构为 MCP 集成提供了良好的基础。通过渐进式集成策略，可以在不影响核心功能的前提下，逐步增强项目的自动化能力和智能水平。

**核心价值**:
1. **需求同步**: 减少手动录入，保持需求一致性
2. **智能分析**: 自动依赖检测，减少人工标注
3. **执行监控**: CI/CD 深度集成，自动化质量验证
4. **知识复用**: 跨项目经验积累，避免重复踩坑
5. **团队协作**: 实时通知，提升协作效率

**建议下一步**:
1. 实现 GitHub Issues MCP 作为 MVP
2. 添加 Slack 通知集成
3. 构建 Vector DB 知识库
4. 逐步扩展其他 MCP 服务器
