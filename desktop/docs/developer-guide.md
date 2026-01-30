# Plan Cascade Desktop Developer Guide

**Version**: 5.0.0
**Last Updated**: 2026-01-30

This guide is for developers who want to understand the architecture, contribute to the codebase, or extend Plan Cascade Desktop.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Development Setup](#development-setup)
3. [Project Structure](#project-structure)
4. [Rust Backend](#rust-backend)
5. [TypeScript Frontend](#typescript-frontend)
6. [API Layer](#api-layer)
7. [Adding New Features](#adding-new-features)
8. [Testing](#testing)
9. [Building & Releasing](#building--releasing)
10. [Contributing Guidelines](#contributing-guidelines)

---

## Architecture Overview

Plan Cascade Desktop v5.0 uses a pure Rust backend architecture, eliminating the previous Python sidecar dependency.

### High-Level Architecture

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

### Key Design Decisions

1. **Pure Rust Backend**: All business logic in Rust for performance and single-binary distribution
2. **Tauri IPC**: Direct communication between frontend and backend via Tauri commands
3. **SQLite Storage**: Embedded database for analytics, sessions, and configuration
4. **OS Keychain**: Secure storage for API keys using system keyring
5. **Event-Based Streaming**: Real-time updates via Tauri events

---

## Development Setup

### Prerequisites

- **Node.js**: 18.x or later
- **Rust**: 1.70 or later
- **pnpm**: 8.x or later (recommended) or npm
- **Platform-specific**:
  - Windows: Visual Studio Build Tools
  - macOS: Xcode Command Line Tools
  - Linux: `build-essential`, `libgtk-3-dev`, `libsoup2.4-dev`, `libwebkit2gtk-4.0-dev`

### Initial Setup

```bash
# Clone the repository
git clone https://github.com/anthropics/plan-cascade-desktop
cd plan-cascade-desktop/desktop

# Install frontend dependencies
pnpm install

# Install Rust dependencies (automatic on first build)
```

### Development Commands

```bash
# Start development server (hot reload)
pnpm tauri dev

# Build for production
pnpm tauri build

# Run frontend only (without Tauri)
pnpm dev

# Run tests
pnpm test                    # Frontend tests
cd src-tauri && cargo test   # Backend tests

# Lint
pnpm lint                    # Frontend linting
cd src-tauri && cargo clippy # Backend linting

# Type check
pnpm tsc --noEmit           # TypeScript
cd src-tauri && cargo check # Rust
```

---

## Project Structure

```
desktop/
+-- src/                          # React Frontend
|   +-- main.tsx                  # Entry point
|   +-- App.tsx                   # Root component
|   +-- components/               # UI Components
|   |   +-- Layout/               # Layout components
|   |   +-- Projects/             # Project browser
|   |   +-- Agents/               # Agent library
|   |   +-- Analytics/            # Analytics dashboard
|   |   +-- Timeline/             # Timeline view
|   |   +-- Chat/                 # Chat interface
|   |   +-- Settings/             # Settings
|   +-- hooks/                    # Custom React hooks
|   +-- store/                    # Zustand state management
|   +-- lib/                      # Utilities
|   |   +-- api/                  # TypeScript API wrappers
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
|       +-- commands/             # Tauri Commands (IPC)
|       |   +-- mod.rs
|       |   +-- projects.rs
|       |   +-- agents.rs
|       |   +-- analytics.rs
|       |   +-- quality_gates.rs
|       |   +-- worktree.rs
|       |   +-- standalone.rs
|       |   +-- timeline.rs
|       |   +-- mcp.rs
|       |   +-- markdown.rs
|       |   +-- claude_code.rs
|       |   +-- sessions.rs
|       |   +-- settings.rs
|       |   +-- health.rs
|       |   +-- init.rs
|       +-- services/             # Business Logic
|       |   +-- mod.rs
|       |   +-- agent.rs
|       |   +-- agent_executor.rs
|       |   +-- analytics/
|       |   +-- claude_code/
|       |   +-- llm/
|       |   +-- mcp.rs
|       |   +-- markdown.rs
|       |   +-- orchestrator/
|       |   +-- project.rs
|       |   +-- quality_gates/
|       |   +-- session.rs
|       |   +-- streaming/
|       |   +-- sync/
|       |   +-- timeline.rs
|       |   +-- tools/
|       |   +-- worktree/
|       +-- models/               # Data Models
|       +-- storage/              # Storage Layer
|       +-- utils/                # Utilities
|       +-- state.rs              # Application State
|
+-- package.json
+-- vite.config.ts
+-- tailwind.config.js
+-- tsconfig.json
```

---

## Rust Backend

### Command Layer

Commands are the IPC entry points called from the frontend.

**Example: Creating a new command**

```rust
// src-tauri/src/commands/my_feature.rs

use crate::models::response::CommandResponse;
use crate::models::my_feature::MyData;
use crate::services::my_feature::MyService;

/// List all items
#[tauri::command]
pub async fn list_items(
    state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<Vec<MyData>>, String> {
    let pool = match state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => pool,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let service = MyService::new(pool);

    match service.list_items().await {
        Ok(items) => Ok(CommandResponse::ok(items)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Create a new item
#[tauri::command]
pub async fn create_item(
    state: tauri::State<'_, AppState>,
    name: String,
    data: MyData,
) -> Result<CommandResponse<MyData>, String> {
    // Implementation
}
```

**Registering commands in main.rs**:

```rust
// src-tauri/src/main.rs

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            // Existing commands...
            commands::my_feature::list_items,
            commands::my_feature::create_item,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Service Layer

Services contain the business logic.

**Example: Service implementation**

```rust
// src-tauri/src/services/my_feature.rs

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use crate::models::my_feature::MyData;
use crate::utils::error::{AppError, AppResult};

pub struct MyService {
    pool: Pool<SqliteConnectionManager>,
}

impl MyService {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    pub async fn list_items(&self) -> AppResult<Vec<MyData>> {
        let conn = self.pool.get()?;

        let mut stmt = conn.prepare(
            "SELECT id, name, data FROM my_table ORDER BY created_at DESC"
        )?;

        let items = stmt.query_map([], |row| {
            Ok(MyData {
                id: row.get(0)?,
                name: row.get(1)?,
                data: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    }

    pub async fn create_item(&self, name: &str, data: &MyData) -> AppResult<MyData> {
        let conn = self.pool.get()?;

        conn.execute(
            "INSERT INTO my_table (id, name, data) VALUES (?1, ?2, ?3)",
            rusqlite::params![data.id, name, data.data],
        )?;

        Ok(data.clone())
    }
}
```

### Models

Data structures used throughout the application.

```rust
// src-tauri/src/models/my_feature.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyData {
    pub id: String,
    pub name: String,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional_field: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyCreateRequest {
    pub name: String,
    pub data: String,
}
```

### Storage Layer

Database operations and secure storage.

**SQLite Database**:

```rust
// src-tauri/src/storage/database.rs

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

pub struct Database {
    pool: Pool<SqliteConnectionManager>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self, Error> {
        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::builder()
            .max_size(10)
            .build(manager)?;

        // Run migrations
        let conn = pool.get()?;
        conn.execute_batch(include_str!("../migrations/001_init.sql"))?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> Pool<SqliteConnectionManager> {
        self.pool.clone()
    }
}
```

**Keyring for Secrets**:

```rust
// src-tauri/src/storage/keyring.rs

use keyring::Entry;

pub struct KeyringService {
    service_name: String,
}

impl KeyringService {
    pub fn new() -> Self {
        Self {
            service_name: "plan-cascade-desktop".to_string(),
        }
    }

    pub fn set_api_key(&self, provider: &str, key: &str) -> Result<(), Error> {
        let entry = Entry::new(&self.service_name, provider)?;
        entry.set_password(key)?;
        Ok(())
    }

    pub fn get_api_key(&self, provider: &str) -> Result<Option<String>, Error> {
        let entry = Entry::new(&self.service_name, provider)?;
        match entry.get_password() {
            Ok(key) => Ok(Some(key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
```

---

## TypeScript Frontend

### Component Structure

```typescript
// src/components/MyFeature/index.ts
export { MyFeature } from './MyFeature';
export { MyFeatureItem } from './MyFeatureItem';

// src/components/MyFeature/MyFeature.tsx
import { useState, useEffect } from 'react';
import { useMyFeatureStore } from '@/store/myFeature';
import { MyFeatureItem } from './MyFeatureItem';

export function MyFeature() {
  const { items, loading, error, fetchItems } = useMyFeatureStore();

  useEffect(() => {
    fetchItems();
  }, []);

  if (loading) return <LoadingSpinner />;
  if (error) return <ErrorMessage error={error} />;

  return (
    <div className="space-y-4">
      {items.map((item) => (
        <MyFeatureItem key={item.id} item={item} />
      ))}
    </div>
  );
}
```

### Zustand Store

```typescript
// src/store/myFeature.ts
import { create } from 'zustand';
import { myFeature } from '@/lib/api';
import type { MyData } from '@/lib/api/my-feature';

interface MyFeatureState {
  items: MyData[];
  loading: boolean;
  error: string | null;

  fetchItems: () => Promise<void>;
  createItem: (data: MyData) => Promise<void>;
  updateItem: (id: string, data: Partial<MyData>) => Promise<void>;
  deleteItem: (id: string) => Promise<void>;
}

export const useMyFeatureStore = create<MyFeatureState>((set, get) => ({
  items: [],
  loading: false,
  error: null,

  fetchItems: async () => {
    set({ loading: true, error: null });
    try {
      const items = await myFeature.listItems();
      set({ items, loading: false });
    } catch (error) {
      set({ error: error.message, loading: false });
    }
  },

  createItem: async (data) => {
    try {
      const item = await myFeature.createItem(data);
      set((state) => ({
        items: [item, ...state.items],
      }));
    } catch (error) {
      set({ error: error.message });
    }
  },

  // ... other actions
}));
```

### API Wrappers

```typescript
// src/lib/api/my-feature.ts
import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from './types';
import { ApiError } from './types';

export interface MyData {
  id: string;
  name: string;
  data: string;
}

export async function listItems(): Promise<MyData[]> {
  const result = await invoke<CommandResponse<MyData[]>>('list_items');
  if (!result.success || !result.data) {
    throw ApiError.fromResponse(result);
  }
  return result.data;
}

export async function createItem(data: MyData): Promise<MyData> {
  const result = await invoke<CommandResponse<MyData>>('create_item', {
    name: data.name,
    data: data.data,
  });
  if (!result.success || !result.data) {
    throw ApiError.fromResponse(result);
  }
  return result.data;
}
```

### Hooks

```typescript
// src/hooks/useMyFeature.ts
import { useEffect, useCallback } from 'react';
import { useMyFeatureStore } from '@/store/myFeature';

export function useMyFeature() {
  const store = useMyFeatureStore();

  useEffect(() => {
    store.fetchItems();
  }, []);

  const createItem = useCallback(async (data: MyData) => {
    await store.createItem(data);
  }, []);

  return {
    items: store.items,
    loading: store.loading,
    error: store.error,
    createItem,
    refresh: store.fetchItems,
  };
}
```

---

## API Layer

### Adding a New API Domain

1. **Create Rust commands** (`src-tauri/src/commands/my_feature.rs`)
2. **Create Rust service** (`src-tauri/src/services/my_feature.rs`)
3. **Create Rust models** (`src-tauri/src/models/my_feature.rs`)
4. **Register commands** in `main.rs`
5. **Create TypeScript wrapper** (`src/lib/api/my-feature.ts`)
6. **Export from index** (`src/lib/api/index.ts`)

### Event-Based Communication

For streaming or real-time updates:

**Rust side (emitting events)**:

```rust
use tauri::{AppHandle, Emitter};

pub async fn execute_with_streaming(
    app: AppHandle,
    // ...
) -> Result<(), Error> {
    // Create channel
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    // Spawn task to forward events
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let _ = app_clone.emit("my-event", &event);
        }
    });

    // Send events
    tx.send(MyEvent::Started).await?;
    // ... do work
    tx.send(MyEvent::Progress { percent: 50 }).await?;
    // ... more work
    tx.send(MyEvent::Complete).await?;

    Ok(())
}
```

**TypeScript side (listening)**:

```typescript
import { listen } from '@tauri-apps/api/event';

// In a component or hook
useEffect(() => {
  const setupListener = async () => {
    const unlisten = await listen('my-event', (event) => {
      const data = event.payload as MyEvent;
      handleEvent(data);
    });

    return unlisten;
  };

  const cleanup = setupListener();
  return () => {
    cleanup.then((unlisten) => unlisten());
  };
}, []);
```

---

## Adding New Features

### Feature Checklist

When adding a new feature:

- [ ] **Backend**
  - [ ] Create command module
  - [ ] Create service module
  - [ ] Create data models
  - [ ] Add database migrations (if needed)
  - [ ] Register commands in main.rs
  - [ ] Write unit tests

- [ ] **Frontend**
  - [ ] Create TypeScript API wrapper
  - [ ] Add types to api/types.ts
  - [ ] Export from api/index.ts
  - [ ] Create Zustand store (if needed)
  - [ ] Create React components
  - [ ] Add to routing/navigation
  - [ ] Write component tests

- [ ] **Documentation**
  - [ ] Add to API reference
  - [ ] Update user guide
  - [ ] Add TypeDoc comments

### Example: Adding a "Notes" Feature

**1. Backend Models**

```rust
// src-tauri/src/models/note.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}
```

**2. Backend Service**

```rust
// src-tauri/src/services/note.rs
pub struct NoteService { /* ... */ }

impl NoteService {
    pub async fn list_notes(&self, project_id: &str) -> AppResult<Vec<Note>> { /* ... */ }
    pub async fn create_note(&self, note: &Note) -> AppResult<Note> { /* ... */ }
    pub async fn update_note(&self, id: &str, updates: &NoteUpdate) -> AppResult<Note> { /* ... */ }
    pub async fn delete_note(&self, id: &str) -> AppResult<()> { /* ... */ }
}
```

**3. Backend Commands**

```rust
// src-tauri/src/commands/notes.rs
#[tauri::command]
pub async fn list_notes(project_id: String, ...) -> Result<CommandResponse<Vec<Note>>, String> { /* ... */ }

#[tauri::command]
pub async fn create_note(note: Note, ...) -> Result<CommandResponse<Note>, String> { /* ... */ }
```

**4. TypeScript API**

```typescript
// src/lib/api/notes.ts
export interface Note {
  id: string;
  projectId: string;
  title: string;
  content: string;
  createdAt: string;
  updatedAt: string;
}

export async function listNotes(projectId: string): Promise<Note[]> { /* ... */ }
export async function createNote(note: Note): Promise<Note> { /* ... */ }
```

**5. Zustand Store**

```typescript
// src/store/notes.ts
export const useNotesStore = create<NotesState>((set) => ({
  notes: [],
  fetchNotes: async (projectId) => { /* ... */ },
  createNote: async (note) => { /* ... */ },
}));
```

**6. React Component**

```typescript
// src/components/Notes/NotesList.tsx
export function NotesList({ projectId }: { projectId: string }) {
  const { notes, fetchNotes } = useNotesStore();
  // ...
}
```

---

## Testing

### Backend Tests

```rust
// src-tauri/src/commands/my_feature.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_items() {
        // Setup
        let pool = create_test_pool();
        let service = MyService::new(pool);

        // Test
        let items = service.list_items().await.unwrap();
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn test_create_item() {
        let pool = create_test_pool();
        let service = MyService::new(pool);

        let data = MyData {
            id: "test-1".to_string(),
            name: "Test".to_string(),
            data: "{}".to_string(),
        };

        let result = service.create_item("Test", &data).await.unwrap();
        assert_eq!(result.name, "Test");
    }
}
```

Run backend tests:
```bash
cd src-tauri && cargo test
```

### Frontend Tests

```typescript
// src/lib/api/__tests__/my-feature.test.ts
import { describe, it, expect, vi } from 'vitest';
import { listItems } from '../my-feature';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('my-feature API', () => {
  it('should list items', async () => {
    const mockItems = [{ id: '1', name: 'Test' }];
    vi.mocked(invoke).mockResolvedValue({
      success: true,
      data: mockItems,
    });

    const result = await listItems();
    expect(result).toEqual(mockItems);
  });
});
```

Run frontend tests:
```bash
pnpm test
```

---

## Building & Releasing

### Development Build

```bash
pnpm tauri dev
```

### Production Build

```bash
pnpm tauri build
```

Outputs:
- Windows: `src-tauri/target/release/bundle/msi/*.msi`
- macOS: `src-tauri/target/release/bundle/dmg/*.dmg`
- Linux: `src-tauri/target/release/bundle/appimage/*.AppImage`

### Release Checklist

- [ ] Update version in `package.json`
- [ ] Update version in `src-tauri/Cargo.toml`
- [ ] Update version in `src-tauri/tauri.conf.json`
- [ ] Update CHANGELOG.md
- [ ] Run full test suite
- [ ] Build for all platforms
- [ ] Test installers on each platform
- [ ] Create GitHub release
- [ ] Upload artifacts

---

## Contributing Guidelines

### Code Style

**Rust**:
- Follow `rustfmt` formatting
- Use `clippy` for linting
- Document public APIs with doc comments
- Write unit tests for services

**TypeScript**:
- Follow ESLint configuration
- Use TypeScript strict mode
- Document functions with JSDoc
- Write tests for API wrappers and components

### Commit Messages

Follow conventional commits:
```
type(scope): description

feat(analytics): add export to CSV functionality
fix(agents): resolve duplicate ID issue
docs(api): update standalone commands documentation
refactor(storage): optimize database queries
test(worktree): add integration tests
```

### Pull Request Process

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Run tests: `pnpm test && cd src-tauri && cargo test`
5. Run linting: `pnpm lint && cd src-tauri && cargo clippy`
6. Commit your changes
7. Push to your fork
8. Open a Pull Request

### PR Requirements

- [ ] Tests pass
- [ ] No linting errors
- [ ] Documentation updated
- [ ] CHANGELOG updated (for features/fixes)
- [ ] Reviewed by at least one maintainer

---

## Resources

### Documentation

- [Tauri Documentation](https://tauri.app/v1/guides/)
- [React Documentation](https://react.dev/)
- [Zustand Documentation](https://github.com/pmndrs/zustand)
- [Rust Documentation](https://doc.rust-lang.org/book/)

### Tools

- [Rust Analyzer](https://rust-analyzer.github.io/) - IDE support for Rust
- [Tauri DevTools](https://tauri.app/v1/guides/debugging/) - Debugging Tauri apps
- [React DevTools](https://react.dev/learn/react-developer-tools) - React debugging

### Contact

- **GitHub Issues**: For bugs and feature requests
- **Discussions**: For questions and ideas
- **Discord**: Join our community server
