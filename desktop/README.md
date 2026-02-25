# Plan Cascade Desktop

<div align="center">

![Version](https://img.shields.io/badge/version-0.1.0-blue)
![Tauri](https://img.shields.io/badge/Tauri-2.0-orange)
![React](https://img.shields.io/badge/React-18.3-61dafb)
![Rust](https://img.shields.io/badge/Rust-1.70+-dea584)
![License](https://img.shields.io/badge/license-MIT-green)

**A Production-Grade AI Programming Orchestration Desktop Platform**

*Powered by Rust Backend + React Frontend*

[Features](#-features) • [Quick Start](#-quick-start) • [Architecture](#-architecture) • [Documentation](#-documentation)

</div>

---

## 📖 Overview

Plan Cascade Desktop is a comprehensive AI programming assistant built on **Tauri v2**, combining the performance and security of **Rust** with the flexibility of **React**. It provides intelligent code generation, multi-agent orchestration, and seamless integration with multiple LLM providers.

### Key Highlights

- 🚀 **High Performance**: Rust backend handles complex logic with minimal resource footprint
- 🔒 **Security First**: Native keyring integration for secure API key storage
- 🌐 **Cross-Platform**: Supports Windows, macOS, and Linux
- 🎯 **Type-Safe**: Full-stack TypeScript + Rust with automatic type synchronization
- 🔌 **Extensible**: Modular service architecture with plugin support

---

## ✨ Features

### 🤖 Multi-Mode Execution

| Mode | Description | Use Case |
|------|-------------|----------|
| **Claude Code Mode** | Interactive chat with Claude Code CLI | Real-time coding assistance |
| **Task Mode** | PRD-driven autonomous development | Complex feature implementation |
| **Expert Mode** | Advanced multi-agent orchestration | Large-scale project workflows |
| **Standalone Mode** | Direct LLM API calls | Custom integrations |

### 🧠 Core Capabilities

- **Agent Library**: Create and manage specialized AI agents
  - Custom prompts and behaviors
  - Tool integrations and constraints
  - Execution history and analytics

- **Quality Gates**: Automated code validation pipeline
  - Test execution (unit, integration, e2e)
  - Linting and formatting checks
  - Type checking and security scans
  - Custom validation rules

- **Timeline & Checkpoints**: Session version control
  - Automatic state snapshots
  - Branch and merge workflows
  - Rollback capabilities

- **Git Worktree**: Isolated development environments
  - Automatic branch creation
  - Safe merge workflows
  - Conflict resolution assistance

- **MCP Integration**: Model Context Protocol support
  - Server registry management
  - Custom tool integration
  - Resource provider configuration

### 📊 Analytics Dashboard

- Usage tracking and cost analysis
- Token consumption metrics
- Model performance comparison
- Historical trend visualization

---

## 🏗️ Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────┐
│                   Plan Cascade Desktop                       │
├─────────────────────────────────────────────────────────────┤
│  React Frontend (TypeScript)     │  Rust Backend (Tauri)    │
│  ─────────────────────────────   │  ───────────────────────  │
│  • Components (Radix UI)         │  • 300+ IPC Commands     │
│  • Zustand Stores (39 modules)   │  • Services (33 domains)  │
│  • Monaco Editor Integration     │  • SQLite Storage        │
│  • Tauri API Bindings            │  • Secure Keyring        │
│  • i18next (i18n)                │  • LSP Integration       │
│  • Tailwind CSS Styling          │  • Tree-sitter Parsing   │
└─────────────────────────────────────────────────────────────┘
            │                              │
            └────────── IPC Bridge ────────┘
                         │
         ┌───────────────┼───────────────┐
         ▼               ▼               ▼
   Claude Code      LLM Providers    Git Services
      CLI          (7+ providers)    (Worktree)
```

### Backend Architecture

#### **Entry Layer** (`src/main.rs`)
```rust
tauri::Builder::default()
    .manage(AppState::new())          // 15+ state containers
    .invoke_handler(tauri::generate_handler![
        // 300+ command registrations
    ])
```

#### **Command Layer** (`src/commands/`) - 39 Modules
| Domain | Module | Commands | Key Features |
|--------|--------|----------|--------------|
| Claude Code | `claude_code.rs` | 7 | Session management, streaming |
| Task Execution | `task_mode.rs` | 14 | PRD-driven autonomous execution |
| Pipeline | `pipeline_execution.rs` | 12 | Multi-agent orchestration |
| Standalone | `standalone.rs` | 14 | Direct LLM integration |
| Git | `git.rs` | 18 | Worktree, branches, merge |
| Analytics | `analytics.rs` | 22 | Usage tracking and reporting |
| Quality Gates | `quality_gates.rs` | 13 | Automated validation |

#### **Service Layer** (`src/services/`) - 33 Modules
- **Agent Service** (29.6 KB): Agent execution engine
- **Timeline Service** (53.5 KB): Checkpoint and state management
- **Orchestrator**: Complex workflow coordination
- **LLM Providers**: Unified interface for 7+ LLM APIs
- **Quality Gates**: Code validation pipeline

#### **Workspace Crates**
```
src-tauri/crates/
├── plan-cascade-core/        # Zero-dependency core types
│   ├── context.rs            # Execution contexts
│   ├── tool_trait.rs         # Tool abstractions
│   └── streaming.rs          # Stream event types
├── plan-cascade-llm/         # LLM provider integrations
│   ├── anthropic.rs          # Claude API
│   ├── openai.rs             # GPT API
│   ├── ollama.rs             # Local models
│   └── qwen.rs               # Qwen API
├── plan-cascade-tools/       # Tool execution framework
│   ├── executor.rs           # Tool runtime
│   └── registry.rs           # Tool catalog
└── plan-cascade-quality-gates/ # Quality validation
    ├── pipeline.rs           # Gate execution
    └── detector.rs           # Project type detection
```

### Frontend Architecture

#### **State Management** (Zustand - 39 Stores)
```typescript
// Example: Claude Code Store
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

#### **Component Structure**
```
src/components/
├── Layout/
│   ├── Sidebar.tsx              # Navigation rail
│   ├── MainContent.tsx          # Content area
│   └── RightPanel.tsx           # Contextual panel
├── ClaudeCode/
│   ├── ChatView.tsx             # Chat interface
│   ├── MessageList.tsx          # Message display
│   └── CodeBlock.tsx            # Code rendering
├── TaskMode/
│   ├── TaskInput.tsx            # PRD input
│   ├── ExecutionTimeline.tsx    # Progress visualization
│   └── CheckpointViewer.tsx     # State inspector
├── Pipeline/
│   ├── PipelineDesigner.tsx     # Visual workflow editor
│   ├── NodeEditor.tsx           # Node configuration
│   └── ExecutionMonitor.tsx     # Live execution view
└── shared/
    ├── MonacoEditor.tsx         # Code editor wrapper
    ├── MarkdownRenderer.tsx     # Markdown display
    └── FileTree.tsx             # Project browser
```

---

## 🚀 Quick Start

### Prerequisites

- **Node.js**: 18.x or later
- **Rust**: 1.70 or later
- **pnpm**: 8.x or later (recommended)
- **System Dependencies**: See [Tauri Prerequisites](https://tauri.app/v1/guides/getting-started/prerequisites)

### Installation

```bash
# Clone the repository
git clone https://github.com/plan-cascade/plan-cascade
cd plan-cascade/desktop

# Install dependencies
pnpm install

# Start development server
pnpm tauri:dev
```

### Production Build

```bash
# Build for current platform
pnpm tauri:build

# Platform-specific builds
pnpm tauri:build:windows    # Windows x64
pnpm tauri:build:macos      # macOS Universal
pnpm tauri:build:linux      # Linux x64
```

### Development Scripts

```bash
# Frontend development
pnpm dev                    # Start Vite dev server
pnpm build                  # Build frontend only
pnpm test                   # Run frontend tests
pnpm lint                   # Lint frontend code

# Backend development
cd src-tauri
cargo test                  # Run Rust tests
cargo clippy               # Lint Rust code
```

---

## 📚 Documentation

### User Guides
- **[User Guide](./docs/user-guide.md)** - Feature walkthrough for end users
- **[API Reference](./docs/api-reference.md)** - Complete command documentation
- **[Migration Guide](./docs/migration-v5.md)** - Upgrade from v4.x to v5.0

### Developer Resources
- **[Developer Guide](./docs/developer-guide.md)** - Architecture and contribution guide
- **[Codebase Index Plan](./docs/codebase-index-iteration-plan.md)** - Semantic search implementation
- **[Memory Skill Plan](./docs/memory-skill-iteration-plan.md)** - Agent memory system

---

## 🔧 Configuration

### LLM Provider Setup

Plan Cascade supports multiple LLM providers:

| Provider | API Key Setup | Models |
|----------|---------------|--------|
| **Anthropic** | Settings → API Keys → Anthropic | Claude 3.5 Sonnet, Claude 3 Opus |
| **OpenAI** | Settings → API Keys → OpenAI | GPT-4, GPT-4 Turbo |
| **DeepSeek** | Settings → API Keys → DeepSeek | DeepSeek Chat, DeepSeek Coder |
| **Ollama** | Settings → Local Models → Ollama | All local models |
| **Qwen** | Settings → API Keys → Qwen | Qwen-Turbo, Qwen-Plus |
| **Moonshot** | Settings → API Keys → Moonshot | Moonshot-v1-8k, Moonshot-v1-32k |
| **MiniMax** | Settings → API Keys → MiniMax | abab5.5-chat, abab5.5s-chat |

### Quality Gates Configuration

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

## 🧪 Testing

### Frontend Testing
```bash
pnpm test                  # Run unit tests
pnpm test:watch            # Watch mode
pnpm test:coverage         # Coverage report
```

### Backend Testing
```bash
cd src-tauri
cargo test                 # All tests
cargo test --lib           # Library tests only
cargo test --test integration  # Integration tests
```

---

## 🤝 Contributing

We welcome contributions! Please follow these steps:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development Guidelines
- Follow the [Developer Guide](./docs/developer-guide.md)
- Ensure all tests pass
- Update documentation for new features
- Follow conventional commit messages

---

## 📦 Tech Stack

### Frontend Dependencies
| Category | Package | Version | Purpose |
|----------|---------|---------|---------|
| Framework | React | 18.3 | UI framework |
| State | Zustand | 5.0 | Global state management |
| UI | Radix UI | Latest | Accessible components |
| Editor | Monaco Editor | 4.7 | Code editing |
| Styling | Tailwind CSS | 3.4 | Utility-first CSS |
| i18n | i18next | 25.8 | Internationalization |
| Markdown | react-markdown | 10.1 | Markdown rendering |
| Drag & Drop | @dnd-kit | Latest | Drag and drop |

### Backend Dependencies
| Category | Package | Version | Purpose |
|----------|---------|---------|---------|
| Framework | Tauri | 2.0 | Desktop framework |
| Runtime | Tokio | 1.x | Async runtime |
| Database | Rusqlite | 0.32 | SQLite database |
| HTTP | Reqwest | 0.12 | HTTP client |
| LLM | ollama-rs | 0.3 | Ollama SDK |
| Security | aes-gcm | 0.10 | API key encryption |
| Parsing | tree-sitter | 0.24 | Code parsing |
| Monitoring | notify | 6.x | File watching |

---

## 🐛 Troubleshooting

### Common Issues

**Issue**: Build fails with "linker 'cc' not found"
```bash
# macOS
xcode-select --install

# Linux (Ubuntu/Debian)
sudo apt install build-essential

# Linux (Fedora)
sudo dnf install gcc
```

**Issue**: Tauri dev server won't start
```bash
# Clear Rust cache
cargo clean

# Reinstall dependencies
rm -rf node_modules pnpm-lock.yaml
pnpm install
```

**Issue**: API keys not saving
- Check system keyring permissions
- Try alternative storage: Settings → Security → Use File Storage

---

## 📄 License

MIT License - see [LICENSE](../LICENSE) file for details.

---

## 🙏 Acknowledgments

- [Tauri](https://tauri.app/) - Cross-platform desktop framework
- [Anthropic](https://www.anthropic.com/) - Claude API
- [Radix UI](https://www.radix-ui.com/) - Accessible UI components
- [Monaco Editor](https://microsoft.github.io/monaco-editor/) - Code editor

---

<div align="center">

**Built with ❤️ by the Plan Cascade Team**

[Website](https://plan-cascade.dev) • [Discord](https://discord.gg/plan-cascade) • [Twitter](https://twitter.com/plan_cascade)

</div>
