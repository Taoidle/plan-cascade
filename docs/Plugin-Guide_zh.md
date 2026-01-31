[English](Plugin-Guide.md)

# Plan Cascade - Claude Code Plugin Guide

**版本**: 4.2.0
**最后更新**: 2026-01-31

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

## 设计文档系统

Plan Cascade 自动生成技术设计文档 (`design_doc.json`)，为 Story 执行提供架构上下文。

### 两级架构

```
┌─────────────────────────────────────────────────────────────┐
│ 第1级: 项目设计 (来自 mega-plan.json)                        │
│ ─────────────────────────────────────────────────────────── │
│ • 全局架构和系统概览                                          │
│ • 跨 Feature 组件和模式                                      │
│ • 项目级 ADR（架构决策记录）                                  │
│ • Feature 映射（哪些模式/决策适用于哪个 Feature）             │
└─────────────────────────────────────────────────────────────┘
                          │
                          │ 继承
                          ▼
┌─────────────────────────────────────────────────────────────┐
│ 第2级: Feature 设计 (来自 prd.json)                          │
│ ─────────────────────────────────────────────────────────── │
│ • Feature 特定组件                                           │
│ • Feature 特定 API 和数据模型                                │
│ • Feature 特定 ADR（前缀 ADR-F###）                          │
│ • Story 映射（每个 Story 对应哪些组件/决策）                  │
└─────────────────────────────────────────────────────────────┘
```

### 自动生成流程

| 命令 | 设计文档生成时机 | 级别 |
|------|----------------|------|
| `mega-plan` | mega-plan.json 生成后自动 | 项目级 |
| `hybrid-worktree` | prd.json 生成后自动 | Feature 级（继承项目级） |
| `hybrid-auto` | prd.json 生成后自动 | Feature 级 |

### 外部设计文档

所有三个主要命令都支持可选的外部设计文档：

```bash
# mega-plan: 第2个参数
/plan-cascade:mega-plan "构建电商平台" ./project-architecture.md

# hybrid-auto: 第2个参数
/plan-cascade:hybrid-auto "实现用户认证" ./auth-design.md

# hybrid-worktree: 第4个参数
/plan-cascade:hybrid-worktree fix-auth main "修复认证" ./architecture.md

# 支持格式：Markdown (.md)、JSON (.json)、HTML (.html)
```

外部文档会自动转换为 `design_doc.json` 格式。

### 设计文档命令

```bash
/plan-cascade:design-generate    # 手动生成设计文档
/plan-cascade:design-review      # 审查设计文档
/plan-cascade:design-import      # 导入外部文档
```

---

## 外部框架技能

Plan Cascade 内置框架特定技能，可自动检测并注入到 Story 执行上下文中。

### 支持的框架

| 框架 | 技能 | 自动检测 |
|------|------|----------|
| React/Next.js | `react-best-practices`, `web-design-guidelines` | `package.json` 包含 `react` 或 `next` |
| Vue/Nuxt | `vue-best-practices`, `vue-router-best-practices`, `vue-pinia-best-practices` | `package.json` 包含 `vue` 或 `nuxt` |
| Rust | `rust-coding-guidelines`, `rust-ownership`, `rust-error-handling`, `rust-concurrency` | 存在 `Cargo.toml` |

### 工作原理

1. **检测**：执行 Story 时，Plan Cascade 扫描项目中的框架标识文件（`package.json`、`Cargo.toml`）
2. **加载**：从 `external-skills/` 中的 Git 子模块加载匹配的技能
3. **注入**：在实现和重试阶段将技能内容注入到 Agent 上下文中

### 初始化

外部技能作为 Git 子模块包含。克隆后需要初始化：

```bash
git submodule update --init --recursive
```

### 技能来源

| 来源 | 仓库 | 技能 |
|------|------|------|
| Vercel | [vercel-labs/agent-skills](https://github.com/vercel-labs/agent-skills) | React, Web Design |
| Vue.js | [vuejs-ai/skills](https://github.com/vuejs-ai/skills) | Vue, Pinia, Router |
| Rust | [actionbook/rust-skills](https://github.com/actionbook/rust-skills) | 编码规范、所有权、错误处理、并发 |

---

## `/plan-cascade:auto` - AI 自动策略

最简单的入口。AI 分析任务描述并自动选择最佳策略。

### 工作原理

1. 用户提供任务描述
2. AI 进行结构化自评估，分析：
   - **范围**：涉及多少功能区域？
   - **复杂度**：是否有子任务依赖？需要架构决策吗？
   - **风险**：可能破坏现有功能吗？需要隔离测试吗？
   - **并行化**：能否并行执行以提高效率？
3. AI 输出结构化分析结果和置信度分数
4. AI 选择最优策略并直接执行

### 策略选择

| 分析结果 | 策略 | 示例 |
|---------|------|------|
| 1个区域，1-2步，低风险 | **direct** | "修复登录按钮样式" |
| 2-3个区域，3-7步，有依赖 | **hybrid-auto** | "实现用户认证" |
| hybrid-auto + 高风险或实验性 | **hybrid-worktree** | "实验性重构支付模块" |
| 4+个区域，多个独立特性 | **mega-plan** | "构建电商平台：用户、商品、订单" |

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
/plan-cascade:mega-approve --auto-prd    # 批准并全自动执行所有批次
/plan-cascade:mega-resume --auto-prd     # 恢复中断的执行
/plan-cascade:mega-status                # 查看进度
/plan-cascade:mega-complete [branch]     # 合并并清理
```

### 使用 --auto-prd 全自动执行

使用 `--auto-prd` 参数，mega-approve 会全自动执行整个 mega-plan：
1. 为当前批次创建 worktree
2. 为每个 feature 生成 PRD（通过 Task agent）
3. 执行所有 stories（通过 Task agent）
4. 监控直到批次完成
5. 合并批次到目标分支
6. 自动继续下一批次
7. 仅在出错或合并冲突时暂停

### 恢复中断的执行

如果执行被中断：
```bash
/plan-cascade:mega-resume --auto-prd
```

此命令会：
- 自动从文件检测当前状态（mega-plan.json、.mega-status.json、worktrees）
- 跳过已完成的 feature 和 story
- 从中断处继续执行

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
/plan-cascade:hybrid-resume --auto                    # 恢复中断的执行
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

**hybrid-auto（PRD 生成）：**
```bash
# 使用默认 agent (claude-code)
/plan-cascade:hybrid-auto "实现用户认证"

# 指定 PRD 生成使用的 agent
/plan-cascade:hybrid-auto "实现用户认证" --agent=codex
```

**approve（Story 执行）：**
```bash
# 全局 agent 覆盖（所有 story）
/plan-cascade:approve --agent=codex

# 按阶段指定 agent
/plan-cascade:approve --impl-agent=claude-code --retry-agent=aider

# 禁用自动降级
/plan-cascade:approve --agent=codex --no-fallback
```

**mega-approve（Feature 执行）：**
```bash
# 不同阶段使用不同 agent
/plan-cascade:mega-approve --auto-prd --prd-agent=codex --impl-agent=aider

# 全局覆盖
/plan-cascade:mega-approve --auto-prd --agent=claude-code
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
/plan-cascade:mega-approve --auto-prd    # 批准并全自动执行所有批次
/plan-cascade:mega-resume --auto-prd     # 恢复中断的执行
/plan-cascade:mega-status                # 查看进度
/plan-cascade:mega-complete [branch]     # 合并并清理
```

### 功能级（Hybrid Ralph）

```bash
/plan-cascade:hybrid-worktree <name> <branch> <desc>  # 创建开发环境
/plan-cascade:hybrid-auto <desc> [--agent <name>]     # 生成 PRD
/plan-cascade:approve [--agent <name>] [--auto-run]   # 执行
/plan-cascade:hybrid-resume --auto                    # 恢复中断的执行
/plan-cascade:auto-run [--mode <mode>]                # 自动迭代
/plan-cascade:iteration-status [--verbose]            # 迭代状态
/plan-cascade:agent-config [--action <action>]        # Agent 配置
/plan-cascade:hybrid-status                           # 状态
/plan-cascade:agent-status [--story-id <id>]          # Agent 状态
/plan-cascade:hybrid-complete [branch]                # 完成
/plan-cascade:edit                                    # 编辑 PRD
/plan-cascade:show-dependencies                       # 依赖图
```

### 设计文档

```bash
/plan-cascade:design-generate            # 自动生成设计文档
/plan-cascade:design-import <path>       # 导入外部设计文档
/plan-cascade:design-review              # 审查设计文档
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
| `design_doc.json` | 规划 | 技术设计文档 |
| `agents.json` | 配置 | Agent 配置 |
| `findings.md` | 共享 | 发现记录 |
| `mega-findings.md` | 共享 | 项目级发现（mega-plan） |
| `progress.txt` | 共享 | 进度日志 |
| `.mega-status.json` | 状态 | Mega-plan 执行状态 |
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
