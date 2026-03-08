# Plan Cascade Desktop Architecture Design Document

**Version**: 1.0.0
**Date**: 2026-03-08
**Scope**: desktop/docs

---

## Table of Contents

1. [Product Positioning and Design Philosophy](#1-product-positioning-and-design-philosophy)
2. [Technology Stack Overview](#2-technology-stack-overview)
3. [Dual-Mode Architecture](#3-dual-mode-architecture)
4. [Current Capabilities Overview](#4-current-capabilities-overview)
5. [Core Differentiating Features](#5-core-differentiating-features)
6. [Design Philosophy Notes](#6-design-philosophy-notes)

---

## 1. Product Positioning and Design Philosophy

### 1.1 Product Positioning

Plan Cascade Desktop is a developer-oriented desktop AI coding assistant, built with a **pure Rust backend architecture** using the Tauri framework to deliver efficient, secure cross-platform desktop applications.

### 1.2 Core Design Principles

- **Single Source of Truth (SSOT)**: Critical states (such as workflow kernel phases) use a single authoritative data source to avoid state inconsistency
- **Event-Driven**: Real-time updates via Tauri event push, reducing polling overhead
- **Memory and Skill System**: Supports cross-session persistent memory and reusable skill templates
- **Codebase Indexing**: Hybrid search combining Tree-sitter, FTS5 full-text search, and HNSW vector indexing

---

## 2. Technology Stack Overview

### 2.1 Core Technology Stack

| Layer | Technology | Description |
|-------|------------|-------------|
| Frontend Framework | React 18 + TypeScript | UI building |
| State Management | Zustand | Lightweight state management |
| Backend Framework | Tauri 2.x | Cross-platform desktop app framework |
| Backend Language | Rust | Core business logic |
| Database | SQLite (r2d2 connection pool) | Local data storage |
| Vector Indexing | hnsw_rs | Approximate nearest neighbor search |
| Full-Text Search | SQLite FTS5 | BM25 ranked search |
| Code Parsing | Tree-sitter | Fast syntax parsing |

### 2.2 Architecture Layers

```
┌─────────────────────────────────────────────────────────────┐
│                    React Frontend (TypeScript)               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │ Components  │  │ Zustand     │  │ API Wrappers        │ │
│  │             │  │ Store       │  │ (Tauri IPC)         │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└────────────────────────────┬────────────────────────────────┘
                             │ Tauri IPC
┌────────────────────────────┴────────────────────────────────┐
│                    Rust Backend (Tauri)                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │ Commands    │  │ Services    │  │ Storage Layer       │ │
│  │ (IPC)       │  │ (Business)  │  │ (SQLite + Keyring)  │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└──────────────────────────────────��──────────────────────────┘
```

---

## 3. Dual-Mode Architecture

### 3.1 Chat / Plan / Task Three Modes

Plan Cascade Desktop adopts a **dual-mode architecture**, supporting switching between different interaction modes within the same application:

| Mode | Purpose | Characteristics |
|------|---------|-----------------|
| **Chat Mode** | Conversational programming | Free dialogue, LLM directly executes tool calls |
| **Plan Mode** | Plan-driven | Structured step planning, batch execution |
| **Task Mode** | Task management | Large task gate checks and quality gates |

### 3.2 Mode Switching Mechanism

- **Chat → Plan/Task**: Intelligent suggestion based on conversation content
- **Plan/Task → Chat**: Can return to conversation mode after execution completes
- **Context Handoff**: Context between modes passed via kernel snapshots

### 3.3 Simple Page V2 Production Path

The Simple page (`src/components/SimpleMode`) follows these production constraints:

- `workflowKernel.modeSnapshots` is the sole authoritative source for Chat/Plan/Task lifecycle
- Frontend consumes kernel push updates via `workflow-kernel-updated` event
- Workflow cards are rendered only from typed payloads (`StreamLine.cardPayload`)
- Plan/Task backend sessions are explicitly linked to kernel sessions via `workflow_link_mode_session`

See [kernel-design.md](./kernel-design.md) for details.

---

## 4. Current Capabilities Overview

### 4.1 Multi-LLM Support

- Supports multiple LLM providers (OpenAI, Anthropic, Ollama, etc.)
- Configurable model selection
- Streaming response support

### 4.2 Codebase Intelligence

| Capability | Technical Implementation |
|------------|--------------------------|
| Semantic Search | HNSW vector indexing |
| Symbol Search | FTS5 full-text search + BM25 ranking |
| File Search | FTS5 path search |
| Type Resolution | LSP enhancement layer (optional) |

See [codebase-index-design.md](./codebase-index-design.md) for details.

### 4.3 Agent Models

Supports multiple agent types:

- **LLM Agent**: Pure LLM decision-making agent
- **Graph Workflow**: Graph-structured workflow
- **Loop Agent**: Loop execution agent
- **Parallel Agent**: Parallel execution agent
- **Conditional Agent**: Conditional branch agent

### 4.4 Quality Gates

- Configurable validation rules
- Step output quality checks
- Retry strategy support

---

## 5. Core Differentiating Features

| Feature | Description | Competitive Difference |
|---------|-------------|----------------------|
| **Pure Rust Backend** | No Python dependencies, single binary | Most competitors rely on external services |
| **Workflow Kernel SSOT** | Single source of truth for phase states | Avoids state drift |
| **Memory System V2** | Cross-session persistent memory | Most competitors are session-level |
| **Skill System** | Reusable task templates | Few competitors have built-in skills |
| **Hybrid Search** | FTS5 + HNSW + LSP | Multi-dimensional code understanding |
| **Quality Gates** | Step-level validation | Finer-grained execution control |

---

## 6. Design Philosophy Notes

### 6.1 Reliability First

- **Kernel Authority**: Critical states managed uniformly by kernel, UI read-only
- **Type Safety**: Full-chain TypeScript + Rust type definitions
- **Error Boundaries**: Graceful degradation, no global collapse from local errors

### 6.2 Performance Optimization

- **Vectorized Search**: HNSW reduces search from O(n) to O(log n)
- **Event-Driven**: Push instead of polling, reducing latency
- **Background Indexing**: Index building does not block main thread

### 6.3 Extensibility Design

- **Hooks System**: Agentic lifecycle hooks support cross-cutting concerns
- **Skill Format Compatibility**: Supports multiple formats including Plan Cascade SKILL.md, adk-rust .skills/, CLAUDE.md
- **Plugin Architecture**: MCP (Model Context Protocol) support for external tool integration

### 6.4 Observability

- **Structured Logging**: All major operations logged with trace IDs
- **Metrics Collection**: ContextOps adds memory-specific SLI fields
- **Error Reporting**: Automatic collection of crash information and diagnostic data
