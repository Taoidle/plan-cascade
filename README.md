# Plan Cascade

> **三层级联的并行开发框架** — 从项目到功能到故事，层层分解、并行执行

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Claude Code Plugin](https://img.shields.io/badge/Claude%20Code-Plugin-blue)](https://claude.ai/code)
[![MCP Server](https://img.shields.io/badge/MCP-Server-purple)](https://modelcontextprotocol.io)
[![Version](https://img.shields.io/badge/version-3.0.0-brightgreen)](https://github.com/Taoidle/plan-cascade)

## 项目起源

本项目 fork 自 [OthmanAdi/planning-with-files](https://github.com/OthmanAdi/planning-with-files)（v2.7.1），在其 Manus 风格的文件规划基础上，大幅扩展了功能：

| 特性 | 原版 planning-with-files | Plan Cascade |
|------|-------------------------|--------------|
| 架构 | 单层规划 | **三层级联**（项目→功能→故事） |
| 并行 | 单任务 | **多层并行**（Feature 并行 + Story 并行） |
| PRD | 无 | **自动生成** + 依赖分析 |
| 编排 | 无 | **Mega Plan 项目级编排** |
| 合并 | 无 | **依赖顺序批量合并** |
| 工具支持 | Claude Code, Cursor, etc. | **Claude Code + MCP 兼容工具** |

---

## 概述

Plan Cascade 提供**三层级联**的并行开发能力，支持 Claude Code 插件和 MCP 服务器两种使用方式：

```
┌─────────────────────────────────────────────────────────────┐
│  Level 1: Mega Plan (项目级)                                 │
│  ├── 将大型项目分解为多个 Feature                             │
│  ├── 管理 Feature 之间的依赖关系                              │
│  └── 统一合并所有完成的 Feature                               │
├─────────────────────────────────────────────────────────────┤
│  Level 2: Hybrid Ralph (功能级)                              │
│  ├── 每个 Feature 在独立的 Git Worktree 中开发                │
│  ├── 自动生成 PRD，分解为多个 Story                           │
│  └── 完成后合并到目标分支                                     │
├─────────────────────────────────────────────────────────────┤
│  Level 3: Stories (故事级)                                   │
│  ├── 每个 Story 由独立 Agent 执行                             │
│  ├── 无依赖的 Story 并行执行                                  │
│  └── 按批次自动或手动流转                                     │
└─────────────────────────────────────────────────────────────┘
```

---

## 支持的工具

| 工具 | 方式 | 状态 |
|------|------|------|
| **Claude Code** | 插件 | ✅ 完整支持 |
| **Cursor** | MCP Server | ✅ 支持 |
| **Windsurf** | MCP Server | ✅ 支持 |
| **Cline** | MCP Server | ✅ 支持 |
| **Continue** | MCP Server | ✅ 支持 |
| **Zed** | MCP Server | ✅ 支持 |
| **Amp Code** | MCP Server | ✅ 支持 |

---

## 安装

### Claude Code 插件

```bash
# 从 GitHub 安装
claude plugins install Taoidle/plan-cascade

# 或克隆后本地安装
git clone https://github.com/Taoidle/plan-cascade.git
claude plugins install ./plan-cascade
```

### MCP 服务器（Cursor、Windsurf 等）

```bash
# 1. 克隆项目
git clone https://github.com/Taoidle/plan-cascade.git
cd plan-cascade

# 2. 安装依赖
pip install 'mcp[cli]'

# 3. 配置你的工具（以 Cursor 为例）
./mcp-configs/setup-mcp.sh cursor   # macOS/Linux
.\mcp-configs\setup-mcp.ps1 cursor  # Windows
```

详细配置见 [mcp-configs/README.md](mcp-configs/README.md)

---

## 使用场景

| 场景 | 推荐方案 | Claude Code 命令 | MCP 工具 |
|------|---------|------------------|----------|
| 大型项目（多个相关功能） | Mega Plan | `/plan-cascade:mega-plan` | `mega_generate` |
| 单个复杂功能 | Hybrid Ralph + Worktree | `/plan-cascade:hybrid-worktree` | `prd_generate` |
| 简单任务 | Hybrid Ralph | `/plan-cascade:hybrid-auto` | `prd_generate` |

### 适用场景详解

#### Mega Plan（项目级编排）

| 类型 | 场景 | 示例 |
|------|------|------|
| ✅ 适用 | 多功能模块的新项目开发 | 构建 SaaS 平台（用户 + 订阅 + 计费 + 后台） |
| ✅ 适用 | 涉及多子系统的大规模重构 | 单体应用重构为微服务架构 |
| ✅ 适用 | 功能群开发 | 电商平台（用户、商品、购物车、订单） |
| ❌ 不适用 | 单个功能开发 | 仅实现用户认证（用 Hybrid Ralph） |
| ❌ 不适用 | Bug 修复 | 修复登录页表单验证问题 |

#### Hybrid Ralph（功能级开发）

| 类型 | 场景 | 示例 |
|------|------|------|
| ✅ 适用 | 包含多子任务的完整功能 | 用户认证（注册 + 登录 + 密码重置） |
| ✅ 适用 | 需要分支隔离的实验功能 | 新支付渠道集成测试 |
| ✅ 适用 | 中等规模重构（5-20 文件） | API 层统一错误处理改造 |
| ❌ 不适用 | 简单单文件修改 | 修改一个组件的样式 |
| ❌ 不适用 | 快速原型验证 | 验证某个库是否可用 |

---

## 快速开始

### Claude Code 使用方式

```bash
# 场景一：大型项目
/plan-cascade:mega-plan "构建电商平台：用户认证、商品管理、购物车、订单处理"
/plan-cascade:mega-approve --auto-prd
/plan-cascade:mega-status
/plan-cascade:mega-complete

# 场景二：单个功能
/plan-cascade:hybrid-worktree feature-auth main "实现用户认证：登录、注册、密码重置"
/plan-cascade:approve
/plan-cascade:hybrid-complete
```

### MCP 工具使用方式（Cursor 等）

```python
# 场景一：大型项目
mega_generate("构建电商平台：用户认证、商品管理、购物车、订单处理")
mega_add_feature("feature-auth", "用户认证", "实现 JWT 认证...")
mega_validate()
mega_get_batches()

# 场景二：单个功能
prd_generate("实现用户认证：登录、注册、密码重置")
prd_add_story("设计用户表", "创建用户数据库 Schema...", priority="high")
prd_validate()
prd_get_batches()

# 执行过程
get_story_context("story-001")
append_findings("决定使用 bcrypt 加密密码...", story_id="story-001")
mark_story_complete("story-001")
```

---

## MCP 服务器

Plan Cascade 提供完整的 MCP 服务器，支持 18 个工具和 8 个资源。

### 可用工具

#### 项目级（Mega Plan）

| 工具 | 说明 |
|------|------|
| `mega_generate` | 从描述生成项目计划 |
| `mega_add_feature` | 添加 Feature 到计划 |
| `mega_validate` | 验证计划结构 |
| `mega_get_batches` | 获取并行执行批次 |
| `mega_update_feature_status` | 更新 Feature 状态 |
| `mega_get_merge_plan` | 获取合并计划 |

#### 功能级（PRD）

| 工具 | 说明 |
|------|------|
| `prd_generate` | 从描述生成 PRD |
| `prd_add_story` | 添加 Story 到 PRD |
| `prd_validate` | 验证 PRD 结构 |
| `prd_get_batches` | 获取执行批次 |
| `prd_update_story_status` | 更新 Story 状态 |
| `prd_detect_dependencies` | 自动检测依赖 |

#### 执行级

| 工具 | 说明 |
|------|------|
| `get_story_context` | 获取 Story 完整上下文 |
| `get_execution_status` | 获取执行状态 |
| `append_findings` | 记录发现 |
| `mark_story_complete` | 标记完成 |
| `get_progress` | 获取进度 |
| `cleanup_locks` | 清理锁文件 |

### 可用资源

| 资源 URI | 说明 |
|----------|------|
| `plan-cascade://prd` | 当前 PRD |
| `plan-cascade://mega-plan` | 当前项目计划 |
| `plan-cascade://findings` | 开发发现 |
| `plan-cascade://progress` | 进度时间线 |
| `plan-cascade://mega-status` | Mega-plan 执行状态 |
| `plan-cascade://mega-findings` | 项目级发现 |
| `plan-cascade://story/{id}` | 特定 Story 详情 |
| `plan-cascade://feature/{id}` | 特定 Feature 详情 |

### 配置示例

```bash
# 查看所有配置示例
ls mcp-configs/

# 快速配置
./mcp-configs/setup-mcp.sh cursor     # Cursor
./mcp-configs/setup-mcp.sh windsurf   # Windsurf
./mcp-configs/setup-mcp.sh claude     # Claude Code
```

详细文档见 [docs/MCP-SERVER-GUIDE.md](docs/MCP-SERVER-GUIDE.md)

---

## 命令参考

### Claude Code 命令

#### 项目级（Mega Plan）

```bash
/plan-cascade:mega-plan <描述>           # 生成项目计划
/plan-cascade:mega-edit                  # 编辑计划
/plan-cascade:mega-approve [--auto-prd]  # 批准并执行
/plan-cascade:mega-status                # 查看进度
/plan-cascade:mega-complete [branch]     # 合并并清理
```

#### 功能级（Hybrid Ralph）

```bash
/plan-cascade:hybrid-worktree <name> <branch> <desc>  # 创建开发环境
/plan-cascade:hybrid-auto <desc>                       # 生成 PRD
/plan-cascade:approve                                  # 执行 PRD
/plan-cascade:hybrid-status                            # 查看状态
/plan-cascade:hybrid-complete [branch]                 # 完成并合并
/plan-cascade:edit                                     # 编辑 PRD
/plan-cascade:show-dependencies                        # 依赖图
```

#### 基础规划

```bash
/plan-cascade:start                      # 开始基础规划模式
/plan-cascade:worktree <name> <branch>   # 创建 Worktree（无 PRD）
/plan-cascade:complete [branch]          # 完成基础规划
```

---

## 项目结构

```
plan-cascade/
├── .claude-plugin/
│   └── plugin.json          # 插件配置
├── commands/                # 顶层命令 (16 个)
│   ├── mega-*.md           # Mega Plan 命令
│   ├── hybrid-*.md         # Hybrid Ralph 命令
│   └── *.md                # 基础命令
├── skills/
│   ├── mega-plan/          # 项目级技能
│   │   ├── SKILL.md
│   │   ├── core/           # Python 核心模块
│   │   └── commands/
│   ├── hybrid-ralph/       # 功能级技能
│   │   ├── SKILL.md
│   │   ├── core/
│   │   └── commands/
│   └── planning-with-files/ # 基础规划技能
│       ├── SKILL.md
│       └── templates/
├── mcp_server/              # MCP 服务器
│   ├── server.py           # 主入口
│   ├── resources.py        # MCP 资源
│   └── tools/              # MCP 工具
│       ├── prd_tools.py
│       ├── mega_tools.py
│       └── execution_tools.py
├── mcp-configs/             # MCP 配置示例
│   ├── README.md
│   ├── cursor-mcp.json
│   ├── windsurf-mcp.json
│   ├── setup-mcp.sh        # 安装脚本 (Unix)
│   └── setup-mcp.ps1       # 安装脚本 (Windows)
└── docs/                    # 文档
    └── MCP-SERVER-GUIDE.md
```

---

## 更新日志

### v3.0.0

- **MCP 服务器** - 支持 Cursor、Windsurf、Cline 等 MCP 兼容工具
- 18 个 MCP 工具 + 8 个 MCP 资源
- 多平台配置示例和安装脚本
- 与 Claude Code 插件完全兼容

### v2.8.0

- **Mega Plan** - 项目级多功能编排系统
- 三层级联架构（项目 → 功能 → 故事）
- 公共 findings 机制
- 依赖驱动的批次执行

### v2.7.x

- Auto/Manual 执行模式
- 操作系统自动检测
- 命令自动批准配置

完整更新日志见 [CHANGELOG.md](CHANGELOG.md)

---

## 致谢

本项目基于以下优秀项目构建：

- **[OthmanAdi/planning-with-files](https://github.com/OthmanAdi/planning-with-files)** - 原始项目，提供了核心的 Manus 风格文件规划模式和基础框架
- **[snarktank/ralph](https://github.com/snarktank/ralph)** - 启发了 PRD 格式和任务分解方法
- **Manus AI** - 上下文工程模式的先驱
- **Anthropic** - Claude Code、Plugin 系统和 MCP 协议

---

## 许可证

MIT License

---

**项目地址**: [Taoidle/plan-cascade](https://github.com/Taoidle/plan-cascade)

[![Star History Chart](https://api.star-history.com/svg?repos=Taoidle/plan-cascade&type=Date)](https://star-history.com/#Taoidle/plan-cascade&Date)
