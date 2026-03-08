# Plan Cascade Desktop 开发者指南

**版本**: 2.0.0
**日期**: 2026-03-08
**范围**: Architecture, development setup, project structure, quick start

---

## 目录

1. [架构概述](#1-架构概述)
2. [开发环境搭建](#2-开发环境搭建)
3. [项目结构](#3-项目结构)
4. [Rust 后端说明](#4-rust-后端说明)
5. [TypeScript 前端说明](#5-typescript-前端说明)
6. [快速入门指南](#6-快速入门指南)
7. [核心概念](#7-核心概念)

---

## 1. 架构概述

> 来源: `developer-guide.md` 第一部分

### 1.1 高层架构

Plan Cascade Desktop v5.0 使用纯 Rust 后端架构，消除了之前的 Python 依赖。

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

### 1.2 关键设计决策

1. **纯 Rust 后端**: 所有业务逻辑在 Rust 中实现，确保高性能和单一二进制分发
2. **Tauri IPC**: 通过 Tauri 命令实现前端与后端直接通信
3. **SQLite 存储**: 嵌入式数据库用于分析、会话和配置
4. **OS Keychain**: 使用系统 keychain 安全存储 API 密钥
5. **事件流式传输**: 通过 Tauri 事件实现实时更新
6. **Simple Plan/Task 内核权威**: Chat/Plan/Task 运行时生命周期是内核快照单一真相源

### 1.3 Simple Plan/Task 生产路径 V2

Simple 页面 (`src/components/SimpleMode`) 遵循以下生产约束:

- `workflowKernel.modeSnapshots` 是 Chat/Plan/Task 的唯一生命周期真相
- 前端通过 `workflow-kernel-updated` 消费内核推送更新
- 工作流卡片仅从类型化 payload 渲染
- Plan/Task 后端会话通过 `workflow_link_mode_session` 链接到内核会话

---

## 2. 开发环境搭建

### 2.1 前置要求

| 工具 | 版本 | 说明 |
|------|------|------|
| Node.js | 18.x+ | 前端运行时 |
| Rust | 1.70+ | 后端运行时 |
| pnpm | 8.x+ | 包管理器（推荐）或 npm |
| 平台特定 | - | 见下表 |

**平台特定要求**:

| 平台 | 依赖 |
|------|------|
| Windows | Visual Studio Build Tools |
| macOS | Xcode Command Line Tools |
| Linux | `build-essential`, `libgtk-3-dev`, `libsoup2.4-dev`, `libwebkit2gtk-4.0-dev` |

### 2.2 初始化步骤

```bash
# 克隆仓库
git clone https://github.com/anthropics/plan-cascade-desktop
cd plan-cascade-desktop/desktop

# 安装前端依赖
pnpm install

# 安装 Rust 依赖（首次构建时自动完成）
```

### 2.3 开发命令

```bash
# 启动开发服务器（热重载）
pnpm tauri dev

# 生产构建
pnpm tauri build

# 仅运行前端（不含 Tauri）
pnpm dev

# 运行测试
pnpm test                    # 前端测试
cd src-tauri && cargo test   # 后端测试

# 代码检查
pnpm lint                    # 前端 lint
cd src-tauri && cargo clippy # 后端 lint

# 类型检查
pnpm tsc --noEmit           # TypeScript
cd src-tauri && cargo check  # Rust
```

---

## 3. 项目结构

```
desktop/
+-- src/                          # React Frontend
|   +-- main.tsx                  # 入口点
|   +-- App.tsx                   # 根组件
|   +-- components/               # UI 组件
|   |   +-- Layout/               # 布局组件
|   |   +-- Projects/             # 项目浏览器
|   |   +-- Agents/               # Agent 库
|   |   +-- Analytics/            # 分析仪表板
|   |   +-- Timeline/             # 时间线视图
|   |   +-- Chat/                 # 聊天界面
|   |   +-- Settings/             # 设置
|   +-- hooks/                    # 自定义 React hooks
|   +-- store/                    # Zustand 状态管理
|   +-- lib/                      # 工具函数
|   |   +-- api.ts                # HTTP API 包装
|   |   +-- codebaseApi.ts        # 代码库 IPC 包装
|   +-- types/                    # TypeScript 类型
|   +-- styles/                   # 全局样式
|   +-- i18n/                     # 国际化
|
+-- src-tauri/                    # Rust Backend
|   +-- Cargo.toml                # Rust 依赖
|   +-- tauri.conf.json           # Tauri 配置
|   +-- src/
|       +-- main.rs               # 入口点
|       +-- lib.rs                # 库根
|       +-- commands/             # Tauri 命令 (IPC)
|       +-- services/             # 业务逻辑
|       +-- models/               # 数据模型
|       +-- storage/              # 存储层
|       +-- utils/                # 工具函数
|       +-- state.rs              # 应用状态
|
+-- package.json
+-- vite.config.ts
+-- tailwind.config.js
+-- tsconfig.json
```

---

## 4. Rust 后端说明

### 4.1 命令层

命令是前端调用的 IPC 入口点。

**创建新命令示例**:

```rust
// src-tauri/src/commands/my_feature.rs

use crate::models::response::CommandResponse;
use crate::models::my_feature::MyData;
use crate::services::my_feature::MyService;

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
```

**在 main.rs 中注册命令**:

```rust
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            // 现有命令...
            commands::my_feature::list_items,
            commands::my_feature::create_item,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 4.2 服务层

服务包含业务逻辑。

```rust
// src-tauri/src/services/my_feature.rs

pub struct MyService {
    pool: Pool<SqliteConnectionManager>,
}

impl MyService {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    pub async fn list_items(&self) -> AppResult<Vec<MyData>> {
        let conn = self.pool.get()?;
        // 业务逻辑...
    }
}
```

### 4.3 存储层

```rust
// SQLite
pub struct Database {
    pool: Pool<SqliteConnectionManager>,
}

// Keyring
pub struct KeyringService {
    service_name: String,
}
```

---

## 5. TypeScript 前端说明

### 5.1 组件结构

```typescript
// src/components/MyFeature/MyFeature.tsx
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

### 5.2 Zustand Store

```typescript
// src/store/myFeature.ts
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
}));
```

### 5.3 API 包装器

```typescript
// src/lib/api/my-feature.ts
export async function listItems(): Promise<MyData[]> {
  const result = await invoke<CommandResponse<MyData[]>>('list_items');
  if (!result.success || !result.data) {
    throw ApiError.fromResponse(result);
  }
  return result.data;
}
```

---

## 6. 快速入门指南

### 6.1 添加新功能检查清单

- [ ] **后端**
  - [ ] 创建命令模块
  - [ ] 创建服务模块
  - [ ] 创建数据模型
  - [ ] 添加数据库迁移（如需要）
  - [ ] 在 main.rs 中注册命令
  - [ ] 编写单元测试

- [ ] **前端**
  - [ ] 创建 TypeScript API 包装器
  - [ ] 在 api/types.ts 添加类型
  - [ ] 在 api/index.ts 导出
  - [ ] 创建 Zustand store（如需要）
  - [ ] 创建 React 组件
  - [ ] 添加到路由/导航
  - [ ] 编写组件测试

- [ ] **文档**
  - [ ] 添加到 API 参考
  - [ ] 更新用户指南
  - [ ] 添加 TypeDoc 注释

### 6.2 示例: 添加 "Notes" 功能

1. **后端模型** (`src-tauri/src/models/note.rs`)
2. **后端服务** (`src-tauri/src/services/note.rs`)
3. **后端命令** (`src-tauri/src/commands/notes.rs`)
4. **TypeScript API** (`src/lib/api/notes.ts`)
5. **Zustand Store** (`src/store/notes.ts`)
6. **React 组件** (`src/components/Notes/NotesList.tsx`)

---

## 7. 核心概念

### 7.1 工作流内核

工作流内核管理 Chat/Plan/Task 生命周期，是所有关键状态的单一真相源。

详见 [kernel-design.md](./kernel-design.md)

### 7.2 内存与技能

内存系统支持跨会话持久化记忆，技能系统提供可复用任务模板。

详见 [memory-skill-design.md](./memory-skill-design.md)

### 7.3 代码库索引

结合 Tree-sitter、FTS5 和 HNSW 的混合搜索能力。

详见 [codebase-index-design.md](./codebase-index-design.md)

---

## 相关文档

- [整体架构设计](./architecture-design.md)
- [内核系统设计](./kernel-design.md)
- [内存与技能设计](./memory-skill-design.md)
- [代码库索引设计](./codebase-index-design.md)
- [API 参考](./api-reference-v2.md)
