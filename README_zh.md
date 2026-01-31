[English](README.md)

<div align="center">

# Plan Cascade

**AI 驱动的级联开发框架**

*将复杂项目分解为可并行执行的任务，支持多 Agent 协作*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-4.2.1-brightgreen)](https://github.com/Taoidle/plan-cascade)
[![Claude Code](https://img.shields.io/badge/Claude%20Code-Plugin-blue)](https://claude.ai/code)
[![MCP](https://img.shields.io/badge/MCP-Server-purple)](https://modelcontextprotocol.io)

| 组件 | 状态 |
|------|------|
| Claude Code 插件 | ![稳定](https://img.shields.io/badge/状态-稳定-brightgreen) |
| MCP 服务器 | ![稳定](https://img.shields.io/badge/状态-稳定-brightgreen) |
| 独立 CLI | ![开发中](https://img.shields.io/badge/状态-开发中-yellow) |
| 桌面应用 | ![开发中](https://img.shields.io/badge/状态-开发中-yellow) |

[功能特性](#功能特性) • [快速开始](#快速开始) • [文档](#文档) • [架构](#架构)

</div>

---

## 为什么选择 Plan Cascade？

传统 AI 编程助手在处理大型复杂项目时力不从心。Plan Cascade 通过以下方式解决这个问题：

- **分解复杂性** — 自动将项目分解为可管理的 Story
- **并行执行** — 使用多个 Agent 同时执行独立任务
- **保持上下文** — 设计文档和 PRD 让 AI 始终聚焦于架构
- **质量保障** — 每一步都有自动化测试和代码检查

## 功能特性

### 三层级联架构

```
┌─────────────────────────────────────────────────────────────┐
│  第1层: Mega Plan                                           │
│  ────────────────                                           │
│  项目级编排                                                  │
│  管理多个 Feature 的并行批次                                 │
│  输出: mega-plan.json + design_doc.json                     │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  第2层: Hybrid Ralph (Feature)                              │
│  ─────────────────────────────                              │
│  功能级开发                                                  │
│  自动生成包含用户故事的 PRD                                  │
│  输出: prd.json + design_doc.json                           │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  第3层: Story 执行                                          │
│  ─────────────────                                          │
│  支持多 Agent 的并行 Story 执行                              │
│  根据任务类型自动选择 Agent                                  │
│  输出: 代码变更                                              │
└─────────────────────────────────────────────────────────────┘
```

### 多 Agent 协作

| Agent | 类型 | 最适用于 |
|-------|------|----------|
| `claude-code` | 内置 | 通用任务（默认） |
| `codex` | CLI | Bug 修复、快速实现 |
| `aider` | CLI | 重构、代码改进 |
| `amp-code` | CLI | 替代实现方案 |

Agent 可根据 Story 类型自动选择，也可手动指定。

### 自动生成设计文档

Plan Cascade 会自动生成与 PRD 配套的技术设计文档：

- **项目级**: 架构、模式、跨功能决策
- **功能级**: 组件设计、API、Story 映射
- **继承机制**: 功能级文档继承项目级上下文

### 质量门控

每个 Story 完成后自动验证：
- TypeScript/Python 类型检查
- 单元测试和集成测试
- 代码检查（ESLint、Ruff）
- 自定义验证脚本

### 外部框架技能

Plan Cascade 内置框架特定技能，可自动检测并注入：

| 框架 | 技能 | 自动检测 |
|------|------|----------|
| React/Next.js | `react-best-practices`, `web-design-guidelines` | `package.json` 包含 `react` 或 `next` |
| Vue/Nuxt | `vue-best-practices`, `vue-router-best-practices`, `vue-pinia-best-practices` | `package.json` 包含 `vue` 或 `nuxt` |
| Rust | `rust-coding-guidelines`, `rust-ownership`, `rust-error-handling`, `rust-concurrency` | 存在 `Cargo.toml` |

技能从 Git 子模块加载，在 Story 执行期间提供框架特定的指导：

```bash
# 初始化外部技能（首次使用）
git submodule update --init --recursive

# 在 React 项目中，技能会自动检测：
/plan-cascade:auto "添加用户资料组件"
# → 自动将 React 最佳实践注入上下文
```

## 快速开始

### 方式一：Claude Code 插件（推荐）

```bash
# 安装插件
claude plugins install Taoidle/plan-cascade

# 让 AI 选择最佳策略
/plan-cascade:auto "构建一个带用户认证和 JWT 的 REST API"

# 或手动选择
/plan-cascade:hybrid-auto "添加密码重置功能"
/plan-cascade:approve --auto-run
```

### 方式二：独立 CLI

> **注意**：独立 CLI 目前正在积极开发中，部分功能可能不完整或不稳定。生产环境建议使用 Claude Code 插件。

```bash
# 安装
pip install plan-cascade

# 配置
plan-cascade config --setup

# 自动策略运行
plan-cascade run "实现用户认证"

# 或使用专家模式获得更多控制
plan-cascade run "实现用户认证" --expert
```

### 方式三：桌面应用

> **注意**：桌面应用目前正在积极开发中，敬请期待。

从 [GitHub Releases](https://github.com/Taoidle/plan-cascade/releases) 下载（即将推出）。

## 使用示例

### 简单任务（直接执行）
```bash
/plan-cascade:auto "修复登录按钮的拼写错误"
# → 无需规划，直接执行
```

### 中等功能（Hybrid Auto）
```bash
/plan-cascade:auto "实现 Google 和 GitHub 的 OAuth2 登录"
# → 生成包含 3-5 个 Story 的 PRD，并行执行
```

### 大型项目（Mega Plan）
```bash
/plan-cascade:auto "构建电商平台：用户、商品、购物车、订单"
# → 创建包含 4 个 Feature 的 mega-plan，每个都有独立 PRD
```

### 使用外部设计文档
```bash
/plan-cascade:mega-plan "构建博客平台" ./architecture.md
# → 转换你的设计文档并用于指导开发
```

### 指定特定 Agent
```bash
/plan-cascade:approve --impl-agent=aider --retry-agent=codex
# → 实现阶段使用 aider，重试阶段使用 codex
```

## 文档

| 文档 | 说明 |
|------|------|
| [插件指南](docs/Plugin-Guide_zh.md) | Claude Code 插件使用 |
| [CLI 指南](docs/CLI-Guide_zh.md) | 独立 CLI 使用 |
| [桌面应用指南](docs/Desktop-Guide_zh.md) | 桌面应用 |
| [MCP 服务器指南](docs/MCP-SERVER-GUIDE.md) | 集成 Cursor、Windsurf |
| [系统架构](docs/System-Architecture_zh.md) | 技术架构 |

## 架构

### 文件结构

```
plan-cascade/
├── src/plan_cascade/       # Python 核心
│   ├── core/               # 编排引擎
│   ├── backends/           # Agent 抽象
│   ├── llm/                # LLM 提供者
│   └── cli/                # CLI 入口
├── commands/               # 插件命令
├── skills/                 # 插件技能
├── mcp_server/             # MCP 服务器
└── desktop/                # 桌面应用（Tauri + React）
```

### 支持的 LLM 后端

| 后端 | 需要 API Key | 备注 |
|------|-------------|------|
| Claude Code | 否 | 默认，通过 Claude Code CLI |
| Claude API | 是 | 直接调用 Anthropic API |
| OpenAI | 是 | GPT-4o 等 |
| DeepSeek | 是 | DeepSeek Chat/Coder |
| Ollama | 否 | 本地模型 |

## v4.2.1 更新内容

- **上下文恢复** — 自动生成上下文文件，在会话中断后恢复 AI 执行状态
- **增强技能检测** — 外部框架技能的详细输出和摘要显示
- **改进的钩子** — PreToolUse/PostToolUse 钩子现在自动更新上下文文件

## v4.2.0 更新内容

- **设计文档系统** — 自动生成两级层次结构的技术设计文档
- **多 Agent 集成** — 在所有执行模式中完整集成
- **AI 策略选择** — 智能任务分析取代关键词匹配
- **外部设计导入** — 支持转换 Markdown/JSON/HTML 设计文档

完整历史记录见 [CHANGELOG.md](CHANGELOG.md)。

## 贡献

欢迎贡献！提交 PR 前请先阅读贡献指南。

1. Fork 仓库
2. 创建功能分支（`git checkout -b feature/amazing-feature`）
3. 提交更改（`git commit -m 'Add amazing feature'`）
4. 推送分支（`git push origin feature/amazing-feature`）
5. 创建 Pull Request

## 致谢

- [OthmanAdi/planning-with-files](https://github.com/OthmanAdi/planning-with-files) — 原始灵感
- [snarktank/ralph](https://github.com/snarktank/ralph) — PRD 格式
- [Anthropic](https://www.anthropic.com/) — Claude Code 和 MCP 协议
- [vercel-labs/agent-skills](https://github.com/vercel-labs/agent-skills) — React/Next.js 最佳实践技能
- [vuejs-ai/skills](https://github.com/vuejs-ai/skills) — Vue.js 最佳实践技能
- [actionbook/rust-skills](https://github.com/actionbook/rust-skills) — Rust 元认知框架技能

## 许可证

[MIT License](LICENSE)

---

<div align="center">

**[GitHub](https://github.com/Taoidle/plan-cascade)** • **[Issues](https://github.com/Taoidle/plan-cascade/issues)** • **[Discussions](https://github.com/Taoidle/plan-cascade/discussions)**

[![Star History Chart](https://api.star-history.com/svg?repos=Taoidle/plan-cascade&type=Date)](https://star-history.com/#Taoidle/plan-cascade&Date)

</div>
