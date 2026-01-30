# Migration Guide: v4.x to v5.0

**Version**: 5.0.0
**Last Updated**: 2026-01-30

This guide covers the migration from Plan Cascade Desktop v4.x (Python backend) to v5.0 (Pure Rust backend).

---

## Table of Contents

1. [Overview of Changes](#overview-of-changes)
2. [Breaking Changes](#breaking-changes)
3. [Architecture Differences](#architecture-differences)
4. [API Migration](#api-migration)
5. [Configuration Migration](#configuration-migration)
6. [Data Migration](#data-migration)
7. [Feature Parity](#feature-parity)
8. [New Features in v5.0](#new-features-in-v50)
9. [Troubleshooting](#troubleshooting)

---

## Overview of Changes

### What Changed

| Aspect | v4.x | v5.0 |
|--------|------|------|
| **Backend** | Python Sidecar (FastAPI) | Pure Rust |
| **IPC** | HTTP/WebSocket | Tauri IPC (direct) |
| **Database** | None (file-based) | SQLite (embedded) |
| **Secrets** | Python keyring | Rust keyring |
| **Distribution** | Python + Tauri | Single binary |
| **Python Required** | Yes (3.10+) | No |

### Benefits of v5.0

1. **Zero Dependencies**: No Python installation required
2. **Single Executable**: Download and run immediately
3. **Better Performance**: Native Rust code, no Python interpreter
4. **Smaller Size**: ~50MB vs ~150MB+ with Python
5. **Faster Startup**: <2 seconds vs 5-10 seconds
6. **Improved Security**: No Python package vulnerabilities

---

## Breaking Changes

### 1. Python Backend Removed

The Python sidecar process is no longer used. All backend logic is now in Rust.

**Impact**:
- Custom Python plugins no longer work
- Python-based extensions need to be rewritten

**Migration**:
- Port Python customizations to TypeScript frontend or request Rust backend features
- Use the new TypeScript API wrappers for all backend communication

### 2. API Endpoint Changes

The HTTP/WebSocket API is replaced with Tauri IPC commands.

**v4.x (HTTP)**:
```typescript
// Old way
const response = await fetch('http://localhost:8765/api/projects');
const projects = await response.json();
```

**v5.0 (Tauri IPC)**:
```typescript
// New way
import { projects } from './lib/api';
const projectList = await projects.listProjects();
```

### 3. Configuration File Location

**v4.x**:
- Config: `~/.plan-cascade/config.yaml`
- Agents: `~/.plan-cascade/agents.yaml`

**v5.0**:
- Config: `~/.plan-cascade/config.json`
- Database: `~/.plan-cascade/data.db`
- Secrets: OS Keychain

### 4. Session Storage

**v4.x**: Sessions stored in memory during Python process lifetime
**v5.0**: Sessions persisted in SQLite database, survive restarts

### 5. Event Names

Streaming events have been renamed and restructured.

**v4.x**:
```typescript
// Old events
socket.on('stream_text', handler);
socket.on('stream_tool', handler);
socket.on('stream_end', handler);
```

**v5.0**:
```typescript
// New events
import { listen } from '@tauri-apps/api/event';
listen('standalone-event', (event) => {
  switch (event.payload.type) {
    case 'text_delta': // was stream_text
    case 'tool_start': // was stream_tool
    case 'complete':   // was stream_end
  }
});
```

---

## Architecture Differences

### v4.x Architecture

```
┌─────────────────┐     HTTP/WS      ┌─────────────────┐
│  Tauri + React  │ <──────────────> │  Python Sidecar │
│   (Frontend)    │                  │   (FastAPI)     │
└─────────────────┘                  └────────┬────────┘
                                              │
                                              ▼
                                        Python packages
                                        (20+ dependencies)
```

### v5.0 Architecture

```
┌─────────────────────────────────────────────────────┐
│              Tauri Desktop Application               │
├─────────────────────────────────────────────────────┤
│  React Frontend  │      Rust Backend                │
│  (TypeScript)    │      (Native Code)               │
│                  │      • All business logic        │
│                  │      • SQLite embedded           │
│                  │      • HTTP client for LLM APIs  │
└─────────────────────────────────────────────────────┘
                    │
                    ▼
              Single executable
              No external dependencies
```

### Communication Changes

| v4.x | v5.0 |
|------|------|
| HTTP REST API | Tauri `invoke()` |
| WebSocket streams | Tauri `listen()` events |
| JSON config files | SQLite database |
| Python keyring | Rust keyring |

---

## API Migration

### Projects API

**v4.x**:
```typescript
// GET /api/projects
const response = await fetch(`${API_URL}/api/projects`);
const projects = await response.json();
```

**v5.0**:
```typescript
import { projects } from './lib/api';

// List projects
const projectList = await projects.listProjects();

// Get single project
const project = await projects.getProject('project-id');

// Search projects
const results = await projects.searchProjects('web app');
```

### Sessions API

**v4.x**:
```typescript
// GET /api/sessions/:projectPath
const response = await fetch(`${API_URL}/api/sessions/${encodeURIComponent(projectPath)}`);
```

**v5.0**:
```typescript
import { sessions } from './lib/api';

// List sessions
const sessionList = await sessions.listSessions('/path/to/project');

// Get session details
const details = await sessions.getSession('/path/to/session.jsonl');

// Resume session
const result = await sessions.resumeSession('/path/to/session.jsonl');
```

### Agents API

**v4.x**:
```typescript
// POST /api/agents
const response = await fetch(`${API_URL}/api/agents`, {
  method: 'POST',
  body: JSON.stringify({ name, systemPrompt, model }),
});
```

**v5.0**:
```typescript
import { agents } from './lib/api';

// Create agent
const agent = await agents.createAgent({
  name: 'Code Reviewer',
  systemPrompt: 'You are an expert code reviewer...',
  model: 'claude-sonnet-4-20250514',
});

// List agents
const agentList = await agents.listAgents();

// Run agent
const run = await agents.runAgent('agent-id', 'Review this code...');
```

### Analytics API

**v4.x**: Analytics was not available in v4.x

**v5.0**:
```typescript
import { analytics } from './lib/api';

// Track usage
await analytics.trackUsage('anthropic', 'claude-sonnet-4-20250514', 1000, 500);

// Get dashboard summary
const summary = await analytics.getDashboardSummary(filter, 'daily');

// Export data
const csv = await analytics.exportByModel(filter, 'csv');
```

### Streaming Events

**v4.x**:
```typescript
const socket = new WebSocket('ws://localhost:8765/ws/stream');
socket.onmessage = (event) => {
  const data = JSON.parse(event.data);
  if (data.type === 'text') {
    appendText(data.content);
  }
};
```

**v5.0**:
```typescript
import { listen } from '@tauri-apps/api/event';

const unlisten = await listen('standalone-event', (event) => {
  const { type, ...data } = event.payload;
  switch (type) {
    case 'text_delta':
      appendText(data.content);
      break;
    case 'thinking_delta':
      appendThinking(data.content);
      break;
    case 'tool_start':
      showToolExecution(data);
      break;
    case 'complete':
      handleComplete(data);
      break;
  }
});

// Remember to clean up
unlisten();
```

---

## Configuration Migration

### API Keys

**v4.x** (config.yaml):
```yaml
providers:
  anthropic:
    api_key: sk-ant-xxx
  openai:
    api_key: sk-xxx
```

**v5.0** (OS Keychain):
```typescript
import { standalone } from './lib/api';

// Keys are stored securely in the OS keychain
await standalone.configureProvider('anthropic', 'sk-ant-xxx');
await standalone.configureProvider('openai', 'sk-xxx');
```

### Agent Configuration

**v4.x** (agents.yaml):
```yaml
agents:
  - name: Code Reviewer
    system_prompt: |
      You are an expert code reviewer...
    model: claude-sonnet-4-20250514
    allowed_tools:
      - read
      - glob
      - grep
```

**v5.0** (SQLite database):
```typescript
// Agents are stored in the database
const agent = await agents.createAgent({
  name: 'Code Reviewer',
  systemPrompt: 'You are an expert code reviewer...',
  model: 'claude-sonnet-4-20250514',
  allowedTools: ['read', 'glob', 'grep'],
});

// Export/import for backup
const json = await agents.exportAgents();
await agents.importAgents(json);
```

### MCP Server Configuration

**v4.x**: Not available

**v5.0**:
```typescript
import { mcp } from './lib/api';

// Add server
await mcp.addMcpServer({
  name: 'filesystem',
  serverType: 'stdio',
  command: 'npx',
  args: ['-y', '@anthropic/mcp-server-filesystem'],
  env: { ALLOWED_PATHS: '/home/user/projects' },
});

// Import from Claude Desktop
await mcp.importFromClaudeDesktop();
```

---

## Data Migration

### Automatic Migration

When you first launch v5.0, it will automatically:

1. Detect existing v4.x configuration
2. Migrate settings to the new format
3. Import API keys to the OS keychain (with user confirmation)
4. Create the SQLite database

### Manual Migration Steps

If automatic migration fails:

1. **Export v4.x data**:
   ```bash
   # Backup your v4.x config
   cp -r ~/.plan-cascade ~/.plan-cascade-v4-backup
   ```

2. **Re-configure API keys**:
   ```typescript
   // In the app settings, re-enter your API keys
   // They will be stored securely in the OS keychain
   ```

3. **Re-create agents**:
   ```typescript
   // Import from your backup if you have JSON exports
   // Or manually recreate in the Agent Library
   ```

### Data Locations

| Data Type | v4.x Location | v5.0 Location |
|-----------|---------------|---------------|
| Config | `~/.plan-cascade/config.yaml` | `~/.plan-cascade/config.json` |
| Agents | `~/.plan-cascade/agents.yaml` | `~/.plan-cascade/data.db` |
| API Keys | Config file (plain text) | OS Keychain |
| Analytics | N/A | `~/.plan-cascade/data.db` |
| Sessions | Memory only | `~/.plan-cascade/data.db` |

---

## Feature Parity

### Features Available in Both Versions

| Feature | v4.x | v5.0 |
|---------|------|------|
| Claude Code GUI Mode | Yes | Yes |
| Standalone LLM Mode | Yes | Yes |
| Project Browser | Yes | Yes |
| Session Management | Limited | Enhanced |
| Agent Library | Yes | Enhanced |
| Settings | Yes | Enhanced |

### New in v5.0

| Feature | Description |
|---------|-------------|
| Analytics Dashboard | Track usage, costs, tokens |
| Quality Gates | Auto-detect project type, run tests/lint |
| Git Worktree Support | Isolated task development |
| MCP Server Management | Centralized MCP configuration |
| Timeline & Checkpoints | Session versioning |
| Crash Recovery | Resume interrupted executions |
| Command Palette | Quick keyboard access |
| File Watching | Real-time file change detection |

### Features Removed

| Feature | Reason | Alternative |
|---------|--------|-------------|
| Python plugins | Architecture change | Use TypeScript customization |
| Custom REST endpoints | No longer HTTP-based | Use Tauri commands |

---

## New Features in v5.0

### 1. Quality Gates (Auto-Detection)

```typescript
import { qualityGates } from './lib/api';

// Detect project type automatically
const detection = await qualityGates.detectProjectType('/path/to/project');
console.log(detection); // { projectType: 'nodejs', confidence: 0.95, ... }

// Run all quality gates
const summary = await qualityGates.runQualityGates('/path/to/project');
console.log(summary); // { total: 3, passed: 2, failed: 1, ... }
```

### 2. Git Worktree Support

```typescript
import { worktree } from './lib/api';

// Create isolated worktree for a task
const wt = await worktree.createWorktree({
  repoPath: '/path/to/repo',
  taskName: 'feature-auth',
  targetBranch: 'main',
});

// Complete task (commit, merge, cleanup)
await worktree.completeWorktree('/path/to/repo', wt.id, 'feat: add auth');
```

### 3. Session-Based Execution

```typescript
import { standalone } from './lib/api';

// Execute with crash recovery
const result = await standalone.executeStandaloneWithSession({
  provider: 'anthropic',
  model: 'claude-sonnet-4-20250514',
  projectPath: '/path/to/project',
  prdPath: '/path/to/prd.json',
  runQualityGates: true,
});

// Resume if interrupted
await standalone.resumeStandaloneExecution({
  sessionId: 'session-id',
  retryFailed: true,
});
```

### 4. Analytics Dashboard

```typescript
import { analytics } from './lib/api';

// Initialize
await analytics.initAnalytics();

// Get dashboard data
const summary = await analytics.getDashboardSummary(
  { startDate: '2026-01-01' },
  'daily'
);

console.log(summary);
// {
//   totalCost: 127.45,
//   totalTokens: 2400000,
//   requestCount: 1234,
//   byModel: [...],
//   byProject: [...],
//   timeSeries: [...]
// }
```

### 5. MCP Server Management

```typescript
import { mcp } from './lib/api';

// Import from Claude Desktop
const result = await mcp.importFromClaudeDesktop();
console.log(`Imported ${result.imported} servers`);

// Add custom server
await mcp.addMcpServer({
  name: 'my-server',
  serverType: 'stdio',
  command: './my-mcp-server',
});

// Test connectivity
const health = await mcp.testMcpServer('server-id');
```

---

## Troubleshooting

### Issue: Application Won't Start

**Symptoms**: App crashes on startup or shows blank screen

**Solutions**:
1. Delete the config file and restart:
   ```bash
   rm ~/.plan-cascade/config.json
   ```
2. Check for port conflicts (old Python process):
   ```bash
   # Kill any lingering Python processes
   pkill -f "plan-cascade"
   ```

### Issue: API Keys Not Working

**Symptoms**: "API key not configured" errors

**Solutions**:
1. Re-enter API keys in Settings
2. Check OS keychain permissions
3. On macOS: Allow "plan-cascade" in Keychain Access

### Issue: Missing Data After Migration

**Symptoms**: Agents or sessions missing

**Solutions**:
1. Check for backup at `~/.plan-cascade-v4-backup/`
2. Manually import agent configurations
3. Sessions from v4.x are not migrated (memory-only)

### Issue: Claude Code Integration Not Working

**Symptoms**: Can't start Claude Code sessions

**Solutions**:
1. Verify Claude Code CLI is installed:
   ```bash
   claude --version
   ```
2. Check PATH includes Claude Code binary
3. Grant terminal permissions (macOS)

### Issue: Slow Performance

**Symptoms**: UI feels sluggish

**Solutions**:
1. Clear old database entries:
   ```typescript
   await analytics.deleteUsageRecords({ endDate: '2025-01-01' });
   await standalone.cleanupStandaloneSessions(30);
   ```
2. Check disk space for SQLite database
3. Restart the application

### Issue: WebSocket Errors in Console

**Symptoms**: WebSocket connection errors in dev tools

**Solutions**:
- This is expected! v5.0 doesn't use WebSocket
- Clear browser cache if using web view
- These errors can be safely ignored

---

## Getting Help

If you encounter issues not covered here:

1. Check the [API Reference](./api-reference.md) for correct usage
2. Review the [Developer Guide](./developer-guide.md) for architecture details
3. File an issue on GitHub with:
   - v4.x version you're migrating from
   - Error messages or screenshots
   - Operating system and version

---

## Rollback Instructions

If you need to return to v4.x:

1. Uninstall v5.0
2. Restore backup:
   ```bash
   mv ~/.plan-cascade ~/.plan-cascade-v5
   mv ~/.plan-cascade-v4-backup ~/.plan-cascade
   ```
3. Install v4.x from releases
4. Ensure Python 3.10+ is available

**Note**: Any data created in v5.0 will not be available in v4.x.
