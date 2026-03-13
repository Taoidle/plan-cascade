<p align="center">
  <img src="../assets/plan-cascade-logo.png" alt="Plan Cascade" width="200">
</p>

<h1 align="center">Plan Cascade Desktop</h1>

<p align="center">
  <strong>本地优先的 AI 开发工作站</strong>
</p>

<p align="center">
  Chat · Plan · Task · Debug — 四种工作模式，统一内核驱动
</p>

<p align="center">
  <a href="#-核心特性">核心特性</a> •
  <a href="#-为什么选择-desktop">为什么选择 Desktop</a> •
  <a href="#-快速开始">快速开始</a> •
  <a href="#-技术栈">技术栈</a> •
  <a href="#-架构深度解析">架构</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Version-0.1.0-orange?style=flat-square" alt="Version">
  <img src="https://img.shields.io/badge/Tauri-2.0-blue?style=flat-square" alt="Tauri">
  <img src="https://img.shields.io/badge/Rust-1.75+-orange?style=flat-square" alt="Rust">
  <img src="https://img.shields.io/badge/React-18.3-61dafb?style=flat-square" alt="React">
  <img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="License">
  <img src="https://img.shields.io/badge/Status-Alpha-yellow?style=flat-square" alt="Status">
</p>

---

## 🎯 核心特性

### 四种工作模式

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Workflow Kernel (SSOT)                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                      Unified State Kernel                              │  │
│  │   ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐             │  │
│  │   │  Chat   │   │  Plan   │   │  Task   │   │  Debug  │             │  │
│  │   │  对话   │   │  计划   │   │  任务   │   │  调试   │             │  │
│  │   └─────────┘   └─────────┘   └─────────┘   └─────────┘             │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                    ↕ HandoffContext (跨模式上下文交接)                       │
└─────────────────────────────────────────────────────────────────────────────┘
```

| 模式 | 用途 | 生命周期 |
|------|------|----------|
| **Chat** | 自由对话、快速问答 | ready → streaming → paused/failed |
| **Plan** | 结构化多步骤执行 | idle → planning → executing → completed |
| **Task** | 完整开发工作流（需求→设计→实现） | idle → interviewing → generating_prd → executing |
| **Debug** | 专业问题诊断和修复 | intaking → hypothesizing → testing → patching → verifying |

### 核心能力

| 能力 | 说明 |
|------|------|
| **统一内核 (SSOT)** | 所有模式共享单一状态源，消除前端状态漂移 |
| **跨模式上下文交接** | Chat → Task 自动导入对话；Task → Chat 结构化摘要回传 |
| **多 LLM 后端** | Claude / OpenAI / DeepSeek / GLM / Qwen / MiniMax / Ollama |
| **离线支持** | Ollama 本地模型，无需联网 |

---

## 🚀 为什么选择 Desktop？

### 与其他 Agent 工具的对比

| 能力 | Plan Cascade Desktop | Cursor | GitHub Copilot | Claude Code |
|------|---------------------|--------|----------------|-------------|
| **多模式工作流** | ✅ 4种模式无缝切换 | ❌ 单一对话 | ❌ 单一补全 | ❌ 单一对话 |
| **跨模式上下文** | ✅ 自动交接 | ❌ | ❌ | ❌ |
| **质量门禁流水线** | ✅ 企业级 + 自动重试 | ❌ | ❌ | ❌ |
| **安全模型** | ✅ 5层 (护栏→门禁→策略→沙箱→审计) | 基础 | 基础 | 基础 |
| **可成长知识系统** | ✅ 技能 + 记忆 + RAG | ❌ | ❌ | ❌ |
| **远程控制** | ✅ A2A + Telegram | ❌ | ❌ | ❌ |
| **MCP 全栈** | ✅ Manager + Client + Server | 部分 | ❌ | Client only |
| **多 LLM 后端** | ✅ 7+ 提供商 | 部分 | ❌ | 仅 Claude |
| **离线使用** | ✅ Ollama | ❌ | ❌ | ❌ |

### 核心差异化优势

<details>
<summary><b>🔒 企业级安全模型</b></summary>

```
┌─────────────────────────────────────────────────────────────┐
│                    5层安全防护架构                           │
├─────────────────────────────────────────────────────────────┤
│  Layer 1: Guardrail        - 敏感数据检测与脱敏             │
│  Layer 2: Permission Gate   - 操作审批（可配置阈值）         │
│  Layer 3: Policy Engine v2  - 细粒度访问控制                │
│  Layer 4: Sandbox           - 文件系统隔离                  │
│  Layer 5: Audit Log         - 完整操作审计追踪              │
└─────────────────────────────────────────────────────────────┘
```

- 自动检测 API 密钥、密码、令牌
- 代码安全扫描（注入攻击检测）
- 可配置的危险操作审批流程
</details>

<details>
<summary><b>✅ 质量门禁流水线</b></summary>

```
┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐
│ Format  │ → │  Lint   │ → │TypeCheck│ → │Security │ → │Complexity│
└─────────┘   └─────────┘   └─────────┘   └─────────┘   └─────────┘
      ↓             ↓             ↓             ↓             ↓
   Auto-fix     Auto-fix      Block        Block         Warn
```

- 自动检测项目类型（Rust/Python/Node 等）
- 多维度检查：格式化 → Lint → 类型 → 安全 → 复杂度
- 迭代式修复循环，最多 3 次自动重试
- 与 Task 模式深度集成
</details>

<details>
<summary><b>🧠 可成长的知识系统</b></summary>

**技能系统 (4来源 + 2阶段选择)**

| 来源 | 优先级 | 说明 |
|------|--------|------|
| 内置技能 | 最高 | hybrid-ralph, mega-plan, planning-with-files |
| 外部技能 | 高 | Git Submodules (React, Vue, Rust 最佳实践) |
| 用户技能 | 中 | 项目级自定义工作流 |
| 动态生成 | 低 | 运行时按需生成 |

**记忆系统 (TF-IDF + 4-信号混合排序)**

```
Score = w₁×Recency + w₂×Frequency + w₃×Semantic + w₄×Importance
```

- 自动衰减与剪枝
- 跨会话持久化
- 上下文感知检索
</details>

<details>
<summary><b>🌐 远程控制 (A2A 协议)</b></summary>

```
用户 → Telegram Bot → Desktop Agent → 执行任务 → 返回结果
                           ↓
                    5层安全保护
```

- JSON-RPC 2.0 + SSE 传输
- 多平台适配器 (Telegram / Discord / Slack)
- 远程启动任务、查看进度、审批操作
</details>

<details>
<summary><b>🔌 MCP 全栈能力</b></summary>

| 能力 | 说明 |
|------|------|
| **MCP Client** | 连接任意 MCP Server，扩展工具能力 |
| **MCP Server** | 暴露自身为 MCP 工具，供其他应用调用 |
| **MCP Manager** | 完整生命周期管理（注册、健康检查、市场发现） |
| **Claude Desktop 导入** | 一键导入 Claude Desktop 的 MCP 配置 |
</details>

---

## 🚀 快速开始

### 前置要求

- **Node.js** 18+ 和 pnpm
- **Rust** 1.75+ (首次构建会自动安装)
- **系统依赖**: Platform-specific (见下方)

### 安装

```bash
# 克隆仓库
git clone https://github.com/anthropics/plan-cascade.git
cd plan-cascade/desktop

# 安装依赖
pnpm install

# 启动开发服务器
pnpm tauri:dev
```

### 系统依赖

| 平台 | 依赖 |
|------|------|
| **macOS** | Xcode Command Line Tools |
| **Windows** | Microsoft Visual Studio C++ Build Tools |
| **Linux** | `webkit2gtk-4.1`, `openssl`, `curl`, `wget`, `file` |

### 构建生产版本

```bash
pnpm tauri:build
```

---

## 📁 工作区概览

```
┌──────────────────────────────────────────────────────────────────────────┐
│                            Simple 工作台                                  │
├──────────────────────────────────────────────────────────────────────────┤
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐            │
│  │  文件面板       │  │   对话面板     │  │  产物面板       │            │
│  │                │  │                │  │                │            │
│  │  📁 项目文件    │  │  💬 与 AI 对话 │  │  📄 PRD        │            │
│  │  📎 拖拽文件    │  │  @ 引用上下文  │  │  📋 设计文档    │            │
│  │  🔍 快速搜索    │  │  ✏️ 编辑消息   │  │  📊 进度追踪    │            │
│  │                │  │                │  │                │            │
│  └────────────────┘  └────────────────┘  └────────────────┘            │
├──────────────────────────────────────────────────────────────────────────┤
│  [Chat] [Plan] [Task] [Debug]                    Quality Gates: ✓ 3/3    │
└──────────────────────────────────────────────────────────────────────────┘
```

### 可用工作区

| 工作区 | 状态 | 核心功能 |
|--------|------|----------|
| **Simple** | ✅ 活跃 | Chat/Plan/Task/Debug 四模式工作台 |
| **MCP Servers** | ✅ 活跃 | MCP Server 注册表、健康检查、市场发现 |
| **Analytics** | ✅ 活跃 | Token 消耗、成本估算、使用历史导出 |
| **Knowledge Base** | ✅ 活跃 | RAG 文档智能、Collection 管理、检索实验室 |
| **Codebase** | ✅ 活跃 | 文件/符号/嵌入概览、HNSW + FTS5 组合搜索 |
| **Settings** | ✅ 活跃 | 集中化配置、主题、快捷键 |

---

## 🛠 技术栈

| 层级 | 技术 | 用途 |
|------|------|------|
| **前端** | React 18.3 + TypeScript | UI 组件 |
| **状态** | Zustand + Immer | 客户端状态管理 |
| **样式** | Tailwind CSS + Radix UI | 设计系统 |
| **后端** | Rust (Tauri 2.0) | 原生性能 |
| **IPC** | Tauri Commands | 前端-Rust 通信桥 |
| **数据库** | SQLite + Tauri SQL | 持久化存储 |
| **索引** | HNSW + FTS5 | 代码与知识搜索 |

### Rust Crates

| Crate | 用途 |
|-------|------|
| `core` | Builders, LLM clients, quality gates |
| `llm` | 多提供商 LLM 抽象 |
| `tools` | ReAct 工具实现 |
| `quality-gates` | 验证流水线 |

---

## 🏗 架构深度解析

### 后端服务

```
┌─────────────────────────────────────────────────────────────────────┐
│                          应用层                                      │
├─────────────────────────────────────────────────────────────────────┤
│  workflow_kernel  │  orchestrator  │  mega  │  agent_composer       │
│  (会话生命周期)     │ (LLM+工具)     │ (多功能) │ (流水线)             │
├─────────────────────────────────────────────────────────────────────┤
│                          服务层                                      │
├─────────────────────────────────────────────────────────────────────┤
│  mcp  │  knowledge  │  memory  │  skills  │  git  │  worktree       │
│  a2a  │  analytics  │  plugins │  webhook │  timeline              │
├─────────────────────────────────────────────────────────────────────┤
│                          基础层                                      │
├─────────────────────────────────────────────────────────────────────┤
│  plan-cascade-core │ plan-cascade-llm │ plan-cascade-tools │ ...    │
└─────────────────────────────────────────────────────────────────────┘
```

### LLM 提供商

| 提供商 | 模型 | 最适用于 |
|--------|------|----------|
| Anthropic | Claude Sonnet, Haiku | 平衡性能 |
| OpenAI | GPT-4o, GPT-4o Mini | 通用场景 |
| DeepSeek | DeepSeek Chat | 高性价比 |
| GLM | 智谱 AI | 中文场景 |
| Qwen | 通义千问 | 中文 + 嵌入 |
| MiniMax | M2 系列 | 语音能力 |
| Ollama | 本地模型 | 隐私优先 |

---

## 📊 路线图与状态

| 工作区 | 状态 | 说明 |
|--------|------|------|
| Simple | ✅ 活跃 | 核心日常工作区 |
| MCP Servers | ✅ 活跃 | 完整生命周期管理 |
| Analytics | ✅ 活跃 | 使用量与成本追踪 |
| Knowledge Base | ✅ 活跃 | RAG 文档智能 |
| Codebase | ✅ 活跃 | 混合代码搜索 |
| Settings | ✅ 活跃 | 集中化配置 |
| Expert | 🚧 开发中 | 高级代理功能 |
| Claude Code | 🚧 开发中 | Claude Code 集成 |
| Projects | 🚧 开发中 | 多项目管理 |
| Artifacts | 🚧 开发中 | 产物浏览器 |

---

## 📚 文档

| 文档 | 说明 |
|------|------|
| [架构设计](./docs/architecture-design_zh.md) | 系统架构概览 |
| [内核设计](./docs/kernel-design_zh.md) | 工作流内核规范 |
| [内存与技能](./docs/memory-and-skills_zh.md) | 可成长知识系统 |
| [MCP 集成](./docs/mcp-integration_zh.md) | MCP Server 管理 |
| [A2A 远程控制](./docs/a2a-remote-control_zh.md) | 远程代理协议 |

---

## 🤝 贡献指南

欢迎参与贡献！详情请参阅 [贡献指南](../CONTRIBUTING.md)。

### 开发环境搭建

1. Fork 并克隆仓库
2. 安装依赖：`pnpm install`
3. 启动开发：`pnpm tauri:dev`
4. 进行修改
5. 运行测试：`pnpm test && cd src-tauri && cargo test`
6. 提交 Pull Request

---

## 📄 许可证

本项目基于 [MIT License](../LICENSE) 开源。

---

## 🙏 致谢

- [Tauri](https://tauri.app/) - 跨平台桌面应用框架
- [Claude](https://www.anthropic.com/claude) - Anthropic 的 AI 助手
- [React](https://react.dev/) - UI 框架
- [Radix UI](https://www.radix-ui.com/) - 无障碍组件库

---

<p align="center">
  <a href="https://github.com/anthropics/plan-cascade/issues">报告问题</a> •
  <a href="https://github.com/anthropics/plan-cascade/discussions">参与讨论</a>
</p>
