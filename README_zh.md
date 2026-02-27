[English](README.md)

<div align="center">

# Plan Cascade

**AI 驱动的级联开发框架**

*将复杂项目分解为可并行执行的任务，支持多 Agent 协作*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-4.4.0-brightgreen)](https://github.com/Taoidle/plan-cascade)
[![Claude Code](https://img.shields.io/badge/Claude%20Code-Plugin-blue)](https://claude.ai/code)
[![MCP](https://img.shields.io/badge/MCP-Server-purple)](https://modelcontextprotocol.io)

| 组件 | 状态 |
|------|------|
| Claude Code 插件 | ![稳定](https://img.shields.io/badge/状态-稳定-brightgreen) |
| MCP 服务器 | ![稳定](https://img.shields.io/badge/状态-稳定-brightgreen) |
| 独立 CLI | ![开发中](https://img.shields.io/badge/状态-开发中-yellow) |
| 桌面应用 | ![Beta](https://img.shields.io/badge/状态-Beta-blue) |

[功能特性](#功能特性) • [快速开始](#快速开始) • [文档](#文档) • [架构](#架构)

</div>

---

## 为什么选择 Plan Cascade？

传统 AI 编程助手在处理大型复杂项目时力不从心。Plan Cascade 通过以下方式解决这个问题：

- **分解复杂性** — 自动将项目分解为可管理的 Story
- **并行执行** — 使用多个 Agent 同时执行独立任务
- **保持上下文** — 设计文档/PRD + 执行上下文（含持久化工具日志）可抵御压缩/截断
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
- **AI 验证门** - 验证实现是否符合验收标准并检测骨架代码

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

# 首次设置（推荐，尤其是 Windows 用户）
/plan-cascade:init

# 让 AI 选择最佳策略
/plan-cascade:auto "构建一个带用户认证和 JWT 的 REST API"
# → 默认 FULL flow（spec auto + TDD on + 确认）。可用 --flow/--tdd/--no-confirm 覆盖。

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

基于 **Tauri 2.0** 构建的跨平台桌面应用（Rust 后端 + React 前端）。提供完整的 GUI 界面，涵盖所有 Plan Cascade 能力，以及多提供商 LLM 对话、知识库 (RAG)、分析仪表板等独立功能。

```bash
cd desktop
pnpm install
pnpm tauri:dev        # 开发模式，支持热重载
pnpm tauri:build      # 生产构建（当前平台）
```

详见 [Desktop README](desktop/README_zh-CN.md)，或跳转至下方[桌面应用](#桌面应用)章节。

## 使用示例

> **说明**：`/plan-cascade:auto` 默认使用 **FULL** flow（spec auto + TDD on + 确认）。如需更快执行，可用 `--flow standard|quick`、`--tdd auto|off`、或 `--no-confirm` 覆盖。

### 简单任务（快速直接执行）
```bash
/plan-cascade:auto --flow quick "修复登录按钮的拼写错误"
# → quick flow 下无需规划，直接执行
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

## 桌面应用

桌面应用是独立于 Python 核心运行的 AI 编程平台，拥有自己的纯 Rust 后端和丰富的 GUI 界面。

### 执行模式

| 模式 | 描述 |
|------|------|
| **Claude Code** | Claude Code CLI 的交互式 GUI，支持工具可视化 |
| **Simple** | 直接 LLM 对话，支持 Agent 工具调用（文件编辑、Shell、搜索） |
| **Expert** | 访谈驱动的 PRD 生成，带依赖关系图可视化 |
| **Task** | PRD 驱动的自主多故事执行，含质量门禁 |
| **Plan** | 多功能 Mega 计划编排 |

### LLM 提供商

对接 **7+ 提供商**，支持流式响应和智能工具调用降级：

| 提供商 | 工具调用 | 本地 |
|--------|:---:|:---:|
| Anthropic (Claude) | 原生 | |
| OpenAI (GPT) | 原生 | |
| DeepSeek | 双通道 | |
| 通义千问 (阿里巴巴) | 双通道 | |
| 智谱 GLM | 双通道 | |
| Ollama | 仅提示词 | 是 |
| MiniMax | 仅提示词 | |

### 核心功能

- **智能体库** — 创建可复用的 AI 智能体，支持自定义提示词、工具约束和执行历史
- **质量门禁** — 每次代码生成后自动运行测试、代码检查和类型检查
- **时间线与检查点** — 会话级版本控制，支持分支、分叉和一键回滚
- **Git 集成** — 完整的暂存、提交、分支、合并、冲突解决 GUI，支持 AI 辅助提交信息（46 个 git 命令）
- **知识库 (RAG)** — 基于 HNSW 向量索引和多提供商 Embedding 的语义文档搜索
- **代码库索引** — Tree-sitter 符号提取（6 种语言），后台索引 + 语义搜索
- **MCP 集成** — Model Context Protocol 服务器管理和自定义工具注册
- **分析仪表板** — Token 使用量、费用追踪、模型性能对比，支持 CSV/JSON 导出
- **智能体编排器** — 可视化画布编辑器，支持多步 Agent 流水线（顺序、并行、条件）
- **图工作流** — 基于 DAG 的工作流编辑器，可拖拽节点和 SVG 连线
- **插件** — 按框架注入技能（React、Vue、Rust），支持市场浏览
- **护栏** — 敏感数据检测、代码安全和自定义正则规则
- **Webhooks** — 事件路由到飞书、Slack、Discord、Telegram 或自定义端点
- **远程控制** — Telegram Bot 网关和 A2A（Agent-to-Agent）协议
- **国际化** — 英文、中文（简体）、日文

### 技术栈

| 层级 | 技术 |
|------|------|
| 前端 | React 18 + TypeScript + Zustand + Radix UI + Tailwind CSS + Monaco Editor |
| 后端 | Rust + Tauri 2.0 + Tokio + SQLite + AES-256-GCM 密钥存储 |
| 代码分析 | Tree-sitter (Python, Rust, TypeScript, JavaScript, Go, Java) |
| 向量搜索 | HNSW (hnsw_rs) + BM25 重排序的混合搜索 |

### 构建目标

```bash
pnpm tauri:build:macos      # macOS Universal (Intel + Apple Silicon)
pnpm tauri:build:windows    # Windows x64 MSI
pnpm tauri:build:linux      # Linux x64 AppImage
```

架构详情、开发环境搭建和贡献指南请参阅 [Desktop README](desktop/README_zh-CN.md)。

---

## 文档

| 文档 | 说明 |
|------|------|
| [插件指南](docs/Plugin-Guide_zh.md) | Claude Code 插件使用 |
| [CLI 指南](docs/CLI-Guide_zh.md) | 独立 CLI 使用 |
| [桌面应用指南](docs/Desktop-Guide_zh.md) | 桌面应用 |
| [Desktop README](desktop/README_zh-CN.md) | 桌面端开发与架构 |
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
├── external-skills/        # 框架技能 (React, Vue, Rust)
└── desktop/                # 桌面应用 (Tauri 2.0 + React 18)
    ├── src/                #   React 前端 (283 个组件)
    │   ├── components/     #     按领域组织 (23 个功能区)
    │   ├── store/          #     Zustand 状态管理 (39 个 store)
    │   └── lib/            #     Tauri IPC 封装 (30+ 文件)
    └── src-tauri/          #   Rust 后端
        ├── src/commands/   #     359 个 IPC 命令 (43 个模块)
        ├── src/services/   #     业务逻辑层 (150+ 文件)
        └── crates/         #     工作空间 crate (core, llm, tools, quality-gates)
```

### 支持的 LLM 后端

| 后端 | 需要 API Key | 备注 |
|------|-------------|------|
| Claude Code | 否 | 默认，通过 Claude Code CLI |
| Claude API | 是 | 直接调用 Anthropic API |
| OpenAI | 是 | GPT-4o 等 |
| DeepSeek | 是 | DeepSeek Chat/Coder |
| Ollama | 否 | 本地模型 |

## v4.4.0 更新内容

- **hybrid-worktree `--agent` 参数** — PRD 生成代理选择：`/plan-cascade:hybrid-worktree task branch "desc" --agent=codex`
- **Spec 访谈（可选）** — 规划期 `spec.json/spec.md` 工作流，并编译为 PRD
- **通用恢复** — `/plan-cascade:resume` 自动识别模式并路由到对应 resume 命令
- **Dashboard + Gates** — 聚合状态视图 + DoR/DoD/TDD 质量门控
- **抗压缩会话日志** — 最近工具活动会持久化到 `.state/claude-session/`，并在 `.hybrid-execution-context.md` / `.mega-execution-context.md` 中展示
- **更安全的 Auto 默认配置** — `/plan-cascade:auto` 默认 FULL flow，并默认启用 `--spec auto`、`--tdd on` 和确认（可通过 `--flow`、`--tdd`、`--no-confirm` 覆盖）

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
