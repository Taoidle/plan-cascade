# Plan Cascade Desktop

A complete AI programming orchestration platform with pure Rust backend.

## Quick Start

### Prerequisites

- **Node.js**: 18.x or later
- **Rust**: 1.70 or later
- **pnpm**: 8.x or later (recommended)

### Installation

```bash
# Clone the repository
git clone https://github.com/anthropics/plan-cascade-desktop
cd plan-cascade-desktop/desktop

# Install dependencies
pnpm install

# Start development server
pnpm tauri dev
```

### Production Build

```bash
pnpm tauri build
```

## Features

### Working Modes

- **Claude Code GUI Mode**: Use Claude Code CLI as the execution backend
- **Standalone Mode**: Direct LLM API calls (Anthropic, OpenAI, DeepSeek, Ollama)

### Core Features

- **Projects & Sessions**: Browse and manage Claude Code projects
- **Agent Library**: Create custom AI agents with specialized behaviors
- **Analytics Dashboard**: Track usage, costs, and token consumption
- **Quality Gates**: Automatic code validation (tests, lint, type check)
- **Git Worktree**: Isolated task development with automatic merge
- **MCP Servers**: Manage Model Context Protocol integrations
- **Timeline & Checkpoints**: Session versioning and restoration
- **CLAUDE.md Editor**: Edit project configurations

## Architecture

```
+-----------------------------------------------------------+
|                 Plan Cascade Desktop v5.0                   |
+-----------------------------------------------------------+
|  React Frontend (TypeScript)  |  Rust Backend (Tauri)      |
|  - Components                 |  - 115 Tauri Commands      |
|  - Zustand Stores            |  - SQLite Storage          |
|  - TypeScript API Wrappers   |  - Keyring Secrets         |
+-----------------------------------------------------------+
```

## Documentation

- [API Reference](./docs/api-reference.md) - All 115 Tauri commands
- [User Guide](./docs/user-guide.md) - Feature guide for end users
- [Developer Guide](./docs/developer-guide.md) - Architecture and contribution
- [Migration Guide](./docs/migration-v5.md) - Migrate from v4.x to v5.0

## Project Structure

```
desktop/
+-- src/                    # React Frontend
|   +-- components/         # UI components
|   +-- store/              # Zustand state management
|   +-- lib/api/            # TypeScript API wrappers
|   +-- hooks/              # Custom React hooks
+-- src-tauri/              # Rust Backend
|   +-- src/
|       +-- commands/       # Tauri IPC commands
|       +-- services/       # Business logic
|       +-- models/         # Data structures
|       +-- storage/        # Database & keyring
+-- docs/                   # Documentation
```

## API Summary

| Domain | Commands | Description |
|--------|----------|-------------|
| Projects | 3 | Project browsing and search |
| Sessions | 4 | Session management and resume |
| Agents | 14 | Agent CRUD, execution, history |
| Analytics | 22 | Usage tracking and reporting |
| Quality Gates | 13 | Code validation automation |
| Worktree | 6 | Git worktree management |
| Standalone | 14 | LLM execution with sessions |
| Timeline | 15 | Checkpoints and branching |
| MCP | 7 | Server registry |
| Markdown | 5 | CLAUDE.md management |
| Claude Code | 7 | CLI integration |

**Total**: 115 commands

## Development

### Running Tests

```bash
# Frontend tests
pnpm test

# Backend tests
cd src-tauri && cargo test
```

### Linting

```bash
# Frontend
pnpm lint

# Backend
cd src-tauri && cargo clippy
```

## Contributing

See the [Developer Guide](./docs/developer-guide.md) for contribution guidelines.

## License

MIT License - see LICENSE file for details.
