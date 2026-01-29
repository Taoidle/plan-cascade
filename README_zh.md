[English](README.md)

# Plan Cascade

> **三层级联的并行开发框架** — 从项目到功能到故事，层层分解、并行执行

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Claude Code Plugin](https://img.shields.io/badge/Claude%20Code-Plugin-blue)](https://claude.ai/code)
[![MCP Server](https://img.shields.io/badge/MCP-Server-purple)](https://modelcontextprotocol.io)
[![Version](https://img.shields.io/badge/version-4.1.1-brightgreen)](https://github.com/Taoidle/plan-cascade)
[![PyPI](https://img.shields.io/pypi/v/plan-cascade)](https://pypi.org/project/plan-cascade/)

---

## 概述

Plan Cascade 是一个**三层级联的 AI 并行开发框架**，专为大型软件项目设计。它将复杂项目逐层分解，通过多 Agent 协作实现高效的并行开发。

### 核心理念

- **层层分解**：项目 → 功能 → 故事，逐级细化任务粒度
- **并行执行**：无依赖的任务在同一批次中并行处理
- **多 Agent 协作**：根据任务特点自动选择最优 Agent
- **质量保障**：自动化质量门控 + 智能重试机制
- **状态追踪**：基于文件的状态共享，支持断点恢复

### 三层架构

| 层级 | 名称 | 职责 | 产物 |
|------|------|------|------|
| **Level 1** | Mega Plan | 项目级编排，管理多个 Feature | `mega-plan.json` |
| **Level 2** | Hybrid Ralph | 功能级开发，自动生成 PRD | `prd.json` |
| **Level 3** | Stories | 故事级执行，Agent 并行处理 | 代码变更 |

---

## 使用方式

| 方式 | 说明 | 适用场景 | 详细文档 |
|------|------|----------|----------|
| **Standalone CLI** | 独立命令行工具 | 任何终端环境 | [CLI Guide](docs/CLI-Guide_zh.md) |
| **Claude Code Plugin** | 原生集成，功能最完整 | Claude Code 用户 | [Plugin Guide](docs/Plugin-Guide_zh.md) |
| **Desktop App** | 图形化界面 | 偏好 GUI 的用户 | [Desktop Guide](docs/Desktop-Guide_zh.md) |
| **MCP Server** | 通过 MCP 协议集成 | Cursor, Windsurf 等 | [MCP Guide](docs/MCP-SERVER-GUIDE.md) |

---

## 快速开始

### Standalone CLI

```bash
# 安装
pip install plan-cascade

# 配置
plan-cascade config --setup

# 简单模式 - 一键执行
plan-cascade run "实现用户登录功能"

# 专家模式 - 更多控制
plan-cascade run "实现用户登录功能" --expert

# 交互式聊天
plan-cascade chat
```

### Claude Code Plugin

```bash
# 安装
claude plugins install Taoidle/plan-cascade

# 使用 - 自动模式（推荐新手使用）
/plan-cascade:auto "你的任务描述"

# 使用 - 手动选择模式
/plan-cascade:hybrid-auto "添加搜索功能"
/plan-cascade:approve --auto-run
```

### Desktop App

从 [GitHub Releases](https://github.com/Taoidle/plan-cascade/releases) 下载适合您平台的安装包。

---

## 核心功能

### 双模式设计

| 模式 | 适用场景 | 特点 |
|------|----------|------|
| **简单模式** | 新手用户、快速任务 | AI 自动判断策略并执行 |
| **专家模式** | 资深用户、精细控制 | PRD 编辑、Agent 指定、质量门控配置 |

### AI 自动策略判断

简单模式下，AI 根据需求自动选择执行策略：

| 输入类型 | 执行策略 |
|----------|----------|
| 小任务（如"添加按钮"） | 直接执行 |
| 中等功能（如"用户登录"） | Hybrid Auto |
| 大型项目（如"电商平台"） | Mega Plan |
| 需要隔离（如"实验性重构"） | Hybrid Worktree |

### 多 LLM 后端

| 后端 | 需要 API Key | 说明 |
|------|-------------|------|
| Claude Code | 否 | 默认，通过 Claude Code CLI |
| Claude Max | 否 | 通过 Claude Code 获取 LLM |
| Claude API | 是 | 直接调用 Anthropic API |
| OpenAI | 是 | GPT-4o 等 |
| DeepSeek | 是 | DeepSeek Chat/Coder |
| Ollama | 否 | 本地模型 |

### 多 Agent 协作

支持使用不同 Agent 执行 Story：

| Agent | 类型 | 说明 |
|-------|------|------|
| claude-code | task-tool | 内置，始终可用 |
| codex | cli | OpenAI Codex |
| aider | cli | AI 结对编程 |
| amp-code | cli | Amp Code |
| cursor-cli | cli | Cursor CLI |

### 质量门控

每个 Story 完成后自动运行质量验证：

| 门控 | 工具 |
|------|------|
| TypeCheck | tsc, mypy, pyright |
| Test | pytest, jest |
| Lint | eslint, ruff |
| Custom | 自定义脚本 |

---

## 命令快速参考

### CLI

```bash
plan-cascade run <description>          # 执行任务
plan-cascade run <description> --expert # 专家模式
plan-cascade chat                       # 交互式聊天
plan-cascade config --setup             # 配置向导
plan-cascade status                     # 查看状态
```

### Claude Code Plugin

```bash
# 自动模式 - AI 自动选择策略
/plan-cascade:auto <描述>               # 自动选择并执行最佳策略

# 项目级
/plan-cascade:mega-plan <描述>          # 生成项目计划
/plan-cascade:mega-approve --auto-prd   # 批准并全自动执行所有批次
/plan-cascade:mega-resume --auto-prd    # 恢复中断的 mega-plan
/plan-cascade:mega-complete             # 完成合并

# 功能级
/plan-cascade:hybrid-auto <描述>        # 生成 PRD
/plan-cascade:approve --auto-run        # 批准并自动执行
/plan-cascade:hybrid-resume --auto      # 恢复中断的任务
/plan-cascade:hybrid-complete           # 完成

# 通用
/plan-cascade:edit                      # 编辑 PRD
/plan-cascade:status                    # 查看状态
```

---

## 项目结构

```
plan-cascade/
├── src/plan_cascade/       # Python 核心包
│   ├── core/               # 编排引擎
│   ├── backends/           # 后端抽象
│   ├── llm/                # LLM 提供者
│   ├── tools/              # 工具执行
│   ├── settings/           # 设置管理
│   └── cli/                # CLI 入口
├── .claude-plugin/         # Plugin 配置
├── commands/               # Plugin 命令
├── skills/                 # Plugin 技能
├── mcp_server/             # MCP 服务器
├── desktop/                # Desktop 应用
└── docs/                   # 文档
    ├── CLI-Guide.md
    ├── Plugin-Guide.md
    ├── Desktop-Guide.md
    └── MCP-SERVER-GUIDE.md
```

---

## 文档索引

| 文档 | 说明 |
|------|------|
| [CLI Guide](docs/CLI-Guide_zh.md) | CLI 详细使用指南 |
| [Plugin Guide](docs/Plugin-Guide_zh.md) | Claude Code 插件详细指南 |
| [Desktop Guide](docs/Desktop-Guide_zh.md) | Desktop 应用指南 |
| [MCP Server Guide](docs/MCP-SERVER-GUIDE.md) | MCP 服务器配置指南 |
| [System Architecture](docs/System-Architecture_zh.md) | 系统架构与流程设计（含流程图） |
| [Design Document](docs/Design-Plan-Cascade-Standalone_zh.md) | 技术设计文档 |
| [PRD Document](docs/PRD-Plan-Cascade-Standalone_zh.md) | 产品需求文档 |

---

## 更新日志

### v4.1.1

- **Mega-Approve 全自动化** - 修复 `/plan-cascade:mega-approve --auto-prd` 不能完全自动执行的问题
  - 现在可以全自动执行整个 mega-plan，无需手动干预
  - 仅在出错或合并冲突时暂停
- **恢复命令** - 新增中断恢复命令
  - `/plan-cascade:mega-resume` - 恢复中断的 mega-plan 执行
  - `/plan-cascade:hybrid-resume` - 恢复中断的 hybrid 任务
  - 自动从文件检测状态，跳过已完成的工作
  - 兼容新旧两种进度标记格式

### v4.1.0

- **自动策略命令** - 新增 `/plan-cascade:auto` 命令
  - AI 自动分析任务并选择最佳策略
  - 支持 4 种策略：direct 直接执行、hybrid-auto、hybrid-worktree、mega-plan
  - 基于关键词检测（非字数判断）
  - 无需用户确认，直接执行

### v4.0.0

- **Standalone CLI 完成** - 独立命令行工具全功能可用
  - 简单模式/专家模式双模式支持
  - 交互式 REPL 聊天模式
  - AI 自动策略判断
- **多 LLM 后端** - 支持 5 种 LLM 提供者
  - Claude Max（无需 API Key）
  - Claude API、OpenAI、DeepSeek、Ollama
- **独立 ReAct 引擎** - 完整的 Think→Act→Observe 循环
- **文档重构** - 拆分为独立的使用指南

### v3.x

- **MCP 服务器** - 支持 Cursor、Windsurf 等
- **多 Agent 协作** - Codex、Aider 等
- **自动迭代循环** - 质量门控、智能重试
- **Mega Plan** - 项目级多功能编排

完整更新日志见 [CHANGELOG.md](CHANGELOG.md)

---

## 项目起源

本项目 fork 自 [OthmanAdi/planning-with-files](https://github.com/OthmanAdi/planning-with-files)（v2.7.1），在其 Manus 风格的文件规划基础上，大幅扩展了功能。

---

## 致谢

- **[OthmanAdi/planning-with-files](https://github.com/OthmanAdi/planning-with-files)** - 原始项目
- **[snarktank/ralph](https://github.com/snarktank/ralph)** - PRD 格式启发
- **Anthropic** - Claude Code、Plugin 系统和 MCP 协议

---

## 许可证

MIT License

---

**项目地址**: [Taoidle/plan-cascade](https://github.com/Taoidle/plan-cascade)

[![Star History Chart](https://api.star-history.com/svg?repos=Taoidle/plan-cascade&type=Date)](https://star-history.com/#Taoidle/plan-cascade&Date)
