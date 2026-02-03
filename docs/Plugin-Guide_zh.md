[English](Plugin-Guide.md)

# Plan Cascade - Claude Code Plugin Guide

**版本**: 4.4.0
**最后更新**: 2026-02-03

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

## 首次设置

使用 Plan Cascade 命令前，请运行初始化命令以确保环境正确配置：

```bash
/plan-cascade:init
```

此命令会：
- 检测操作系统
- 安装 `uv`（快速 Python 包管理器）（如果需要）
- 验证 Python 执行是否正常
- 确认 Plan Cascade 模块可访问

**注意**：这在 Windows 上尤为重要，因为 Windows 的 Python 执行别名可能会干扰直接的 `python3` 调用。

---

## 命令概览

Plan Cascade 提供五个主要入口命令，适用于不同规模的开发场景：

| 入口命令 | 适用场景 | 特点 |
|----------|----------|------|
| `/plan-cascade:auto` | 任意任务（AI 自动选择策略） | 自动策略选择 + 执行流程深度 |
| `/plan-cascade:mega-plan` | 大型项目（多个相关功能） | Feature 级并行 + Story 级并行 |
| `/plan-cascade:hybrid-worktree` | 单个复杂功能 | Worktree 隔离 + Story 并行 |
| `/plan-cascade:hybrid-auto` | 简单功能 | 快速 PRD 生成 + Story 并行 |
| `/plan-cascade:dashboard` | 状态监控 | 跨所有执行的聚合状态视图 |

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

## 三层外部技能系统

Plan Cascade 使用三层技能系统，允许内置、外部和用户定义的技能共存，并具有明确的优先级规则。

### 层级概览

| 层级 | 来源类型 | 优先级范围 | 说明 |
|------|----------|------------|------|
| 1 | 内置 (Builtin) | 1-50 | Plan Cascade 内置（Python、TypeScript、Go、Java） |
| 2 | 子模块 (Submodule) | 51-100 | 来自 Git 子模块（Vercel、Vue、Rust 技能） |
| 3 | 用户 (User) | 101-200 | 用户自定义，最高优先级 |

优先级高的技能会覆盖同名的低优先级技能。

### 技能检测和注入机制

1. **检测**：执行 Story 时，Plan Cascade 扫描项目文件中的框架标识
2. **匹配**：选择 `detect.files` 和 `detect.patterns` 匹配的技能
3. **去重**：如果多个技能具有相同的基础名称，仅保留最高优先级的
4. **注入**：在 `implementation` 和 `retry` 阶段最多注入 3 个技能到 Agent 上下文

### CLI 命令

```bash
# 列出所有可用技能
plan-cascade skills list

# 按来源类型分组列出技能
plan-cascade skills list --group

# 显示适用于当前项目的技能
plan-cascade skills detect

# 显示技能及覆盖详情
plan-cascade skills detect --overrides

# 从本地路径添加用户技能
plan-cascade skills add my-skill --path ./my-skills/SKILL.md

# 从远程 URL 添加用户技能
plan-cascade skills add remote-skill --url https://example.com/skills/SKILL.md

# 使用自定义选项添加
plan-cascade skills add my-skill --path ./SKILL.md --priority 150 --level project \
  --detect-files package.json,tsconfig.json --detect-patterns typescript \
  --inject-into implementation,retry

# 删除用户技能
plan-cascade skills remove my-skill

# 从特定级别删除
plan-cascade skills remove my-skill --level user

# 验证所有技能配置
plan-cascade skills validate

# 详细模式验证
plan-cascade skills validate --verbose

# 刷新缓存的远程技能（重新下载）
plan-cascade skills refresh --all

# 刷新特定技能
plan-cascade skills refresh remote-skill

# 清除缓存而不重新下载
plan-cascade skills refresh --all --clear

# 显示缓存统计信息
plan-cascade skills cache
```

### 配置文件

用户技能在 `.plan-cascade/skills.json`（项目级）或 `~/.plan-cascade/skills.json`（用户级）中配置。

**项目级配置优先于用户级配置。**

```json
{
  "version": "1.0.0",
  "skills": [
    {
      "name": "my-custom-skill",
      "path": "./my-skills/custom/SKILL.md",
      "detect": {
        "files": ["package.json"],
        "patterns": ["my-framework"]
      },
      "priority": 150,
      "inject_into": ["implementation"]
    },
    {
      "name": "company-coding-standards",
      "path": "../shared-skills/coding-standards/SKILL.md",
      "detect": {
        "files": ["pyproject.toml", "package.json", "Cargo.toml"],
        "patterns": []
      },
      "priority": 180,
      "inject_into": ["implementation", "retry"]
    },
    {
      "name": "remote-skill",
      "url": "https://raw.githubusercontent.com/example/skills/main/advanced/SKILL.md",
      "detect": {
        "files": ["config.json"],
        "patterns": ["advanced-feature"]
      },
      "priority": 160,
      "inject_into": ["implementation"]
    }
  ]
}
```

**配置字段说明：**

| 字段 | 必需 | 说明 |
|------|------|------|
| `name` | 是 | 唯一技能名称 |
| `path` | 是* | SKILL.md 的本地路径（相对于配置文件） |
| `url` | 是* | 技能目录的远程 URL |
| `detect.files` | 否 | 触发此技能的文件（如 `["package.json"]`） |
| `detect.patterns` | 否 | 在检测文件中匹配的模式（如 `["react"]`） |
| `priority` | 否 | 优先级 101-200（默认：150） |
| `inject_into` | 否 | 注入阶段：`implementation`、`retry`（默认：两者都注入） |

*`path` 或 `url` 必须提供其一，但不能同时提供。

### 创建自定义技能

自定义技能在带有 YAML frontmatter 的 SKILL.md 文件中定义。

**SKILL.md 格式：**

```markdown
---
name: my-custom-skill
description: 简要描述此技能提供的功能。
license: MIT
metadata:
  author: your-name
  version: "1.0.0"
---

# 我的自定义技能

## 适用场景

描述何时应该使用此技能...

## 指南

| 规则 | 说明 |
|------|------|
| 规则 1 | 描述 |
| 规则 2 | 描述 |

## 代码示例

\`\`\`typescript
// 示例代码...
\`\`\`

## 反模式

| 避免 | 替代方案 |
|------|----------|
| 不好的模式 | 好的模式 |
```

**Frontmatter 字段：**

| 字段 | 必需 | 说明 |
|------|------|------|
| `name` | 是 | 技能标识符 |
| `description` | 是 | 何时使用此技能 |
| `license` | 否 | 许可证类型 |
| `metadata.author` | 否 | 技能作者 |
| `metadata.version` | 否 | 技能版本 |

### 优先级和覆盖规则

1. **高优先级胜出**：当技能共享相同的基础名称时，使用最高优先级的技能
2. **用户技能可覆盖**：优先级 150 的用户技能会覆盖优先级 30 的内置技能
3. **项目 > 用户**：项目级 `.plan-cascade/skills.json` 优先于 `~/.plan-cascade/skills.json`

**覆盖示例：**

```
builtin/typescript (优先级: 30) <- 被覆盖
submodule/vercel-react (优先级: 75) <- 生效
user/my-typescript (优先级: 150) <- 生效（覆盖 builtin/typescript）
```

**检查生效的技能：**

```bash
# 查看实际会使用哪些技能
plan-cascade skills detect --overrides

# 输出显示：
# - 总匹配数: 5
# - 去重后生效: 3
# - 覆盖详情: "user/my-typescript (150) 覆盖 builtin/typescript (30)"
```

### 远程技能缓存

远程 URL 技能会缓存到本地以提高性能和支持离线访问。

**缓存详情：**
- 位置：`~/.plan-cascade/cache/skills/`
- 默认 TTL：7 天
- 优雅降级：网络失败时使用过期缓存

**缓存命令：**

```bash
# 查看缓存统计信息
plan-cascade skills cache

# 强制刷新所有缓存的技能
plan-cascade skills refresh --all

# 清除所有缓存
plan-cascade skills refresh --all --clear
```

### 最佳实践

1. **使用项目级配置**：用于团队共享的技能
2. **使用用户级配置**：用于个人编码风格偏好
3. **谨慎设置优先级**：一般覆盖使用 150，关键规则使用 180+
4. **保持技能专注**：每个技能专注一个关注点（如测试、错误处理）
5. **包含检测模式**：更具体的模式可减少误匹配
6. **使用 `skills detect` 测试**：执行前验证技能是否正确检测

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
4. AI 选择最优策略和执行流程深度，然后执行

### 执行流程深度

Plan Cascade 使用三个工作流深度级别来控制门控严格程度：

| 流程 | 描述 | 门控模式 | AI 验证 | 需要确认 |
|------|------|----------|---------|----------|
| `quick` | 最快路径，最小门控 | soft | 禁用 | 否 |
| `standard` | 平衡速度和质量（默认） | soft | 启用 | 否 |
| `full` | 严格方法论 + 严格门控 | hard | 启用 + 代码审查 | 是 |

### 策略选择

| 分析结果 | 策略 | 示例 |
|---------|------|------|
| 1个区域，1-2步，低风险 | **direct** | "修复登录按钮样式" |
| 2-3个区域，3-7步，有依赖 | **hybrid-auto** | "实现用户认证" |
| hybrid-auto + 高风险或实验性 | **hybrid-worktree** | "实验性重构支付模块" |
| 4+个区域，多个独立特性 | **mega-plan** | "构建电商平台：用户、商品、订单" |

### 命令行参数

| 参数 | 描述 | 示例 |
|------|------|------|
| `--flow <quick\|standard\|full>` | 覆盖执行流程深度 | `--flow full` |
| `--explain` | 显示分析结果但不执行 | `--explain` |
| `--confirm` | 等待用户确认后执行 | `--confirm` |
| `--tdd <off\|on\|auto>` | 控制 TDD 模式 | `--tdd on` |

### 使用示例

```bash
# AI 自动判断策略
/plan-cascade:auto "修复 README 中的拼写错误"
# → 使用 direct 策略，quick 流程

/plan-cascade:auto "实现 OAuth 用户登录"
# → 使用 hybrid-auto 策略，standard 流程

/plan-cascade:auto "实验性重构 API 层"
# → 使用 hybrid-worktree 策略

/plan-cascade:auto "构建博客平台：用户、文章、评论、RSS"
# → 使用 mega-plan 策略

# 带参数使用
/plan-cascade:auto --flow full --tdd on "实现支付处理"
/plan-cascade:auto --explain "构建用户认证"
/plan-cascade:auto --confirm "关键数据库迁移"
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
/plan-cascade:hybrid-worktree <name> <branch> <desc> [--agent <name>]  # 创建 Worktree + PRD
/plan-cascade:hybrid-auto <desc> [--agent <name>]     # 生成 PRD（无 Worktree）
/plan-cascade:approve [--auto-run]                    # 执行 PRD
/plan-cascade:hybrid-resume --auto                    # 恢复中断的执行
/plan-cascade:hybrid-status                           # 查看状态
/plan-cascade:hybrid-complete [branch] [--force]      # 完成并合并（--force 跳过未提交检查）
```

| 参数 | 说明 |
|------|------|
| `<name>` | 任务名称（用于 Worktree 和分支） |
| `<branch>` | 完成后合并到的目标分支 |
| `<desc>` | 任务描述或现有 PRD 文件路径 |
| `--agent` | 可选。PRD 生成使用的 Agent（覆盖 agents.json 配置） |

### 使用示例

```bash
# 创建隔离开发环境（使用 agents.json 中的默认 agent）
/plan-cascade:hybrid-worktree feature-auth main "实现用户认证：登录、注册、密码重置"

# 创建 Worktree 并指定 PRD 生成 Agent
/plan-cascade:hybrid-worktree feature-auth main "实现用户认证" --agent=codex

# 生成 PRD（无 Worktree）
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

## `/plan-cascade:dashboard` - 状态监控

提供跨所有 Plan Cascade 执行的聚合状态视图。

### 显示内容

- **执行状态**：当前 mega-plan、hybrid 或 direct 执行的状态
- **Story 进度**：完成百分比、批次进度、待处理/失败的 Story
- **最近活动**：带时间戳的最近操作时间线
- **建议操作**：根据上下文推荐的下一步操作

### 使用

```bash
# 显示聚合状态
/plan-cascade:dashboard

# 显示详细输出
/plan-cascade:dashboard --verbose
```

### 输出示例

```
============================================================
PLAN CASCADE DASHBOARD
============================================================

当前执行: hybrid-auto
状态: IN_PROGRESS (62%)
流程: standard

Story 进度:
  ✓ story-001: 已完成
  ✓ story-002: 已完成
  → story-003: 进行中 (claude-code)
  ○ story-004: 待处理
  ○ story-005: 待处理

最近活动:
  [10:30] story-002 通过 claude-code 完成
  [10:28] story-002 通过 claude-code 开始
  [10:25] story-001 通过 claude-code 完成

建议操作:
  • 继续: 等待 story-003 完成
  • 查看: 检查 progress.txt 了解详情
============================================================
```

---

## DoR/DoD 门控（就绪定义/完成定义）

Plan Cascade 提供验证门控以确保执行边界的质量。

### 就绪定义（DoR）

DoR 门控在 Story/Feature 执行**之前**运行，验证先决条件：

| 检查 | 描述 | 模式 |
|------|------|------|
| 验收标准 | 验证标准可测试且明确 | SOFT/HARD |
| 依赖有效 | 确保所有依赖已解决 | SOFT/HARD |
| 风险显式 | 验证风险评估已记录 | SOFT/HARD |
| 验证提示 | 检查 AI 验证提示是否存在 | SOFT/HARD |

**门控模式：**
- **SOFT**：仅警告，执行继续
- **HARD**：阻塞，失败时停止执行（用于 Full 流程）

### 完成定义（DoD）

DoD 门控在 Story/Feature 执行**之后**运行，验证完成条件：

| 级别 | 检查 | 使用场景 |
|------|------|----------|
| **STANDARD** | 质量门控通过、AI 验证、变更摘要 | 默认 |
| **FULL** | 上述 + 代码审查、测试变更、部署说明 | Full 流程 |

**DoD 检查：**
- 质量门控（typecheck、test、lint）通过
- 未检测到骨架代码
- 验收标准已验证
- 变更摘要已生成
- 代码审查通过（Full 级别）

### 在 PRD 中配置门控

```json
{
  "execution_config": {
    "flow": "standard",
    "dor_mode": "soft",
    "dod_level": "standard"
  }
}
```

---

## TDD 支持

Plan Cascade 支持在 Story 级别使用可选的测试驱动开发（TDD）节奏。

### TDD 模式

| 模式 | 描述 | 使用场景 |
|------|------|----------|
| `off` | 禁用 TDD | 简单变更、文档 |
| `on` | 启用 TDD，带提示和合规检查 | 关键功能、安全代码 |
| `auto` | 基于风险评估自动启用 | 大多数开发任务（默认） |

### TDD 工作流

当 TDD 启用时：

1. **红灯阶段**：根据验收标准编写失败的测试
2. **绿灯阶段**：最小实现以通过测试
3. **重构阶段**：改进代码同时保持测试通过

### TDD 合规检查

Story 完成后，质量门控验证：
- 测试文件与代码变更一起修改
- 高风险 Story 有对应的测试
- 满足测试覆盖率要求（如果配置）

### 使用

```bash
# 为关键功能启用 TDD
/plan-cascade:auto --tdd on "实现支付处理"

# 为文档禁用 TDD
/plan-cascade:auto --tdd off "更新 README"

# 让自动检测决定（默认）
/plan-cascade:auto "添加用户资料功能"
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
      {"name": "format", "type": "format", "required": false, "check_only": false},
      {"name": "typecheck", "type": "typecheck", "required": true},
      {"name": "tests", "type": "test", "required": true},
      {"name": "lint", "type": "lint", "required": false},
      {"name": "code-review", "type": "code_review", "required": false, "min_score": 0.7, "block_on_critical": true}
    ]
  }
}
```

**门控执行顺序：**
1. **PRE_VALIDATION**: FORMAT（自动格式化代码）
2. **VALIDATION**: TYPECHECK、TEST、LINT（并行）
3. **POST_VALIDATION**: CODE_REVIEW、IMPLEMENTATION_VERIFY（并行）

**门控类型：**

| 类型 | 说明 | 选项 |
|------|------|------|
| `format` | 自动格式化代码 | `check_only`: 仅检查，不修改 |
| `typecheck` | 类型检查（mypy/tsc） | - |
| `test` | 运行测试（pytest/jest） | - |
| `lint` | 代码检查（ruff/eslint） | - |
| `code_review` | AI 代码审查 | `min_score`、`block_on_critical` |
| `implementation_verify` | AI 实现验证 | - |

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

# 禁用 AI 验证门（默认启用）
/plan-cascade:approve --no-verify

# 禁用 AI 代码审查（默认启用）
/plan-cascade:approve --no-review

# 指定代码审查 agent
/plan-cascade:approve --review-agent=claude-code
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
/plan-cascade:auto <描述> [选项]         # AI 自动选择并执行策略

选项:
  --flow <quick|standard|full>    覆盖执行流程深度
  --explain                       显示分析结果但不执行
  --confirm                       等待用户确认后执行
  --tdd <off|on|auto>             控制 TDD 模式
```

### 状态监控

```bash
/plan-cascade:dashboard [--verbose]      # 显示聚合状态视图
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
/plan-cascade:hybrid-worktree <name> <branch> <desc> [--agent <name>]  # 创建 Worktree + PRD
/plan-cascade:hybrid-auto <desc> [--agent <name>]     # 生成 PRD（无 Worktree）
/plan-cascade:approve [--agent <name>] [--auto-run] [--no-verify] [--no-review]  # 执行
/plan-cascade:hybrid-resume --auto                    # 恢复中断的执行
/plan-cascade:auto-run [--mode <mode>] [--no-verify] [--no-review]  # 自动迭代
/plan-cascade:iteration-status [--verbose]            # 迭代状态
/plan-cascade:agent-config [--action <action>]        # Agent 配置
/plan-cascade:hybrid-status                           # 状态
/plan-cascade:agent-status [--story-id <id>]          # Agent 状态
/plan-cascade:hybrid-complete [branch] [--force]      # 完成（--force 跳过未提交检查）
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
/plan-cascade:complete [branch] [--force]  # 完成（--force 跳过未提交检查）
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
| `.state/stage-state.json` | 状态 | 阶段状态机状态 (v4.4.0+) |
| `.state/dor-results.json` | 状态 | DoR 门控结果 (v4.4.0+) |
| `.state/dod-results.json` | 状态 | DoD 门控结果 (v4.4.0+) |
| `.hybrid-execution-context.md` | 上下文 | Hybrid 任务上下文，用于 AI 恢复 |
| `.mega-execution-context.md` | 上下文 | Mega-plan 上下文，用于 AI 恢复 |
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

### 中断执行恢复

如果 mega-plan 或 hybrid 任务被中断（例如连接断开、Claude Code 崩溃）：

```bash
# 对于 mega-plan
/plan-cascade:mega-resume --auto-prd

# 对于 hybrid 任务
/plan-cascade:hybrid-resume --auto
```

这些命令会：
- 自动从现有文件检测当前状态
- 跳过已完成的工作
- 从中断处继续执行
- 支持新旧两种进度标记格式

### 长会话后的上下文恢复

Plan Cascade 会自动生成上下文文件，帮助在以下情况后恢复执行状态：
- 上下文压缩（AI 总结旧消息）
- 上下文截断（旧消息被删除）
- 新的对话会话
- Claude Code 重启

**生成的上下文文件：**
| 文件 | 模式 | 说明 |
|------|------|------|
| `.hybrid-execution-context.md` | Hybrid | 当前批次、待执行 Story、进度摘要 |
| `.mega-execution-context.md` | Mega Plan | 活动的 worktree、并行执行状态 |

这些文件通过钩子在执行期间自动更新。如果发现 AI 丢失了上下文：

```bash
# 通用恢复命令（自动检测模式）
/plan-cascade:resume

# 或直接查看上下文文件
cat .hybrid-execution-context.md
cat .mega-execution-context.md
```
