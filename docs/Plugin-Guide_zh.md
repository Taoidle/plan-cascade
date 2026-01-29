[English](Plugin-Guide.md)

# Plan Cascade - Claude Code Plugin Guide

**版本**: 4.1.0
**最后更新**: 2026-01-29

本文档详细介绍 Plan Cascade 作为 Claude Code 插件的使用方法。

---

## 安装

```bash
# 从 GitHub 安装
claude plugins install Taoidle/plan-cascade

# 或克隆后本地安装
git clone https://github.com/Taoidle/plan-cascade.git
claude plugins install ./plan-cascade
```

---

## 命令概览

Plan Cascade 提供四个主要入口命令，适用于不同规模的开发场景：

| 入口命令 | 适用场景 | 特点 |
|----------|----------|------|
| `/plan-cascade:auto` | 任意任务（AI 自动选择策略） | 自动策略选择 + 直接执行 |
| `/plan-cascade:mega-plan` | 大型项目（多个相关功能） | Feature 级并行 + Story 级并行 |
| `/plan-cascade:hybrid-worktree` | 单个复杂功能 | Worktree 隔离 + Story 并行 |
| `/plan-cascade:hybrid-auto` | 简单功能 | 快速 PRD 生成 + Story 并行 |

---

## `/plan-cascade:auto` - AI 自动策略

最简单的入口。AI 分析任务描述并自动选择最佳策略。

### 工作原理

1. 用户提供任务描述
2. AI 分析关键词和模式
3. AI 选择最优策略（direct、hybrid-auto、hybrid-worktree 或 mega-plan）
4. 无需确认，直接执行

### 策略选择

| 策略 | 触发关键词 | 示例 |
|------|------------|------|
| **direct** | fix、typo、update、simple、single | "修复登录按钮样式" |
| **hybrid-auto** | implement、create、feature、api | "实现用户认证" |
| **hybrid-worktree** | experimental、refactor、isolated | "实验性重构支付模块" |
| **mega-plan** | platform、system、3+ 个模块 | "构建电商平台：用户、商品、订单" |

### 使用示例

```bash
# AI 自动判断策略
/plan-cascade:auto "修复 README 中的拼写错误"
# → 使用 direct 策略

/plan-cascade:auto "实现 OAuth 用户登录"
# → 使用 hybrid-auto 策略

/plan-cascade:auto "实验性重构 API 层"
# → 使用 hybrid-worktree 策略

/plan-cascade:auto "构建博客平台：用户、文章、评论、RSS"
# → 使用 mega-plan 策略
```

---

## `/plan-cascade:mega-plan` 流程

适用于包含多个相关功能模块的大型项目开发。

### 适用场景

| 类型 | 场景 | 示例 |
|------|------|------|
| ✅ 适用 | 多功能模块的新项目开发 | 构建 SaaS 平台（用户 + 订阅 + 计费 + 后台） |
| ✅ 适用 | 涉及多子系统的大规模重构 | 单体应用重构为微服务架构 |
| ✅ 适用 | 功能群开发 | 电商平台（用户、商品、购物车、订单） |
| ❌ 不适用 | 单个功能开发 | 仅实现用户认证（用 Hybrid Ralph） |
| ❌ 不适用 | Bug 修复 | 修复登录页表单验证问题 |

### 批次间顺序执行

Mega-plan 使用**批次间顺序执行**模式，确保每个批次从更新后的目标分支创建 worktree：

```
mega-approve (第1次) → 启动 Batch 1
    ↓ Batch 1 完成
mega-approve (第2次) → 合并 Batch 1 → 从更新后的分支创建 Batch 2
    ↓ Batch 2 完成
mega-approve (第3次) → 合并 Batch 2 → ...
    ↓ 所有批次完成
mega-complete → 清理计划文件
```

关键点：
- `mega-approve` 需要多次调用（每个批次一次）
- 每个批次从**更新后的目标分支**创建 worktree
- 计划文件不会被提交（已加入 .gitignore）

### 命令参考

```bash
/plan-cascade:mega-plan <描述>           # 生成项目计划
/plan-cascade:mega-edit                  # 编辑计划
/plan-cascade:mega-approve [--auto-prd]  # 批准并执行
/plan-cascade:mega-status                # 查看进度
/plan-cascade:mega-complete [branch]     # 合并并清理
```

### 使用示例

```bash
# 场景：构建电商平台
/plan-cascade:mega-plan "构建电商平台：用户认证、商品管理、购物车、订单处理"

# 查看生成的计划
/plan-cascade:mega-status

# 编辑计划（可选）
/plan-cascade:mega-edit

# 批准第一批次
/plan-cascade:mega-approve --auto-prd

# 查看执行进度
/plan-cascade:mega-status

# 批次完成后，批准下一批次
/plan-cascade:mega-approve

# 全部完成后清理
/plan-cascade:mega-complete main
```

---

## `/plan-cascade:hybrid-worktree` 流程

适用于需要分支隔离的单个复杂功能开发。

### 适用场景

| 类型 | 场景 | 示例 |
|------|------|------|
| ✅ 适用 | 包含多子任务的完整功能 | 用户认证（注册 + 登录 + 密码重置） |
| ✅ 适用 | 需要分支隔离的实验功能 | 新支付渠道集成测试 |
| ✅ 适用 | 中等规模重构（5-20 文件） | API 层统一错误处理改造 |
| ❌ 不适用 | 简单单文件修改 | 修改一个组件的样式 |
| ❌ 不适用 | 快速原型验证 | 验证某个库是否可用 |

### 命令参考

```bash
/plan-cascade:hybrid-worktree <name> <branch> <desc>  # 创建开发环境
/plan-cascade:hybrid-auto <desc> [--agent <name>]     # 生成 PRD
/plan-cascade:approve [--auto-run]                    # 执行 PRD
/plan-cascade:hybrid-status                           # 查看状态
/plan-cascade:hybrid-complete [branch]                # 完成并合并
```

### 使用示例

```bash
# 创建隔离开发环境
/plan-cascade:hybrid-worktree feature-auth main "实现用户认证：登录、注册、密码重置"

# 生成 PRD
/plan-cascade:hybrid-auto "实现用户认证功能"

# 查看并编辑 PRD
/plan-cascade:edit

# 批准并自动执行
/plan-cascade:approve --auto-run

# 查看执行进度
/plan-cascade:hybrid-status

# 完成后合并到 main
/plan-cascade:hybrid-complete main
```

---

## `/plan-cascade:hybrid-auto` 流程

适用于简单功能的快速开发，无需 Worktree 隔离。

### 命令参考

```bash
/plan-cascade:hybrid-auto <desc> [--agent <name>]  # 生成 PRD
/plan-cascade:approve [--auto-run]                 # 执行
/plan-cascade:edit                                 # 编辑 PRD
/plan-cascade:show-dependencies                    # 查看依赖图
```

### 使用示例

```bash
# 快速生成 PRD
/plan-cascade:hybrid-auto "添加密码重置功能"

# 批准并自动执行
/plan-cascade:approve --auto-run
```

---

## 自动迭代与质量门控

### 启动自动迭代

```bash
# 批准后立即开始自动迭代
/plan-cascade:approve --auto-run

# 或单独启动
/plan-cascade:auto-run

# 限制最大迭代次数
/plan-cascade:auto-run --mode max_iterations --max-iterations 10

# 仅执行当前批次
/plan-cascade:auto-run --mode batch_complete
```

### 迭代模式

| 模式 | 说明 |
|------|------|
| `until_complete` | 持续执行直到所有 Story 完成（默认） |
| `max_iterations` | 执行最多 N 次迭代后停止 |
| `batch_complete` | 仅执行当前批次后停止 |

### 质量门控配置

在 `prd.json` 中配置：

```json
{
  "quality_gates": {
    "enabled": true,
    "gates": [
      {"name": "typecheck", "type": "typecheck", "required": true},
      {"name": "tests", "type": "test", "required": true},
      {"name": "lint", "type": "lint", "required": false}
    ]
  }
}
```

### 查看迭代状态

```bash
/plan-cascade:iteration-status [--verbose]
```

---

## 多 Agent 协作

### 支持的 Agent

| Agent | 类型 | 说明 |
|-------|------|------|
| `claude-code` | task-tool | Claude Code Task tool（内置，始终可用） |
| `codex` | cli | OpenAI Codex CLI |
| `amp-code` | cli | Amp Code CLI |
| `aider` | cli | Aider AI 结对编程助手 |
| `cursor-cli` | cli | Cursor CLI |

### 指定 Agent

```bash
# 使用默认 agent (claude-code)
/plan-cascade:hybrid-auto "实现用户认证"

# 指定使用 codex 执行
/plan-cascade:hybrid-auto "实现用户认证" --agent codex

# 不同阶段使用不同 Agent
/plan-cascade:approve --impl-agent claude-code --retry-agent aider

# 禁用自动降级
/plan-cascade:approve --agent codex --no-fallback
```

### 在 PRD 中指定 Agent

```json
{
  "stories": [
    {
      "id": "story-001",
      "agent": "aider",
      "title": "重构数据层",
      ...
    }
  ]
}
```

### Agent 配置文件 (agents.json)

```json
{
  "default_agent": "claude-code",
  "agents": {
    "claude-code": {"type": "task-tool"},
    "codex": {"type": "cli", "command": "codex"},
    "aider": {"type": "cli", "command": "aider"}
  },
  "phase_defaults": {
    "implementation": {
      "default_agent": "claude-code",
      "fallback_chain": ["codex", "aider"],
      "story_type_overrides": {
        "refactor": "aider",
        "bugfix": "codex"
      }
    }
  }
}
```

### Agent 优先级

```
1. 命令参数 --agent              # 最高优先级
2. 阶段覆盖 --impl-agent 等
3. Story 级别 agent 字段
4. Story 类型覆盖               # bugfix → codex, refactor → aider
5. 阶段默认 Agent
6. 降级链
7. claude-code                  # 最终降级
```

---

## 完整命令参考

### 自动策略

```bash
/plan-cascade:auto <描述>                # AI 自动选择并执行策略
```

### 项目级（Mega Plan）

```bash
/plan-cascade:mega-plan <描述>           # 生成项目计划
/plan-cascade:mega-edit                  # 编辑计划
/plan-cascade:mega-approve [--auto-prd]  # 批准并执行
/plan-cascade:mega-status                # 查看进度
/plan-cascade:mega-complete [branch]     # 合并并清理
```

### 功能级（Hybrid Ralph）

```bash
/plan-cascade:hybrid-worktree <name> <branch> <desc>  # 创建开发环境
/plan-cascade:hybrid-auto <desc> [--agent <name>]     # 生成 PRD
/plan-cascade:approve [--agent <name>] [--auto-run]   # 执行
/plan-cascade:auto-run [--mode <mode>]                # 自动迭代
/plan-cascade:iteration-status [--verbose]            # 迭代状态
/plan-cascade:agent-config [--action <action>]        # Agent 配置
/plan-cascade:hybrid-status                           # 状态
/plan-cascade:agent-status [--story-id <id>]          # Agent 状态
/plan-cascade:hybrid-complete [branch]                # 完成
/plan-cascade:edit                                    # 编辑 PRD
/plan-cascade:show-dependencies                       # 依赖图
```

### 基础规划

```bash
/plan-cascade:start                      # 开始基础规划
/plan-cascade:worktree <name> <branch>   # 创建 Worktree
/plan-cascade:complete [branch]          # 完成
```

---

## 状态文件说明

| 文件 | 类型 | 说明 |
|------|------|------|
| `prd.json` | 规划 | PRD 文档 |
| `mega-plan.json` | 规划 | 项目计划 |
| `agents.json` | 配置 | Agent 配置 |
| `findings.md` | 共享 | 发现记录 |
| `progress.txt` | 共享 | 进度日志 |
| `.agent-status.json` | 状态 | Agent 状态 |
| `.iteration-state.json` | 状态 | 迭代状态 |
| `.retry-state.json` | 状态 | 重试记录 |
| `.agent-outputs/` | 输出 | Agent 日志 |

---

## 故障排除

### Agent 不可用

```
[AgentExecutor] Agent 'codex' unavailable (CLI 'codex' not found in PATH)
```

解决：安装对应 Agent 或使用 `--no-fallback` 禁用降级。

### 质量门控失败

```
[QualityGate] typecheck failed: error TS2304
```

解决：修复类型错误后重试，或在 prd.json 中禁用该门控。

### Worktree 冲突

```
fatal: 'feature-xxx' is already checked out
```

解决：使用 `/plan-cascade:hybrid-complete` 清理现有 worktree。
