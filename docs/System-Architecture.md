[中文版](System-Architecture_zh.md)

# Plan Cascade - System Architecture and Workflow Design

**Version**: 4.1.0
**Last Updated**: 2026-01-29

This document contains detailed architecture diagrams, flowcharts, and system design for Plan Cascade.

---

## Table of Contents

1. [Three-Tier Architecture](#1-three-tier-architecture)
2. [Core Components](#2-core-components)
3. [Complete Workflow](#3-complete-workflow)
4. [Auto Strategy Workflow](#4-auto-strategy-workflow)
5. [Mega Plan Workflow](#5-mega-plan-workflow)
6. [Hybrid Worktree Workflow](#6-hybrid-worktree-workflow)
7. [Hybrid Auto Workflow](#7-hybrid-auto-workflow)
8. [Auto-Iteration Workflow](#8-auto-iteration-workflow)
9. [Data Flow and State Files](#9-data-flow-and-state-files)
10. [Dual-Mode Architecture](#10-dual-mode-architecture)
11. [Multi-Agent Collaboration Architecture](#11-multi-agent-collaboration-architecture)

---

## 1. Three-Tier Architecture

```mermaid
graph TB
    subgraph "Level 1: Mega Plan Project Level"
        MP[mega-plan.json] --> F1[Feature 1]
        MP --> F2[Feature 2]
        MP --> F3[Feature 3]
    end

    subgraph "Level 2: Hybrid Ralph Feature Level"
        F1 --> W1[Worktree 1]
        F2 --> W2[Worktree 2]
        F3 --> W3[Worktree 3]
        W1 --> PRD1[prd.json]
        W2 --> PRD2[prd.json]
        W3 --> PRD3[prd.json]
    end

    subgraph "Level 3: Stories Story Level"
        PRD1 --> S1[Story 1-1]
        PRD1 --> S2[Story 1-2]
        PRD2 --> S3[Story 2-1]
        PRD2 --> S4[Story 2-2]
        PRD3 --> S5[Story 3-1]
    end

    subgraph "Agents"
        S1 --> A1[Claude Code]
        S2 --> A2[Codex]
        S3 --> A3[Aider]
        S4 --> A1
        S5 --> A2
    end
```

### Tier Details

| Tier | Name | Responsibility | Artifact |
|------|------|----------------|----------|
| **Level 1** | Mega Plan | Project-level orchestration, manages dependencies and execution order of multiple Features | `mega-plan.json` |
| **Level 2** | Hybrid Ralph | Feature-level development, executes in isolated Worktree, auto-generates PRD | `prd.json`, `findings.md` |
| **Level 3** | Stories | Story-level execution, processed in parallel by Agents, supports quality gates and retries | Code changes, `progress.txt` |

---

## 2. Core Components

```mermaid
graph LR
    subgraph "Orchestration Layer"
        O[Orchestrator<br/>Orchestrator]
        IL[IterationLoop<br/>Iteration Loop]
    end

    subgraph "Execution Layer"
        AE[AgentExecutor<br/>Agent Executor]
        PM[PhaseManager<br/>Phase Manager]
        CPD[CrossPlatformDetector<br/>Cross-Platform Detection]
    end

    subgraph "Quality Layer"
        QG[QualityGate<br/>Quality Gate]
        RM[RetryManager<br/>Retry Manager]
    end

    subgraph "State Layer"
        SM[StateManager<br/>State Manager]
        CF[ContextFilter<br/>Context Filter]
    end

    O --> IL
    IL --> AE
    AE --> PM
    PM --> CPD
    IL --> QG
    QG --> RM
    O --> SM
    SM --> CF
```

### Component Descriptions

| Component | Responsibility |
|-----------|----------------|
| **Orchestrator** | Core orchestrator, coordinates all components |
| **IterationLoop** | Auto-iteration loop, manages batch execution |
| **AgentExecutor** | Agent execution abstraction, supports multiple Agents |
| **PhaseManager** | Phase management, selects Agent based on phase |
| **QualityGate** | Quality gates, validates code quality |
| **RetryManager** | Retry management, handles failure retries |
| **StateManager** | State management, persists execution state |
| **ContextFilter** | Context filter, optimizes Agent input |

---

## 3. Complete Workflow

```mermaid
flowchart TB
    subgraph "Entry Selection"
        START{Project Scale?}
        START -->|Multiple Feature Modules| MEGA["/plan-cascade:mega-plan"]
        START -->|Single Feature + Isolation| HW["/plan-cascade:hybrid-worktree"]
        START -->|Simple Feature| HA["/plan-cascade:hybrid-auto"]
    end

    subgraph "Mega Plan Flow"
        MEGA --> MP_GEN[Generate mega-plan.json]
        MP_GEN --> MP_EDIT{Edit?}
        MP_EDIT -->|Yes| MP_MODIFY["/plan-cascade:mega-edit"]
        MP_MODIFY --> MP_GEN
        MP_EDIT -->|No| MP_APPROVE["/plan-cascade:mega-approve"]
        MP_APPROVE --> MP_BATCH[Create Worktrees by Batch]
        MP_BATCH --> MP_PRD[Generate PRD for each Feature]
    end

    subgraph "Hybrid Worktree Flow"
        HW --> HW_CREATE[Create Worktree + Branch]
        HW_CREATE --> HW_PRD["/plan-cascade:hybrid-auto Generate PRD"]
    end

    subgraph "Hybrid Auto Flow"
        HA --> HA_GEN[Analyze Task + Generate PRD]
    end

    MP_PRD --> PRD_REVIEW
    HW_PRD --> PRD_REVIEW
    HA_GEN --> PRD_REVIEW

    subgraph "PRD Review"
        PRD_REVIEW[Display PRD Preview]
        PRD_REVIEW --> PRD_EDIT{Edit?}
        PRD_EDIT -->|Yes| PRD_MODIFY["/plan-cascade:edit"]
        PRD_MODIFY --> PRD_REVIEW
        PRD_EDIT -->|No| APPROVE["/plan-cascade:approve"]
    end

    subgraph "Execution Phase"
        APPROVE --> EXEC_MODE{Execution Mode?}
        EXEC_MODE -->|Manual| MANUAL[Manual Batch Progression]
        EXEC_MODE -->|Auto| AUTO[Auto-Iteration Loop]

        AUTO --> BATCH[Execute Current Batch]
        MANUAL --> BATCH
        BATCH --> PARALLEL[Start Agents in Parallel]
        PARALLEL --> WAIT[Wait for Completion]
        WAIT --> QG{Quality Gate}
        QG -->|Pass| NEXT{Next Batch?}
        QG -->|Fail| RETRY{Retry?}
        RETRY -->|Yes| BATCH
        RETRY -->|No| FAIL[Mark Failed]
        NEXT -->|Yes| BATCH
        NEXT -->|No| DONE[Execution Complete]
    end

    subgraph "Completion Phase"
        DONE --> COMPLETE["/plan-cascade:complete or<br/>/plan-cascade:mega-complete"]
        COMPLETE --> MERGE[Merge to Target Branch]
        MERGE --> CLEANUP[Cleanup Worktree]
    end
```

---

## 4. Auto Strategy Workflow

The `/plan-cascade:auto` command provides AI-driven automatic strategy selection based on task analysis.

### Strategy Selection Flowchart

```mermaid
flowchart TD
    A["/plan-cascade:auto<br/>Task Description"] --> B[Gather Project Context]
    B --> C[AI Strategy Analysis]

    C --> D{Keyword Detection}

    D -->|"platform, system,<br/>architecture, 3+ modules"| E[MEGA_PLAN]
    D -->|"implement, create +<br/>experimental, refactor"| F[HYBRID_WORKTREE]
    D -->|"implement, create,<br/>build, feature"| G[HYBRID_AUTO]
    D -->|"fix, update, simple<br/>or default"| H[DIRECT]

    E --> I["/plan-cascade:mega-plan"]
    F --> J["/plan-cascade:hybrid-worktree"]
    G --> K["/plan-cascade:hybrid-auto"]
    H --> L[Direct Execution]

    I --> M[Multi-Feature Orchestration]
    J --> N[Isolated Development]
    K --> O[PRD + Story Execution]
    L --> P[Task Complete]
```

### Strategy Detection Rules

| Priority | Strategy | Keywords | Condition |
|----------|----------|----------|-----------|
| 1 | **MEGA_PLAN** | platform, system, architecture, microservices | OR 3+ independent modules listed |
| 2 | **HYBRID_WORKTREE** | (feature keywords) + experimental, refactor, isolated | Both conditions required |
| 3 | **HYBRID_AUTO** | implement, create, build, feature, api | Without isolation keywords |
| 4 | **DIRECT** | fix, typo, update, simple, single | Default fallback |

### Example Strategy Mappings

| Task Description | Detected Keywords | Selected Strategy |
|-----------------|-------------------|-------------------|
| "Fix the typo in README" | fix, typo | DIRECT |
| "Implement user authentication with OAuth" | implement, authentication | HYBRID_AUTO |
| "Experimental refactoring of payment module" | refactoring + experimental | HYBRID_WORKTREE |
| "Build e-commerce platform with users, products, cart, orders" | platform + 4 modules | MEGA_PLAN |

---

## 5. Mega Plan Workflow

Suitable for large project development containing multiple related feature modules.

### Use Cases

| Type | Scenario | Example |
|------|----------|---------|
| ✅ Suitable | Multi-module new project development | Build SaaS platform (user + subscription + billing + admin) |
| ✅ Suitable | Large-scale refactoring involving multiple subsystems | Monolith to microservices architecture |
| ✅ Suitable | Feature group development | E-commerce platform (users, products, cart, orders) |
| ❌ Not suitable | Single feature development | Only implement user authentication (use Hybrid Ralph) |
| ❌ Not suitable | Bug fixes | Fix login page form validation issue |

### Sequential Execution Between Batches

```
mega-approve (1st time) → Start Batch 1
    ↓ Batch 1 complete
mega-approve (2nd time) → Merge Batch 1 → Create Batch 2 from updated branch
    ↓ Batch 2 complete
mega-approve (3rd time) → Merge Batch 2 → ...
    ↓ All batches complete
mega-complete → Clean up planning files
```

### Detailed Flowchart

```mermaid
flowchart TD
    A["<b>/plan-cascade:mega-plan</b><br/>E-commerce platform: users, products, orders"] --> B[Analyze Project Requirements]
    B --> C[Identify Feature Modules]
    C --> D[Generate Feature List]
    D --> E[Analyze Feature Dependencies]
    E --> F[Generate mega-plan.json]

    F --> G{User Action}
    G -->|Edit| H["/plan-cascade:mega-edit"]
    H --> F
    G -->|Approve| I["<b>/plan-cascade:mega-approve</b><br/>(1st time)"]

    I --> J[Create Batch 1 Worktrees]
    J --> K[Batch 1: Infrastructure]

    subgraph "Feature Parallel Development (Batch 1)"
        K --> L1["Feature: User System<br/>Worktree: .worktrees/user"]
        K --> L2["Feature: Product System<br/>Worktree: .worktrees/product"]

        L1 --> M1[Auto-generate PRD]
        L2 --> M2[Auto-generate PRD]

        M1 --> N1[Execute Stories<br/>+ Quality Gates + Retry]
        M2 --> N2[Execute Stories<br/>+ Quality Gates + Retry]
    end

    N1 --> O1[Feature Complete]
    N2 --> O2[Feature Complete]

    O1 --> P["<b>/plan-cascade:mega-approve</b><br/>(2nd time)"]
    O2 --> P
    P --> P1[Merge Batch 1 to Target Branch]
    P1 --> P2[Create Batch 2 from Updated Branch]
    P2 --> Q[Batch 2: Order System<br/>Depends on User+Product]

    Q --> R[Continue Execution...]
    R --> S[All Features Complete]
    S --> T["<b>/plan-cascade:mega-complete</b>"]
    T --> U[Clean Up Planning Files]
    U --> V[Clean Up All Worktrees]
```

---

## 6. Hybrid Worktree Workflow

Suitable for single complex feature development requiring branch isolation.

### Use Cases

| Type | Scenario | Example |
|------|----------|---------|
| ✅ Suitable | Complete feature with multiple subtasks | User authentication (registration + login + password reset) |
| ✅ Suitable | Experimental feature requiring branch isolation | New payment channel integration test |
| ✅ Suitable | Medium-scale refactoring (5-20 files) | API layer unified error handling |
| ❌ Not suitable | Simple single-file modification | Modify a component's style |
| ❌ Not suitable | Quick prototype validation | Verify if a library is usable |

### Detailed Flowchart

```mermaid
flowchart TD
    A["<b>/plan-cascade:hybrid-worktree</b><br/>feature-auth main User Authentication"] --> B[Create Git Branch]
    B --> C[Create Worktree Directory]
    C --> D[Initialize Planning Files]
    D --> E["<b>/plan-cascade:hybrid-auto</b><br/>Generate PRD"]

    E --> F[Analyze Task Description]
    F --> G[Scan Codebase Structure]
    G --> H[Generate prd.json]
    H --> I[Display PRD Preview]

    I --> J{User Action}
    J -->|Edit| K["/plan-cascade:edit"]
    K --> I
    J -->|Approve| L["<b>/plan-cascade:approve</b>"]

    L --> M{Execution Mode}
    M -->|"--auto-run"| N[Auto-Iteration Mode]
    M -->|Manual| O[Manual Mode]

    subgraph "Auto-Iteration"
        N --> P[Execute Batch 1]
        P --> Q[Parallel Agent Execution]
        Q --> R[Quality Gate Check]
        R --> S{Pass?}
        S -->|Yes| T{More Batches?}
        S -->|No| U[Smart Retry]
        U --> Q
        T -->|Yes| P
        T -->|No| V[All Complete]
    end

    subgraph "Manual Mode"
        O --> W[Execute Batch 1]
        W --> X["/plan-cascade:status View Progress"]
        X --> Y[Manual Advance to Next Batch]
        Y --> W
    end

    V --> Z["<b>/plan-cascade:hybrid-complete</b>"]
    Z --> AA[Merge to main Branch]
    AA --> AB[Delete Worktree]
```

---

## 7. Hybrid Auto Workflow

Suitable for quick development of simple features without Worktree isolation.

### Detailed Flowchart

```mermaid
flowchart TD
    A["<b>/plan-cascade:hybrid-auto</b><br/>Add Password Reset Functionality"] --> B[Parse Task Description]
    B --> C[Analyze Codebase Context]
    C --> D{Generate PRD}

    D --> E[Goal: Main Objective]
    D --> F[Objectives: Sub-objectives List]
    D --> G[Stories: User Stories]

    G --> H[Story 1: Design API]
    G --> I[Story 2: Implement Backend]
    G --> J[Story 3: Add Email]
    G --> K[Story 4: Frontend Page]

    H --> L[Dependency Analysis]
    I --> L
    J --> L
    K --> L

    L --> M[Generate Execution Batches]
    M --> N["Batch 1: Story 1<br/>Batch 2: Story 2, 3<br/>Batch 3: Story 4"]

    N --> O[Display PRD Preview]
    O --> P{User Action}

    P -->|Edit| Q["/plan-cascade:edit"]
    Q --> O
    P -->|Approve| R["<b>/plan-cascade:approve</b>"]
    P -->|"Approve+Auto"| S["<b>/plan-cascade:approve --auto-run</b>"]

    R --> T[Manual Execution Mode]
    S --> U[Auto-Iteration Mode]

    subgraph "Execution Details"
        T --> V[Start Batch 1]
        U --> V
        V --> W["Agent Parallel Execution<br/>(Multiple Agents Supported)"]
        W --> X[Quality Gates]
        X --> Y{Check Result}
        Y -->|typecheck ❌| Z[Retry + Failure Context]
        Y -->|test ❌| Z
        Y -->|Pass ✓| AA[Advance to Next Batch]
        Z --> W
        AA --> V
    end

    AA --> AB[All Stories Complete]
    AB --> AC[Display Execution Summary]
```

---

## 8. Auto-Iteration Workflow

Auto-iteration loop started by `/plan-cascade:approve --auto-run` or `/plan-cascade:auto-run` command:

```mermaid
flowchart TD
    A[Start Auto-Iteration] --> B[Load Configuration]
    B --> C{Iteration Mode}

    C -->|until_complete| D[Loop Until All Complete]
    C -->|max_iterations| E[Execute at Most N Times]
    C -->|batch_complete| F[Execute Current Batch Only]

    D --> G[Initialize Iteration State]
    E --> G
    F --> G

    G --> H[Get Current Batch Stories]
    H --> I{Pending Tasks?}

    I -->|No| J[Check Completion Condition]
    I -->|Yes| K[Resolve Agent Assignment]

    K --> L[Phase: Implementation]
    L --> M{Agent Selection}
    M --> N1[Story Type: feature → claude-code]
    M --> N2[Story Type: bugfix → codex]
    M --> N3[Story Type: refactor → aider]

    N1 --> O[Start Agents in Parallel]
    N2 --> O
    N3 --> O

    O --> P[Poll Wait<br/>poll_interval: 10s]
    P --> Q{Story Complete?}

    Q -->|Running| P
    Q -->|Complete| R{Quality Gates Enabled?}
    Q -->|Timeout| S[Record Timeout Failure]

    R -->|No| T[Mark Complete]
    R -->|Yes| U[Execute Quality Checks]

    U --> V{TypeCheck}
    V -->|✓| W{Tests}
    V -->|✗| X[Record Failure Details]

    W -->|✓| Y{Lint}
    W -->|✗| X

    Y -->|✓| T
    Y -->|✗ and required| X
    Y -->|✗ but optional| T

    X --> Z{Can Retry?}
    S --> Z

    Z -->|Yes| AA[Build Retry Prompt]
    Z -->|No| AB[Mark Final Failure]

    AA --> AC[Inject Failure Context]
    AC --> AD[Select Retry Agent]
    AD --> O

    T --> AE[Update Iteration State]
    AB --> AE

    AE --> AF{Batch Complete?}
    AF -->|No| H
    AF -->|Yes| AG[Advance to Next Batch]

    AG --> AH{More Batches?}
    AH -->|Yes| H
    AH -->|No| J

    J --> AI{All Successful?}
    AI -->|Yes| AJ[Status: COMPLETED]
    AI -->|No| AK[Status: FAILED]

    AJ --> AL[Save Final State]
    AK --> AL
    AL --> AM[Generate Execution Report]
```

### Iteration Modes

| Mode | Description |
|------|-------------|
| `until_complete` | Continue execution until all Stories complete (default) |
| `max_iterations` | Stop after executing at most N iterations |
| `batch_complete` | Stop after executing current batch only |

---

## 9. Data Flow and State Files

```mermaid
graph TB
    subgraph "Input"
        U[User Description] --> CMD[Command Parser]
        CFG[agents.json] --> CMD
    end

    subgraph "Planning Files"
        CMD --> PRD[prd.json<br/>PRD Document]
        CMD --> MP[mega-plan.json<br/>Project Plan]
    end

    subgraph "Execution State"
        PRD --> AS[.agent-status.json<br/>Agent Status]
        PRD --> IS[.iteration-state.json<br/>Iteration State]
        PRD --> RS[.retry-state.json<br/>Retry State]
    end

    subgraph "Shared Context"
        AS --> FD[findings.md<br/>Findings Record]
        AS --> PG[progress.txt<br/>Progress Log]
    end

    subgraph "Agent Output"
        AS --> AO[.agent-outputs/<br/>├─ story-001.log<br/>├─ story-001.prompt.txt<br/>└─ story-001.result.json]
    end

    subgraph "Cache"
        AD[.agent-detection.json<br/>Agent Detection Cache]
        LK[.locks/<br/>File Locks]
    end

    style PRD fill:#e1f5fe
    style MP fill:#e1f5fe
    style AS fill:#fff3e0
    style IS fill:#fff3e0
    style RS fill:#fff3e0
    style FD fill:#e8f5e9
    style PG fill:#e8f5e9
```

### File Descriptions

| File | Type | Description |
|------|------|-------------|
| `prd.json` | Planning | PRD document, contains goals, stories, dependencies |
| `mega-plan.json` | Planning | Project-level plan, manages multiple Features |
| `agents.json` | Configuration | Agent configuration, includes phase defaults and fallback chains |
| `findings.md` | Shared | Agent findings record, supports tag filtering |
| `progress.txt` | Shared | Progress timeline, includes Agent execution info |
| `.agent-status.json` | State | Agent running/completed/failed status |
| `.iteration-state.json` | State | Auto-iteration progress and batch results |
| `.retry-state.json` | State | Retry history and failure records |
| `.agent-detection.json` | Cache | Cross-platform Agent detection results (1-hour TTL) |
| `.agent-outputs/` | Output | Agent logs, prompts, and result files |

---

## 10. Dual-Mode Architecture

### Mode Switching Design

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Plan Cascade                                     │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─────────────────────────┐     ┌─────────────────────────┐           │
│   │      Simple Mode         │     │      Expert Mode         │           │
│   │                         │     │                         │           │
│   │  User enters description │     │  User enters description │           │
│   │       ↓                 │     │       ↓                 │           │
│   │  AI auto-determines      │     │  Generate PRD (editable) │           │
│   │  strategy               │     │       ↓                 │           │
│   │       ↓                 │     │  User Review/Modify      │           │
│   │  Auto-generate PRD      │     │       ↓                 │           │
│   │       ↓                 │     │  Select Strategy/Agent   │           │
│   │  Auto-execute           │     │       ↓                 │           │
│   │       ↓                 │     │  Execute                │           │
│   │  Complete               │     │                         │           │
│   └─────────────────────────┘     └─────────────────────────┘           │
│                                                                          │
│                              Shared Core                                 │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │  Orchestrator │ PRDGenerator │ QualityGate │ AgentExecutor      │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Dual Working Mode Architecture

**Core Philosophy: Plan Cascade = Brain (Orchestration), Execution Layer = Hands (Tool Execution)**

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Plan Cascade                                   │
│                    (Orchestration Layer - Shared by Both Modes)          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │                    Orchestration Engine (Shared)                  │   │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │   │
│   │  │ PRD Generator│  │ Dependency  │  │  Batch     │              │   │
│   │  │             │  │ Analyzer    │  │  Scheduler │              │   │
│   │  └─────────────┘  └─────────────┘  └─────────────┘              │   │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │   │
│   │  │ State       │  │ Quality    │  │  Retry     │              │   │
│   │  │ Manager     │  │ Gates      │  │  Manager   │              │   │
│   │  └─────────────┘  └─────────────┘  └─────────────┘              │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                              │                                           │
│                    ┌─────────┴─────────┐                                │
│                    │  Execution Layer   │                                │
│                    │  Selection         │                                │
│                    └─────────┬─────────┘                                │
│              ┌───────────────┴───────────────┐                          │
│              ▼                               ▼                          │
│   ┌─────────────────────────┐    ┌─────────────────────────┐           │
│   │  Standalone Orchestration│    │  Claude Code GUI Mode   │           │
│   │  Mode                    │    │                         │           │
│   ├─────────────────────────┤    ├─────────────────────────┤           │
│   │                         │    │                         │           │
│   │   Built-in Tool Engine  │    │   Claude Code CLI       │           │
│   │   ┌───────────────┐     │    │   ┌───────────────┐     │           │
│   │   │ Read/Write    │     │    │   │ Claude Code   │     │           │
│   │   │ Edit/Bash     │     │    │   │ Executes Tools│     │           │
│   │   │ Glob/Grep     │     │    │   │ (stream-json) │     │           │
│   │   └───────────────┘     │    │   └───────────────┘     │           │
│   │          │              │    │          │              │           │
│   │          ▼              │    │          ▼              │           │
│   │   ┌───────────────┐     │    │   ┌───────────────┐     │           │
│   │   │ LLM Abstraction│    │    │   │ Plan Cascade  │     │           │
│   │   │ Layer          │    │    │   │ Visual UI     │     │           │
│   │   │ (Multiple)    │     │    │   └───────────────┘     │           │
│   │   └───────────────┘     │    │                         │           │
│   │          │              │    │                         │           │
│   │   ┌──────┴──────┐       │    │                         │           │
│   │   ▼      ▼      ▼       │    │                         │           │
│   │ Claude Claude OpenAI    │    │                         │           │
│   │ Max    API    etc.      │    │                         │           │
│   │                         │    │                         │           │
│   └─────────────────────────┘    └─────────────────────────┘           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘

Both modes support: PRD-driven development, batch execution, quality gates, state tracking
```

### Standalone Orchestration Mode Architecture Details

```
┌─────────────────────────────────────────────────────────────────────────┐
│                       Standalone Orchestration Mode                       │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─ Orchestration Layer ─────────────────────────────────────────────┐  │
│  │                                                                    │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                │  │
│  │  │ Intent      │  │ Strategy    │  │  PRD        │                │  │
│  │  │ Classifier  │  │ Analyzer    │  │  Generator  │                │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                │  │
│  │         │               │               │                          │  │
│  │         └───────────────┴───────────────┘                          │  │
│  │                         │                                          │  │
│  │                         ▼                                          │  │
│  │  ┌─────────────────────────────────────────────────────────────┐  │  │
│  │  │                   Orchestrator                               │  │  │
│  │  │  • Batch dependency analysis                                 │  │  │
│  │  │  • Parallel execution coordination                           │  │  │
│  │  │  • Quality gate checks                                       │  │  │
│  │  │  • Retry management                                          │  │  │
│  │  └─────────────────────────────────────────────────────────────┘  │  │
│  │                         │                                          │  │
│  └─────────────────────────┼──────────────────────────────────────────┘  │
│                            ▼                                              │
│  ┌─ Execution Layer ─────────────────────────────────────────────────┐  │
│  │                                                                    │  │
│  │  ┌─────────────────────────────────────────────────────────────┐  │  │
│  │  │                   ReAct Execution Engine                     │  │  │
│  │  │                                                              │  │  │
│  │  │   ┌─────────┐     ┌─────────┐     ┌─────────┐               │  │  │
│  │  │   │  Think  │ ──→ │   Act   │ ──→ │ Observe │ ──→ (loop)    │  │  │
│  │  │   │  (LLM)  │     │ (Tool)  │     │ (Result)│               │  │  │
│  │  │   └─────────┘     └─────────┘     └─────────┘               │  │  │
│  │  │                                                              │  │  │
│  │  └─────────────────────────────────────────────────────────────┘  │  │
│  │                         │                                          │  │
│  │                         ▼                                          │  │
│  │  ┌─────────────────────────────────────────────────────────────┐  │  │
│  │  │                   Tool Execution Engine                      │  │  │
│  │  │                                                              │  │  │
│  │  │   ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐   │  │  │
│  │  │   │  Read  │ │ Write  │ │  Edit  │ │  Bash  │ │  Glob  │   │  │  │
│  │  │   └────────┘ └────────┘ └────────┘ └────────┘ └────────┘   │  │  │
│  │  │   ┌────────┐ ┌────────┐                                     │  │  │
│  │  │   │  Grep  │ │   LS   │                                     │  │  │
│  │  │   └────────┘ └────────┘                                     │  │  │
│  │  │                                                              │  │  │
│  │  └─────────────────────────────────────────────────────────────┘  │  │
│  │                                                                    │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                            │                                              │
│                            ▼                                              │
│  ┌─ LLM Layer ───────────────────────────────────────────────────────┐  │
│  │                                                                    │  │
│  │  ┌─────────────────────────────────────────────────────────────┐  │  │
│  │  │                   LLM Abstraction Layer                      │  │  │
│  │  │              (Only provides thinking, no tool execution)     │  │  │
│  │  └─────────────────────────────────────────────────────────────┘  │  │
│  │                         │                                          │  │
│  │       ┌─────────────────┼─────────────────┐                       │  │
│  │       ▼                 ▼                 ▼                       │  │
│  │  ┌─────────┐       ┌─────────┐       ┌─────────┐                 │  │
│  │  │ Claude  │       │ Claude  │       │ OpenAI  │                 │  │
│  │  │   Max   │       │   API   │       │ DeepSeek│                 │  │
│  │  │(via CC) │       │         │       │ Ollama  │                 │  │
│  │  └─────────┘       └─────────┘       └─────────┘                 │  │
│  │                                                                    │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 11. Multi-Agent Collaboration Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                       Multi-Agent Collaboration Architecture             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   Plan Cascade Orchestration Layer                                       │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │  Orchestrator → AgentExecutor → PhaseAgentManager               │   │
│   │       │              │               │                           │   │
│   │       │              │               └─ Phase/Type → Agent Map   │   │
│   │       │              └─ Resolve Best Agent                       │   │
│   │       └─ Schedule Story Execution                                │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                              │                                           │
│              ┌───────────────┴───────────────┐                          │
│              ▼                               ▼                          │
│   ┌─────────────────────────┐    ┌─────────────────────────┐           │
│   │  Standalone Orchestration│    │  Claude Code GUI Mode   │           │
│   │  Mode                    │    │                         │           │
│   │                         │    │                         │           │
│   │   Default Agent:         │    │   Default Agent:         │           │
│   │   Built-in ReAct Engine │    │   Claude Code CLI       │           │
│   │                         │    │                         │           │
│   │   Optional CLI Agents:   │    │   Optional CLI Agents:   │           │
│   │   codex, aider, amp...  │    │   codex, aider, amp...  │           │
│   │                         │    │                         │           │
│   └─────────────────────────┘    └─────────────────────────┘           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Phase-Based Agent Assignment

| Phase | Default Agent | Fallback Chain | Story Type Override |
|-------|--------------|----------------|---------------------|
| `planning` | codex | claude-code | - |
| `implementation` | claude-code | codex, aider | bugfix→codex, refactor→aider |
| `retry` | claude-code | aider | - |
| `refactor` | aider | claude-code | - |
| `review` | claude-code | codex | - |

### Agent Priority Resolution

```
1. --agent command parameter              # Highest priority (global override)
2. Phase override --impl-agent etc.       # Phase-specific override
3. Agent specified in Story               # story.agent field
4. Story type override                    # bugfix → codex, refactor → aider
5. Phase default Agent                    # phase_defaults configuration
6. Fallback chain                         # fallback_chain
7. claude-code                            # Ultimate fallback (always available)
```

---

## Appendix: Two Working Modes Comparison

| Feature | Standalone Orchestration Mode | Claude Code GUI Mode |
|---------|------------------------------|----------------------|
| Orchestration Layer | Plan Cascade | Plan Cascade |
| Tool Execution | Plan Cascade executes itself | Claude Code CLI executes |
| LLM Source | Claude Max/API, OpenAI, DeepSeek, Ollama | Claude Code |
| PRD-Driven | ✅ Full support | ✅ Full support |
| Batch Execution | ✅ Full support | ✅ Full support |
| Offline Available | ✅ (using Ollama) | ❌ |
| Use Case | Need other LLMs or offline use | Have Claude Max/Code subscription |

| Component | Standalone Orchestration Mode | Claude Code GUI Mode |
|-----------|------------------------------|----------------------|
| PRD Generation | Plan Cascade (LLM) | Plan Cascade (Claude Code) |
| Dependency Analysis | Plan Cascade | Plan Cascade |
| Batch Scheduling | Plan Cascade | Plan Cascade |
| Story Execution | Plan Cascade (ReAct) | Claude Code CLI |
| Tool Calls | Built-in Tool Engine | Claude Code |
| State Tracking | Plan Cascade | Plan Cascade |
| Quality Gates | Plan Cascade | Plan Cascade |
