<div align="center">

# Plan Cascade Desktop

**AI 驱动的编程编排平台**

跨平台桌面应用，将复杂开发任务分解为可并行执行的工作流，支持多智能体协作。基于 Rust 后端与 React 前端构建。

[![版本](https://img.shields.io/badge/版本-0.1.0-blue)](package.json)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-FFC131?logo=tauri&logoColor=white)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-18.3-61dafb?logo=react&logoColor=white)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-2021_Edition-dea584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![许可证](https://img.shields.io/badge/许可证-MIT-green)](../LICENSE)

[English](./README.md) | [简体中文](./README_zh-CN.md)

</div>

---

## 项目概览

Plan Cascade Desktop 是基于 **Tauri 2.0** 构建的综合性 AI 编程助手。它对接 7+ 个大语言模型提供商，提供智能代码生成与 Agent 工具调用，并编排多步骤开发工作流——从简单的问答对话到完全自主的 PRD 驱动功能开发。

### 为什么选择 Plan Cascade Desktop？

- **纯 Rust 后端** — 极低内存占用，运行时无需 Python/Node 环境
- **安全优先** — API 密钥使用 AES-256-GCM 加密存储，仅保留在本地，不会外传
- **兼容你的模型** — 支持 Anthropic、OpenAI、DeepSeek、Ollama (本地)、通义千问、智谱 GLM、MiniMax
- **多种执行模式** — 为不同任务选择合适的自治程度
- **全栈类型安全** — TypeScript 严格模式 + Rust 编译期检查
- **跨平台** — Windows、macOS (Universal 二进制) 和 Linux

---

## 功能特性

### 多模式执行

| 模式 | 描述 | 适用场景 |
|------|------|----------|
| **Claude Code** | 与 Claude Code CLI 集成的交互式对话 | 实时结对编程 |
| **Simple** | 直接 LLM 对话，支持 Agent 工具调用 | 快速任务和问答 |
| **Expert** | PRD 生成与依赖关系图可视化 | 功能规划与拆解 |
| **Task** | PRD 驱动的自主多故事执行 | 复杂功能实现 |
| **Plan** | 多功能 Mega 计划编排 | 项目级协调 |

### 智能体库

创建和管理专业化 AI 智能体，支持自定义系统提示词、工具约束、模型选择和执行历史。智能体可在项目和会话间复用。

### 质量门禁

每次代码生成后自动运行的验证流水线：
- 测试执行（单元测试、集成测试、端到端测试）
- 代码检查和格式化
- 类型检查
- 按项目自定义验证规则

### 时间线与检查点

AI 生成变更的会话级版本控制：
- 在关键里程碑自动创建状态快照
- 分支和分叉工作流，探索不同方案
- 一键回滚到任意检查点

### Git 工作树集成

隔离的开发环境，支持并行任务执行：
- 自动创建分支和工作树
- 安全的合并工作流，自动检测冲突
- 多任务并行开发

### 知识库 (RAG)

基于向量嵌入的语义文档搜索：
- 索引项目文档、设计规格和参考资料
- 多提供商 Embedding 支持
- 自动变更检测与重新索引

### 代码库索引

AI 驱动的代码搜索与理解：
- 基于 Tree-sitter 的符号提取（函数、类、结构体、枚举）
- 后台索引 + 文件监听自动更新
- HNSW 向量搜索，支持语义化代码查询

### MCP 集成

完整的 [Model Context Protocol](https://modelcontextprotocol.io/) 支持：
- 服务器注册表管理
- 自定义工具和资源提供者配置
- 支持 SSE 和 stdio 传输协议

### 分析仪表板

跨所有 LLM 提供商跟踪使用量、成本和性能：
- 按模型/提供商的 Token 消耗与费用明细
- 历史趋势可视化
- 会话级使用归因

### 更多能力

- **护栏 (Guardrails)** — 基于规则约束工具执行，保障安全
- **Webhooks** — 事件路由到飞书、Slack、Discord 或自定义端点
- **远程控制** — Telegram Bot 和 A2A 协议 Agent 发现
- **插件系统** — 按框架注入专业技能（React、Vue、Rust）
- **PDF / 图片导出** — 导出对话和制品
- **国际化** — 支持英文、中文（简体）、日文

---

## 架构

```
┌─────────────────────────────────────────────────────────────────┐
│                     Plan Cascade Desktop                        │
├───────────────────────────┬─────────────────────────────────────┤
│   React 前端              │   Rust 后端 (Tauri 2.0)             │
│   ───────────────────     │   ─────────────────────────         │
│   Radix UI 组件库         │   300+ IPC 命令                     │
│   Zustand 状态管理 (50)   │   42+ 服务模块                      │
│   Monaco 代码编辑器       │   SQLite + r2d2 连接池              │
│   i18next (3 种语言)      │   AES-256-GCM 密钥存储             │
│   Tailwind CSS            │   Tree-sitter 代码解析              │
│   Fuse.js 模糊搜索        │   HNSW 向量搜索                    │
├───────────────────────────┴─────────────────────────────────────┤
│                       Tauri IPC 桥接                            │
├───────────┬──────────────┬──────────────┬───────────────────────┤
│ Claude    │ LLM          │ Git          │ MCP                   │
│ Code CLI  │ 提供商       │ 工作树       │ 服务器                │
│           │ (7+)         │              │                       │
└───────────┴──────────────┴──────────────┴───────────────────────┘
```

### Cargo 工作空间

Rust 后端由 5 个 crate 组成：

| Crate | 用途 |
|-------|------|
| `plan-cascade-desktop` | Tauri 主应用 — 命令、服务、存储 |
| `plan-cascade-core` | 核心 trait、错误类型、上下文层级、流式事件 |
| `plan-cascade-llm` | LLM 提供商抽象与流式适配器 |
| `plan-cascade-tools` | 工具执行框架与定义 |
| `plan-cascade-quality-gates` | 质量门禁流水线与项目类型检测 |

### 项目结构

```
desktop/
├── src/                          # React 前端
│   ├── components/               #   按领域组织的 UI 组件
│   │   ├── Agents/               #     智能体库
│   │   ├── Analytics/            #     使用量与费用仪表板
│   │   ├── ClaudeCodeMode/       #     Claude Code CLI 集成
│   │   ├── ExpertMode/           #     PRD 与策略规划
│   │   ├── SimpleMode/           #     直接 LLM 对话
│   │   ├── TaskMode/             #     自主任务执行
│   │   ├── KnowledgeBase/        #     RAG 文档搜索
│   │   ├── Timeline/             #     检查点浏览器
│   │   ├── MCP/                  #     MCP 服务器管理
│   │   ├── Settings/             #     配置界面
│   │   └── shared/               #     通用组件
│   ├── store/                    #   Zustand 状态管理
│   ├── lib/                      #   IPC API 封装
│   ├── i18n/                     #   翻译文件 (en, zh, ja)
│   └── types/                    #   TypeScript 类型定义
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── commands/             #   Tauri IPC 命令处理器
│   │   ├── services/             #   业务逻辑层
│   │   ├── models/               #   数据结构
│   │   └── storage/              #   SQLite、密钥存储、配置
│   └── crates/                   #   工作空间 crate
│       ├── core/                 #     核心 trait 与类型
│       ├── llm/                  #     LLM 提供商适配器
│       ├── tools/                #     工具执行框架
│       └── quality-gates/        #     验证流水线
└── docs/                         # 文档
```

---

## 快速开始

### 环境要求

| 依赖 | 版本 | 说明 |
|------|------|------|
| [Node.js](https://nodejs.org/) | 18+ | 前端构建工具链 |
| [pnpm](https://pnpm.io/) | 8+ | 包管理器 |
| [Rust](https://rustup.rs/) | 1.70+ | 后端编译 |
| 系统库 | — | 参见 [Tauri 系统依赖](https://v2.tauri.app/start/prerequisites/) |

### 安装与运行

```bash
# 克隆仓库
git clone https://github.com/plan-cascade/plan-cascade
cd plan-cascade/desktop

# 安装前端依赖
pnpm install

# 启动开发模式（前端 + 后端，支持热重载）
pnpm tauri:dev
```

首次启动时，Rust 后端需从源码编译，耗时几分钟。后续启动会非常快。

### 生产构建

```bash
# 构建当前平台
pnpm tauri:build

# 特定平台构建
pnpm tauri:build:macos      # macOS Universal (Intel + Apple Silicon)
pnpm tauri:build:windows    # Windows x64 MSI
pnpm tauri:build:linux      # Linux x64 AppImage
```

---

## 开发

### 常用命令

```bash
# 前端
pnpm dev                    # 仅启动 Vite 开发服务器 (端口 8173)
pnpm build                  # TypeScript 编译 + Vite 构建
pnpm lint                   # ESLint (零警告策略)
pnpm typecheck              # TypeScript 严格模式检查
pnpm test                   # 运行测试 (Vitest)
pnpm test:watch             # 监听模式
pnpm test:coverage          # 覆盖率报告 (60% 阈值)

# 后端（在 src-tauri/ 目录下）
cargo test                  # 单元测试 + 集成测试
cargo clippy                # Rust 代码检查
cargo check                 # 类型检查
cargo build --features browser  # 构建（含无头浏览器支持）

# 完整应用
pnpm tauri:dev              # 开发模式，支持热重载和 devtools
pnpm tauri:build:dev        # Debug 构建
```

### 代码质量

- **TypeScript**：严格模式，启用 `noUnusedLocals` 和 `noUnusedParameters`
- **ESLint**：零警告策略 (`--max-warnings 0`)
- **Prettier**：通过 pre-commit 钩子强制执行（Husky + lint-staged）
- **Rust**：clippy 代码检查，Release 构建启用 LTO 和符号裁剪
- **提交规范**：约定式格式 — `type(scope): description`

---

## 支持的 LLM 提供商

| 提供商 | 工具调用 | 本地 | 说明 |
|--------|:---:|:---:|------|
| [Anthropic](https://www.anthropic.com/) (Claude) | 原生 | | 支持 Prompt Caching |
| [OpenAI](https://openai.com/) (GPT) | 原生 | | |
| [DeepSeek](https://www.deepseek.com/) | 双通道 | | 原生 + 提示词降级 |
| [通义千问](https://www.alibabacloud.com/en/solutions/generative-ai/qwen) (阿里巴巴) | 双通道 | | |
| [智谱 GLM](https://www.zhipuai.cn/) | 双通道 | | |
| [Ollama](https://ollama.com/) | 仅提示词 | 是 | 支持任意本地模型 |
| [MiniMax](https://www.minimaxi.com/) | 仅提示词 | | |

**双通道**：工具同时通过原生 API 和提示词降级方式传递，提高可靠性。

---

## 文档

| 文档 | 描述 |
|------|------|
| [用户手册](./docs/user-guide.md) | 终端用户功能详解 |
| [开发者指南](./docs/developer-guide.md) | 架构深入解析与贡献指引 |
| [API 参考](./docs/api-reference.md) | 完整 IPC 命令文档 |
| [迁移指南](./docs/migration-v5.md) | 从 v4.x 升级到 v5.0 |
| [代码库索引计划](./docs/codebase-index-iteration-plan.md) | 语义搜索迭代路线图 |
| [记忆技能计划](./docs/memory-skill-iteration-plan.md) | 智能体记忆系统设计 |

---

## 贡献

欢迎参与贡献！架构细节和代码规范请参阅 [开发者指南](./docs/developer-guide.md)。

```bash
# 1. Fork 并克隆
git clone https://github.com/<your-username>/plan-cascade
cd plan-cascade/desktop

# 2. 创建功能分支
git checkout -b feat/your-feature

# 3. 开发完成后，确保质量检查通过
pnpm lint && pnpm typecheck && pnpm test

# 4. 使用约定式提交信息提交
git commit -m "feat(scope): add your feature"

# 5. 推送并创建 Pull Request
git push origin feat/your-feature
```

### 开发规范

- 合并前所有测试必须通过
- ESLint 零警告策略——禁止无理由的规则屏蔽
- 面向用户的变更需同步更新文档
- 新增命令、服务和组件应遵循现有模式

---

## 故障排除

**构建失败，提示 "linker 'cc' not found"**

```bash
# macOS
xcode-select --install

# Ubuntu / Debian
sudo apt install build-essential libwebkit2gtk-4.1-dev libappindicator3-dev

# Fedora
sudo dnf install gcc webkit2gtk4.1-devel libappindicator-gtk3-devel
```

**Tauri 开发服务器无法启动**

```bash
cargo clean                           # 清除 Rust 构建缓存
rm -rf node_modules && pnpm install   # 重新安装前端依赖
```

**API 密钥无法保存**

应用使用本地加密文件存储（AES-256-GCM），而非系统钥匙串。请检查应用对数据目录是否有写入权限。

---

## 技术栈

### 前端

| 类别 | 库 | 版本 |
|------|-----|------|
| 框架 | React | 18.3 |
| 状态管理 | Zustand | 5.0 |
| UI 基础组件 | Radix UI | latest |
| 代码编辑器 | Monaco Editor | 4.7 |
| 样式 | Tailwind CSS | 3.4 |
| 国际化 | i18next | 25.8 |
| Markdown | react-markdown + rehype + remark | 10.1 |
| 数学公式 | KaTeX | 0.16 |
| 拖拽 | @dnd-kit | latest |
| 语法高亮 | Prism React Renderer | 2.4 |
| 模糊搜索 | Fuse.js | 7.1 |

### 后端

| 类别 | 库 | 版本 |
|------|-----|------|
| 桌面框架 | Tauri | 2.0 |
| 异步运行时 | Tokio | 1.x |
| 数据库 | rusqlite (内置 SQLite) | 0.32 |
| 连接池 | r2d2 | latest |
| HTTP 客户端 | Reqwest | 0.12 |
| 加密 | aes-gcm | 0.10 |
| 代码解析 | tree-sitter | 0.24 |
| 向量搜索 | hnsw_rs | 0.3 |
| 文件监听 | notify | 6.x |
| LLM SDK | ollama-rs, async-dashscope, anthropic-async, zai-rs | various |

---

## 许可证

[MIT](../LICENSE)

---

## 致谢

- [Tauri](https://tauri.app/) — 跨平台桌面框架
- [Anthropic](https://www.anthropic.com/) — Claude API 与 Claude Code
- [Radix UI](https://www.radix-ui.com/) — 无障碍、无样式 UI 基础组件
- [Monaco Editor](https://microsoft.github.io/monaco-editor/) — 代码编辑器组件
- [Tree-sitter](https://tree-sitter.github.io/) — 增量代码解析
