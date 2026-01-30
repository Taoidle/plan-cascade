[中文版](Design-Plan-Cascade-Standalone_zh.md)

# Plan Cascade Desktop - Technical Design Document

**Version**: 5.0.0
**Date**: 2026-01-30
**Author**: Plan Cascade Team
**Status**: Architecture Redesign

---

## Implementation Status Overview

> **Current Progress**: Architecture redesign in progress
> **Last Updated**: 2026-01-30

### Architecture Changes (v5.0)

| Component | Previous (v4.x) | New (v5.0) |
|-----------|-----------------|------------|
| Backend | Python Sidecar (FastAPI) | Pure Rust |
| IPC | HTTP/WebSocket | Tauri IPC (direct) |
| Database | None | SQLite (embedded) |
| Secrets | Python keyring | Rust keyring |
| Distribution | Python + Tauri | Single binary |

### Module Implementation Status

| Module | Status | Location | Notes |
|--------|--------|----------|-------|
| **Rust Backend Core** | | | |
| Tauri Commands | 🔄 Redesign | `src-tauri/src/commands/` | All business logic |
| Claude Code Integration | 🔄 Redesign | `src-tauri/src/services/claude_code/` | CLI wrapper |
| SQLite Storage | ⏳ Planning | `src-tauri/src/storage/` | Embedded database |
| **Services** | | | |
| Project Manager | ⏳ Planning | `src-tauri/src/services/project/` | ~/.claude/projects/ |
| Agent Executor | ⏳ Planning | `src-tauri/src/services/agent/` | Custom agents |
| Analytics Tracker | ⏳ Planning | `src-tauri/src/services/analytics/` | Usage tracking |
| MCP Registry | ⏳ Planning | `src-tauri/src/services/mcp/` | Server management |
| Timeline Manager | ⏳ Planning | `src-tauri/src/services/timeline/` | Checkpoints |
| Markdown Editor | ⏳ Planning | `src-tauri/src/services/markdown/` | CLAUDE.md |
| **Execution Layer** | | | |
| Claude Code Mode | 🔄 Redesign | `src-tauri/src/execution/claude_code/` | CLI integration |
| Standalone Mode | ⏳ Planning | `src-tauri/src/execution/standalone/` | Direct LLM API |
| **Frontend** | | | |
| React Components | 🔄 In Progress | `src/components/` | UI components |
| Zustand Stores | 🔄 In Progress | `src/store/` | State management |

---

## 1. Design Goals

### 1.1 Core Objectives

1. **Pure Rust Backend**: No Python dependency, single executable distribution
2. **Comprehensive Features**: Projects, Agents, Analytics, MCP, Timeline, CLAUDE.md
3. **Dual Working Modes**: Claude Code GUI mode + Standalone orchestration mode
4. **High Performance**: Fast startup (<2s), low memory (<200MB), responsive UI

### 1.2 Design Constraints

| Constraint | Description |
|------------|-------------|
| Single Binary | All functionality in one executable, no external dependencies |
| Cross-Platform | Windows, macOS, Linux support with consistent behavior |
| Claude Code Compatible | Full integration with Claude Code CLI when available |
| Offline Capable | Core features work without internet (except LLM calls) |
| Secure Storage | API keys in OS keychain, encrypted local database |

---

## 2. System Architecture

### 2.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Plan Cascade Desktop v5.0                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                      React Frontend (TypeScript)                      │   │
│   │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐       │   │
│   │  │Projects │ │ Agents  │ │Analytics│ │   MCP   │ │Timeline │       │   │
│   │  │ Browser │ │ Library │ │Dashboard│ │ Servers │ │  View   │       │   │
│   │  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘       │   │
│   │       └───────────┴───────────┴───────────┴───────────┘             │   │
│   │                              │                                       │   │
│   │                     Zustand State Management                         │   │
│   │                              │                                       │   │
│   └──────────────────────────────┼───────────────────────────────────────┘   │
│                                  │                                           │
│                          Tauri IPC Bridge                                    │
│                                  │                                           │
│   ┌──────────────────────────────┼───────────────────────────────────────┐   │
│   │                      Rust Backend                                     │   │
│   │                              │                                        │   │
│   │   ┌──────────────────────────┴──────────────────────────────────┐    │   │
│   │   │                    Command Layer                             │    │   │
│   │   │  projects:: │ agents:: │ analytics:: │ mcp:: │ timeline::   │    │   │
│   │   └──────────────────────────┬──────────────────────────────────┘    │   │
│   │                              │                                        │   │
│   │   ┌──────────────────────────┴──────────────────────────────────┐    │   │
│   │   │                    Service Layer                             │    │   │
│   │   │  ┌────────────┐ ┌────────────┐ ┌────────────┐               │    │   │
│   │   │  │  Project   │ │   Agent    │ │ Analytics  │               │    │   │
│   │   │  │  Manager   │ │  Executor  │ │  Tracker   │               │    │   │
│   │   │  └────────────┘ └────────────┘ └────────────┘               │    │   │
│   │   │  ┌────────────┐ ┌────────────┐ ┌────────────┐               │    │   │
│   │   │  │    MCP     │ │  Timeline  │ │  Markdown  │               │    │   │
│   │   │  │  Registry  │ │  Manager   │ │   Editor   │               │    │   │
│   │   │  └────────────┘ └────────────┘ └────────────┘               │    │   │
│   │   └──────────────────────────┬──────────────────────────────────┘    │   │
│   │                              │                                        │   │
│   │   ┌──────────────────────────┴──────────────────────────────────┐    │   │
│   │   │                   Execution Layer                            │    │   │
│   │   │  ┌─────────────────────┐    ┌─────────────────────┐         │    │   │
│   │   │  │  Claude Code Mode   │    │   Standalone Mode   │         │    │   │
│   │   │  │  ┌───────────────┐  │    │  ┌───────────────┐  │         │    │   │
│   │   │  │  │ CLI Executor  │  │    │  │ LLM Providers │  │         │    │   │
│   │   │  │  │ Stream Parser │  │    │  │ Tool Executor │  │         │    │   │
│   │   │  │  │ Session Mgmt  │  │    │  │ Orchestrator  │  │         │    │   │
│   │   │  │  └───────────────┘  │    │  └───────────────┘  │         │    │   │
│   │   │  └─────────────────────┘    └─────────────────────┘         │    │   │
│   │   └──────────────────────────┬──────────────────────────────────┘    │   │
│   │                              │                                        │   │
│   │   ┌──────────────────────────┴──────────────────────────────────┐    │   │
│   │   │                    Storage Layer                             │    │   │
│   │   │  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐    │    │   │
│   │   │  │  SQLite   │ │  Keyring  │ │   File    │ │   JSON    │    │    │   │
│   │   │  │ Database  │ │  Secrets  │ │  System   │ │  Config   │    │    │   │
│   │   │  └───────────┘ └───────────┘ └───────────┘ └───────────┘    │    │   │
│   │   └─────────────────────────────────────────────────────────────┘    │   │
│   │                                                                       │   │
│   └───────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Directory Structure

```
desktop/
├── src/                              # React Frontend
│   ├── main.tsx                      # Entry point
│   ├── App.tsx                       # Root component
│   │
│   ├── components/                   # UI Components
│   │   ├── Layout/                   # Layout components
│   │   │   ├── Sidebar.tsx
│   │   │   ├── Header.tsx
│   │   │   └── MainContent.tsx
│   │   │
│   │   ├── Projects/                 # Project & Session Management
│   │   │   ├── ProjectBrowser.tsx
│   │   │   ├── ProjectCard.tsx
│   │   │   ├── SessionList.tsx
│   │   │   ├── SessionDetail.tsx
│   │   │   └── SearchBar.tsx
│   │   │
│   │   ├── Agents/                   # CC Agents
│   │   │   ├── AgentLibrary.tsx
│   │   │   ├── AgentCard.tsx
│   │   │   ├── AgentEditor.tsx
│   │   │   ├── AgentRunner.tsx
│   │   │   └── RunHistory.tsx
│   │   │
│   │   ├── Analytics/                # Usage Analytics
│   │   │   ├── Dashboard.tsx
│   │   │   ├── CostChart.tsx
│   │   │   ├── TokenBreakdown.tsx
│   │   │   ├── UsageTable.tsx
│   │   │   └── ExportButton.tsx
│   │   │
│   │   ├── MCP/                      # MCP Server Management
│   │   │   ├── ServerRegistry.tsx
│   │   │   ├── ServerCard.tsx
│   │   │   ├── AddServerDialog.tsx
│   │   │   ├── ImportDialog.tsx
│   │   │   └── HealthIndicator.tsx
│   │   │
│   │   ├── Timeline/                 # Timeline & Checkpoints
│   │   │   ├── TimelineView.tsx
│   │   │   ├── CheckpointNode.tsx
│   │   │   ├── BranchTree.tsx
│   │   │   ├── DiffViewer.tsx
│   │   │   └── RestoreDialog.tsx
│   │   │
│   │   ├── Markdown/                 # CLAUDE.md Editor
│   │   │   ├── MarkdownEditor.tsx
│   │   │   ├── FileTree.tsx
│   │   │   ├── Preview.tsx
│   │   │   └── SyntaxHighlight.tsx
│   │   │
│   │   ├── Execution/                # Task Execution (existing)
│   │   │   ├── SimpleMode/
│   │   │   ├── ExpertMode/
│   │   │   └── ClaudeCodeMode/
│   │   │
│   │   └── Settings/                 # Settings (existing)
│   │       └── ...
│   │
│   ├── store/                        # Zustand State Management
│   │   ├── index.ts
│   │   ├── projects.ts               # Projects & sessions state
│   │   ├── agents.ts                 # Agent library state
│   │   ├── analytics.ts              # Analytics data state
│   │   ├── mcp.ts                    # MCP servers state
│   │   ├── timeline.ts               # Timeline state
│   │   ├── markdown.ts               # Markdown editor state
│   │   ├── execution.ts              # Execution state
│   │   ├── settings.ts               # Settings state
│   │   └── claudeCode.ts             # Claude Code mode state
│   │
│   ├── lib/                          # Utilities
│   │   ├── tauri.ts                  # Tauri API wrapper
│   │   ├── api.ts                    # Backend API calls
│   │   └── utils.ts                  # Helper functions
│   │
│   └── i18n/                         # Internationalization
│       ├── index.ts
│       └── locales/
│
├── src-tauri/                        # Rust Backend
│   ├── Cargo.toml                    # Rust dependencies
│   ├── tauri.conf.json               # Tauri configuration
│   ├── build.rs                      # Build script
│   │
│   └── src/
│       ├── main.rs                   # Entry point
│       ├── lib.rs                    # Library root
│       │
│       ├── commands/                 # Tauri Commands (IPC handlers)
│       │   ├── mod.rs
│       │   ├── projects.rs           # Project management commands
│       │   ├── agents.rs             # Agent management commands
│       │   ├── analytics.rs          # Analytics commands
│       │   ├── mcp.rs                # MCP server commands
│       │   ├── timeline.rs           # Timeline commands
│       │   ├── markdown.rs           # Markdown editor commands
│       │   ├── execution.rs          # Task execution commands
│       │   └── settings.rs           # Settings commands
│       │
│       ├── services/                 # Business Logic
│       │   ├── mod.rs
│       │   │
│       │   ├── project/              # Project & Session Management
│       │   │   ├── mod.rs
│       │   │   ├── scanner.rs        # Scan ~/.claude/projects/
│       │   │   ├── session.rs        # Session management
│       │   │   ├── search.rs         # Search functionality
│       │   │   └── metadata.rs       # Parse session metadata
│       │   │
│       │   ├── agent/                # CC Agents
│       │   │   ├── mod.rs
│       │   │   ├── registry.rs       # Agent registry
│       │   │   ├── executor.rs       # Agent execution
│       │   │   ├── builder.rs        # Agent builder
│       │   │   └── history.rs        # Execution history
│       │   │
│       │   ├── analytics/            # Usage Analytics
│       │   │   ├── mod.rs
│       │   │   ├── tracker.rs        # Usage tracking
│       │   │   ├── cost.rs           # Cost calculation
│       │   │   ├── aggregator.rs     # Data aggregation
│       │   │   └── export.rs         # Data export
│       │   │
│       │   ├── mcp/                  # MCP Server Management
│       │   │   ├── mod.rs
│       │   │   ├── registry.rs       # Server registry
│       │   │   ├── config.rs         # Configuration
│       │   │   ├── health.rs         # Health checking
│       │   │   └── importer.rs       # Import from Claude Desktop
│       │   │
│       │   ├── timeline/             # Timeline & Checkpoints
│       │   │   ├── mod.rs
│       │   │   ├── checkpoint.rs     # Checkpoint management
│       │   │   ├── branch.rs         # Branch management
│       │   │   ├── diff.rs           # Diff calculation
│       │   │   └── restore.rs        # State restoration
│       │   │
│       │   └── markdown/             # CLAUDE.md Management
│       │       ├── mod.rs
│       │       ├── scanner.rs        # Find CLAUDE.md files
│       │       ├── editor.rs         # Edit operations
│       │       └── renderer.rs       # Markdown rendering
│       │
│       ├── execution/                # Execution Layer
│       │   ├── mod.rs
│       │   │
│       │   ├── claude_code/          # Claude Code Mode
│       │   │   ├── mod.rs
│       │   │   ├── cli.rs            # CLI execution
│       │   │   ├── parser.rs         # Stream JSON parser
│       │   │   └── session.rs        # Session management
│       │   │
│       │   ├── standalone/           # Standalone Mode
│       │   │   ├── mod.rs
│       │   │   ├── llm/              # LLM Providers
│       │   │   │   ├── mod.rs
│       │   │   │   ├── provider.rs   # Provider trait
│       │   │   │   ├── anthropic.rs  # Claude API
│       │   │   │   ├── openai.rs     # OpenAI API
│       │   │   │   ├── deepseek.rs   # DeepSeek API
│       │   │   │   └── ollama.rs     # Ollama (local)
│       │   │   │
│       │   │   └── tools/            # Tool Execution
│       │   │       ├── mod.rs
│       │   │       ├── registry.rs   # Tool registry
│       │   │       ├── file.rs       # Read/Write/Edit
│       │   │       ├── search.rs     # Glob/Grep
│       │   │       └── shell.rs      # Bash execution
│       │   │
│       │   └── orchestration/        # Shared Orchestration
│       │       ├── mod.rs
│       │       ├── prd.rs            # PRD generation
│       │       ├── strategy.rs       # Strategy analysis
│       │       └── batch.rs          # Batch scheduling
│       │
│       ├── storage/                  # Storage Layer
│       │   ├── mod.rs
│       │   ├── database.rs           # SQLite operations
│       │   ├── keyring.rs            # Secure secret storage
│       │   ├── config.rs             # JSON configuration
│       │   └── cache.rs              # Cache management
│       │
│       ├── models/                   # Data Models
│       │   ├── mod.rs
│       │   ├── project.rs            # Project & Session models
│       │   ├── agent.rs              # Agent models
│       │   ├── analytics.rs          # Analytics models
│       │   ├── mcp.rs                # MCP server models
│       │   ├── timeline.rs           # Timeline models
│       │   └── settings.rs           # Settings models
│       │
│       └── utils/                    # Utilities
│           ├── mod.rs
│           ├── paths.rs              # Path handling
│           ├── json.rs               # JSON utilities
│           └── error.rs              # Error handling
│
├── package.json                      # Frontend dependencies
├── vite.config.ts                    # Vite configuration
├── tailwind.config.js                # Tailwind CSS
└── tsconfig.json                     # TypeScript config
```

---

## 3. Worktree Management Service

### 3.1 Worktree Manager

```rust
// src-tauri/src/services/worktree/manager.rs

use std::path::PathBuf;
use anyhow::Result;

pub struct WorktreeManager {
    project_root: PathBuf,
    worktrees_dir: PathBuf,
}

impl WorktreeManager {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            worktrees_dir: project_root.join(".worktrees"),
            project_root,
        }
    }

    /// Create a new worktree for a task
    pub async fn create(
        &self,
        task_name: &str,
        target_branch: &str,
    ) -> Result<WorktreeInfo> {
        let worktree_path = self.worktrees_dir.join(task_name);
        let task_branch = format!("task/{}", task_name);

        // Create branch from target
        self.run_git(&["checkout", "-b", &task_branch, target_branch]).await?;

        // Create worktree
        self.run_git(&["worktree", "add", worktree_path.to_str().unwrap(), &task_branch]).await?;

        // Initialize planning config
        let config = PlanningConfig {
            mode: "hybrid".to_string(),
            task_name: task_name.to_string(),
            task_branch: task_branch.clone(),
            target_branch: target_branch.to_string(),
            created_at: chrono::Utc::now(),
        };
        self.write_planning_config(&worktree_path, &config).await?;

        Ok(WorktreeInfo {
            path: worktree_path,
            branch: task_branch,
            target_branch: target_branch.to_string(),
        })
    }

    /// Complete a worktree task (commit, merge, cleanup)
    pub async fn complete(
        &self,
        task_name: &str,
        commit_message: &str,
    ) -> Result<()> {
        let worktree_path = self.worktrees_dir.join(task_name);
        let config = self.read_planning_config(&worktree_path).await?;

        // Stage and commit (exclude planning files)
        self.run_git_in(&worktree_path, &["add", "-A"]).await?;
        self.run_git_in(&worktree_path, &[
            "reset", "HEAD", "--",
            "prd.json", "progress.txt", "findings.md",
            ".agent-status.json", ".iteration-state.json",
            ".planning-config.json"
        ]).await?;
        self.run_git_in(&worktree_path, &["commit", "-m", commit_message]).await?;

        // Switch to target branch and merge
        self.run_git(&["checkout", &config.target_branch]).await?;
        self.run_git(&["merge", &config.task_branch]).await?;

        // Cleanup worktree and branch
        self.run_git(&["worktree", "remove", worktree_path.to_str().unwrap()]).await?;
        self.run_git(&["branch", "-d", &config.task_branch]).await?;

        Ok(())
    }

    /// List all active worktrees
    pub async fn list(&self) -> Result<Vec<WorktreeInfo>> {
        let output = self.run_git(&["worktree", "list", "--porcelain"]).await?;
        self.parse_worktree_list(&output)
    }

    async fn run_git(&self, args: &[&str]) -> Result<String> {
        use tokio::process::Command;
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.project_root)
            .output()
            .await?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: String,
    pub target_branch: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanningConfig {
    pub mode: String,
    pub task_name: String,
    pub task_branch: String,
    pub target_branch: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

### 3.2 Mega Plan Orchestrator

```rust
// src-tauri/src/services/mega/orchestrator.rs

use crate::models::mega::{MegaPlan, Feature, FeatureStatus, MegaStatus};
use crate::services::worktree::WorktreeManager;
use anyhow::Result;

pub struct MegaOrchestrator {
    worktree_manager: WorktreeManager,
    mega_plan: MegaPlan,
    status: MegaStatus,
}

impl MegaOrchestrator {
    /// Execute the mega plan with full automation
    pub async fn execute_auto(&mut self) -> Result<()> {
        loop {
            // Get current batch features
            let batch = self.get_current_batch();
            if batch.is_empty() {
                break;  // All batches complete
            }

            // Create worktrees for batch (from updated target branch)
            for feature in &batch {
                let worktree = self.worktree_manager.create(
                    &feature.id,
                    &self.mega_plan.target_branch,
                ).await?;

                self.status.features.insert(feature.id.clone(), FeatureState {
                    status: FeatureStatus::InProgress,
                    worktree: Some(worktree.path.clone()),
                    prd_generated: false,
                    stories_completed: 0,
                    stories_total: 0,
                });
            }

            // Generate PRDs for each feature (parallel Task agents)
            self.generate_prds_parallel(&batch).await?;

            // Execute stories for each feature (parallel Task agents)
            self.execute_features_parallel(&batch).await?;

            // Wait for all features in batch to complete
            self.wait_for_batch_completion(&batch).await?;

            // Merge all features in batch to target branch
            for feature in &batch {
                self.worktree_manager.complete(
                    &feature.id,
                    &format!("feat({}): {}", feature.id, feature.name),
                ).await?;

                self.status.features.get_mut(&feature.id).unwrap().status =
                    FeatureStatus::Completed;
            }

            // Advance to next batch
            self.status.completed_batches.push(self.status.current_batch);
            self.status.current_batch += 1;
            self.save_status().await?;
        }

        Ok(())
    }

    /// Get features for current batch (based on dependencies)
    fn get_current_batch(&self) -> Vec<&Feature> {
        let completed: std::collections::HashSet<_> = self.status.features
            .iter()
            .filter(|(_, s)| s.status == FeatureStatus::Completed)
            .map(|(id, _)| id.as_str())
            .collect();

        self.mega_plan.features
            .iter()
            .filter(|f| {
                f.status != FeatureStatus::Completed &&
                f.dependencies.iter().all(|dep| completed.contains(dep.as_str()))
            })
            .collect()
    }

    /// Generate PRDs for features using parallel Task agents
    async fn generate_prds_parallel(&mut self, features: &[&Feature]) -> Result<()> {
        use futures::future::join_all;

        let tasks: Vec<_> = features.iter().map(|f| {
            self.spawn_prd_generation_agent(&f.id, &f.description)
        }).collect();

        let results = join_all(tasks).await;

        for (feature, result) in features.iter().zip(results) {
            result?;
            self.status.features.get_mut(&feature.id).unwrap().prd_generated = true;
        }

        Ok(())
    }

    /// Execute all stories for features using parallel Task agents
    async fn execute_features_parallel(&mut self, features: &[&Feature]) -> Result<()> {
        use futures::future::join_all;

        let tasks: Vec<_> = features.iter().map(|f| {
            self.spawn_feature_execution_agent(&f.id)
        }).collect();

        join_all(tasks).await;
        Ok(())
    }
}
```

---

## 4. Dependency Analysis

### 4.1 Batch Generation Algorithm

```rust
// src-tauri/src/services/dependency/analyzer.rs

use crate::models::prd::{Prd, Story};
use std::collections::{HashMap, HashSet};

pub struct DependencyAnalyzer;

impl DependencyAnalyzer {
    /// Generate execution batches from PRD stories
    pub fn generate_batches(prd: &Prd) -> Result<Vec<Batch>, DependencyError> {
        let mut batches = Vec::new();
        let mut completed: HashSet<String> = HashSet::new();
        let mut remaining: HashSet<String> = prd.stories
            .iter()
            .map(|s| s.id.clone())
            .collect();

        while !remaining.is_empty() {
            let mut batch = Vec::new();

            for story in &prd.stories {
                if remaining.contains(&story.id) {
                    // Check if all dependencies are satisfied
                    let deps_satisfied = story.dependencies
                        .iter()
                        .all(|dep| completed.contains(dep));

                    if deps_satisfied {
                        batch.push(story.id.clone());
                    }
                }
            }

            if batch.is_empty() {
                // Circular dependency detected
                let cycle = Self::find_cycle(&prd.stories, &remaining);
                return Err(DependencyError::CircularDependency(cycle));
            }

            // Move batch items to completed
            for id in &batch {
                remaining.remove(id);
                completed.insert(id.clone());
            }

            batches.push(Batch {
                index: batches.len() + 1,
                story_ids: batch,
            });
        }

        Ok(batches)
    }

    /// Detect circular dependencies
    fn find_cycle(stories: &[Story], remaining: &HashSet<String>) -> Vec<String> {
        let story_map: HashMap<_, _> = stories
            .iter()
            .map(|s| (s.id.as_str(), s))
            .collect();

        for start_id in remaining {
            let mut visited = HashSet::new();
            let mut path = Vec::new();

            if Self::dfs_find_cycle(start_id, &story_map, &mut visited, &mut path) {
                return path;
            }
        }

        Vec::new()
    }

    fn dfs_find_cycle(
        current: &str,
        story_map: &HashMap<&str, &Story>,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> bool {
        if path.contains(&current.to_string()) {
            path.push(current.to_string());
            return true;
        }

        if visited.contains(current) {
            return false;
        }

        visited.insert(current.to_string());
        path.push(current.to_string());

        if let Some(story) = story_map.get(current) {
            for dep in &story.dependencies {
                if Self::dfs_find_cycle(dep, story_map, visited, path) {
                    return true;
                }
            }
        }

        path.pop();
        false
    }

    /// Generate visual dependency graph (ASCII art)
    pub fn generate_graph_ascii(prd: &Prd) -> String {
        let mut output = String::new();
        output.push_str("Dependency Graph:\n");
        output.push_str("─────────────────\n\n");

        for story in &prd.stories {
            let deps_str = if story.dependencies.is_empty() {
                "(no dependencies)".to_string()
            } else {
                format!("depends on: {}", story.dependencies.join(", "))
            };
            output.push_str(&format!(
                "  {} - {}\n    {}\n\n",
                story.id, story.title, deps_str
            ));
        }

        output
    }
}

#[derive(Debug, Clone)]
pub struct Batch {
    pub index: usize,
    pub story_ids: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum DependencyError {
    #[error("Circular dependency detected: {0:?}")]
    CircularDependency(Vec<String>),
}
```

---

## 5. Auto-Iteration System

### 5.1 Iteration Loop Implementation

```rust
// src-tauri/src/services/iteration/loop.rs

use crate::models::iteration::{IterationConfig, IterationMode, IterationState};
use crate::services::agent::AgentExecutor;
use crate::services::quality::QualityGate;
use anyhow::Result;

pub struct IterationLoop {
    config: IterationConfig,
    state: IterationState,
    agent_executor: AgentExecutor,
    quality_gate: QualityGate,
}

impl IterationLoop {
    /// Main iteration loop
    pub async fn run(&mut self) -> Result<IterationResult> {
        self.state.status = IterationStatus::Running;
        self.save_state().await?;

        loop {
            // Check termination conditions
            match self.config.mode {
                IterationMode::UntilComplete => {
                    if self.all_stories_complete() {
                        break;
                    }
                }
                IterationMode::MaxIterations(max) => {
                    if self.state.iteration_count >= max {
                        break;
                    }
                }
                IterationMode::BatchComplete => {
                    if self.current_batch_complete() {
                        break;
                    }
                }
            }

            // Get pending stories in current batch
            let pending = self.get_pending_stories();
            if pending.is_empty() {
                // Advance to next batch
                if !self.advance_to_next_batch() {
                    break;  // No more batches
                }
                continue;
            }

            // Execute stories in parallel
            let results = self.execute_stories_parallel(&pending).await?;

            // Run quality gates for completed stories
            for (story_id, result) in results {
                if result.success {
                    let gate_result = self.quality_gate.run(&story_id).await?;

                    if gate_result.passed {
                        self.mark_story_complete(&story_id);
                    } else if self.can_retry(&story_id) {
                        self.queue_retry(&story_id, &gate_result);
                    } else {
                        self.mark_story_failed(&story_id, &gate_result.error);
                    }
                }
            }

            self.state.iteration_count += 1;
            self.save_state().await?;

            // Poll interval
            tokio::time::sleep(
                tokio::time::Duration::from_secs(self.config.poll_interval_seconds)
            ).await;
        }

        self.state.status = IterationStatus::Completed;
        self.save_state().await?;

        Ok(self.generate_result())
    }

    /// Execute multiple stories in parallel using Task agents
    async fn execute_stories_parallel(
        &self,
        story_ids: &[String],
    ) -> Result<Vec<(String, ExecutionResult)>> {
        use futures::future::join_all;

        let tasks: Vec<_> = story_ids.iter().map(|id| {
            self.agent_executor.execute_story(id)
        }).collect();

        let results = join_all(tasks).await;

        Ok(story_ids.iter().cloned().zip(results).collect())
    }

    fn can_retry(&self, story_id: &str) -> bool {
        let retry_count = self.state.retry_counts.get(story_id).unwrap_or(&0);
        *retry_count < self.config.max_retries
    }

    fn queue_retry(&mut self, story_id: &str, gate_result: &QualityGateResult) {
        let count = self.state.retry_counts.entry(story_id.to_string()).or_insert(0);
        *count += 1;

        self.state.retry_queue.push(RetryEntry {
            story_id: story_id.to_string(),
            failure_context: gate_result.error.clone(),
            retry_number: *count,
        });
    }
}
```

---

## 6. Quality Gates with Auto-Detection

### 6.1 Quality Gate Service

```rust
// src-tauri/src/services/quality/gate.rs

use anyhow::Result;
use std::path::PathBuf;

pub struct QualityGate {
    project_root: PathBuf,
    config: QualityGateConfig,
    detected_project_type: Option<ProjectType>,
}

#[derive(Debug, Clone, Copy)]
pub enum ProjectType {
    NodeJs,
    Python,
    Rust,
    Go,
    Unknown,
}

impl QualityGate {
    /// Auto-detect project type from files
    pub async fn detect_project_type(&mut self) -> ProjectType {
        if self.project_root.join("package.json").exists() {
            ProjectType::NodeJs
        } else if self.project_root.join("pyproject.toml").exists()
            || self.project_root.join("setup.py").exists()
        {
            ProjectType::Python
        } else if self.project_root.join("Cargo.toml").exists() {
            ProjectType::Rust
        } else if self.project_root.join("go.mod").exists() {
            ProjectType::Go
        } else {
            ProjectType::Unknown
        }
    }

    /// Get default commands for project type
    fn get_default_commands(&self, project_type: ProjectType) -> GateCommands {
        match project_type {
            ProjectType::NodeJs => GateCommands {
                typecheck: vec!["npx", "tsc", "--noEmit"],
                test: vec!["npm", "test"],
                lint: vec!["npx", "eslint", "."],
            },
            ProjectType::Python => GateCommands {
                typecheck: vec!["mypy", "."],
                test: vec!["pytest"],
                lint: vec!["ruff", "check", "."],
            },
            ProjectType::Rust => GateCommands {
                typecheck: vec!["cargo", "check"],
                test: vec!["cargo", "test"],
                lint: vec!["cargo", "clippy"],
            },
            ProjectType::Go => GateCommands {
                typecheck: vec!["go", "vet", "./..."],
                test: vec!["go", "test", "./..."],
                lint: vec!["golangci-lint", "run"],
            },
            ProjectType::Unknown => GateCommands {
                typecheck: vec![],
                test: vec![],
                lint: vec![],
            },
        }
    }

    /// Run all quality gates
    pub async fn run(&self, story_id: &str) -> Result<QualityGateResult> {
        let project_type = self.detected_project_type
            .unwrap_or(ProjectType::Unknown);
        let commands = self.get_default_commands(project_type);

        let mut errors = Vec::new();
        let mut passed = true;

        // TypeCheck
        if self.config.typecheck.enabled && !commands.typecheck.is_empty() {
            let result = self.run_command(&commands.typecheck).await?;
            if !result.success {
                if self.config.typecheck.required {
                    passed = false;
                }
                errors.push(GateError {
                    gate: "typecheck".to_string(),
                    output: result.output,
                });
            }
        }

        // Test
        if self.config.test.enabled && !commands.test.is_empty() {
            let result = self.run_command(&commands.test).await?;
            if !result.success {
                if self.config.test.required {
                    passed = false;
                }
                errors.push(GateError {
                    gate: "test".to_string(),
                    output: result.output,
                });
            }
        }

        // Lint
        if self.config.lint.enabled && !commands.lint.is_empty() {
            let result = self.run_command(&commands.lint).await?;
            if !result.success {
                if self.config.lint.required {
                    passed = false;
                }
                errors.push(GateError {
                    gate: "lint".to_string(),
                    output: result.output,
                });
            }
        }

        // Custom script
        if self.config.custom.enabled {
            if let Some(script) = &self.config.custom.script {
                let result = self.run_command(&[script.as_str()]).await?;
                if !result.success {
                    if self.config.custom.required {
                        passed = false;
                    }
                    errors.push(GateError {
                        gate: "custom".to_string(),
                        output: result.output,
                    });
                }
            }
        }

        Ok(QualityGateResult {
            passed,
            errors,
            error: if errors.is_empty() {
                None
            } else {
                Some(self.format_errors(&errors))
            },
        })
    }

    async fn run_command(&self, args: &[&str]) -> Result<CommandResult> {
        use tokio::process::Command;

        let output = Command::new(args[0])
            .args(&args[1..])
            .current_dir(&self.project_root)
            .output()
            .await?;

        Ok(CommandResult {
            success: output.status.success(),
            output: String::from_utf8_lossy(&output.stdout).to_string()
                + &String::from_utf8_lossy(&output.stderr),
        })
    }
}
```

---

## 7. Real-time Streaming Chat Implementation

### 7.1 Unified Stream Event Interface

The unified streaming abstraction layer provides a common interface for all LLM providers:

```rust
// src-tauri/src/services/streaming/unified.rs

use serde::{Deserialize, Serialize};

/// Unified stream events consumed by frontend
/// All provider-specific formats are converted to this
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UnifiedStreamEvent {
    /// Incremental text content
    TextDelta {
        content: String,
    },

    /// Thinking block started (Claude only)
    ThinkingStart {
        id: String,
    },

    /// Thinking content delta (Claude only)
    ThinkingDelta {
        id: String,
        content: String,
    },

    /// Thinking block ended (Claude only)
    ThinkingEnd {
        id: String,
        duration_ms: u64,
    },

    /// Tool execution started
    ToolStart {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },

    /// Tool execution completed
    ToolResult {
        id: String,
        success: bool,
        output: String,
        duration_ms: u64,
    },

    /// Token usage update
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: Option<f64>,
    },

    /// Error occurred
    Error {
        message: String,
        recoverable: bool,
    },

    /// Stream completed
    Complete {
        session_id: String,
        total_duration_ms: u64,
    },
}

/// Trait for converting provider-specific events to unified format
pub trait StreamAdapter: Send + Sync {
    /// Provider name for logging
    fn provider_name(&self) -> &'static str;

    /// Whether this provider supports thinking blocks
    fn supports_thinking(&self) -> bool;

    /// Whether this provider supports tool calls
    fn supports_tools(&self) -> bool;

    /// Convert provider-specific line/event to unified events
    fn adapt(&self, raw: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError>;
}
```

### 7.2 Provider-Specific Adapters

#### Claude Code CLI Adapter

```rust
// src-tauri/src/services/streaming/adapters/claude_code.rs

use super::{StreamAdapter, UnifiedStreamEvent};

/// Adapts Claude Code CLI `stream-json` format
pub struct ClaudeCodeAdapter;

impl StreamAdapter for ClaudeCodeAdapter {
    fn provider_name(&self) -> &'static str { "claude-code" }
    fn supports_thinking(&self) -> bool { true }
    fn supports_tools(&self) -> bool { true }

    fn adapt(&self, raw: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError> {
        let event: ClaudeCodeEvent = serde_json::from_str(raw)?;

        Ok(match event {
            ClaudeCodeEvent::Assistant { content } => {
                vec![UnifiedStreamEvent::TextDelta { content }]
            }
            ClaudeCodeEvent::Thinking { id } => {
                vec![UnifiedStreamEvent::ThinkingStart { id }]
            }
            ClaudeCodeEvent::ThinkingDelta { id, content } => {
                vec![UnifiedStreamEvent::ThinkingDelta { id, content }]
            }
            ClaudeCodeEvent::ThinkingEnd { id, duration_ms } => {
                vec![UnifiedStreamEvent::ThinkingEnd { id, duration_ms }]
            }
            ClaudeCodeEvent::ToolUse { id, name, input } => {
                vec![UnifiedStreamEvent::ToolStart {
                    id, name, arguments: input
                }]
            }
            ClaudeCodeEvent::ToolResult { id, success, output, duration_ms } => {
                vec![UnifiedStreamEvent::ToolResult {
                    id, success, output, duration_ms
                }]
            }
            ClaudeCodeEvent::Result { session_id, cost_usd, duration_ms } => {
                vec![
                    UnifiedStreamEvent::Usage {
                        input_tokens: 0, // Not provided in this event
                        output_tokens: 0,
                        cost_usd,
                    },
                    UnifiedStreamEvent::Complete {
                        session_id,
                        total_duration_ms: duration_ms,
                    }
                ]
            }
            ClaudeCodeEvent::Error { message } => {
                vec![UnifiedStreamEvent::Error {
                    message,
                    recoverable: false,
                }]
            }
        })
    }
}
```

#### Claude API Adapter

```rust
// src-tauri/src/services/streaming/adapters/claude_api.rs

use super::{StreamAdapter, UnifiedStreamEvent};

/// Adapts Claude API SSE format
pub struct ClaudeApiAdapter {
    current_thinking_id: Option<String>,
}

impl StreamAdapter for ClaudeApiAdapter {
    fn provider_name(&self) -> &'static str { "claude-api" }
    fn supports_thinking(&self) -> bool { true }
    fn supports_tools(&self) -> bool { true }

    fn adapt(&self, raw: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError> {
        // Parse SSE format: "data: {...}"
        let data = raw.strip_prefix("data: ").ok_or(AdapterError::InvalidFormat)?;
        if data == "[DONE]" {
            return Ok(vec![]);
        }

        let event: ClaudeApiEvent = serde_json::from_str(data)?;

        Ok(match event {
            ClaudeApiEvent::ContentBlockStart { index, content_block } => {
                match content_block.block_type.as_str() {
                    "thinking" => {
                        let id = format!("thinking_{}", index);
                        vec![UnifiedStreamEvent::ThinkingStart { id }]
                    }
                    "tool_use" => {
                        vec![UnifiedStreamEvent::ToolStart {
                            id: content_block.id.unwrap_or_default(),
                            name: content_block.name.unwrap_or_default(),
                            arguments: serde_json::Value::Object(Default::default()),
                        }]
                    }
                    _ => vec![]
                }
            }
            ClaudeApiEvent::ContentBlockDelta { index, delta } => {
                match delta.delta_type.as_str() {
                    "thinking_delta" => {
                        vec![UnifiedStreamEvent::ThinkingDelta {
                            id: format!("thinking_{}", index),
                            content: delta.thinking.unwrap_or_default(),
                        }]
                    }
                    "text_delta" => {
                        vec![UnifiedStreamEvent::TextDelta {
                            content: delta.text.unwrap_or_default(),
                        }]
                    }
                    _ => vec![]
                }
            }
            ClaudeApiEvent::MessageDelta { usage, .. } => {
                vec![UnifiedStreamEvent::Usage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    cost_usd: None, // Calculate from token counts
                }]
            }
            ClaudeApiEvent::MessageStop => {
                vec![UnifiedStreamEvent::Complete {
                    session_id: String::new(),
                    total_duration_ms: 0,
                }]
            }
            _ => vec![]
        })
    }
}
```

#### OpenAI Adapter (with o1/o3 Reasoning Support)

```rust
// src-tauri/src/services/streaming/adapters/openai.rs

use super::{StreamAdapter, UnifiedStreamEvent};
use regex::Regex;

/// Adapts OpenAI SSE format with reasoning support for o1/o3 models
pub struct OpenAIAdapter {
    model: String,
    thinking_id: Option<String>,
}

impl OpenAIAdapter {
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
            thinking_id: None,
        }
    }

    /// Check if model supports reasoning (o1, o1-mini, o1-pro, o3-mini, o3)
    fn is_reasoning_model(&self) -> bool {
        self.model.starts_with("o1") || self.model.starts_with("o3")
    }
}

impl StreamAdapter for OpenAIAdapter {
    fn provider_name(&self) -> &'static str { "openai" }

    fn supports_thinking(&self) -> bool {
        self.is_reasoning_model()
    }

    fn supports_tools(&self) -> bool { true }

    fn adapt(&self, raw: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError> {
        let data = raw.strip_prefix("data: ").ok_or(AdapterError::InvalidFormat)?;
        if data == "[DONE]" {
            return Ok(vec![UnifiedStreamEvent::Complete {
                session_id: String::new(),
                total_duration_ms: 0,
            }]);
        }

        let event: OpenAIStreamChunk = serde_json::from_str(data)?;
        let mut events = Vec::new();

        for choice in event.choices {
            if let Some(delta) = choice.delta {
                // Reasoning content (o1/o3 models)
                if let Some(reasoning) = delta.reasoning_content {
                    if !reasoning.is_empty() {
                        let id = format!("reasoning_{}", choice.index);
                        events.push(UnifiedStreamEvent::ThinkingDelta {
                            id,
                            content: reasoning,
                        });
                    }
                }

                // Text content
                if let Some(content) = delta.content {
                    if !content.is_empty() {
                        events.push(UnifiedStreamEvent::TextDelta { content });
                    }
                }

                // Tool calls
                if let Some(tool_calls) = delta.tool_calls {
                    for tc in tool_calls {
                        if let Some(function) = tc.function {
                            events.push(UnifiedStreamEvent::ToolStart {
                                id: tc.id.unwrap_or_default(),
                                name: function.name.unwrap_or_default(),
                                arguments: serde_json::from_str(
                                    &function.arguments.unwrap_or_default()
                                ).unwrap_or_default(),
                            });
                        }
                    }
                }
            }
        }

        // Usage info (if present)
        if let Some(usage) = event.usage {
            events.push(UnifiedStreamEvent::Usage {
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
                cost_usd: None,
            });
        }

        Ok(events)
    }
}

/// OpenAI stream chunk with reasoning support
#[derive(Debug, Deserialize)]
struct OpenAIStreamChunk {
    choices: Vec<OpenAIChoice>,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    index: u32,
    delta: Option<OpenAIDelta>,
}

#[derive(Debug, Deserialize)]
struct OpenAIDelta {
    content: Option<String>,
    reasoning_content: Option<String>,  // o1/o3 reasoning
    tool_calls: Option<Vec<OpenAIToolCall>>,
}
```

#### DeepSeek Adapter (with R1 Thinking Support)

```rust
// src-tauri/src/services/streaming/adapters/deepseek.rs

use super::{StreamAdapter, UnifiedStreamEvent};
use regex::Regex;

/// Adapts DeepSeek SSE format with <think> tag parsing for R1 models
pub struct DeepSeekAdapter {
    model: String,
    think_buffer: String,
    in_think_block: bool,
    thinking_id: Option<String>,
}

impl DeepSeekAdapter {
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
            think_buffer: String::new(),
            in_think_block: false,
            thinking_id: None,
        }
    }

    /// Check if model supports thinking (R1 series)
    fn is_r1_model(&self) -> bool {
        self.model.contains("r1") || self.model.contains("R1")
    }

    /// Parse content for <think> tags
    fn parse_think_tags(&mut self, content: &str) -> Vec<UnifiedStreamEvent> {
        let mut events = Vec::new();
        let mut remaining = content.to_string();

        // Handle <think> tag start
        if let Some(pos) = remaining.find("<think>") {
            // Emit any text before <think>
            if pos > 0 {
                events.push(UnifiedStreamEvent::TextDelta {
                    content: remaining[..pos].to_string(),
                });
            }
            remaining = remaining[pos + 7..].to_string();
            self.in_think_block = true;
            let id = format!("think_{}", uuid::Uuid::new_v4());
            self.thinking_id = Some(id.clone());
            events.push(UnifiedStreamEvent::ThinkingStart { id });
        }

        // Handle </think> tag end
        if let Some(pos) = remaining.find("</think>") {
            if self.in_think_block {
                // Emit thinking content before </think>
                if pos > 0 {
                    events.push(UnifiedStreamEvent::ThinkingDelta {
                        id: self.thinking_id.clone().unwrap_or_default(),
                        content: remaining[..pos].to_string(),
                    });
                }
                events.push(UnifiedStreamEvent::ThinkingEnd {
                    id: self.thinking_id.take().unwrap_or_default(),
                    duration_ms: 0,  // Will be calculated at complete
                });
                self.in_think_block = false;
            }
            remaining = remaining[pos + 8..].to_string();
        }

        // Handle remaining content
        if !remaining.is_empty() {
            if self.in_think_block {
                events.push(UnifiedStreamEvent::ThinkingDelta {
                    id: self.thinking_id.clone().unwrap_or_default(),
                    content: remaining,
                });
            } else {
                events.push(UnifiedStreamEvent::TextDelta { content: remaining });
            }
        }

        events
    }
}

impl StreamAdapter for DeepSeekAdapter {
    fn provider_name(&self) -> &'static str { "deepseek" }

    fn supports_thinking(&self) -> bool {
        self.is_r1_model()
    }

    fn supports_tools(&self) -> bool { true }

    fn adapt(&self, raw: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError> {
        let data = raw.strip_prefix("data: ").ok_or(AdapterError::InvalidFormat)?;
        if data == "[DONE]" {
            return Ok(vec![UnifiedStreamEvent::Complete {
                session_id: String::new(),
                total_duration_ms: 0,
            }]);
        }

        let event: OpenAIStreamChunk = serde_json::from_str(data)?;
        let mut events = Vec::new();

        for choice in event.choices {
            if let Some(delta) = choice.delta {
                if let Some(content) = delta.content {
                    if !content.is_empty() {
                        if self.is_r1_model() {
                            // Parse <think> tags for R1 models
                            events.extend(self.parse_think_tags(&content));
                        } else {
                            events.push(UnifiedStreamEvent::TextDelta { content });
                        }
                    }
                }

                // Tool calls (same as OpenAI format)
                if let Some(tool_calls) = delta.tool_calls {
                    for tc in tool_calls {
                        if let Some(function) = tc.function {
                            events.push(UnifiedStreamEvent::ToolStart {
                                id: tc.id.unwrap_or_default(),
                                name: function.name.unwrap_or_default(),
                                arguments: serde_json::from_str(
                                    &function.arguments.unwrap_or_default()
                                ).unwrap_or_default(),
                            });
                        }
                    }
                }
            }
        }

        Ok(events)
    }
}
```

#### Ollama Adapter (with Model-Dependent Thinking)

```rust
// src-tauri/src/services/streaming/adapters/ollama.rs

use super::{StreamAdapter, UnifiedStreamEvent};

/// Models known to support thinking in Ollama
const THINKING_MODELS: &[&str] = &[
    "deepseek-r1", "deepseek-r1:latest", "deepseek-r1:7b", "deepseek-r1:14b",
    "qwq", "qwq:latest", "qwq:32b",
];

/// Adapts Ollama JSON stream format with model-dependent thinking support
pub struct OllamaAdapter {
    model: String,
    in_think_block: bool,
    thinking_id: Option<String>,
}

impl OllamaAdapter {
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_lowercase(),
            in_think_block: false,
            thinking_id: None,
        }
    }

    /// Check if hosted model supports thinking
    fn model_supports_thinking(&self) -> bool {
        THINKING_MODELS.iter().any(|m| self.model.starts_with(m))
            || self.model.contains("r1")
            || self.model.contains("qwq")
    }

    /// Parse content for <think> tags (same format as DeepSeek R1)
    fn parse_think_tags(&mut self, content: &str) -> Vec<UnifiedStreamEvent> {
        // Same implementation as DeepSeekAdapter::parse_think_tags
        let mut events = Vec::new();
        let mut remaining = content.to_string();

        if let Some(pos) = remaining.find("<think>") {
            if pos > 0 {
                events.push(UnifiedStreamEvent::TextDelta {
                    content: remaining[..pos].to_string(),
                });
            }
            remaining = remaining[pos + 7..].to_string();
            self.in_think_block = true;
            let id = format!("think_{}", uuid::Uuid::new_v4());
            self.thinking_id = Some(id.clone());
            events.push(UnifiedStreamEvent::ThinkingStart { id });
        }

        if let Some(pos) = remaining.find("</think>") {
            if self.in_think_block {
                if pos > 0 {
                    events.push(UnifiedStreamEvent::ThinkingDelta {
                        id: self.thinking_id.clone().unwrap_or_default(),
                        content: remaining[..pos].to_string(),
                    });
                }
                events.push(UnifiedStreamEvent::ThinkingEnd {
                    id: self.thinking_id.take().unwrap_or_default(),
                    duration_ms: 0,
                });
                self.in_think_block = false;
            }
            remaining = remaining[pos + 8..].to_string();
        }

        if !remaining.is_empty() {
            if self.in_think_block {
                events.push(UnifiedStreamEvent::ThinkingDelta {
                    id: self.thinking_id.clone().unwrap_or_default(),
                    content: remaining,
                });
            } else {
                events.push(UnifiedStreamEvent::TextDelta { content: remaining });
            }
        }

        events
    }
}

impl StreamAdapter for OllamaAdapter {
    fn provider_name(&self) -> &'static str { "ollama" }

    fn supports_thinking(&self) -> bool {
        self.model_supports_thinking()
    }

    fn supports_tools(&self) -> bool { true }  // Ollama 0.4+ supports tools

    fn adapt(&self, raw: &str) -> Result<Vec<UnifiedStreamEvent>, AdapterError> {
        let event: OllamaResponse = serde_json::from_str(raw)?;
        let mut events = Vec::new();

        if !event.response.is_empty() {
            if self.model_supports_thinking() {
                // Parse <think> tags for reasoning models
                events.extend(self.parse_think_tags(&event.response));
            } else {
                events.push(UnifiedStreamEvent::TextDelta {
                    content: event.response,
                });
            }
        }

        if event.done {
            if let Some(total_duration) = event.total_duration {
                events.push(UnifiedStreamEvent::Complete {
                    session_id: String::new(),
                    total_duration_ms: total_duration / 1_000_000, // ns to ms
                });
            }

            if let (Some(prompt_tokens), Some(eval_tokens)) =
                (event.prompt_eval_count, event.eval_count)
            {
                events.push(UnifiedStreamEvent::Usage {
                    input_tokens: prompt_tokens,
                    output_tokens: eval_tokens,
                    cost_usd: None,  // Ollama is free
                });
            }
        }

        Ok(events)
    }
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
    done: bool,
    total_duration: Option<u64>,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
}
```

### 7.3 Adapter Factory

```rust
// src-tauri/src/services/streaming/factory.rs

use super::adapters::*;
use super::StreamAdapter;

pub struct AdapterFactory;

impl AdapterFactory {
    /// Create adapter based on provider and model
    /// Model is needed to determine thinking support (e.g., o1 vs gpt-4)
    pub fn create(provider: &str, model: &str) -> Box<dyn StreamAdapter> {
        match provider {
            "claude-code" => Box::new(ClaudeCodeAdapter),
            "claude-api" | "claude" => Box::new(ClaudeApiAdapter::new()),
            "openai" => Box::new(OpenAIAdapter::new(model)),
            "deepseek" => Box::new(DeepSeekAdapter::new(model)),
            "ollama" => Box::new(OllamaAdapter::new(model)),
            _ => Box::new(OpenAIAdapter::new(model)),  // Default fallback
        }
    }
}
```

### 7.4 Unified Streaming Service

```rust
// src-tauri/src/services/streaming/service.rs

use super::{AdapterFactory, StreamAdapter, UnifiedStreamEvent};
use tokio::sync::mpsc;

pub struct UnifiedStreamingService {
    adapter: Box<dyn StreamAdapter>,
    event_tx: mpsc::Sender<UnifiedStreamEvent>,
    provider: String,
    model: String,
}

impl UnifiedStreamingService {
    /// Create streaming service with provider and model info
    /// Model is required to determine thinking support for providers like OpenAI (o1/o3)
    pub fn new(
        provider: &str,
        model: &str,
        event_tx: mpsc::Sender<UnifiedStreamEvent>,
    ) -> Self {
        Self {
            adapter: AdapterFactory::create(provider, model),
            event_tx,
            provider: provider.to_string(),
            model: model.to_string(),
        }
    }

    /// Process a raw line from any provider
    pub async fn process_line(&self, line: &str) -> Result<(), StreamError> {
        let events = self.adapter.adapt(line)?;
        for event in events {
            self.event_tx.send(event).await?;
        }
        Ok(())
    }

    /// Check if current provider/model supports thinking
    pub fn supports_thinking(&self) -> bool {
        self.adapter.supports_thinking()
    }

    /// Get thinking format description for UI hints
    pub fn thinking_format(&self) -> &'static str {
        match self.provider.as_str() {
            "claude-code" | "claude-api" | "claude" => "Extended Thinking",
            "openai" => if self.model.starts_with("o1") || self.model.starts_with("o3") {
                "Reasoning"
            } else {
                "Not Supported"
            },
            "deepseek" => if self.model.contains("r1") || self.model.contains("R1") {
                "DeepThink"
            } else {
                "Not Supported"
            },
            "ollama" => "Model Dependent",
            _ => "Not Supported",
        }
    }
}
```

### 7.5 Claude Code CLI Streaming Handler (Original)

```rust
// src-tauri/src/services/chat/streaming.rs

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Streaming message types from Claude Code CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    /// Assistant text content (streaming)
    #[serde(rename = "assistant")]
    AssistantText { content: String },

    /// Thinking block start
    #[serde(rename = "thinking")]
    ThinkingStart { id: String },

    /// Thinking content (streaming)
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { id: String, content: String },

    /// Thinking block end
    #[serde(rename = "thinking_end")]
    ThinkingEnd { id: String, duration_ms: u64 },

    /// Tool call started
    #[serde(rename = "tool_use")]
    ToolStart {
        id: String,
        name: String,
        #[serde(rename = "input")]
        arguments: serde_json::Value,
    },

    /// Tool call completed
    #[serde(rename = "tool_result")]
    ToolResult {
        id: String,
        success: bool,
        output: String,
        duration_ms: u64,
    },

    /// Message complete
    #[serde(rename = "result")]
    Complete {
        session_id: String,
        cost_usd: Option<f64>,
        duration_ms: u64,
    },

    /// Error occurred
    #[serde(rename = "error")]
    Error { message: String },
}

pub struct StreamingChatService {
    claude_path: String,
    event_sender: mpsc::Sender<StreamEvent>,
}

impl StreamingChatService {
    /// Execute Claude Code with streaming output
    pub async fn execute_streaming(
        &self,
        prompt: &str,
        project_path: &str,
    ) -> Result<(), ChatError> {
        use tokio::process::Command;
        use tokio::io::{AsyncBufReadExt, BufReader};

        let mut child = Command::new(&self.claude_path)
            .args([
                "--print", prompt,
                "--output-format", "stream-json",
                "--include-partial-messages",
                "--project-root", project_path,
            ])
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout).lines();

        while let Some(line) = reader.next_line().await? {
            if let Ok(event) = serde_json::from_str::<StreamEvent>(&line) {
                self.event_sender.send(event).await?;
            }
        }

        Ok(())
    }
}
```

### 7.2 Thinking Display Handler

```rust
// src-tauri/src/services/chat/thinking.rs

use std::collections::HashMap;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize)]
pub struct ThinkingBlock {
    pub id: String,
    pub content: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub collapsed: bool,
}

pub struct ThinkingManager {
    blocks: HashMap<String, ThinkingBlock>,
    auto_collapse: bool,
}

impl ThinkingManager {
    pub fn start_thinking(&mut self, id: String) -> ThinkingBlock {
        let block = ThinkingBlock {
            id: id.clone(),
            content: String::new(),
            started_at: Utc::now(),
            ended_at: None,
            duration_ms: None,
            collapsed: false,
        };
        self.blocks.insert(id, block.clone());
        block
    }

    pub fn append_thinking(&mut self, id: &str, content: &str) -> Option<&ThinkingBlock> {
        if let Some(block) = self.blocks.get_mut(id) {
            block.content.push_str(content);
            Some(block)
        } else {
            None
        }
    }

    pub fn end_thinking(&mut self, id: &str, duration_ms: u64) -> Option<ThinkingBlock> {
        if let Some(block) = self.blocks.get_mut(id) {
            block.ended_at = Some(Utc::now());
            block.duration_ms = Some(duration_ms);
            if self.auto_collapse {
                block.collapsed = true;
            }
            Some(block.clone())
        } else {
            None
        }
    }
}
```

### 7.3 Tool Execution Visualization

```rust
// src-tauri/src/services/chat/tool_tracker.rs

use std::collections::VecDeque;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize)]
pub enum ToolStatus {
    Pending,
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    pub arguments_preview: String,  // Truncated preview for UI
    pub status: ToolStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub output: Option<String>,
    pub output_preview: Option<String>,  // Truncated for UI
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileChange {
    pub path: String,
    pub change_type: FileChangeType,
    pub diff: String,
    pub lines_added: u32,
    pub lines_removed: u32,
}

#[derive(Debug, Clone, Serialize)]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
}

pub struct ToolTracker {
    history: VecDeque<ToolCall>,
    current: Option<ToolCall>,
    file_changes: Vec<FileChange>,
    max_history: usize,
}

impl ToolTracker {
    pub fn start_tool(&mut self, id: String, name: String, arguments: serde_json::Value) {
        let preview = self.format_arguments_preview(&name, &arguments);

        let tool = ToolCall {
            id,
            name,
            arguments,
            arguments_preview: preview,
            status: ToolStatus::Running,
            started_at: Utc::now(),
            ended_at: None,
            duration_ms: None,
            output: None,
            output_preview: None,
            error: None,
        };

        self.current = Some(tool);
    }

    pub fn complete_tool(&mut self, id: &str, success: bool, output: String, duration_ms: u64) {
        if let Some(ref mut tool) = self.current {
            if tool.id == id {
                tool.status = if success { ToolStatus::Success } else { ToolStatus::Failed };
                tool.ended_at = Some(Utc::now());
                tool.duration_ms = Some(duration_ms);
                tool.output_preview = Some(self.truncate_output(&output, 200));
                tool.output = Some(output);

                // Track file changes for Edit/Write tools
                if tool.name == "edit" || tool.name == "write" {
                    self.track_file_change(&tool);
                }

                // Move to history
                if let Some(completed) = self.current.take() {
                    self.history.push_front(completed);
                    if self.history.len() > self.max_history {
                        self.history.pop_back();
                    }
                }
            }
        }
    }

    fn format_arguments_preview(&self, name: &str, args: &serde_json::Value) -> String {
        match name {
            "read" | "write" | "edit" => {
                args.get("file_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string()
            }
            "glob" => {
                args.get("pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("*")
                    .to_string()
            }
            "bash" => {
                let cmd = args.get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                self.truncate_output(cmd, 50)
            }
            _ => format!("{:?}", args),
        }
    }

    fn truncate_output(&self, s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len])
        }
    }
}
```

### 7.4 Chat Interaction Manager

```rust
// src-tauri/src/services/chat/interaction.rs

use tokio::sync::watch;

#[derive(Debug, Clone)]
pub enum ChatState {
    Idle,
    Streaming { can_interrupt: bool },
    WaitingForTool { tool_id: String },
    Interrupted,
    Error { message: String },
}

pub struct ChatInteractionManager {
    state: watch::Sender<ChatState>,
    interrupt_flag: std::sync::atomic::AtomicBool,
}

impl ChatInteractionManager {
    /// Interrupt the current operation
    pub fn interrupt(&self) -> Result<(), InterruptError> {
        if let ChatState::Streaming { can_interrupt: true } = *self.state.borrow() {
            self.interrupt_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            self.state.send(ChatState::Interrupted)?;
            Ok(())
        } else {
            Err(InterruptError::NotInterruptible)
        }
    }

    /// Check if interrupted
    pub fn is_interrupted(&self) -> bool {
        self.interrupt_flag.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Reset state for new operation
    pub fn reset(&self) {
        self.interrupt_flag.store(false, std::sync::atomic::Ordering::SeqCst);
        let _ = self.state.send(ChatState::Idle);
    }
}
```

### 7.5 Frontend Tauri Commands for Chat

```rust
// src-tauri/src/commands/chat.rs

use tauri::{command, State, Window};

/// Start streaming chat
#[command]
pub async fn start_chat(
    window: Window,
    prompt: String,
    project_path: String,
    state: State<'_, ChatState>,
) -> Result<String, String> {
    let session_id = uuid::Uuid::new_v4().to_string();

    // Start streaming in background
    tokio::spawn(async move {
        let service = StreamingChatService::new();

        let (tx, mut rx) = tokio::sync::mpsc::channel(100);

        // Forward events to frontend
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                window.emit("chat-event", &event).ok();
            }
        });

        service.execute_streaming(&prompt, &project_path, tx).await
    });

    Ok(session_id)
}

/// Interrupt current operation
#[command]
pub async fn interrupt_chat(
    state: State<'_, ChatInteractionManager>,
) -> Result<(), String> {
    state.interrupt().map_err(|e| e.to_string())
}

/// Get tool call details
#[command]
pub async fn get_tool_details(
    tool_id: String,
    state: State<'_, ToolTracker>,
) -> Result<ToolCall, String> {
    state.get_tool(&tool_id)
        .ok_or_else(|| "Tool not found".to_string())
}

/// Get file changes
#[command]
pub async fn get_file_changes(
    state: State<'_, ToolTracker>,
) -> Result<Vec<FileChange>, String> {
    Ok(state.get_file_changes().clone())
}

/// Revert a file change
#[command]
pub async fn revert_file_change(
    path: String,
    state: State<'_, ToolTracker>,
) -> Result<(), String> {
    state.revert_change(&path).map_err(|e| e.to_string())
}
```

### 7.6 Frontend React Components

```typescript
// src/components/Chat/StreamingMessage.tsx

interface StreamingMessageProps {
  content: string;
  isStreaming: boolean;
  thinking?: ThinkingBlock;
  toolCalls: ToolCall[];
}

export function StreamingMessage({
  content,
  isStreaming,
  thinking,
  toolCalls
}: StreamingMessageProps) {
  return (
    <div className="message assistant">
      {/* Thinking Block */}
      {thinking && (
        <ThinkingDisplay
          thinking={thinking}
          collapsible={true}
          defaultCollapsed={thinking.ended_at != null}
        />
      )}

      {/* Main Content */}
      <div className="message-content">
        <MarkdownRenderer content={content} />
        {isStreaming && <span className="cursor blink">█</span>}
      </div>

      {/* Tool Calls */}
      {toolCalls.length > 0 && (
        <ToolCallPanel tools={toolCalls} />
      )}
    </div>
  );
}

// src/components/Chat/ThinkingDisplay.tsx

interface ThinkingDisplayProps {
  thinking: ThinkingBlock;
  collapsible: boolean;
  defaultCollapsed: boolean;
}

export function ThinkingDisplay({
  thinking,
  collapsible,
  defaultCollapsed
}: ThinkingDisplayProps) {
  const [collapsed, setCollapsed] = useState(defaultCollapsed);

  return (
    <div className="thinking-block">
      <div
        className="thinking-header"
        onClick={() => collapsible && setCollapsed(!collapsed)}
      >
        <span className="thinking-icon">💭</span>
        <span className="thinking-label">Thinking</span>
        {thinking.duration_ms && (
          <span className="thinking-duration">
            {(thinking.duration_ms / 1000).toFixed(1)}s
          </span>
        )}
        {collapsible && (
          <span className="collapse-icon">
            {collapsed ? '▶' : '▼'}
          </span>
        )}
      </div>

      {!collapsed && (
        <div className="thinking-content">
          {thinking.content}
          {!thinking.ended_at && <span className="cursor blink">█</span>}
        </div>
      )}
    </div>
  );
}
```

---

## 8. Data Models

### 3.1 Project & Session Models

```rust
// src-tauri/src/models/project.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a Claude Code project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Unique identifier (hash of path)
    pub id: String,
    /// Project name (directory name)
    pub name: String,
    /// Full path to project
    pub path: PathBuf,
    /// Last accessed timestamp
    pub last_accessed: DateTime<Utc>,
    /// Number of sessions
    pub session_count: u32,
    /// Total message count across all sessions
    pub total_messages: u32,
    /// Whether project has CLAUDE.md
    pub has_claude_md: bool,
}

/// Represents a coding session within a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier
    pub id: String,
    /// Parent project ID
    pub project_id: String,
    /// First user message (preview)
    pub first_message: String,
    /// Session creation time
    pub created_at: DateTime<Utc>,
    /// Last update time
    pub updated_at: DateTime<Utc>,
    /// Number of messages
    pub message_count: u32,
    /// Number of checkpoints
    pub checkpoint_count: u32,
    /// Session status
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionStatus {
    Active,
    Completed,
    Abandoned,
}

/// Session with full message history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDetail {
    pub session: Session,
    pub messages: Vec<Message>,
    pub checkpoints: Vec<CheckpointSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
    pub result: Option<String>,
}
```

### 3.2 Agent Models

```rust
// src-tauri/src/models/agent.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Custom AI Agent definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Unique agent identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Description of what the agent does
    pub description: String,
    /// Custom system prompt
    pub system_prompt: String,
    /// LLM model to use
    pub model: String,
    /// Allowed tools
    pub allowed_tools: Vec<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last modified timestamp
    pub updated_at: DateTime<Utc>,
    /// Whether agent is enabled
    pub enabled: bool,
    /// Number of times agent has been run
    pub run_count: u32,
}

/// Agent execution run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRun {
    /// Unique run identifier
    pub id: String,
    /// Agent ID
    pub agent_id: String,
    /// Run status
    pub status: RunStatus,
    /// Input prompt/task
    pub input: String,
    /// Output result
    pub output: Option<String>,
    /// Token usage
    pub tokens: TokenUsage,
    /// Cost in USD
    pub cost_usd: f64,
    /// Start time
    pub started_at: DateTime<Utc>,
    /// End time
    pub completed_at: Option<DateTime<Utc>>,
    /// Error message if failed
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}
```

### 3.3 Analytics Models

```rust
// src-tauri/src/models/analytics.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Individual usage record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub model: String,
    pub project_id: Option<String>,
    pub agent_id: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub source: UsageSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UsageSource {
    ClaudeCode,
    Standalone,
    Agent,
}

/// Aggregated usage summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSummary {
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_cost: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub request_count: u32,
    pub by_model: HashMap<String, ModelUsage>,
    pub by_project: HashMap<String, f64>,
    pub by_day: Vec<DailyUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub request_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyUsage {
    pub date: String,  // YYYY-MM-DD
    pub cost_usd: f64,
    pub tokens: u64,
}
```

### 3.4 MCP Server Models

```rust
// src-tauri/src/models/mcp.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MCP Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    /// Unique server identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Server type
    pub server_type: McpServerType,
    /// Command to run
    pub command: String,
    /// Command arguments
    pub args: Vec<String>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Whether server is enabled
    pub enabled: bool,
    /// Last health check time
    pub last_health_check: Option<DateTime<Utc>>,
    /// Current health status
    pub health_status: HealthStatus,
    /// Server description
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum McpServerType {
    Stdio,
    Sse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Unknown,
    Healthy,
    Unhealthy,
    Checking,
}

/// Claude Desktop MCP configuration format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeDesktopConfig {
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, ClaudeDesktopServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeDesktopServer {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}
```

### 3.5 Timeline Models

```rust
// src-tauri/src/models/timeline.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Session checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique checkpoint identifier
    pub id: String,
    /// Session ID
    pub session_id: String,
    /// Optional name
    pub name: Option<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Parent checkpoint (for branching)
    pub parent_id: Option<String>,
    /// Message index at checkpoint
    pub message_index: u32,
    /// Snapshot of session state
    pub snapshot: SessionSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    /// Messages up to this point
    pub message_count: u32,
    /// Files changed since last checkpoint
    pub files_changed: Vec<FileChange>,
    /// Custom metadata
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub change_type: ChangeType,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

/// Branch in timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub id: String,
    pub session_id: String,
    pub name: String,
    pub base_checkpoint_id: String,
    pub head_checkpoint_id: String,
    pub created_at: DateTime<Utc>,
    pub status: BranchStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BranchStatus {
    Active,
    Merged,
    Abandoned,
}

/// Checkpoint summary for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointSummary {
    pub id: String,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub message_count: u32,
    pub files_changed_count: u32,
}
```

---

## 4. Service Implementations

### 4.1 Project Scanner Service

```rust
// src-tauri/src/services/project/scanner.rs

use crate::models::project::{Project, Session};
use anyhow::Result;
use std::path::PathBuf;
use walkdir::WalkDir;

pub struct ProjectScanner {
    claude_projects_dir: PathBuf,
}

impl ProjectScanner {
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("Could not find home directory");
        Self {
            claude_projects_dir: home.join(".claude").join("projects"),
        }
    }

    /// Scan all projects in ~/.claude/projects/
    pub async fn scan_all(&self) -> Result<Vec<Project>> {
        let mut projects = Vec::new();

        if !self.claude_projects_dir.exists() {
            return Ok(projects);
        }

        for entry in WalkDir::new(&self.claude_projects_dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() {
                if let Ok(project) = self.scan_project(entry.path()).await {
                    projects.push(project);
                }
            }
        }

        // Sort by last accessed (most recent first)
        projects.sort_by(|a, b| b.last_accessed.cmp(&a.last_accessed));

        Ok(projects)
    }

    /// Scan a single project directory
    async fn scan_project(&self, path: &std::path::Path) -> Result<Project> {
        let id = self.path_to_id(path);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        // Count sessions
        let sessions_dir = path.join("sessions");
        let session_count = if sessions_dir.exists() {
            std::fs::read_dir(&sessions_dir)?.count() as u32
        } else {
            0
        };

        // Check for CLAUDE.md
        let has_claude_md = path.join("CLAUDE.md").exists();

        // Get last modified time
        let metadata = std::fs::metadata(path)?;
        let last_accessed = metadata
            .modified()
            .map(|t| chrono::DateTime::from(t))
            .unwrap_or_else(|_| chrono::Utc::now());

        Ok(Project {
            id,
            name,
            path: path.to_path_buf(),
            last_accessed,
            session_count,
            total_messages: 0, // TODO: Calculate from sessions
            has_claude_md,
        })
    }

    /// Get sessions for a project
    pub async fn get_sessions(&self, project_id: &str) -> Result<Vec<Session>> {
        // TODO: Implement session loading from ~/.claude/projects/{id}/sessions/
        Ok(Vec::new())
    }

    fn path_to_id(&self, path: &std::path::Path) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        path.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}
```

### 4.2 Claude Code Integration

```rust
// src-tauri/src/execution/claude_code/cli.rs

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Event types from Claude Code stream-json output
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClaudeEvent {
    #[serde(rename = "system")]
    System { message: String },

    #[serde(rename = "assistant")]
    Assistant { message: AssistantMessage },

    #[serde(rename = "stream_event")]
    StreamEvent { event: StreamEventData },

    #[serde(rename = "result")]
    Result { result: ResultData },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEventData {
    #[serde(rename = "type")]
    pub event_type: String,
    pub delta: Option<DeltaData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaData {
    #[serde(rename = "type")]
    pub delta_type: String,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultData {
    pub success: bool,
    pub duration_ms: u64,
}

/// Claude Code CLI executor
pub struct ClaudeCodeExecutor {
    claude_path: String,
}

impl ClaudeCodeExecutor {
    pub fn new() -> Self {
        Self {
            claude_path: "claude".to_string(),
        }
    }

    /// Check if Claude Code CLI is available
    pub async fn is_available(&self) -> bool {
        Command::new(&self.claude_path)
            .arg("--version")
            .output()
            .await
            .is_ok()
    }

    /// Execute a prompt and stream events
    pub async fn execute<F>(
        &self,
        prompt: &str,
        project_path: Option<&str>,
        on_event: F,
    ) -> Result<()>
    where
        F: Fn(ClaudeEvent) + Send + 'static,
    {
        let mut cmd = Command::new(&self.claude_path);

        cmd.arg("--print")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose")
            .arg(prompt)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(path) = project_path {
            cmd.current_dir(path);
        }

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("Failed to get stdout");
        let mut reader = BufReader::new(stdout).lines();

        while let Some(line) = reader.next_line().await? {
            if let Ok(event) = serde_json::from_str::<ClaudeEvent>(&line) {
                on_event(event);
            }
        }

        child.wait().await?;
        Ok(())
    }

    /// Resume a session
    pub async fn resume_session<F>(
        &self,
        session_id: &str,
        on_event: F,
    ) -> Result<()>
    where
        F: Fn(ClaudeEvent) + Send + 'static,
    {
        let mut cmd = Command::new(&self.claude_path);

        cmd.arg("--resume")
            .arg(session_id)
            .arg("--output-format")
            .arg("stream-json")
            .stdout(Stdio::piped());

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("Failed to get stdout");
        let mut reader = BufReader::new(stdout).lines();

        while let Some(line) = reader.next_line().await? {
            if let Ok(event) = serde_json::from_str::<ClaudeEvent>(&line) {
                on_event(event);
            }
        }

        child.wait().await?;
        Ok(())
    }
}
```

### 4.3 MCP Server Registry

```rust
// src-tauri/src/services/mcp/registry.rs

use crate::models::mcp::{ClaudeDesktopConfig, HealthStatus, McpServer, McpServerType};
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct McpRegistry {
    servers: HashMap<String, McpServer>,
    config_path: PathBuf,
}

impl McpRegistry {
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("Could not find home directory");
        Self {
            servers: HashMap::new(),
            config_path: home.join(".plan-cascade").join("mcp-servers.json"),
        }
    }

    /// Load servers from config
    pub async fn load(&mut self) -> Result<()> {
        if self.config_path.exists() {
            let content = tokio::fs::read_to_string(&self.config_path).await?;
            self.servers = serde_json::from_str(&content)?;
        }
        Ok(())
    }

    /// Save servers to config
    pub async fn save(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.servers)?;
        if let Some(parent) = self.config_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&self.config_path, content).await?;
        Ok(())
    }

    /// Get all servers
    pub fn list(&self) -> Vec<McpServer> {
        self.servers.values().cloned().collect()
    }

    /// Add a server
    pub fn add(&mut self, server: McpServer) {
        self.servers.insert(server.id.clone(), server);
    }

    /// Remove a server
    pub fn remove(&mut self, id: &str) -> Option<McpServer> {
        self.servers.remove(id)
    }

    /// Update server
    pub fn update(&mut self, server: McpServer) {
        self.servers.insert(server.id.clone(), server);
    }

    /// Import from Claude Desktop config
    pub async fn import_from_claude_desktop(&mut self) -> Result<Vec<McpServer>> {
        let home = dirs::home_dir().expect("Could not find home directory");

        // Try different possible locations
        let possible_paths = vec![
            home.join(".config").join("claude").join("claude_desktop_config.json"),
            home.join("Library").join("Application Support").join("Claude").join("claude_desktop_config.json"),
            home.join("AppData").join("Roaming").join("Claude").join("claude_desktop_config.json"),
        ];

        for path in possible_paths {
            if path.exists() {
                let content = tokio::fs::read_to_string(&path).await?;
                let config: ClaudeDesktopConfig = serde_json::from_str(&content)?;

                let mut imported = Vec::new();
                for (name, server_config) in config.mcp_servers {
                    let server = McpServer {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: name.clone(),
                        server_type: McpServerType::Stdio,
                        command: server_config.command,
                        args: server_config.args,
                        env: server_config.env,
                        enabled: true,
                        last_health_check: None,
                        health_status: HealthStatus::Unknown,
                        description: None,
                    };

                    self.servers.insert(server.id.clone(), server.clone());
                    imported.push(server);
                }

                return Ok(imported);
            }
        }

        Ok(Vec::new())
    }

    /// Test server health
    pub async fn check_health(&mut self, id: &str) -> Result<HealthStatus> {
        if let Some(server) = self.servers.get_mut(id) {
            server.health_status = HealthStatus::Checking;

            // Try to spawn the server process briefly
            let result = tokio::process::Command::new(&server.command)
                .args(&server.args)
                .envs(&server.env)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();

            let status = match result {
                Ok(mut child) => {
                    // Give it a moment, then kill
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    let _ = child.kill().await;
                    HealthStatus::Healthy
                }
                Err(_) => HealthStatus::Unhealthy,
            };

            server.health_status = status.clone();
            server.last_health_check = Some(chrono::Utc::now());

            return Ok(status);
        }

        Ok(HealthStatus::Unknown)
    }
}
```

### 4.4 Analytics Tracker

```rust
// src-tauri/src/services/analytics/tracker.rs

use crate::models::analytics::{DailyUsage, ModelUsage, UsageRecord, UsageSummary, UsageSource};
use crate::storage::database::Database;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

/// Model pricing (per 1M tokens)
const PRICING: &[(&str, f64, f64)] = &[
    ("claude-opus-4-20250514", 15.0, 75.0),
    ("claude-sonnet-4-20250514", 3.0, 15.0),
    ("claude-3-5-haiku-20241022", 1.0, 5.0),
    ("gpt-4o", 5.0, 15.0),
    ("gpt-4o-mini", 0.15, 0.6),
    ("deepseek-chat", 0.14, 0.28),
];

pub struct AnalyticsTracker {
    db: Database,
}

impl AnalyticsTracker {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Record a usage event
    pub async fn record(&self, record: UsageRecord) -> Result<()> {
        self.db.insert_usage_record(&record).await
    }

    /// Calculate cost for a model
    pub fn calculate_cost(model: &str, input_tokens: u64, output_tokens: u64) -> f64 {
        let (input_rate, output_rate) = PRICING
            .iter()
            .find(|(m, _, _)| model.contains(m))
            .map(|(_, i, o)| (*i, *o))
            .unwrap_or((3.0, 15.0)); // Default to Sonnet pricing

        let input_cost = (input_tokens as f64 / 1_000_000.0) * input_rate;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * output_rate;

        input_cost + output_cost
    }

    /// Get usage summary for a period
    pub async fn get_summary(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<UsageSummary> {
        let records = self.db.get_usage_records(start, end).await?;

        let mut total_cost = 0.0;
        let mut total_input = 0u64;
        let mut total_output = 0u64;
        let mut by_model: HashMap<String, ModelUsage> = HashMap::new();
        let mut by_project: HashMap<String, f64> = HashMap::new();
        let mut by_day: HashMap<String, (f64, u64)> = HashMap::new();

        for record in &records {
            total_cost += record.cost_usd;
            total_input += record.input_tokens;
            total_output += record.output_tokens;

            // By model
            let model_usage = by_model.entry(record.model.clone()).or_insert(ModelUsage {
                input_tokens: 0,
                output_tokens: 0,
                cost_usd: 0.0,
                request_count: 0,
            });
            model_usage.input_tokens += record.input_tokens;
            model_usage.output_tokens += record.output_tokens;
            model_usage.cost_usd += record.cost_usd;
            model_usage.request_count += 1;

            // By project
            if let Some(project_id) = &record.project_id {
                *by_project.entry(project_id.clone()).or_insert(0.0) += record.cost_usd;
            }

            // By day
            let day = record.timestamp.format("%Y-%m-%d").to_string();
            let daily = by_day.entry(day).or_insert((0.0, 0));
            daily.0 += record.cost_usd;
            daily.1 += record.input_tokens + record.output_tokens;
        }

        let by_day: Vec<DailyUsage> = by_day
            .into_iter()
            .map(|(date, (cost, tokens))| DailyUsage {
                date,
                cost_usd: cost,
                tokens,
            })
            .collect();

        Ok(UsageSummary {
            period_start: start,
            period_end: end,
            total_cost,
            total_input_tokens: total_input,
            total_output_tokens: total_output,
            request_count: records.len() as u32,
            by_model,
            by_project,
            by_day,
        })
    }

    /// Export data to CSV
    pub async fn export_csv(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<String> {
        let records = self.db.get_usage_records(start, end).await?;

        let mut csv = String::from("timestamp,model,project,input_tokens,output_tokens,cost_usd,source\n");

        for record in records {
            csv.push_str(&format!(
                "{},{},{},{},{},{:.6},{:?}\n",
                record.timestamp.to_rfc3339(),
                record.model,
                record.project_id.unwrap_or_default(),
                record.input_tokens,
                record.output_tokens,
                record.cost_usd,
                record.source,
            ));
        }

        Ok(csv)
    }
}
```

---

## 5. Tauri Commands

### 5.1 Project Commands

```rust
// src-tauri/src/commands/projects.rs

use crate::models::project::{Project, Session, SessionDetail};
use crate::services::project::scanner::ProjectScanner;
use tauri::State;

/// List all projects
#[tauri::command]
pub async fn list_projects() -> Result<Vec<Project>, String> {
    let scanner = ProjectScanner::new();
    scanner
        .scan_all()
        .await
        .map_err(|e| e.to_string())
}

/// Get sessions for a project
#[tauri::command]
pub async fn get_project_sessions(project_id: String) -> Result<Vec<Session>, String> {
    let scanner = ProjectScanner::new();
    scanner
        .get_sessions(&project_id)
        .await
        .map_err(|e| e.to_string())
}

/// Get session detail
#[tauri::command]
pub async fn get_session_detail(session_id: String) -> Result<SessionDetail, String> {
    // TODO: Implement
    Err("Not implemented".to_string())
}

/// Search projects and sessions
#[tauri::command]
pub async fn search_projects(query: String) -> Result<Vec<Project>, String> {
    let scanner = ProjectScanner::new();
    let all_projects = scanner.scan_all().await.map_err(|e| e.to_string())?;

    let query_lower = query.to_lowercase();
    let filtered: Vec<Project> = all_projects
        .into_iter()
        .filter(|p| p.name.to_lowercase().contains(&query_lower))
        .collect();

    Ok(filtered)
}
```

### 5.2 Agent Commands

```rust
// src-tauri/src/commands/agents.rs

use crate::models::agent::{Agent, AgentRun};
use crate::services::agent::registry::AgentRegistry;
use tauri::State;
use std::sync::Mutex;

pub struct AgentState {
    pub registry: Mutex<AgentRegistry>,
}

/// List all agents
#[tauri::command]
pub async fn list_agents(state: State<'_, AgentState>) -> Result<Vec<Agent>, String> {
    let registry = state.registry.lock().map_err(|e| e.to_string())?;
    Ok(registry.list())
}

/// Create a new agent
#[tauri::command]
pub async fn create_agent(
    state: State<'_, AgentState>,
    agent: Agent,
) -> Result<Agent, String> {
    let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
    registry.add(agent.clone());
    registry.save().await.map_err(|e| e.to_string())?;
    Ok(agent)
}

/// Update an agent
#[tauri::command]
pub async fn update_agent(
    state: State<'_, AgentState>,
    agent: Agent,
) -> Result<Agent, String> {
    let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
    registry.update(agent.clone());
    registry.save().await.map_err(|e| e.to_string())?;
    Ok(agent)
}

/// Delete an agent
#[tauri::command]
pub async fn delete_agent(
    state: State<'_, AgentState>,
    agent_id: String,
) -> Result<(), String> {
    let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
    registry.remove(&agent_id);
    registry.save().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Run an agent
#[tauri::command]
pub async fn run_agent(
    state: State<'_, AgentState>,
    agent_id: String,
    input: String,
) -> Result<String, String> {
    // TODO: Implement agent execution
    Err("Not implemented".to_string())
}

/// Get agent run history
#[tauri::command]
pub async fn get_agent_runs(
    state: State<'_, AgentState>,
    agent_id: String,
) -> Result<Vec<AgentRun>, String> {
    // TODO: Implement
    Ok(Vec::new())
}
```

### 5.3 MCP Commands

```rust
// src-tauri/src/commands/mcp.rs

use crate::models::mcp::{HealthStatus, McpServer};
use crate::services::mcp::registry::McpRegistry;
use std::sync::Mutex;
use tauri::State;

pub struct McpState {
    pub registry: Mutex<McpRegistry>,
}

/// List all MCP servers
#[tauri::command]
pub async fn list_mcp_servers(state: State<'_, McpState>) -> Result<Vec<McpServer>, String> {
    let registry = state.registry.lock().map_err(|e| e.to_string())?;
    Ok(registry.list())
}

/// Add MCP server
#[tauri::command]
pub async fn add_mcp_server(
    state: State<'_, McpState>,
    server: McpServer,
) -> Result<McpServer, String> {
    let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
    registry.add(server.clone());
    registry.save().await.map_err(|e| e.to_string())?;
    Ok(server)
}

/// Update MCP server
#[tauri::command]
pub async fn update_mcp_server(
    state: State<'_, McpState>,
    server: McpServer,
) -> Result<McpServer, String> {
    let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
    registry.update(server.clone());
    registry.save().await.map_err(|e| e.to_string())?;
    Ok(server)
}

/// Delete MCP server
#[tauri::command]
pub async fn delete_mcp_server(
    state: State<'_, McpState>,
    server_id: String,
) -> Result<(), String> {
    let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
    registry.remove(&server_id);
    registry.save().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Import from Claude Desktop
#[tauri::command]
pub async fn import_mcp_from_claude_desktop(
    state: State<'_, McpState>,
) -> Result<Vec<McpServer>, String> {
    let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
    let imported = registry
        .import_from_claude_desktop()
        .await
        .map_err(|e| e.to_string())?;
    registry.save().await.map_err(|e| e.to_string())?;
    Ok(imported)
}

/// Test MCP server health
#[tauri::command]
pub async fn check_mcp_health(
    state: State<'_, McpState>,
    server_id: String,
) -> Result<HealthStatus, String> {
    let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
    registry
        .check_health(&server_id)
        .await
        .map_err(|e| e.to_string())
}
```

---

## 6. Storage Layer

### 6.1 SQLite Database Schema

```sql
-- ~/.plan-cascade/data.db

-- Usage analytics
CREATE TABLE IF NOT EXISTS usage_records (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    model TEXT NOT NULL,
    project_id TEXT,
    agent_id TEXT,
    input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    cost_usd REAL NOT NULL,
    source TEXT NOT NULL
);

CREATE INDEX idx_usage_timestamp ON usage_records(timestamp);
CREATE INDEX idx_usage_project ON usage_records(project_id);
CREATE INDEX idx_usage_model ON usage_records(model);

-- Agent run history
CREATE TABLE IF NOT EXISTS agent_runs (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    status TEXT NOT NULL,
    input TEXT NOT NULL,
    output TEXT,
    input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    cost_usd REAL NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    error TEXT
);

CREATE INDEX idx_runs_agent ON agent_runs(agent_id);
CREATE INDEX idx_runs_started ON agent_runs(started_at);

-- Checkpoints
CREATE TABLE IF NOT EXISTS checkpoints (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    name TEXT,
    created_at TEXT NOT NULL,
    parent_id TEXT,
    message_index INTEGER NOT NULL,
    snapshot TEXT NOT NULL
);

CREATE INDEX idx_checkpoints_session ON checkpoints(session_id);

-- Branches
CREATE TABLE IF NOT EXISTS branches (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    name TEXT NOT NULL,
    base_checkpoint_id TEXT NOT NULL,
    head_checkpoint_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    status TEXT NOT NULL
);

CREATE INDEX idx_branches_session ON branches(session_id);
```

### 6.2 Database Implementation

```rust
// src-tauri/src/storage/database.rs

use crate::models::analytics::UsageRecord;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new() -> Result<Self> {
        let home = dirs::home_dir().expect("Could not find home directory");
        let db_path = home.join(".plan-cascade").join("data.db");

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;

        // Initialize schema
        conn.execute_batch(include_str!("schema.sql"))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub async fn insert_usage_record(&self, record: &UsageRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO usage_records
             (id, timestamp, model, project_id, agent_id, input_tokens, output_tokens, cost_usd, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.id,
                record.timestamp.to_rfc3339(),
                record.model,
                record.project_id,
                record.agent_id,
                record.input_tokens,
                record.output_tokens,
                record.cost_usd,
                format!("{:?}", record.source),
            ],
        )?;
        Ok(())
    }

    pub async fn get_usage_records(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UsageRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, model, project_id, agent_id, input_tokens, output_tokens, cost_usd, source
             FROM usage_records
             WHERE timestamp >= ?1 AND timestamp <= ?2
             ORDER BY timestamp DESC"
        )?;

        let records = stmt.query_map(
            params![start.to_rfc3339(), end.to_rfc3339()],
            |row| {
                Ok(UsageRecord {
                    id: row.get(0)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                        .unwrap()
                        .with_timezone(&Utc),
                    model: row.get(2)?,
                    project_id: row.get(3)?,
                    agent_id: row.get(4)?,
                    input_tokens: row.get(5)?,
                    output_tokens: row.get(6)?,
                    cost_usd: row.get(7)?,
                    source: crate::models::analytics::UsageSource::ClaudeCode, // TODO: Parse
                })
            },
        )?;

        records.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }
}
```

---

## 7. Cargo Dependencies

```toml
# src-tauri/Cargo.toml

[package]
name = "plan-cascade-desktop"
version = "5.0.0"
edition = "2021"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
# Tauri
tauri = { version = "2", features = ["shell-open"] }
tauri-plugin-shell = "2"

# Async runtime
tokio = { version = "1", features = ["full", "process"] }

# HTTP client
reqwest = { version = "0.12", features = ["json", "stream"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Database
rusqlite = { version = "0.31", features = ["bundled"] }

# Secure storage
keyring = "2"

# File system
walkdir = "2"
glob = "0.3"
notify = "6"
dirs = "5"

# Time
chrono = { version = "0.4", features = ["serde"] }

# UUID
uuid = { version = "1", features = ["v4", "serde"] }

# Markdown
pulldown-cmark = "0.10"

# Diff
similar = "2"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Error handling
thiserror = "1"
anyhow = "1"

# Regex
regex = "1"

# Async utilities
futures = "0.3"
async-trait = "0.1"

# Concurrency
parking_lot = "0.12"
dashmap = "5"

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

---

## 8. Development Roadmap

### Phase 1: Rust Backend Foundation (2 weeks)

```
Week 1:
├── [ ] Setup Rust project structure
├── [ ] Implement storage layer (SQLite, keyring)
├── [ ] Implement project scanner
├── [ ] Basic Tauri commands
└── [ ] Remove Python sidecar

Week 2:
├── [ ] Claude Code CLI integration
├── [ ] Stream JSON parser
├── [ ] Session management
└── [ ] Frontend integration
```

### Phase 2: Core Features (3 weeks)

```
Week 3:
├── [ ] Project Browser UI
├── [ ] Session list and detail
├── [ ] Search functionality
└── [ ] CLAUDE.md scanner

Week 4:
├── [ ] CLAUDE.md Editor
├── [ ] Markdown preview
├── [ ] MCP Server Registry
└── [ ] Import from Claude Desktop

Week 5:
├── [ ] Basic Analytics
├── [ ] Usage tracking
├── [ ] Cost display
└── [ ] Export functionality
```

### Phase 3: Advanced Features (3 weeks)

```
Week 6:
├── [ ] Agent Library UI
├── [ ] Agent Editor
├── [ ] Agent execution
└── [ ] Run history

Week 7:
├── [ ] Timeline View
├── [ ] Checkpoint creation
├── [ ] Branch management
└── [ ] Diff viewer

Week 8:
├── [ ] Standalone Mode
├── [ ] LLM providers
├── [ ] Tool execution
└── [ ] Orchestration
```

### Phase 4: Polish & Release (2 weeks)

```
Week 9-10:
├── [ ] UI/UX polish
├── [ ] Performance optimization
├── [ ] Error handling
├── [ ] Documentation
├── [ ] Auto-update system
├── [ ] Release builds
└── [ ] Testing
```

---

## 9. Appendix

### 9.1 File Locations

| Data | Location |
|------|----------|
| Claude Projects | `~/.claude/projects/` |
| Claude Sessions | `~/.claude/projects/{id}/sessions/` |
| Desktop Config | `~/.plan-cascade/config.json` |
| Desktop Database | `~/.plan-cascade/data.db` |
| Agent Library | `~/.plan-cascade/agents.json` |
| MCP Servers | `~/.plan-cascade/mcp-servers.json` |

### 9.2 Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | Claude API key (Standalone mode) |
| `OPENAI_API_KEY` | OpenAI API key (Standalone mode) |
| `PLAN_CASCADE_DATA_DIR` | Override data directory |
| `PLAN_CASCADE_DEBUG` | Enable debug logging |

### 9.3 Glossary

| Term | Definition |
|------|------------|
| Claude Code GUI Mode | Desktop serves as visual interface for Claude Code CLI |
| Standalone Mode | Desktop operates independently with direct LLM API calls |
| CC Agent | Custom AI agent with specific system prompt and tools |
| Checkpoint | Saved snapshot of session state at a point in time |
| MCP Server | Model Context Protocol server for extended capabilities |
| Stream JSON | Claude Code's JSON streaming output format |

