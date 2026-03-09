[English](README.md)

<div align="center">

# Plan Cascade

**AI 驱动的级联开发框架**

*将复杂项目分解为可并行执行的任务，支持多 Provider 执行*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Version](https://img.shields.io/badge/version-4.4.0-brightgreen)](https://github.com/Taoidle/plan-cascade)
[![Claude Code](https://img.shields.io/badge/Claude%20Code-Plugin-blue)](https://claude.ai/code)
[![MCP](https://img.shields.io/badge/MCP-Server-purple)](https://modelcontextprotocol.io)

| 组件 | 状态 |
|------|------|
| Claude Code 插件 | ![稳定](https://img.shields.io/badge/状态-稳定-brightgreen) |
| MCP 服务器 | ![稳定](https://img.shields.io/badge/状态-稳定-brightgreen) |
| 独立 CLI | ![开发中](https://img.shields.io/badge/状态-开发中-yellow) |
| 桌面应用 | ![Alpha](https://img.shields.io/badge/状态-Alpha-red) |

[功能特性](#功能特性) • [快速开始](#快速开始) • [文档](#文档) • [架构](#架构)

</div>

---

## 为什么选择 Plan Cascade？

传统 AI 编程助手在处理大型复杂项目时力不从心。Plan Cascade 通过以下方式解决这个问题：

- **分解复杂性** — 自动将项目分解为可管理的 Story
- **并行执行** — 使用多个 Agent 同时执行独立任务
- **保持上下文** — 设计文档/PRD + 执行上下文（含持久化工具日志）可��御压缩/截断
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
│  支持多 Provider 的并行 Story 执行                              │
│  可配置的 LLM Provider 选择                                   │
│  输出: 代码变更                                              │
└─────────────────────────────────────────────────────────────┘
```

### 核心运行模式

| 模式 | 说明 | 最适用于 |
|------|------|----------|
| **Chat** | 轻量级聊天界面，用于简单问答 | 快速问答、文件查看、简单任务 |
| **Plan** | 域自适应任务拆解，支持 Steps | 内容创作、研究、多步骤工作流 |
| **Task** | PRD 驱动的代码开发，包含 Stories 和质量门禁 | 功能开发、复杂实现 |

### Workflow Kernel

三种模式都由统一的 **Workflow Kernel** 管理，提供：

- **统一会话生命周期** — 跨模式的一致性状态管理
- **类型化事件流** — 实时进度更新
- **模式切换** — Chat/Plan/Task 之间无缝切换
- **轻量级检查点** — 支持中断后恢复

### Plan 模式 — 域自适应任务拆解

**Plan 模式**提供域无关的任务拆解框架：

- **域适配器** — 针对不同任务类型的专门处理器（通用、写作、研究、营销、数据分析等）
- **Step 拆解** — 将任务拆解为可执行的 Steps 及依赖关系
- **批次执行** — 并行执行独立的 Steps
- **输出验证** — 验证 Step 输出是否符合验收标准

### Task 模式 — PRD 驱动的代码开发

**Task 模式**是最强大的模式，实现完整的软件工程方法论：

- **PRD 生成** — 根据任务描述自动生成产品需求文档
- **Story 拆解** — 将需求拆解���可执行的 Stories 及依赖关系
- **Kahn 算法** — 拓扑排序优化批次执行顺序
- **7 级 Agent 优先级** — 智能 Agent 选择（全局 > 阶段 > Story > 推断 > 默认 > 回退 > claude-code）
- **完整质量门禁** — 每个 Story 后的完整验证流水线：
  - DoR (Definition of Ready)
  - DoD (Definition of Done)
  - AI 验证（检测骨架代码）
  - 代码审查（质量评分）
  - TDD 合规
- **自动重试** — 失败时指数退避重试（5s → 10s → 20s）

### 多 Provider 执行

Plan Cascade 支持为 Story 执行配置不同的 LLM Provider。在设置 → 阶段代理 中配置。

| Provider | 类型 | 最适用于 |
|----------|------|----------|
| `claude-sonnet` | LLM (默认) | 平衡能力和速度 |
| `claude-opus` | LLM | 复杂任务，最强能力 |
| `claude-haiku` | LLM | 简单任务，最快响应 |

支持的 LLM Provider：Anthropic、OpenAI、Ollama、DeepSeek。

**注意**：CLI 工具执行（codex, aider）支持尚未实现。

### Story 执行模式

| 模式 | 说明 |
|------|------|
| `LLM` | 通过 OrchestratorService 使用直接 LLM API（默认） |
| `CLI` | 使用外部 CLI 工具（尚未实现） |

Story 执行模式根据是否配置了 LLM Provider 自动确定。

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
- **AI 验证门** — 验证实现是否符合验收标准并检测骨架代码
- **代码审查门** — AI 驱动的代码审查和质量评分
- **TDD 合规门** — 确保代码变更伴随测试变更

### 桌面应用功能

桌面应用提供完整的 GUI，包含 **50+ Tauri 命令**，涵盖：

| 类别 | 命令 |
|------|------|
| **Task 模式** | 进入/退出 Task 模式、生成/审批 PRD、执行状态、取消/报告 |
| **Plan 模式** | 计划生成、会话分析、生命周期报告 |
| **Git 操作** | 工作树管理、分支操作、提交历史 |
| **内存** | 基于可配置嵌入提供商的语义存储和检索 |
| **知识库** | 项目知识管理，混合搜索（HNSW + FTS5） |
| **代码库索引** | 基于 LSP 的代码智能、语义搜索 |
| **技能** | 外部框架技能加载（React、Vue、Rust 等） |
| **MCP** | 模型上下文协议服务器集成 |
| **插件** | 可扩展插件架构 |
| **设置** | 用户偏好管理 |
| **Webhook** | 外部回调 |
| **评估** | 代码质量评估 |

### 记忆系统

桌面应用包含一个复杂的**跨会话持久化记忆系统**，能够从用户交互和项目上下文中学习：

| 功能 | 描述 |
|------|------|
| **存储** | 基于 SQLite 的持久化存储，支持 TF-IDF 向量嵌入 |
| **作用域** | 项目级、全局级（跨项目）、会话级记忆 |
| **检索** | 4信号混合排序（嵌入 + 关键词 + 重要性 + 最近访问） |
| **类别** | 偏好、约定、模式、纠正、事实 |
| **提取** | LLM 驱动的对话自动记忆提取 |
| **命令** | 显式记忆命令：`remember that...`、`forget about...`、`what do you remember about...` |
| **维护** | 自动衰减、修剪和压缩 |

**记忆类别：**

| 类别 | 描述 |
|------|------|
| `Preference` | 用户偏好和习惯 |
| `Convention` | 项目特定的约定 |
| `Pattern` | 代码库中发现的模式 |
| `Correction` | 开发过程中做的纠正 |
| `Fact` | 关于项目的事实性知识 |

**显式记忆命令：**
- `remember that [内容]` — 存储新记忆
- `forget about [主题]` — 删除关于某主题的记忆
- `what do you remember about [主题]` — 查询关于某主题的记忆

**全局记忆：**
标记为 `__global__` 的记忆会在所有项目和会话之间共享，实现用户偏好的持久化。

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

### 方式二：桌面应用（Alpha）

> **注意**：桌面应用目前处于 **Alpha** 阶段，核心能力已完成，但部分功能可能不稳定。

基于 **Tauri 2.0** 构建，Rust 后端 + React 前端：

```bash
# 从源码构建
cd desktop
pnpm install
pnpm tauri dev

# 或运行构建后的应用（构建后）
pnpm tauri build
```

### 方式三：独立 CLI

> **注意**：独立 CLI 目前正在积极开发中，部分功能可能不完整或不稳定。生产环境建议使用 Claude Code 插件。

```bash
# 安装
pip install plan-cascade

# 配置
plan-cascade config --setup

# 自动策略运行
...
```

## 文档

| 文档 | 说明 |
|------|------|
| [架构设计](desktop/docs/architecture-design_zh.md) | 整体系统架构 |
| [内核设计](desktop/docs/kernel-design_zh.md) | 工作流内核、Simple Plan/Task 生产规范 |
| [内存与技能设计](desktop/docs/memory-skill-design_zh.md) | 内存和技能架构 |
| [代码库索引设计](desktop/docs/codebase-index-design_zh.md) | HNSW/FTS5/LSP 索引设计 |
| [开发者指南](desktop/docs/developer-guide-v2_zh.md) | 开发环境搭建、项目结构 |
| [API 参考](desktop/docs/api-reference-v2_zh.md) | Tauri 命令参考 |

## 架构

### 桌面应用架构

```
┌─────────────────────────────────────────────────────────────┐
│                     前端 (React + TypeScript)                 │
├─────────────────────────────────────────────────────────────┤
│  110+ Zustand Store  │  React Query  │  Radix UI        │
└─────────────────────────────────────────────────────────────┘
                              │
                    Tauri IPC (50+ 命令)
                              │
┌─────────────────────────────────────────────────────────────┐
│                   后端 (Rust + Tauri 2.0)                    │
├─────────────────────────────────────────────────────────────┤
│  Core Crate    │  LLM Crate   │  Quality Gates  │  Tools   │
│  上下文/      │  OpenAI/    │  Detector/     │  Executor│
│  事件/       │  Anthropic/ │  Validator/   │  Traits  │
│  流式处理      │  DeepSeek/   │  Pipeline     │          │
│                │  Ollama/...   │               │          │
└─────────────────────────────────────────────────────────────┘
```

### 支持的 LLM 提供商

| 提供商 | 模型 |
|--------|------|
| OpenAI | GPT-4o, GPT-4o Mini |
| Anthropic | Claude Sonnet, Claude Haiku |
| DeepSeek | DeepSeek Chat |
| GLM | 智谱 AI (对话 + 嵌入) |
| MiniMax | M2, M2.1, M2.5 (通过 Anthropic 兼容 API) |
| Ollama | 本地模型 |
| Qwen | 通义千问 (对话 + 嵌入) |

### 搜索能力

- **语义搜索** — 可配置的嵌入提供商 (OpenAI, Qwen, GLM, Ollama, TF-IDF)
- **混合搜索** — HNSW + FTS5 组合
- **基于 LSP** — 语言服务器协议的代码智能

## 许可证

MIT 许可证 - 详见 [LICENSE](LICENSE)。
