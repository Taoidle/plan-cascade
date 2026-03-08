# Plan Cascade Desktop Developer Guide

**Version**: 2.0.0
**Date**: 2026-03-08
**Scope**: Architecture, development setup, project structure, quick start

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Development Environment Setup](#2-development-environment-setup)
3. [Project Structure](#3-project-structure)
4. [Rust Backend Guide](#4-rust-backend-guide)
5. [TypeScript Frontend Guide](#5-typescript-frontend-guide)
6. [Quick Start Guide](#6-quick-start-guide)
7. [Core Concepts](#7-core-concepts)

---

## 1. Architecture Overview

> Source: First part of `developer-guide.md`

### 1.1 High-Level Architecture

Plan Cascade Desktop v5.0 uses a pure Rust backend architecture, eliminating previous Python dependencies.

```
+-------------------------------------------------------------+
|                  Plan Cascade Desktop v5.0                   |
+-------------------------------------------------------------+
|                                                              |
|   +-------------------+         +-------------------------+  |
|   |  React Frontend   |         |     Rust Backend        |  |
|   |  (TypeScript)     |         |     (Tauri)             |  |
|   |                   |         |                         |  |
|   | +---------------+ | Tauri   | +---------------------+ |  |
|   | | Components    | | IPC     | | Commands Layer      | |  |
|   | | - Projects    |<--------->| | - projects.rs       | |  |
|   | | - Agents      | |         | | - agents.rs         | |  |
|   | | - Analytics   | |         | | - analytics.rs      | |  |
|   | | - Timeline    | |         | | - etc.              | |  |
|   | +---------------+ |         | +---------------------+ |  |
|   |        |          |         |          |              |  |
|   | +---------------+ |         | +---------------------+ |  |
|   | | Zustand Store | |         | | Services Layer      | |  |
|   | | - state mgmt  | |         | | - Business logic    | |  |
|   | +---------------+ |         | | - LLM integration   | |  |
|   |        |          |         | | - Tool execution    | |  |
|   | +---------------+ |         | +---------------------+ |  |
|   | | API Wrappers  | |         |          |              |  |
|   | | - lib/api/    | |         | +---------------------+ |  |
|   | +---------------+ |         | | Storage Layer       | |  |
|   +-------------------+         | | - SQLite            | |  |
|                                 | | - Keyring           | |  |
|                                 | | - File System       | |  |
|                                 | +---------------------+ |  |
|                                 +-------------------------+  |
+-------------------------------------------------------------+
```

### 1.2 Key Design Decisions

1. **Pure Rust Backend**: All business logic implemented in Rust for high performance and single-binary distribution
2. **Tauri IPC**: Frontend communicates with backend via Tauri commands
3. **SQLite Storage**: Embedded database for analytics, sessions, and configuration
4. **OS Keychain**: Use system keychain for secure API key storage
5. **Event Streaming**: Real-time updates via Tauri events
6. **Simple Plan/Task Kernel Authority**: Chat/Plan/Task runtime lifecycle is the kernel snapshot Single Source of Truth

### 1.3 Simple Plan/Task Production Path V2

The Simple page (`src/components/SimpleMode`) follows these production constraints:

- `workflowKernel.modeSnapshots` is the sole lifecycle truth for Chat/Plan/Task
- Frontend consumes kernel push updates via `workflow-kernel-updated`
- Workflow cards render only from typed payloads
- Plan/Task backend sessions link to kernel sessions via `workflow_link_mode_session`

---

## 2. Development Environment Setup

### 2.1 Prerequisites

| Tool | Version | Description |
|------|---------|-------------|
| Node.js | 18.x+ | Frontend runtime |
| Rust | 1.70+ | Backend runtime |
| pnpm | 8.x+ | Package manager (recommended) or npm |
| Platform-specific | - | See below |

**Platform-specific requirements**:

| Platform | Dependencies |
|----------|--------------|
| Windows | Visual Studio Build Tools |
| macOS | Xcode Command Line Tools |
| Linux | `build-essential`, `libgtk-3-dev`, `libsoup2.4-dev`, `libwebkit2gtk-4.0-dev` |

### 2.2 Initialization Steps

```bash
# Clone repository
git clone https://github.com/anthropics/plan-cascade-desktop
cd plan-cascade-desktop/desktop

# Install frontend dependencies
pnpm install

# Install Rust dependencies (automatically done on first build)
```

### 2.3 Development Commands

```bash
# Start development server (hot reload)
pnpm tauri dev

# Production build
pnpm tauri build

# Run frontend only (without Tauri)
pnpm dev

# Run tests
pnpm test                    # Frontend tests
cd src-tauri && cargo test   # Backend tests

# Linting
pnpm lint                    # Frontend lint
cd src-tauri && cargo clippy # Backend lint

# Type checking
pnpm tsc --noEmit           # TypeScript
cd src-tauri && cargo check  # Rust
```

---

## 3. Project Structure

```
desktop/
+-- src/                          # React Frontend
|   +-- main.tsx                  # Entry point
|   +-- App.tsx                   # Root component
|   +-- components/               # UI components
|   |   +-- Layout/               # Layout components
|   |   +-- Projects/              # Project browser
|   |   +-- Agents/                # Agent library
|   |   +-- Analytics/             # Analytics dashboard
|   |   +-- Timeline/              # Timeline view
|   |   +-- Chat/                 # Chat interface
|   |   +-- Settings/             # Settings
|   +-- hooks/                    # Custom React hooks
|   +-- store/                    # Zustand state management
|   +-- lib/                      # Utility functions
|   |   +-- api.ts                # HTTP API wrapper
|   |   +-- codebaseApi.ts       # Codebase IPC wrapper
|   +-- types/                    # TypeScript types
|   +-- styles/                   # Global styles
|   +-- i18n/                     # Internationalization
|
+-- src-tauri/                    # Rust Backend
|   +-- Cargo.toml                # Rust dependencies
|   +-- tauri.conf.json           # Tauri configuration
|   +-- src/
|       +-- main.rs               # Entry point
|       +-- lib.rs                # Library root
|       +-- commands/              # Tauri commands (IPC)
|       +-- services/              # Business logic
|       +-- models/               # Data models
|       +-- storage/              # Storage layer
|       +-- utils/                # Utilities
|       +-- state.rs              # Application state
|
+-- package.json
+-- vite.config.ts
+-- tailwind.config.js
+-- tsconfig.json
```

---

## 4. Rust Backend Guide

### 4.1 Command Layer

Commands are IPC entry points called by the frontend.

**Creating a new command example**:

```rust
// src-tauri/src/commands/my_feature.rs

use crate::models::response::CommandResponse;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MyFeatureInput {
    pub name: String,
    pub value: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MyFeatureOutput {
    pub result: String,
    pub processed: bool,
}

#[tauri::command]
pub async fn my_feature_command(
    input: MyFeatureInput,
) -> Result<CommandResponse<MyFeatureOutput>, String> {
    // Business logic here
    let output = MyFeatureOutput {
        result: format!("Hello, {}!", input.name),
        processed: true,
    };
    
    Ok(CommandResponse::success(output))
}
```

### 4.2 Service Layer

Services contain business logic and are called by commands.

**Service structure example**:

```rust
// src-tauri/src/services/my_service.rs

pub struct MyService {
    // Service state
}

impl MyService {
    pub fn new() -> Self {
        Self {}
    }
    
    pub async fn process(&self, input: Input) -> Result<Output, Error> {
        // Business logic
    }
}
```

### 4.3 Model Layer

Models define data structures shared between frontend and backend.

```rust
// src-tauri/src/models/my_model.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyModel {
    pub id: String,
    pub name: String,
    pub created_at: String,
}
```

### 4.4 Storage Layer

SQLite database operations are encapsulated in the storage layer.

```rust
// src-tauri/src/storage/my_repository.rs

use crate::storage::Pool;

pub struct MyRepository {
    pool: Pool,
}

impl MyRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
    
    pub async fn find_by_id(&self, id: &str) -> Result<Option<MyModel>, Error> {
        // SQL query
    }
}
```

---

## 5. TypeScript Frontend Guide

### 5.1 Component Structure

React components follow a consistent pattern:

```typescript
// src/components/MyComponent/MyComponent.tsx

import React, { useState, useEffect } from 'react';
import { useMyStore } from '@/store/myStore';

interface MyComponentProps {
  initialValue?: string;
}

export const MyComponent: React.FC<MyComponentProps> = ({ 
  initialValue = '' 
}) => {
  const [value, setValue] = useState(initialValue);
  const { storeValue, setStoreValue } = useMyStore();
  
  useEffect(() => {
    // Side effects
  }, []);
  
  return (
    <div className="my-component">
      <input 
        value={value}
        onChange={(e) => setValue(e.target.value)}
      />
    </div>
  );
};
```

### 5.2 State Management (Zustand)

**Store definition**:

```typescript
// src/store/myStore.ts

import { create } from 'zustand';

interface MyState {
  value: string;
  setValue: (value: string) => void;
}

export const useMyStore = create<MyState>((set) => ({
  value: '',
  setValue: (value) => set({ value }),
}));
```

### 5.3 Tauri IPC Integration

**Calling backend commands**:

```typescript
// src/lib/api.ts

import { invoke } from '@tauri-apps/api/core';

export async function callMyCommand(input: MyInput): Promise<MyOutput> {
  const response = await invoke<CommandResponse<MyOutput>>('my_command', {
    input,
  });
  
  if (!response.success) {
    throw new Error(response.error);
  }
  
  return response.data!;
}
```

### 5.4 Event Listening

**Subscribing to backend events**:

```typescript
import { listen } from '@tauri-apps/api/event';

// Subscribe to events
const unlisten = await listen<MyEvent>('my-event', (event) => {
  console.log('Received:', event.payload);
});

// Unsubscribe when done
unlisten();
```

---

## 6. Quick Start Guide

### 6.1 First-Time Setup

1. Install prerequisites (Node.js, Rust, pnpm)
2. Clone repository
3. Run `pnpm install`
4. Run `pnpm tauri dev`

### 6.2 Making Your First Change

**Frontend change**:

1. Navigate to `src/components/`
2. Find or create a component
3. Make changes
4. Hot reload automatically applies

**Backend change**:

1. Navigate to `src-tauri/src/`
2. Modify command/service/model
3. Changes apply on next Tauri reload

### 6.3 Adding a New Command

1. Create command in `src-tauri/src/commands/`
2. Register in `src-tauri/src/lib.rs`
3. Create frontend API wrapper in `src/lib/`
4. Use in component

### 6.4 Running Tests

```bash
# Frontend unit tests
pnpm test

# Backend unit tests
cd src-tauri && cargo test

# Integration tests
pnpm test:integration

# E2E tests (requires dev server running)
pnpm test:e2e
```

---

## 7. Core Concepts

### 7.1 Simple Mode Architecture

Simple mode is the core interaction pattern with Chat/Plan/Task modes:

- **Chat Mode**: Conversational interaction with LLM
- **Plan Mode**: Structured planning with step-by-step execution
- **Task Mode**: Task management with quality gates

See [kernel-design.md](./kernel-design.md) for details.

### 7.2 Workflow Kernel

The workflow kernel manages the lifecycle of Simple mode sessions:

- Single Source of Truth for session state
- Event-driven updates to frontend
- Cross-mode context handoff

### 7.3 Memory System

Persistent memory across sessions:

- **Semantic Memory**: Facts about projects and users
- **Episodic Memory**: Past interaction records
- **Skills**: Reusable task templates

See [memory-skill-design.md](./memory-skill-design.md) for details.

### 7.4 Codebase Indexing

Hybrid search combining multiple techniques:

- **FTS5**: Full-text search with BM25 ranking
- **HNSW**: Vector similarity search
- **LSP**: Type and reference information

See [codebase-index-design.md](./codebase-index-design.md) for details.

### 7.5 Quality Gates

Configurable validation at each step:

- Step output verification
- Retry strategies
- Custom validation rules

---

## Appendix: Debugging Tips

### Frontend Debugging

- Use React DevTools for component inspection
- Check Zustand store with browser extension
- Inspect Tauri IPC calls in network tab

### Backend Debugging

- Use `println!` for quick debugging
- Check logs in `~/.local/share/plan-cascade/logs/`
- Use `cargo watch` for automatic rebuilds

### Common Issues

| Issue | Solution |
|-------|----------|
| Hot reload not working | Restart `pnpm tauri dev` |
| Type errors | Run `pnpm tsc --noEmit` |
| Build failures | Run `cd src-tauri && cargo build --verbose` |
| Database errors | Check logs and verify SQLite permissions |
