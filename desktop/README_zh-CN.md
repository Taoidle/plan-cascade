# Plan Cascade Desktop

<div align="center">

![版本](https://img.shields.io/badge/版本-0.1.0-blue)
![Tauri](https://img.shields.io/badge/Tauri-2.0-orange)
![React](https://img.shields.io/badge/React-18.3-61dafb)
![Rust](https://img.shields.io/badge/Rust-1.70+-dea584)
![许可证](https://img.shields.io/badge/许可证-MIT-green)

**生产级 AI 编程编排桌面平台**

*基于 Rust 后端 + React 前端*

[功能特性](#-功能特性) • [快速开始](#-快速开始) • [架构设计](#️-架构设计) • [文档资源](#-文档资源)

</div>

---

## 📖 项目概览

Plan Cascade Desktop 是基于 **Tauri v2** 构建的综合 AI 编程助手，结合了 **Rust** 的高性能和安全性以及 **React** 的灵活性。它提供智能代码生成、多智能体编排以及与多个 LLM 提供商的无缝集成。

### 核心亮点

- 🚀 **高性能**: Rust 后端以极低的资源占用处理复杂逻辑
- 🔒 **安全优先**: 原生密钥环集成，安全存储 API 密钥
- 🌐 **跨平台**: 支持 Windows、macOS 和 Linux
- 🎯 **类型安全**: 全栈 TypeScript + Rust，自动类型同步
- 🔌 **可扩展**: 模块化服务架构，支持插件扩展

---

## ✨ 功能特性

### 🤖 多模式执行

| 模式 | 描述 | 使用场景 |
|------|------|----------|
| **Claude Code 模式** | 与 Claude Code CLI 交互式对话 | 实时编程辅助 |
| **任务模式** | PRD 驱动的自主开发 | 复杂功能实现 |
| **专家模式** | 高级多智能体编排 | 大型项目工作流 |
| **独立模式** | 直接调用 LLM API | 自定义集成 |

### 🧠 核心能力

- **智能体库**: 创建和管理专业化 AI 智能体
  - 自定义提示词和行为
  - 工具集成和约束
  - 执行历史和分析

- **质量门禁**: 自动化代码验证流水线
  - 测试执行（单元测试、集成测试、端到端测试）
  - 代码检查和格式化
  - 类型检查和安全扫描
  - 自定义验证规则

- **时间线与检查点**: 会话版本控制
  - 自动状态快照
  - 分支和合并工作流
  - 回滚能力

- **Git 工作树**: 隔离的开发环境
  - 自动分支创建
  - 安全的合并工作流
  - 冲突解决辅助

- **MCP 集成**: 模型上下文协议支持
  - 服务器注册表管理
  - 自定义工具集成
  - 资源提供者配置

### 📊 分析仪表板

- 使用跟踪和成本分析
- Token 消耗指标
- 模型性能对比
- 历史趋势可视化

---

## 🏗️ 架构设计

### 系统概览

```
┌─────────────────────────────────────────────────────────────┐
│                   Plan Cascade Desktop                       │
├─────────────────────────────────────────────────────────────┤
│  React 前端 (TypeScript)         │  Rust 后端 (Tauri)       │
│  ─────────────────────────────   │  ───────────────────────  │
│  • 组件库 (Radix UI)             │  • 300+ IPC 命令         │
│  • Zustand 状态管理 (39 模块)    │  • 服务层 (33 领域)      │
│  • Monaco 编辑器集成             │  • SQLite 存储           │
│  • Tauri API 绑定                │  • 安全密钥环            │
│  • i18next 国际化                │  • LSP 集成              │
│  • Tailwind CSS 样式             │  • Tree-sitter 解析      │
└─────────────────────────────────────────────────────────────┘
            │                              │
            └────────── IPC 桥接 ──────────┘
                         │
         ┌───────────────┼───────────────┐
         ▼               ▼               ▼
   Claude Code      LLM 提供商       Git 服务
      CLI          (7+ 提供商)      (工作树)
```

### 后端架构

#### **入口层** (`src/main.rs`)
```rust
tauri::Builder::default()
    .manage(AppState::new())          // 15+ 个状态容器
    .invoke_handler(tauri::generate_handler![
        // 300+ 个命令注册
    ])
```

#### **命令层** (`src/commands/`) - 39 个模块
| 领域 | 模块 | 命令数 | 关键功能 |
|------|------|--------|----------|
| Claude Code | `claude_code.rs` | 7 | 会话管理、流式响应 |
| 任务执行 | `task_mode.rs` | 14 | PRD 驱动的自主执行 |
| 流水线 | `pipeline_execution.rs` | 12 | 多智能体编排 |
| 独立模式 | `standalone.rs` | 14 | 直接 LLM 集成 |
| Git | `git.rs` | 18 | 工作树、分支、合并 |
| 分析统计 | `analytics.rs` | 22 | 使用跟踪和报告 |
| 质量门禁 | `quality_gates.rs` | 13 | 自动化验证 |

#### **服务层** (`src/services/`) - 33 个模块
- **智能体服务** (29.6 KB): 智能体执行引擎
- **时间线服务** (53.5 KB): 检查点和状态管理
- **编排器**: 复杂工作流协调
- **LLM 提供商**: 7+ 个 LLM API 的统一接口
- **质量门禁**: 代码验证流水线

#### **Workspace Crates**
```
src-tauri/crates/
├── plan-cascade-core/        # 零依赖核心类型
│   ├── context.rs            # 执行上下文
│   ├── tool_trait.rs         # 工具抽象
│   └── streaming.rs          # 流事件类型
├── plan-cascade-llm/         # LLM 提供商集成
│   ├── anthropic.rs          # Claude API
│   ├── openai.rs             # GPT API
│   ├── ollama.rs             # 本地模型
│   └── qwen.rs               # 通义千问 API
├── plan-cascade-tools/       # 工具执行框架
│   ├── executor.rs           # 工具运行时
│   └── registry.rs           # 工具目录
└── plan-cascade-quality-gates/ # 质量验证
    ├── pipeline.rs           # 门禁执行
    └── detector.rs           # 项目类型检测
```

### 前端架构

#### **状态管理** (Zustand - 39 个 Store)
```typescript
// 示例: Claude Code Store
export const useClaudeCodeStore = create<ClaudeCodeState>()(
  persist(
    (set, get) => ({
      currentSession: null,
      messages: [],
      
      startChat: async (request) => {
        const client = getClaudeCodeClient();
        const session = await client.startChat(request);
        set({ currentSession: session });
      },
    }),
    { name: 'claude-code-store' }
  )
);
```

#### **组件结构**
```
src/components/
├── Layout/
│   ├── Sidebar.tsx              # 导航栏
│   ├── MainContent.tsx          # 内容区域
│   └── RightPanel.tsx           # 上下文面板
├── ClaudeCode/
│   ├── ChatView.tsx             # 聊天界面
│   ├── MessageList.tsx          # 消息展示
│   └── CodeBlock.tsx            # 代码渲染
├── TaskMode/
│   ├── TaskInput.tsx            # PRD 输入
│   ├── ExecutionTimeline.tsx    # 进度可视化
│   └── CheckpointViewer.tsx     # 状态检查器
├── Pipeline/
│   ├── PipelineDesigner.tsx     # 可视化工作流编辑器
│   ├── NodeEditor.tsx           # 节点配置
│   └── ExecutionMonitor.tsx     # 实时执行视图
└── shared/
    ├── MonacoEditor.tsx         # 代码编辑器封装
    ├── MarkdownRenderer.tsx     # Markdown 展示
    └── FileTree.tsx             # 项目浏览器
```

---

## 🚀 快速开始

### 环境要求

- **Node.js**: 18.x 或更高版本
- **Rust**: 1.70 或更高版本
- **pnpm**: 8.x 或更高版本（推荐）
- **系统依赖**: 参考 [Tauri 环境要求](https://tauri.app/v1/guides/getting-started/prerequisites)

### 安装步骤

```bash
# 克隆仓库
git clone https://github.com/plan-cascade/plan-cascade
cd plan-cascade/desktop

# 安装依赖
pnpm install

# 启动开发服务器
pnpm tauri:dev
```

### 生产构建

```bash
# 构建当前平台
pnpm tauri:build

# 特定平台构建
pnpm tauri:build:windows    # Windows x64
pnpm tauri:build:macos      # macOS Universal
pnpm tauri:build:linux      # Linux x64
```

### 开发脚本

```bash
# 前端开发
pnpm dev                    # 启动 Vite 开发服务器
pnpm build                  # 仅构建前端
pnpm test                   # 运行前端测试
pnpm lint                   # 代码检查

# 后端开发
cd src-tauri
cargo test                  # 运行 Rust 测试
cargo clippy               # Rust 代码检查
```

---

## 📚 文档资源

### 用户指南
- **[用户手册](./docs/user-guide.md)** - 终端用户功能指南
- **[API 参考](./docs/api-reference.md)** - 完整的命令文档
- **[迁移指南](./docs/migration-v5.md)** - 从 v4.x 升级到 v5.0

### 开发者资源
- **[开发者指南](./docs/developer-guide.md)** - 架构和贡献指南
- **[代码库索引计划](./docs/codebase-index-iteration-plan.md)** - 语义搜索实现
- **[记忆技能计划](./docs/memory-skill-iteration-plan.md)** - 智能体记忆系统

---

## 🔧 配置

### LLM 提供商设置

Plan Cascade 支持多个 LLM 提供商：

| 提供商 | API 密钥设置 | 模型 |
|--------|--------------|------|
| **Anthropic** | 设置 → API 密钥 → Anthropic | Claude 3.5 Sonnet, Claude 3 Opus |
| **OpenAI** | 设置 → API 密钥 → OpenAI | GPT-4, GPT-4 Turbo |
| **DeepSeek** | 设置 → API 密钥 → DeepSeek | DeepSeek Chat, DeepSeek Coder |
| **Ollama** | 设置 → 本地模型 → Ollama | 所有本地模型 |
| **通义千问** | 设置 → API 密钥 → Qwen | Qwen-Turbo, Qwen-Plus |
| **Moonshot** | 设置 → API 密钥 → Moonshot | Moonshot-v1-8k, Moonshot-v1-32k |
| **MiniMax** | 设置 → API 密钥 → MiniMax | abab5.5-chat, abab5.5s-chat |

### 质量门禁配置

```toml
# .plan-cascade/quality-gates.toml
[lint]
enabled = true
command = "eslint"
args = ["--max-warnings", "0"]

[test]
enabled = true
command = "pnpm"
args = ["test"]

[type_check]
enabled = true
command = "tsc"
args = ["--noEmit"]
```

---

## 🧪 测试

### 前端测试
```bash
pnpm test                  # 运行单元测试
pnpm test:watch            # 监听模式
pnpm test:coverage         # 覆盖率报告
```

### 后端测试
```bash
cd src-tauri
cargo test                 # 所有测试
cargo test --lib           # 仅库测试
cargo test --test integration  # 集成测试
```

---

## 🤝 贡献指南

我们欢迎各种形式的贡献！请遵循以下步骤：

1. Fork 本仓库
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

### 开发规范
- 遵循 [开发者指南](./docs/developer-guide.md)
- 确保所有测试通过
- 更新相关文档
- 遵循约定式提交消息

---

## 📦 技术栈

### 前端依赖
| 类别 | 包 | 版本 | 用途 |
|------|-----|------|------|
| 框架 | React | 18.3 | UI 框架 |
| 状态管理 | Zustand | 5.0 | 全局状态管理 |
| UI | Radix UI | 最新 | 无障碍组件 |
| 编辑器 | Monaco Editor | 4.7 | 代码编辑 |
| 样式 | Tailwind CSS | 3.4 | 实用优先 CSS |
| 国际化 | i18next | 25.8 | 多语言支持 |
| Markdown | react-markdown | 10.1 | Markdown 渲染 |
| 拖拽 | @dnd-kit | 最新 | 拖拽交互 |

### 后端依赖
| 类别 | 包 | 版本 | 用途 |
|------|-----|------|------|
| 框架 | Tauri | 2.0 | 桌面框架 |
| 运行时 | Tokio | 1.x | 异步运行时 |
| 数据库 | Rusqlite | 0.32 | SQLite 数据库 |
| HTTP | Reqwest | 0.12 | HTTP 客户端 |
| LLM | ollama-rs | 0.3 | Ollama SDK |
| 安全 | aes-gcm | 0.10 | API 密钥加密 |
| 解析 | tree-sitter | 0.24 | 代码解析 |
| 监控 | notify | 6.x | 文件监控 |

---

## 🐛 故障排除

### 常见问题

**问题**: 构建失败，提示 "linker 'cc' not found"
```bash
# macOS
xcode-select --install

# Linux (Ubuntu/Debian)
sudo apt install build-essential

# Linux (Fedora)
sudo dnf install gcc
```

**问题**: Tauri 开发服务器无法启动
```bash
# 清除 Rust 缓存
cargo clean

# 重新安装依赖
rm -rf node_modules pnpm-lock.yaml
pnpm install
```

**问题**: API 密钥无法保存
- 检查系统密钥环权限
- 尝试替代存储：设置 → 安全 → 使用文件存储

---

## 📄 许可证

MIT 许可证 - 详见 [LICENSE](../LICENSE) 文件。

---

## 🙏 致谢

- [Tauri](https://tauri.app/) - 跨平台桌面框架
- [Anthropic](https://www.anthropic.com/) - Claude API
- [Radix UI](https://www.radix-ui.com/) - 无障碍 UI 组件
- [Monaco Editor](https://microsoft.github.io/monaco-editor/) - 代码编辑器

---

<div align="center">

**由 Plan Cascade 团队用 ❤️ 构建**

[官网](https://plan-cascade.dev) • [Discord](https://discord.gg/plan-cascade) • [Twitter](https://twitter.com/plan_cascade)

</div>
