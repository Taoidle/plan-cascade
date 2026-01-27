# Planning with Files

> **并行开发多个复杂功能的利器** — 在隔离环境中同时推进多个任务

> **⚡ Enhanced fork** with improved Hybrid Ralph execution modes and workflow automation

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Claude Code Plugin](https://img.shields.io/badge/Claude%20Code-Plugin-blue)](https://code.claude.com/docs/en/plugins)
[![Version](https://img.shields.io/badge/version-2.7.9-brightgreen)](https://github.com/Taoidle/planning-with-files)

## 核心功能：多任务并发开发

这是 planning-with-files 的增强版本，专注于**多任务并行开发**场景。

### 解决的问题

在软件开发中，经常需要同时推进多个功能：
- 🔨 正在开发 Feature A
- 🔧 同时需要修复 Bug B
- 📊 还要重构模块 C
- 📝 文档更新任务 D

传统方式：串行开发或频繁切换分支，效率低下

**我们的方案：并行推进，互不干扰**

```
工作台/
├── .worktree/feature-auth/         ← 终端 1: 开发认证功能
├── .worktree/fix-api-bug/          ← 终端 2: 修复 API bug
├── .worktree/refactor-database/    ← 终端 3: 重构数据库
└── .worktree/update-docs/          ← 终端 4: 更新文档

每个任务都有独立的：
- Git 分支
- 工作目录
- PRD（需求分解）
- 执行进度
```

### 工作流程

```bash
# 1. 创建多个并行任务（每个在独立的 worktree 中）
/planning-with-files:hybrid-worktree feature-auth main "实现用户认证"
/planning-with-files:hybrid-worktree fix-api-bug main "修复API超时bug"
/planning-with-files:hybrid-worktree refactor-db main "重构数据库层"

# 2. 每个任务自动生成 PRD，分解成多个可并行执行的 story

# 3. 在 Auto 模式下，stories 自动并行执行，批次自动流转
# 在 Manual 模式下，每批次完成后确认，再继续下一批次

# 4. 任务完成后，自动合并到主分支
/planning-with-files:hybrid-complete main
```

### 与原版的区别

| 特性 | 原版 planning-with-files | **这个 fork** |
|------|------------------------|--------------|
| 核心场景 | 单任务规划管理 | **多任务并行开发** |
| Worktree | 可选功能 | **核心功能** - 多任务隔离的基础 |
| PRD 执行 | 需要人工介入每个批次 | **Auto 模式全自动流转** |
| 并行粒度 | 单个任务 | **任务级并行** + **Story级并行** |

### 为什么选择这个版本

**选择这个 fork，如果你需要：**

✅ **同时推进多个功能** - 三个功能一起开发，互不影响
✅ **快速试错** - Feature A 写一半发现不行，直接丢弃，不影响其他任务
✅ **代码审查友好** - 每个 Feature 独立一个 PR，清晰易懂
✅ **团队协作** - 不同开发者可以在不同的 worktree 中并行工作

**使用原版，如果：**

- 只需要规划单个任务
- 不需要并行开发
- 不需要 PRD 驱动的开发模式

## 快速开始

### 安装

```bash
claude plugins install Taoidle/planning-with-files
```

### 多任务并行开发示例

```bash
# === 终端 1: 开发用户认证功能 ===
/planning-with-files:hybrid-worktree feature-auth main "实现JWT认证和用户管理"
/planning-with-files:approve  # 选择 Auto 模式，自动执行所有 story
# ... 工作在隔离环境中 ...

# === 终端 2: 同时修复 API bug ===
/planning-with-files:hybrid-worktree fix-api-timeout main "修复API超时问题"
/planning-with-files:approve
# ... 同时进行，互不影响 ...

# === 终端 1 完成 ===
cd .worktree/feature-auth
/planning-with-files:hybrid-complete main  # 合并到 main 分支

# === 终端 2 完成 ===
cd .worktree/fix-api-timeout
/planning-with-files:hybrid-complete main
```

## Hybrid Ralph 工作流

这是本 fork 的核心功能 - 将复杂功能自动分解为可并行执行的 story。

### PRD 自动生成

```bash
# 描述你的功能，自动生成 PRD
/planning-with-files:hybrid-auto "实现用户认证系统，包括登录、注册、密码重置"
```

生成的 PRD 包含：
- **Goal**: 一句话目标
- **Stories**: 3-7 个用户故事
- **Dependencies**: Story 之间的依赖关系
- **Batches**: 自动计算并行执行批次

### 批次自动流转

**Auto Mode** (默认):
```
Batch 1 (3个story并行) → 完成 → 自动启动 Batch 2 → 完成 → 自动启动 Batch 3
```

**Manual Mode**:
```
Batch 1 完成 → 你审查 → 确认 → Batch 2 启动 → 完成 → 你审查 → 确认 → Batch 3
```

### 执行模式选择

| 模式 | 适用场景 | 控制粒度 |
|------|---------|---------|
| **Auto** | 日常开发、可信 PRD | 批次级自动 |
| **Manual** | 关键功能、需要仔细审查 | 批次级手动确认 |

**注意**: 两种模式下，agent 都会直接执行命令，不在命令级别打断你。

## 命令参考

### 核心命令

| 命令 | 说明 |
|------|------|
| `/planning-with-files:hybrid-worktree <name> <branch> <desc>` | **创建隔离的并行任务环境** |
| `/planning-with-files:approve` | **选择模式并执行 PRD** |
| `/planning-with-files:hybrid-complete [branch]` | **完成任务并合并** |
| `/planning-with-files:hybrid-status` | 查看执行状态 |
| `/planning-with-files:hybrid-auto <desc>` | 生成 PRD（非 worktree 模式） |

### Worktree 目录结构

```
.worktree/feature-auth/
├── [项目文件完整副本]
├── .git/                      # 独立的 Git 仓库
├── prd.json                   # 这个任务的需求分解
├── findings.md                # 研究发现
├── progress.txt               # 执行进度
├── .planning-config.json      # 任务元数据
└── .agent-outputs/            # 各个 story agent 的输出
```

## v2.7.9 更新

**新增:**
- 🚀 **Auto/Manual 执行模式** - 选择批次流转方式
- 📊 **模式选择对话框** - 启动时清晰选择执行模式

**修复:**
- 🔧 **Worktree 路径修复** - 规划文件不再误入根目录
- 🐛 **后台任务等待修复** - PRD 生成不再卡住
- 📝 **执行语义明确** - 模式只控制批次，不控制命令

## 文件结构

```
planning-with-files/
├── commands/                   # Claude Code 命令定义
│   ├── hybrid-worktree.md     # 创建并行任务环境
│   ├── approve.md              # 执行 PRD（含模式选择）
│   └── hybrid-complete.md      # 完成并合并
├── skills/hybrid-ralph/        # Hybrid Ralph 技能
│   └── commands/              # 技能命令
└── docs/                      # 文档
```

## 文档

| 文档 | 说明 |
|------|------|
| [CHANGELOG.md](CHANGELOG.md) | 详细更新日志 |
| [docs/installation.md](docs/installation.md) | 安装指南 |
| [docs/troubleshooting.md](docs/troubleshooting.md) | 常见问题 |

## 致谢

本项目基于以下优秀项目：

- **[OthmanAdi/planning-with-files](https://github.com/OthmanAdi/planning-with-files)** - 提供了核心的 3 文件规划模式、worktree 支持和基础框架

- **[snarktank/ralph](https://github.com/snarktank/ralph)** - 启发了 PRD 格式、progress.txt 模式和小任务分解方法，我们将其适配为 Hybrid Ralph 工作流

- **Manus AI** - 开创了上下文工程模式

- **Anthropic** - Claude Code、Agent Skills 和 Plugin 系统

## 贡献

欢迎贡献！请：
1. Fork 本仓库
2. 创建功能分支
3. 提交 Pull Request

## 许可证

MIT License — 自由使用、修改和分发

---

**项目地址**: [Taoidle/planning-with-files](https://github.com/Taoidle/planning-with-files)

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Taoidle/planning-with-files&type=Date)](https://star-history.com/#Taoidle/planning-with-files&Date)
