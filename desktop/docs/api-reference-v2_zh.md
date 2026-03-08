# Plan Cascade Desktop API 参考文档

**版本**: 2.0.0
**日期**: 2026-03-08
**范围**: Tauri commands API reference

---

## 目录

1. [通用类型定义](#1-通用类型定义)
2. [核心命令分类](#2-核心命令分类)
3. [工作流内核命令](#3-工作流内核命令)
4. [代码库索引命令](#4-代码库索引命令)

---

## 1. 通用类型定义

### 1.1 CommandResponse

所有命令遵循统一的响应模式:

```typescript
interface CommandResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}
```

### 1.2 HealthResponse

```typescript
interface HealthResponse {
  service: string;      // "plan-cascade-desktop"
  status: string;       // "healthy" | "degraded"
  database: boolean;
  keyring: boolean;
  config: boolean;
}
```

### 1.3 AppConfig

```typescript
interface AppConfig {
  theme: 'light' | 'dark' | 'system';
  locale: string;
  telemetry_enabled: boolean;
  default_provider: string;
  default_model: string;
}
```

### 1.4 UnifiedStreamEvent

```typescript
interface UnifiedStreamEvent {
  type: 'text_delta' | 'thinking_start' | 'thinking_delta' | 'thinking_end' |
        'tool_start' | 'tool_result' | 'usage' | 'error' | 'complete';
  // 事件特定数据
}
```

---

## 2. 核心命令分类

### 2.1 命令统计

| 类别 | 命令数 |
|------|--------|
| 初始化 | 2 |
| 健康检查 | 1 |
| 设置 | 2 |
| 项目 | 3 |
| 会话 | 4 |
| Agent | 14 |
| 分析 | 22 |
| 质量门 | 13 |
| Worktree | 6 |
| 独立执行 | 14 |
| 时间线 | 15 |
| MCP | 24 |
| Markdown | 5 |
| Claude Code | 7 |
| 代码库 | 8 |
| 工作流内核 | 10 |
| **总计** | **150** |

### 2.2 主要类别

| 类别 | 说明 |
|------|------|
| **Initialization** | 应用初始化和版本获取 |
| **Health** | 服务健康状态检查 |
| **Settings** | 应用配置管理 |
| **Projects** | 项目列表和搜索 |
| **Sessions** | 会话管理 |
| **Agents** | Agent 创建和管理 |
| **Analytics** | 使用统计和分析 |
| **Quality Gates** | 质量门验证 |
| **Worktree** | Git worktree 管理 |
| **Standalone** | 独立模式执行 |
| **Timeline** | 检查点和分支管理 |
| **MCP** | Model Context Protocol |
| **Codebase** | 代码库索引和搜索 |
| **Workflow Kernel** | 工作流生命周期管理 |

---

## 3. 工作流内核命令

### 3.1 workflow_open_session

打开新的工作流内核会话并初始化模式快照。

```typescript
const result = await invoke<CommandResponse<WorkflowSession>>('workflow_open_session', {
  initialMode: 'chat',
  initialContext: {
    conversationContext: [],
    artifactRefs: [],
    contextSources: ['simple_mode'],
    metadata: { entry: 'simple_mode_mount' }
  }
});
```

### 3.2 workflow_get_session_state

获取完整会话状态，包括事件和检查点。

```typescript
const state = await invoke<CommandResponse<WorkflowSessionState>>('workflow_get_session_state', {
  sessionId: 'wf_123',
});
```

### 3.3 workflow_link_mode_session

将后端模式会话（`task` 或 `plan`）绑定到内核会话，用于追踪/恢复。

```typescript
const linked = await invoke<CommandResponse<WorkflowSession>>('workflow_link_mode_session', {
  sessionId: 'wf_123',
  mode: 'task',
  modeSessionId: 'task-session-42',
});
```

### 3.4 workflow_append_context_items

低级工作流命令，用于将交接上下文追加到会话。

```typescript
const result = await invoke<CommandResponse<WorkflowSession>>('workflow_append_context_items', {
  sessionId: 'wf_123',
  targetMode: 'task',
  handoff: {
    conversationContext: [],
    artifactRefs: ['src/main.rs#bootstrap'],
    contextSources: ['codebase'],
    metadata: { source: 'codebase' }
  }
});
```

### 3.5 工作流内核事件

前端监听器应订阅:

- `workflow-kernel-updated`

**Payload 格式**:

```typescript
interface WorkflowKernelUpdatedEvent {
  source: string;
  revision: number;
  sessionState: WorkflowSessionState;
}
```

---

## 4. 代码库索引命令

### 4.1 codebase_list_projects_v2

返回带有实时状态快照的索引工作区。

```typescript
const result = await invoke<CommandResponse<IndexedProjectStatusEntry[]>>('codebase_list_projects_v2');
```

### 4.2 codebase_search_v2

统一搜索端点，支持 `hybrid/symbol/path/semantic` 模式和过滤器。

```typescript
const result = await invoke<CommandResponse<CodeSearchResponse>>('codebase_search_v2', {
  request: {
    project_path: '/abs/project',
    query: 'index manager',
    modes: ['hybrid'],
    limit: 20,
    offset: 0,
    include_snippet: true,
    filters: {
      component: 'core',
      language: 'rust',
      file_path_prefix: 'src/services'
    }
  }
});
```

**CodeSearchResponse** 包含:
- `query_id`
- `diagnostics` (`active_channels`, `semantic_degraded`, `provider_display`, `hnsw_used` 等)
- 每条结果的元数据 (`line_start`, `line_end`, `component`, `language`, `channels`)

### 4.3 codebase_add_context

将选中的代码库项目直接追加到工作流会话上下文。

```typescript
const result = await invoke<CommandResponse<CodebaseContextAppendResult>>('codebase_add_context', {
  targetMode: 'plan',
  sessionId: 'wf_123',
  items: [
    {
      type: 'search_result',
      project_path: '/abs/project',
      file_path: 'src/main.rs',
      line_start: 42,
      line_end: 61
    }
  ]
});
```

**失败代码**:
- `session_not_found`
- `mode_mismatch`
- `context_validation_failed`

---

## 5. 错误处理

所有命令返回 `CommandResponse<T>` 并包含错误信息:

```typescript
try {
  const result = await someCommand();
  if (!result.success) {
    console.error('Command failed:', result.error);
  }
} catch (error) {
  // Network or IPC error
  console.error('Command error:', error);
}
```

---

## 相关文档

- [整体架构设计](./architecture-design.md)
- [内核系统设计](./kernel-design.md)
- [内存与技能设计](./memory-skill-design.md)
- [代码库索引设计](./codebase-index-design.md)
- [开发者指南](./developer-guide-v2.md)
