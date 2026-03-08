# Plan Cascade Desktop API Reference Document

**Version**: 2.0.0
**Date**: 2026-03-08
**Scope**: Tauri commands API reference

---

## Table of Contents

1. [Common Type Definitions](#1-common-type-definitions)
2. [Core Command Categories](#2-core-command-categories)
3. [Workflow Kernel Commands](#3-workflow-kernel-commands)
4. [Codebase Index Commands](#4-codebase-index-commands)

---

## 1. Common Type Definitions

### 1.1 CommandResponse

All commands follow a unified response pattern:

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
  // Event-specific data
}
```

---

## 2. Core Command Categories

### 2.1 Command Statistics

| Category | Command Count |
|----------|---------------|
| Initialization | 2 |
| Health Check | 1 |
| Settings | 2 |
| Projects | 3 |
| Sessions | 4 |
| Agent | 14 |
| Analytics | 22 |
| Quality Gates | 13 |
| Worktree | 6 |
| Standalone Execution | 14 |
| Timeline | 15 |
| MCP | 24 |
| Markdown | 5 |
| Claude Code | 7 |
| Codebase | 8 |
| Workflow Kernel | 10 |
| **Total** | **150** |

### 2.2 Main Categories

| Category | Description |
|----------|-------------|
| **Initialization** | Application initialization and version retrieval |
| **Health** | Service health status checks |
| **Settings** | Application configuration management |
| **Projects** | Project listing and search |
| **Sessions** | Session management |
| **Agents** | Agent creation and management |
| **Analytics** | Usage statistics and analysis |
| **Quality Gates** | Quality gate validation |
| **Worktree** | Git worktree management |
| **Standalone** | Standalone mode execution |
| **Timeline** | Checkpoint and branch management |
| **MCP** | Model Context Protocol |
| **Codebase** | Codebase indexing and search |
| **Workflow Kernel** | Workflow lifecycle management |

---

## 3. Workflow Kernel Commands

### 3.1 workflow_open_session

Opens a new workflow kernel session and initializes mode snapshots.

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

Gets the complete session state, including events and checkpoints.

```typescript
const state = await invoke<CommandResponse<WorkflowSessionState>>('workflow_get_session_state', {
  sessionId: 'wf_123',
});
```

### 3.3 workflow_link_mode_session

Binds a backend mode session (`task` or `plan`) to the kernel session for tracking/recovery.

```typescript
const linked = await invoke<CommandResponse<WorkflowSession>>('workflow_link_mode_session', {
  sessionId: 'wf_123',
  mode: 'task',
  modeSessionId: 'task-session-42',
});
```

### 3.4 workflow_append_context_items

Low-level workflow command for appending handoff context to a session.

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

### 3.5 Workflow Kernel Events

Frontend listeners should subscribe to:

- `workflow-kernel-updated`

**Payload format**:

```typescript
interface WorkflowKernelUpdatedEvent {
  source: string;
  revision: number;
  sessionState: WorkflowSessionState;
}
```

---

## 4. Codebase Index Commands

### 4.1 codebase_list_projects_v2

Returns indexed workspaces with real-time status snapshots.

```typescript
const result = await invoke<CommandResponse<IndexedProjectStatusEntry[]>>('codebase_list_projects_v2');
```

### 4.2 codebase_search_v2

Unified search endpoint supporting `hybrid/symbol/path/semantic` modes and filters.

```typescript
const results = await invoke<CommandResponse<CodebaseSearchResult>>('codebase_search_v2', {
  query: 'search term',
  mode: 'hybrid',
  filters: {
    language: 'rust',
    file_path_prefix: 'src/'
  },
  limit: 20,
  offset: 0
});
```

### 4.3 Search Modes

| Mode | Description |
|------|-------------|
| `hybrid` | Combines FTS5 and HNSW using RRF fusion |
| `symbol` | FTS5 full-text search on symbols |
| `path` | FTS5 search on file paths |
| `semantic` | HNSW vector similarity search |

### 4.4 Search Response

```typescript
interface CodebaseSearchResult {
  results: SearchResultItem[];
  total: number;
  mode: string;
  query_time_ms: number;
}

interface SearchResultItem {
  file_path: string;
  chunk_text: string;
  line_start: number;
  line_end: number;
  score: number;
  metadata?: {
    symbol_name?: string;
    symbol_kind?: string;
    resolved_type?: string;
    reference_count?: number;
  };
}
```

---

## Appendix: Event Types

### Stream Events

| Event Type | Description |
|------------|-------------|
| `text_delta` | Streaming text response |
| `thinking_start` | LLM reasoning started |
| `thinking_delta` | Reasoning content update |
| `thinking_end` | Reasoning completed |
| `tool_start` | Tool execution started |
| `tool_result` | Tool execution result |
| `usage` | Token usage information |
| `error` | Error occurrence |
| `complete` | Response complete |

### Workflow Events

| Event Type | Description |
|------------|-------------|
| `workflow-kernel-updated` | Kernel state changed |
| `workflow-mode-transcript-updated` | Transcript updated |
| `workflow-step-completed` | Step execution finished |
| `workflow-story-completed` | Story milestone reached |

---

## Appendix: Error Handling

### Error Response Format

```typescript
interface CommandError {
  code: string;           // Error code
  message: string;       // Human-readable message
  details?: Record<string, unknown>; // Additional context
}
```

### Common Error Codes

| Code | Description |
|------|-------------|
| `SESSION_NOT_FOUND` | Session does not exist |
| `INVALID_MODE` | Invalid mode transition |
| `KERNEL_ERROR` | Workflow kernel error |
| `INDEX_ERROR` | Codebase indexing error |
| `TOOL_EXECUTION_FAILED` | Tool execution failed |
| `LLM_ERROR` | LLM provider error |
| `VALIDATION_FAILED` | Input validation failed |
