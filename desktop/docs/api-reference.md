# Plan Cascade Desktop API Reference

**Version**: 5.0.0
**Last Updated**: 2026-01-30

This document provides a comprehensive reference for all Tauri commands available in Plan Cascade Desktop v5.0.

---

## Table of Contents

1. [Overview](#overview)
2. [Common Types](#common-types)
3. [Initialization Commands](#initialization-commands)
4. [Health Commands](#health-commands)
5. [Settings Commands](#settings-commands)
6. [Project Commands](#project-commands)
7. [Session Commands](#session-commands)
8. [Agent Commands](#agent-commands)
9. [Analytics Commands](#analytics-commands)
10. [Quality Gates Commands](#quality-gates-commands)
11. [Worktree Commands](#worktree-commands)
12. [Standalone Execution Commands](#standalone-execution-commands)
13. [Timeline Commands](#timeline-commands)
14. [MCP Commands](#mcp-commands)
15. [Markdown Commands](#markdown-commands)
16. [Claude Code Commands](#claude-code-commands)

---

## Overview

All commands follow a consistent response pattern using `CommandResponse<T>`:

```typescript
interface CommandResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}
```

### TypeScript API

All commands are wrapped in type-safe TypeScript functions in `src/lib/api/`:

```typescript
import {
  projects,
  sessions,
  agents,
  analytics,
  qualityGates,
  worktree,
  standalone,
  timeline,
  mcp
} from './lib/api';
```

---

## Common Types

### CommandResponse

```typescript
interface CommandResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}
```

### HealthResponse

```typescript
interface HealthResponse {
  service: string;      // "plan-cascade-desktop"
  status: string;       // "healthy" | "degraded"
  database: boolean;
  keyring: boolean;
  config: boolean;
}
```

### AppConfig

```typescript
interface AppConfig {
  theme: 'light' | 'dark' | 'system';
  locale: string;
  telemetry_enabled: boolean;
  default_provider: string;
  default_model: string;
}
```

---

## Initialization Commands

### init_app

Initialize the application on startup.

```typescript
// TypeScript
import { initApp } from './lib/api';
const message = await initApp();

// Rust signature
#[tauri::command]
pub async fn init_app(state: State<'_, AppState>) -> Result<CommandResponse<String>, String>
```

**Returns**: Success message string

**Example**:
```typescript
try {
  const result = await initApp();
  console.log('Initialized:', result); // "Application initialized successfully"
} catch (error) {
  console.error('Init failed:', error);
}
```

### get_version

Get the application version.

```typescript
// TypeScript
import { getVersion } from './lib/api';
const version = await getVersion();

// Rust signature
#[tauri::command]
pub fn get_version() -> CommandResponse<String>
```

**Returns**: Version string (e.g., "5.0.0")

---

## Health Commands

### get_health

Get the health status of all backend services.

```typescript
// TypeScript
import { getHealth } from './lib/api';
const health = await getHealth();

// Rust signature
#[tauri::command]
pub async fn get_health(state: State<'_, AppState>) -> Result<CommandResponse<HealthResponse>, String>
```

**Returns**: `HealthResponse` object

**Example**:
```typescript
const health = await getHealth();
if (health.status === 'healthy') {
  console.log('All services operational');
} else {
  console.log('Degraded:', {
    database: health.database,
    keyring: health.keyring,
    config: health.config
  });
}
```

---

## Settings Commands

### get_settings

Get current application settings.

```typescript
// TypeScript
import { getSettings } from './lib/api';
const settings = await getSettings();

// Rust signature
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<CommandResponse<AppConfig>, String>
```

### update_settings

Update application settings.

```typescript
// TypeScript
import { updateSettings } from './lib/api';
const updated = await updateSettings({ theme: 'dark' });

// Rust signature
#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    update: SettingsUpdate,
) -> Result<CommandResponse<AppConfig>, String>
```

**Parameters**:
- `update`: Partial settings update object

---

## Project Commands

### list_projects

List all projects with sorting and pagination.

```typescript
// TypeScript
import { projects } from './lib/api';
const projectList = await projects.listProjects();

// Rust signature
#[tauri::command]
pub fn list_projects(
    sort_by: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<CommandResponse<Vec<Project>>, String>
```

**Parameters**:
- `sort_by`: Optional. "name", "last_accessed", "created_at"
- `limit`: Optional. Default 50
- `offset`: Optional. Default 0

**Returns**: Array of `Project` objects

### get_project

Get a single project by ID.

```typescript
// TypeScript
const project = await projects.getProject('project-id');

// Rust signature
#[tauri::command]
pub fn get_project(project_id: String) -> Result<CommandResponse<Project>, String>
```

### search_projects

Search projects by name or path.

```typescript
// TypeScript
const results = await projects.searchProjects('web app');

// Rust signature
#[tauri::command]
pub fn search_projects(query: String) -> Result<CommandResponse<Vec<Project>>, String>
```

---

## Session Commands

### list_sessions

List all sessions for a project.

```typescript
// TypeScript
import { sessions } from './lib/api';
const sessionList = await sessions.listSessions('/path/to/project');

// Rust signature
#[tauri::command]
pub fn list_sessions(project_path: String) -> Result<CommandResponse<Vec<Session>>, String>
```

### get_session

Get detailed session information.

```typescript
// TypeScript
const details = await sessions.getSession('/path/to/session.jsonl');

// Rust signature
#[tauri::command]
pub fn get_session(session_path: String) -> Result<CommandResponse<SessionDetails>, String>
```

### resume_session

Prepare to resume a session.

```typescript
// TypeScript
const result = await sessions.resumeSession('/path/to/session.jsonl');

// Rust signature
#[tauri::command]
pub fn resume_session(session_path: String) -> Result<CommandResponse<ResumeResult>, String>
```

### search_sessions

Search sessions within a project.

```typescript
// TypeScript
const results = await sessions.searchSessions('/path/to/project', 'authentication');

// Rust signature
#[tauri::command]
pub fn search_sessions(
    project_path: String,
    query: String,
) -> Result<CommandResponse<Vec<Session>>, String>
```

---

## Agent Commands

### list_agents

List all agents.

```typescript
// TypeScript
import { agents } from './lib/api';
const agentList = await agents.listAgents();

// Rust signature
#[tauri::command]
pub async fn list_agents(state: State<'_, AppState>) -> Result<CommandResponse<Vec<Agent>>, String>
```

### list_agents_with_stats

List all agents with their statistics.

```typescript
// TypeScript
const agentsWithStats = await agents.listAgentsWithStats();

// Rust signature
#[tauri::command]
pub async fn list_agents_with_stats(
    state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<AgentWithStats>>, String>
```

### get_agent

Get a single agent by ID.

```typescript
// TypeScript
const agent = await agents.getAgent('agent-id');

// Rust signature
#[tauri::command]
pub async fn get_agent(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandResponse<Option<Agent>>, String>
```

### create_agent

Create a new agent.

```typescript
// TypeScript
const newAgent = await agents.createAgent({
  name: 'Code Reviewer',
  description: 'Reviews code for bugs and security issues',
  systemPrompt: 'You are an expert code reviewer...',
  model: 'claude-sonnet-4-20250514',
  allowedTools: ['read', 'glob', 'grep'],
  isActive: true,
});

// Rust signature
#[tauri::command]
pub async fn create_agent(
    state: State<'_, AppState>,
    request: AgentCreateRequest,
) -> Result<CommandResponse<Agent>, String>
```

**AgentCreateRequest**:
```typescript
interface AgentCreateRequest {
  name: string;
  description?: string;
  systemPrompt: string;
  model: string;
  allowedTools?: string[];
  isActive?: boolean;
}
```

### update_agent

Update an existing agent.

```typescript
// TypeScript
const updated = await agents.updateAgent('agent-id', { name: 'New Name' });

// Rust signature
#[tauri::command]
pub async fn update_agent(
    state: State<'_, AppState>,
    id: String,
    request: AgentUpdateRequest,
) -> Result<CommandResponse<Agent>, String>
```

### delete_agent

Delete an agent.

```typescript
// TypeScript
await agents.deleteAgent('agent-id');

// Rust signature
#[tauri::command]
pub async fn delete_agent(
    state: State<'_, AppState>,
    id: String,
) -> Result<CommandResponse<()>, String>
```

### get_agent_history

Get agent run history with pagination.

```typescript
// TypeScript
const history = await agents.getAgentHistory('agent-id', 20, 0);

// Rust signature
#[tauri::command]
pub async fn get_agent_history(
    state: State<'_, AppState>,
    agent_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<CommandResponse<AgentRunList>, String>
```

### get_agent_stats

Get statistics for an agent.

```typescript
// TypeScript
const stats = await agents.getAgentStats('agent-id');

// Rust signature
#[tauri::command]
pub async fn get_agent_stats(
    state: State<'_, AppState>,
    agent_id: String,
) -> Result<CommandResponse<AgentStats>, String>
```

### get_agent_run

Get a single agent run by ID.

```typescript
// TypeScript
const run = await agents.getAgentRun('run-id');

// Rust signature
#[tauri::command]
pub async fn get_agent_run(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<CommandResponse<Option<AgentRun>>, String>
```

### run_agent

Run an agent with the given input.

```typescript
// TypeScript
const run = await agents.runAgent('agent-id', 'Review this code...');

// Rust signature
#[tauri::command]
pub async fn run_agent(
    state: State<'_, AppState>,
    agent_id: String,
    input: String,
) -> Result<CommandResponse<AgentRun>, String>
```

### prune_agent_runs

Prune old runs for an agent.

```typescript
// TypeScript
const deleted = await agents.pruneAgentRuns('agent-id', 100);

// Rust signature
#[tauri::command]
pub async fn prune_agent_runs(
    state: State<'_, AppState>,
    agent_id: String,
    keep_count: u32,
) -> Result<CommandResponse<u32>, String>
```

### export_agents

Export agents as JSON.

```typescript
// TypeScript
const json = await agents.exportAgents(['agent-1', 'agent-2']);

// Rust signature
#[tauri::command]
pub async fn export_agents(
    state: State<'_, AppState>,
    agent_ids: Option<Vec<String>>,
) -> Result<CommandResponse<String>, String>
```

### import_agents

Import agents from JSON.

```typescript
// TypeScript
const imported = await agents.importAgents(jsonString);

// Rust signature
#[tauri::command]
pub async fn import_agents(
    state: State<'_, AppState>,
    json: String,
) -> Result<CommandResponse<Vec<Agent>>, String>
```

---

## Analytics Commands

### init_analytics

Initialize the analytics service.

```typescript
// TypeScript
import { analytics } from './lib/api';
await analytics.initAnalytics();

// Rust signature
#[tauri::command]
pub async fn init_analytics(
    app_state: State<'_, AppState>,
    analytics_state: State<'_, AnalyticsState>,
) -> Result<CommandResponse<bool>, String>
```

### track_usage

Track API usage.

```typescript
// TypeScript
await analytics.trackUsage('anthropic', 'claude-sonnet-4-20250514', 1000, 500);

// Rust signature
#[tauri::command]
pub async fn track_usage(
    analytics_state: State<'_, AnalyticsState>,
    provider: String,
    model_name: String,
    input_tokens: i64,
    output_tokens: i64,
    session_id: Option<String>,
    project_id: Option<String>,
) -> Result<CommandResponse<bool>, String>
```

### get_usage_statistics

Get usage statistics with optional filtering.

```typescript
// TypeScript
const stats = await analytics.getUsageStatistics({
  provider: 'anthropic',
  startDate: '2026-01-01',
  endDate: '2026-01-31',
});

// Rust signature
#[tauri::command]
pub async fn get_usage_statistics(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
) -> Result<CommandResponse<UsageStats>, String>
```

**UsageFilter**:
```typescript
interface UsageFilter {
  provider?: string;
  modelName?: string;
  projectId?: string;
  sessionId?: string;
  startDate?: string;  // ISO 8601
  endDate?: string;    // ISO 8601
}
```

### list_usage_records

List usage records with filtering and pagination.

```typescript
// TypeScript
const records = await analytics.listUsageRecords(filter, 100, 0);

// Rust signature
#[tauri::command]
pub async fn list_usage_records(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<CommandResponse<Vec<UsageRecord>>, String>
```

### count_usage_records

Get usage record count.

```typescript
// TypeScript
const count = await analytics.countUsageRecords(filter);

// Rust signature
#[tauri::command]
pub async fn count_usage_records(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
) -> Result<CommandResponse<i64>, String>
```

### aggregate_by_model

Get usage aggregated by model.

```typescript
// TypeScript
const byModel = await analytics.aggregateByModel(filter);

// Rust signature
#[tauri::command]
pub async fn aggregate_by_model(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
) -> Result<CommandResponse<Vec<ModelUsage>>, String>
```

### aggregate_by_project

Get usage aggregated by project.

```typescript
// TypeScript
const byProject = await analytics.aggregateByProject(filter);

// Rust signature
#[tauri::command]
pub async fn aggregate_by_project(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
) -> Result<CommandResponse<Vec<ProjectUsage>>, String>
```

### get_time_series

Get time series data.

```typescript
// TypeScript
const timeSeries = await analytics.getTimeSeries(filter, 'daily');

// Rust signature
#[tauri::command]
pub async fn get_time_series(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
    period: AggregationPeriod,
) -> Result<CommandResponse<Vec<TimeSeriesPoint>>, String>
```

**AggregationPeriod**: `"hourly"` | `"daily"` | `"weekly"` | `"monthly"`

### get_dashboard_summary

Get dashboard summary with all data.

```typescript
// TypeScript
const summary = await analytics.getDashboardSummary(filter, 'daily');

// Rust signature
#[tauri::command]
pub async fn get_dashboard_summary(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
    period: AggregationPeriod,
) -> Result<CommandResponse<DashboardSummary>, String>
```

### get_summary_statistics

Get summary statistics with percentiles.

```typescript
// TypeScript
const stats = await analytics.getSummaryStatistics(filter);

// Rust signature
#[tauri::command]
pub async fn get_summary_statistics(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
) -> Result<CommandResponse<SummaryStatistics>, String>
```

### calculate_usage_cost

Calculate cost for a given usage.

```typescript
// TypeScript
const cost = await analytics.calculateUsageCost('anthropic', 'claude-sonnet-4-20250514', 1000, 500);

// Rust signature
#[tauri::command]
pub async fn calculate_usage_cost(
    analytics_state: State<'_, AnalyticsState>,
    provider: String,
    model_name: String,
    input_tokens: i64,
    output_tokens: i64,
) -> Result<CommandResponse<i64>, String>
```

### get_model_pricing

Get pricing for a model.

```typescript
// TypeScript
const pricing = await analytics.getModelPricing('anthropic', 'claude-sonnet-4-20250514');

// Rust signature
#[tauri::command]
pub async fn get_model_pricing(
    analytics_state: State<'_, AnalyticsState>,
    provider: String,
    model_name: String,
) -> Result<CommandResponse<Option<ModelPricing>>, String>
```

### list_model_pricing

List all model pricing.

```typescript
// TypeScript
const allPricing = await analytics.listModelPricing();

// Rust signature
#[tauri::command]
pub async fn list_model_pricing(
    analytics_state: State<'_, AnalyticsState>,
) -> Result<CommandResponse<Vec<ModelPricing>>, String>
```

### set_custom_pricing

Set custom pricing for a model.

```typescript
// TypeScript
await analytics.setCustomPricing({
  provider: 'anthropic',
  modelName: 'claude-sonnet-4-20250514',
  inputPricePerMillion: 3000,
  outputPricePerMillion: 15000,
});

// Rust signature
#[tauri::command]
pub async fn set_custom_pricing(
    analytics_state: State<'_, AnalyticsState>,
    pricing: ModelPricing,
) -> Result<CommandResponse<bool>, String>
```

### remove_custom_pricing

Remove custom pricing for a model.

```typescript
// TypeScript
await analytics.removeCustomPricing('anthropic', 'claude-sonnet-4-20250514');

// Rust signature
#[tauri::command]
pub async fn remove_custom_pricing(
    analytics_state: State<'_, AnalyticsState>,
    provider: String,
    model_name: String,
) -> Result<CommandResponse<bool>, String>
```

### export_usage

Export usage data.

```typescript
// TypeScript
const result = await analytics.exportUsage({
  filter: { provider: 'anthropic' },
  format: 'csv',
});

// Rust signature
#[tauri::command]
pub async fn export_usage(
    analytics_state: State<'_, AnalyticsState>,
    request: ExportRequest,
) -> Result<CommandResponse<ExportResult>, String>
```

### export_by_model

Export usage data by model.

```typescript
// TypeScript
const csv = await analytics.exportByModel(filter, 'csv');

// Rust signature
#[tauri::command]
pub async fn export_by_model(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
    format: ExportFormat,
) -> Result<CommandResponse<String>, String>
```

### export_by_project

Export usage data by project.

```typescript
// TypeScript
const json = await analytics.exportByProject(filter, 'json');

// Rust signature
#[tauri::command]
pub async fn export_by_project(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
    format: ExportFormat,
) -> Result<CommandResponse<String>, String>
```

### export_time_series

Export time series data.

```typescript
// TypeScript
const csv = await analytics.exportTimeSeries(filter, 'daily', 'csv');

// Rust signature
#[tauri::command]
pub async fn export_time_series(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
    period: AggregationPeriod,
    format: ExportFormat,
) -> Result<CommandResponse<String>, String>
```

### export_pricing

Export pricing data.

```typescript
// TypeScript
const json = await analytics.exportPricing();

// Rust signature
#[tauri::command]
pub async fn export_pricing(
    analytics_state: State<'_, AnalyticsState>,
) -> Result<CommandResponse<String>, String>
```

### delete_usage_records

Delete usage records matching filter.

```typescript
// TypeScript
const deleted = await analytics.deleteUsageRecords(filter);

// Rust signature
#[tauri::command]
pub async fn delete_usage_records(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
) -> Result<CommandResponse<i64>, String>
```

### check_analytics_health

Check if analytics service is healthy.

```typescript
// TypeScript
const healthy = await analytics.checkAnalyticsHealth();

// Rust signature
#[tauri::command]
pub async fn check_analytics_health(
    analytics_state: State<'_, AnalyticsState>,
) -> Result<CommandResponse<bool>, String>
```

---

## Quality Gates Commands

### init_quality_gates

Initialize the quality gates service.

```typescript
// TypeScript
import { qualityGates } from './lib/api';
await qualityGates.initQualityGates();

// Rust signature
#[tauri::command]
pub async fn init_quality_gates(
    app_state: State<'_, AppState>,
    quality_state: State<'_, QualityGatesState>,
) -> Result<CommandResponse<bool>, String>
```

### detect_project_type_cmd

Detect the project type for a given path.

```typescript
// TypeScript
const result = await qualityGates.detectProjectType('/path/to/project');

// Rust signature
#[tauri::command]
pub async fn detect_project_type_cmd(
    project_path: String,
) -> Result<CommandResponse<ProjectDetectionResult>, String>
```

**ProjectDetectionResult**:
```typescript
interface ProjectDetectionResult {
  projectType: ProjectType;
  confidence: number;
  indicators: string[];
}
```

**ProjectType**: `"nodejs"` | `"python"` | `"rust"` | `"go"` | `"java"` | `"unknown"`

### get_available_gates

Get available quality gates for a project type.

```typescript
// TypeScript
const gates = await qualityGates.getAvailableGates('nodejs');

// Rust signature
#[tauri::command]
pub async fn get_available_gates(
    project_type: ProjectType,
    quality_state: State<'_, QualityGatesState>,
) -> Result<CommandResponse<Vec<QualityGate>>, String>
```

### list_all_gates

Get all registered quality gates.

```typescript
// TypeScript
const allGates = await qualityGates.listAllGates();

// Rust signature
#[tauri::command]
pub async fn list_all_gates(
    quality_state: State<'_, QualityGatesState>,
) -> Result<CommandResponse<Vec<QualityGate>>, String>
```

### run_quality_gates

Run all quality gates for a project.

```typescript
// TypeScript
const summary = await qualityGates.runQualityGates('/path/to/project', 'session-123');

// Rust signature
#[tauri::command]
pub async fn run_quality_gates(
    app_state: State<'_, AppState>,
    project_path: String,
    session_id: Option<String>,
) -> Result<CommandResponse<GatesSummary>, String>
```

**GatesSummary**:
```typescript
interface GatesSummary {
  total: number;
  passed: number;
  failed: number;
  skipped: number;
  duration_ms: number;
  results: GateResult[];
}
```

### run_specific_gates

Run specific quality gates by ID.

```typescript
// TypeScript
const summary = await qualityGates.runSpecificGates(
  '/path/to/project',
  ['typecheck', 'lint'],
  'session-123'
);

// Rust signature
#[tauri::command]
pub async fn run_specific_gates(
    app_state: State<'_, AppState>,
    project_path: String,
    gate_ids: Vec<String>,
    session_id: Option<String>,
) -> Result<CommandResponse<GatesSummary>, String>
```

### run_custom_gates

Run custom quality gates from configuration.

```typescript
// TypeScript
const summary = await qualityGates.runCustomGates('/path/to/project', [
  { id: 'custom-lint', command: 'npm run lint', required: true }
]);

// Rust signature
#[tauri::command]
pub async fn run_custom_gates(
    app_state: State<'_, AppState>,
    project_path: String,
    custom_gates: Vec<CustomGateConfig>,
    session_id: Option<String>,
) -> Result<CommandResponse<GatesSummary>, String>
```

### get_gate_results

Get stored gate results for a project.

```typescript
// TypeScript
const results = await qualityGates.getGateResults('/path/to/project', 10);

// Rust signature
#[tauri::command]
pub async fn get_gate_results(
    quality_state: State<'_, QualityGatesState>,
    project_path: String,
    limit: Option<i64>,
) -> Result<CommandResponse<Vec<StoredGateResult>>, String>
```

### get_session_gate_results

Get stored gate results for a session.

```typescript
// TypeScript
const results = await qualityGates.getSessionGateResults('session-123');

// Rust signature
#[tauri::command]
pub async fn get_session_gate_results(
    quality_state: State<'_, QualityGatesState>,
    session_id: String,
) -> Result<CommandResponse<Vec<StoredGateResult>>, String>
```

### get_gate_result

Get a single gate result by ID.

```typescript
// TypeScript
const result = await qualityGates.getGateResult(123);

// Rust signature
#[tauri::command]
pub async fn get_gate_result(
    quality_state: State<'_, QualityGatesState>,
    result_id: i64,
) -> Result<CommandResponse<Option<StoredGateResult>>, String>
```

### cleanup_gate_results

Clean up old gate results.

```typescript
// TypeScript
const deleted = await qualityGates.cleanupGateResults(30);

// Rust signature
#[tauri::command]
pub async fn cleanup_gate_results(
    quality_state: State<'_, QualityGatesState>,
    days_old: i64,
) -> Result<CommandResponse<i64>, String>
```

### get_default_gates_for_type

Get default gates for a project type.

```typescript
// TypeScript
const gates = await qualityGates.getDefaultGatesForType('nodejs');

// Rust signature
#[tauri::command]
pub async fn get_default_gates_for_type(
    project_type: ProjectType,
) -> Result<CommandResponse<Vec<QualityGate>>, String>
```

### check_quality_gates_health

Check if quality gates service is healthy.

```typescript
// TypeScript
const healthy = await qualityGates.checkQualityGatesHealth();

// Rust signature
#[tauri::command]
pub async fn check_quality_gates_health(
    quality_state: State<'_, QualityGatesState>,
) -> Result<CommandResponse<bool>, String>
```

---

## Worktree Commands

### create_worktree

Create a new worktree for isolated task execution.

```typescript
// TypeScript
import { worktree } from './lib/api';
const wt = await worktree.createWorktree({
  repoPath: '/path/to/repo',
  taskName: 'feature-auth',
  targetBranch: 'main',
  prdPath: '/path/to/prd.json',
  executionMode: 'auto',
});

// Rust signature
#[tauri::command]
pub async fn create_worktree(
    state: tauri::State<'_, WorktreeState>,
    repo_path: String,
    task_name: String,
    target_branch: String,
    base_path: Option<String>,
    prd_path: Option<String>,
    execution_mode: Option<String>,
) -> Result<CommandResponse<Worktree>, String>
```

**Worktree**:
```typescript
interface Worktree {
  id: string;
  path: string;
  branch: string;
  targetBranch: string;
  taskName: string;
  status: WorktreeTaskStatus;
  createdAt: string;
}
```

### list_worktrees

List all active worktrees in a repository.

```typescript
// TypeScript
const worktrees = await worktree.listWorktrees('/path/to/repo');

// Rust signature
#[tauri::command]
pub async fn list_worktrees(
    state: tauri::State<'_, WorktreeState>,
    repo_path: String,
) -> Result<CommandResponse<Vec<Worktree>>, String>
```

### get_worktree

Get a specific worktree by ID.

```typescript
// TypeScript
const wt = await worktree.getWorktree('/path/to/repo', 'worktree-id');

// Rust signature
#[tauri::command]
pub async fn get_worktree(
    state: tauri::State<'_, WorktreeState>,
    repo_path: String,
    worktree_id: String,
) -> Result<CommandResponse<Worktree>, String>
```

### get_worktree_status

Get the status of a worktree.

```typescript
// TypeScript
const status = await worktree.getWorktreeStatus('/path/to/repo', 'worktree-id');

// Rust signature
#[tauri::command]
pub async fn get_worktree_status(
    state: tauri::State<'_, WorktreeState>,
    repo_path: String,
    worktree_id: String,
) -> Result<CommandResponse<WorktreeStatus>, String>
```

### remove_worktree

Remove a worktree.

```typescript
// TypeScript
await worktree.removeWorktree('/path/to/repo', 'worktree-id', true);

// Rust signature
#[tauri::command]
pub async fn remove_worktree(
    state: tauri::State<'_, WorktreeState>,
    repo_path: String,
    worktree_id: String,
    force: Option<bool>,
) -> Result<CommandResponse<()>, String>
```

### complete_worktree

Complete a worktree: commit code changes, merge to target branch, cleanup.

```typescript
// TypeScript
const result = await worktree.completeWorktree(
  '/path/to/repo',
  'worktree-id',
  'feat: implement user authentication'
);

// Rust signature
#[tauri::command]
pub async fn complete_worktree(
    state: tauri::State<'_, WorktreeState>,
    repo_path: String,
    worktree_id: String,
    commit_message: Option<String>,
) -> Result<CommandResponse<CompleteWorktreeResult>, String>
```

---

## Standalone Execution Commands

### list_providers

List all supported providers and their models.

```typescript
// TypeScript
import { standalone } from './lib/api';
const providers = await standalone.listProviders();

// Rust signature
#[tauri::command]
pub async fn list_providers() -> CommandResponse<Vec<ProviderInfo>>
```

**ProviderInfo**:
```typescript
interface ProviderInfo {
  providerType: string;   // "anthropic" | "openai" | "deepseek" | "ollama"
  name: string;
  models: ModelInfo[];
  requiresApiKey: boolean;
  defaultBaseUrl?: string;
}
```

### configure_provider

Configure a provider (store API key securely).

```typescript
// TypeScript
await standalone.configureProvider('anthropic', 'sk-ant-xxx', null);

// Rust signature
#[tauri::command]
pub async fn configure_provider(
    provider: String,
    api_key: Option<String>,
    base_url: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<bool>, String>
```

### check_provider_health

Check provider health (validate API key and connectivity).

```typescript
// TypeScript
const health = await standalone.checkProviderHealth('anthropic', 'claude-sonnet-4-20250514');

// Rust signature
#[tauri::command]
pub async fn check_provider_health(
    provider: String,
    model: String,
    base_url: Option<String>,
) -> CommandResponse<HealthCheckResult>
```

### execute_standalone

Execute a message in standalone mode.

```typescript
// TypeScript
const result = await standalone.executeStandalone({
  message: 'Implement user authentication',
  provider: 'anthropic',
  model: 'claude-sonnet-4-20250514',
  projectPath: '/path/to/project',
  enableTools: true,
});

// Rust signature
#[tauri::command]
pub async fn execute_standalone(
    message: String,
    provider: String,
    model: String,
    project_path: String,
    system_prompt: Option<String>,
    enable_tools: bool,
    app: AppHandle,
) -> CommandResponse<ExecutionResult>
```

### execute_standalone_with_session

Execute a PRD with session tracking for crash recovery.

```typescript
// TypeScript
const result = await standalone.executeStandaloneWithSession({
  provider: 'anthropic',
  model: 'claude-sonnet-4-20250514',
  projectPath: '/path/to/project',
  prdPath: '/path/to/prd.json',
  storyIds: ['story-001', 'story-002'],
  runQualityGates: true,
});

// Rust signature
#[tauri::command]
pub async fn execute_standalone_with_session(
    request: ExecuteWithSessionRequest,
    app: AppHandle,
    app_state: State<'_, AppState>,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<SessionExecutionResult>, String>
```

### cancel_standalone_execution

Cancel a running standalone execution.

```typescript
// TypeScript
await standalone.cancelStandaloneExecution('session-id');

// Rust signature
#[tauri::command]
pub async fn cancel_standalone_execution(
    session_id: String,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<bool>, String>
```

### get_standalone_status

Get status of all standalone executions.

```typescript
// TypeScript
const status = await standalone.getStandaloneStatus();

// Rust signature
#[tauri::command]
pub async fn get_standalone_status(
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<StandaloneStatus>, String>
```

### get_standalone_progress

Get detailed progress for a specific session.

```typescript
// TypeScript
const progress = await standalone.getStandaloneProgress('session-id');

// Rust signature
#[tauri::command]
pub async fn get_standalone_progress(
    session_id: String,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<ExecutionProgress>, String>
```

### resume_standalone_execution

Resume a paused or failed execution.

```typescript
// TypeScript
const result = await standalone.resumeStandaloneExecution({
  sessionId: 'session-id',
  skipCurrent: false,
  retryFailed: true,
});

// Rust signature
#[tauri::command]
pub async fn resume_standalone_execution(
    request: ResumeExecutionRequest,
    app: AppHandle,
    app_state: State<'_, AppState>,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<SessionExecutionResult>, String>
```

### get_standalone_session

Get a specific execution session by ID.

```typescript
// TypeScript
const session = await standalone.getStandaloneSession('session-id');

// Rust signature
#[tauri::command]
pub async fn get_standalone_session(
    session_id: String,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<ExecutionSession>, String>
```

### list_standalone_sessions

List all execution sessions with optional status filter.

```typescript
// TypeScript
const sessions = await standalone.listStandaloneSessions('running', 50);

// Rust signature
#[tauri::command]
pub async fn list_standalone_sessions(
    status: Option<String>,
    limit: Option<usize>,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<ExecutionSessionSummary>>, String>
```

### delete_standalone_session

Delete an execution session.

```typescript
// TypeScript
await standalone.deleteStandaloneSession('session-id');

// Rust signature
#[tauri::command]
pub async fn delete_standalone_session(
    session_id: String,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<bool>, String>
```

### cleanup_standalone_sessions

Cleanup old completed sessions.

```typescript
// TypeScript
const deleted = await standalone.cleanupStandaloneSessions(30);

// Rust signature
#[tauri::command]
pub async fn cleanup_standalone_sessions(
    days: i64,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<usize>, String>
```

### get_usage_stats

Get usage statistics from the database.

```typescript
// TypeScript
const stats = await standalone.getUsageStats('anthropic', 'claude-sonnet-4-20250514');

// Rust signature
#[tauri::command]
pub async fn get_usage_stats(
    provider: Option<String>,
    model: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<UsageStatistics>, String>
```

---

## Timeline Commands

### create_checkpoint

Create a new checkpoint.

```typescript
// TypeScript
import { timeline } from './lib/api';
const checkpoint = await timeline.createCheckpoint(
  '/path/to/project',
  'session-id',
  'Before OAuth implementation',
  ['src/auth.ts', 'src/config.ts']
);

// Rust signature
#[tauri::command]
pub fn create_checkpoint(
    project_path: String,
    session_id: String,
    label: String,
    tracked_files: Vec<String>,
) -> Result<CommandResponse<Checkpoint>, String>
```

### list_checkpoints

List all checkpoints for a session.

```typescript
// TypeScript
const checkpoints = await timeline.listCheckpoints('/path/to/project', 'session-id');

// Rust signature
#[tauri::command]
pub fn list_checkpoints(
    project_path: String,
    session_id: String,
    branch_id: Option<String>,
) -> Result<CommandResponse<Vec<Checkpoint>>, String>
```

### get_checkpoint

Get a single checkpoint by ID.

```typescript
// TypeScript
const checkpoint = await timeline.getCheckpoint('/path/to/project', 'session-id', 'checkpoint-id');

// Rust signature
#[tauri::command]
pub fn get_checkpoint(
    project_path: String,
    session_id: String,
    checkpoint_id: String,
) -> Result<CommandResponse<Checkpoint>, String>
```

### delete_checkpoint

Delete a checkpoint.

```typescript
// TypeScript
await timeline.deleteCheckpoint('/path/to/project', 'session-id', 'checkpoint-id');

// Rust signature
#[tauri::command]
pub fn delete_checkpoint(
    project_path: String,
    session_id: String,
    checkpoint_id: String,
) -> Result<CommandResponse<()>, String>
```

### get_timeline

Get the full timeline metadata for a session.

```typescript
// TypeScript
const metadata = await timeline.getTimeline('/path/to/project', 'session-id');

// Rust signature
#[tauri::command]
pub fn get_timeline(
    project_path: String,
    session_id: String,
) -> Result<CommandResponse<TimelineMetadata>, String>
```

### restore_checkpoint

Restore to a checkpoint.

```typescript
// TypeScript
const result = await timeline.restoreCheckpoint(
  '/path/to/project',
  'session-id',
  'checkpoint-id',
  true, // create backup
  ['src/auth.ts']
);

// Rust signature
#[tauri::command]
pub fn restore_checkpoint(
    project_path: String,
    session_id: String,
    checkpoint_id: String,
    create_backup: bool,
    current_tracked_files: Vec<String>,
) -> Result<CommandResponse<RestoreResult>, String>
```

### fork_branch

Fork a new branch from a checkpoint.

```typescript
// TypeScript
const branch = await timeline.forkBranch(
  '/path/to/project',
  'session-id',
  'checkpoint-id',
  'try-jwt-approach'
);

// Rust signature
#[tauri::command]
pub fn fork_branch(
    project_path: String,
    session_id: String,
    checkpoint_id: String,
    branch_name: String,
) -> Result<CommandResponse<CheckpointBranch>, String>
```

### list_branches

List all branches for a session.

```typescript
// TypeScript
const branches = await timeline.listBranches('/path/to/project', 'session-id');

// Rust signature
#[tauri::command]
pub fn list_branches(
    project_path: String,
    session_id: String,
) -> Result<CommandResponse<Vec<CheckpointBranch>>, String>
```

### get_branch

Get a single branch by ID.

```typescript
// TypeScript
const branch = await timeline.getBranch('/path/to/project', 'session-id', 'branch-id');

// Rust signature
#[tauri::command]
pub fn get_branch(
    project_path: String,
    session_id: String,
    branch_id: String,
) -> Result<CommandResponse<CheckpointBranch>, String>
```

### switch_branch

Switch to a different branch.

```typescript
// TypeScript
const branch = await timeline.switchBranch('/path/to/project', 'session-id', 'branch-id');

// Rust signature
#[tauri::command]
pub fn switch_branch(
    project_path: String,
    session_id: String,
    branch_id: String,
) -> Result<CommandResponse<CheckpointBranch>, String>
```

### delete_branch

Delete a branch.

```typescript
// TypeScript
await timeline.deleteBranch('/path/to/project', 'session-id', 'branch-id');

// Rust signature
#[tauri::command]
pub fn delete_branch(
    project_path: String,
    session_id: String,
    branch_id: String,
) -> Result<CommandResponse<()>, String>
```

### rename_branch

Rename a branch.

```typescript
// TypeScript
const branch = await timeline.renameBranch('/path/to/project', 'session-id', 'branch-id', 'new-name');

// Rust signature
#[tauri::command]
pub fn rename_branch(
    project_path: String,
    session_id: String,
    branch_id: String,
    new_name: String,
) -> Result<CommandResponse<CheckpointBranch>, String>
```

### get_checkpoint_diff

Calculate diff between two checkpoints.

```typescript
// TypeScript
const diff = await timeline.getCheckpointDiff(
  '/path/to/project',
  'session-id',
  'from-checkpoint-id',
  'to-checkpoint-id'
);

// Rust signature
#[tauri::command]
pub fn get_checkpoint_diff(
    project_path: String,
    session_id: String,
    from_checkpoint_id: String,
    to_checkpoint_id: String,
) -> Result<CommandResponse<CheckpointDiff>, String>
```

### get_diff_from_current

Get diff from a checkpoint to current state.

```typescript
// TypeScript
const diff = await timeline.getDiffFromCurrent(
  '/path/to/project',
  'session-id',
  'checkpoint-id',
  ['src/auth.ts']
);

// Rust signature
#[tauri::command]
pub fn get_diff_from_current(
    project_path: String,
    session_id: String,
    checkpoint_id: String,
    tracked_files: Vec<String>,
) -> Result<CommandResponse<CheckpointDiff>, String>
```

---

## MCP Commands

### list_mcp_servers

List all MCP servers.

```typescript
// TypeScript
import { mcp } from './lib/api';
const servers = await mcp.listMcpServers();

// Rust signature
#[tauri::command]
pub fn list_mcp_servers() -> Result<CommandResponse<Vec<McpServer>>, String>
```

### add_mcp_server

Add a new MCP server.

```typescript
// TypeScript
const server = await mcp.addMcpServer({
  name: 'filesystem',
  serverType: 'stdio',
  command: 'npx',
  args: ['-y', '@anthropic/mcp-server-filesystem'],
  env: { ALLOWED_PATHS: '/home/user/projects' },
});

// Rust signature
#[tauri::command]
pub fn add_mcp_server(
    name: String,
    server_type: String,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    url: Option<String>,
    headers: Option<HashMap<String, String>>,
) -> Result<CommandResponse<McpServer>, String>
```

### update_mcp_server

Update an existing MCP server.

```typescript
// TypeScript
const server = await mcp.updateMcpServer('server-id', { enabled: false });

// Rust signature
#[tauri::command]
pub fn update_mcp_server(
    id: String,
    name: Option<String>,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    url: Option<String>,
    headers: Option<HashMap<String, String>>,
    enabled: Option<bool>,
) -> Result<CommandResponse<McpServer>, String>
```

### remove_mcp_server

Remove an MCP server.

```typescript
// TypeScript
await mcp.removeMcpServer('server-id');

// Rust signature
#[tauri::command]
pub fn remove_mcp_server(id: String) -> Result<CommandResponse<()>, String>
```

### test_mcp_server

Test an MCP server connection.

```typescript
// TypeScript
const result = await mcp.testMcpServer('server-id');

// Rust signature
#[tauri::command]
pub async fn test_mcp_server(id: String) -> Result<CommandResponse<HealthCheckResult>, String>
```

### toggle_mcp_server

Toggle MCP server enabled status.

```typescript
// TypeScript
const server = await mcp.toggleMcpServer('server-id', true);

// Rust signature
#[tauri::command]
pub fn toggle_mcp_server(id: String, enabled: bool) -> Result<CommandResponse<McpServer>, String>
```

### import_from_claude_desktop

Import MCP servers from Claude Desktop configuration.

```typescript
// TypeScript
const result = await mcp.importFromClaudeDesktop();

// Rust signature
#[tauri::command]
pub fn import_from_claude_desktop() -> Result<CommandResponse<ImportResult>, String>
```

---

## Markdown Commands

### scan_claude_md

Scan a directory for all CLAUDE.md files.

```typescript
// TypeScript
import { scanClaudeMd } from './lib/api';
const files = await scanClaudeMd('/path/to/projects');

// Rust signature
#[tauri::command]
pub fn scan_claude_md(root_path: String) -> Result<CommandResponse<Vec<ClaudeMdFile>>, String>
```

### read_claude_md

Read the content of a CLAUDE.md file.

```typescript
// TypeScript
import { readClaudeMd } from './lib/api';
const content = await readClaudeMd('/path/to/CLAUDE.md');

// Rust signature
#[tauri::command]
pub fn read_claude_md(path: String) -> Result<CommandResponse<ClaudeMdContent>, String>
```

### save_claude_md

Save content to a CLAUDE.md file.

```typescript
// TypeScript
import { saveClaudeMd } from './lib/api';
const result = await saveClaudeMd('/path/to/CLAUDE.md', '# My Project\n...');

// Rust signature
#[tauri::command]
pub fn save_claude_md(path: String, content: String) -> Result<CommandResponse<SaveResult>, String>
```

### create_claude_md

Create a new CLAUDE.md file from a template.

```typescript
// TypeScript
import { createClaudeMd } from './lib/api';
const result = await createClaudeMd('/path/to/new/CLAUDE.md', '# New Project\n...');

// Rust signature
#[tauri::command]
pub fn create_claude_md(
    path: String,
    template_content: String,
) -> Result<CommandResponse<SaveResult>, String>
```

### get_claude_md_metadata

Get file metadata for a CLAUDE.md file.

```typescript
// TypeScript
import { getClaudeMdMetadata } from './lib/api';
const metadata = await getClaudeMdMetadata('/path/to/CLAUDE.md');

// Rust signature
#[tauri::command]
pub fn get_claude_md_metadata(path: String) -> Result<CommandResponse<FileMetadata>, String>
```

---

## Claude Code Commands

### start_chat

Start a new Claude Code chat session.

```typescript
// TypeScript
// Internal command - used by ClaudeCodeState

// Rust signature
#[tauri::command]
pub async fn start_chat(
    request: StartChatRequest,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<StartChatResponse>, String>
```

### send_message

Send a message to a Claude Code session.

```typescript
// TypeScript
// Internal command - used by ClaudeCodeState

// Rust signature
#[tauri::command]
pub async fn send_message(
    request: SendMessageRequest,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<bool>, String>
```

### cancel_execution

Cancel the current execution in a session.

```typescript
// TypeScript
// Internal command - used by ClaudeCodeState

// Rust signature
#[tauri::command]
pub async fn cancel_execution(
    session_id: String,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<bool>, String>
```

### get_session_history

Get the history/details for a session.

```typescript
// TypeScript
// Internal command - used by ClaudeCodeState

// Rust signature
#[tauri::command]
pub async fn get_session_history(
    session_id: String,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<ClaudeCodeSession>, String>
```

### list_active_sessions

List all active Claude Code sessions.

```typescript
// TypeScript
// Internal command - used by ClaudeCodeState

// Rust signature
#[tauri::command]
pub async fn list_active_sessions(
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<Vec<ActiveSessionInfo>>, String>
```

### remove_session

Remove a session completely.

```typescript
// TypeScript
// Internal command - used by ClaudeCodeState

// Rust signature
#[tauri::command]
pub async fn remove_session(
    session_id: String,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<bool>, String>
```

### get_session_info

Get information about a specific session including process status.

```typescript
// TypeScript
// Internal command - used by ClaudeCodeState

// Rust signature
#[tauri::command]
pub async fn get_session_info(
    session_id: String,
    state: State<'_, ClaudeCodeState>,
) -> Result<CommandResponse<ActiveSessionInfo>, String>
```

---

## Events

The application emits several Tauri events for real-time updates:

### Standalone Events

- `standalone-event`: General streaming events during execution
- `session-event-{session_id}`: Session-specific streaming events
- `standalone-session-event`: Dashboard-level session events

### Event Types

```typescript
interface UnifiedStreamEvent {
  type: 'text_delta' | 'thinking_start' | 'thinking_delta' | 'thinking_end' |
        'tool_start' | 'tool_result' | 'usage' | 'error' | 'complete';
  // Event-specific data
}
```

**Example: Listening to events**
```typescript
import { listen } from '@tauri-apps/api/event';

// Listen to session events
const unlisten = await listen('standalone-event', (event) => {
  const streamEvent = event.payload as UnifiedStreamEvent;
  switch (streamEvent.type) {
    case 'text_delta':
      // Handle text streaming
      break;
    case 'tool_start':
      // Handle tool execution start
      break;
    case 'complete':
      // Handle completion
      break;
  }
});

// Clean up when done
unlisten();
```

---

## Error Handling

All commands return `CommandResponse<T>` with error information:

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

Using the TypeScript API wrapper with `ApiError`:

```typescript
import { ApiError } from './lib/api';

try {
  const data = await projects.listProjects();
} catch (error) {
  if (error instanceof ApiError) {
    console.error('API Error:', error.message);
  }
}
```

---

## Command Count Summary

| Category | Commands |
|----------|----------|
| Initialization | 2 |
| Health | 1 |
| Settings | 2 |
| Projects | 3 |
| Sessions | 4 |
| Agents | 14 |
| Analytics | 22 |
| Quality Gates | 13 |
| Worktree | 6 |
| Standalone | 14 |
| Timeline | 15 |
| MCP | 7 |
| Markdown | 5 |
| Claude Code | 7 |
| **Total** | **115** |
