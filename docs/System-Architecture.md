[中文版](System-Architecture_zh.md)

# Plan Cascade - System Architecture and Workflow Design

**Version**: 4.3.1
**Last Updated**: 2026-02-02

This document contains detailed architecture diagrams, flowcharts, and system design for Plan Cascade.

---

## Table of Contents

1. [Three-Tier Architecture](#1-three-tier-architecture)
2. [Core Components](#2-core-components)
3. [Complete Workflow](#3-complete-workflow)
4. [Auto Strategy Workflow](#4-auto-strategy-workflow)
5. [Design Document System](#5-design-document-system)
6. [Mega Plan Workflow](#6-mega-plan-workflow)
7. [Hybrid Worktree Workflow](#7-hybrid-worktree-workflow)
8. [Hybrid Auto Workflow](#8-hybrid-auto-workflow)
9. [Approve and Execute Workflow](#9-approve-and-execute-workflow)
10. [Auto-Iteration Workflow](#10-auto-iteration-workflow)
11. [Path Storage Modes](#11-path-storage-modes)
12. [Data Flow and State Files](#12-data-flow-and-state-files)
13. [Dual-Mode Architecture](#13-dual-mode-architecture)
14. [Multi-Agent Collaboration Architecture](#14-multi-agent-collaboration-architecture)

---

## 1. Three-Tier Architecture

```mermaid
graph TB
    subgraph "Level 1: Mega Plan Project Level"
        MP[mega-plan.json] --> DD1[design_doc.json<br/>Project-level]
        MP --> F1[Feature 1]
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
        PRD1 --> DD2[design_doc.json<br/>Feature-level]
        PRD2 --> DD3[design_doc.json<br/>Feature-level]
        PRD3 --> DD4[design_doc.json<br/>Feature-level]
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

    DD1 -.->|inheritance| DD2
    DD1 -.->|inheritance| DD3
    DD1 -.->|inheritance| DD4
```

### Tier Details

| Tier | Name | Responsibility | Artifact |
|------|------|----------------|----------|
| **Level 1** | Mega Plan | Project-level orchestration, manages dependencies and execution order of multiple Features | `mega-plan.json`, `design_doc.json` (project-level) |
| **Level 2** | Hybrid Ralph | Feature-level development, executes in isolated Worktree, auto-generates PRD and design doc | `prd.json`, `design_doc.json` (feature-level), `findings.md` |
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
        VG[VerificationGate<br/>Implementation Verify]
        RM[RetryManager<br/>Retry Manager]
        GC[GateCache<br/>Gate Cache]
        EP[ErrorParser<br/>Error Parser]
        CFD[ChangedFilesDetector<br/>Changed Files]
    end

    subgraph "State Layer"
        SM[StateManager<br/>State Manager]
        CF[ContextFilter<br/>Context Filter]
        ESL[ExternalSkillLoader<br/>External Skills]
    end

    O --> IL
    IL --> AE
    AE --> PM
    PM --> CPD
    IL --> QG
    QG --> RM
    QG --> GC
    QG --> EP
    QG --> CFD
    O --> SM
    SM --> CF
    CF --> ESL
```

### Component Descriptions

| Component | Responsibility |
|-----------|----------------|
| **Orchestrator** | Core orchestrator, coordinates all components |
| **IterationLoop** | Auto-iteration loop, manages batch execution |
| **AgentExecutor** | Agent execution abstraction, supports multiple Agents |
| **PhaseManager** | Phase management, selects Agent based on phase |
| **QualityGate** | Quality gates with parallel async execution, fail-fast, incremental checking, and caching support |
| **VerificationGate** | AI-powered implementation verification, detects skeleton code and validates acceptance criteria |
| **RetryManager** | Retry management, handles failure retries with structured error context |
| **GateCache** | Gate result caching based on git commit + working tree hash, avoids redundant checks |
| **ErrorParser** | Structured error parsing for mypy, ruff, pytest, eslint, tsc with ErrorInfo extraction |
| **ChangedFilesDetector** | Git-based change detection for incremental gate execution |
| **StateManager** | State management, persists execution state |
| **ContextFilter** | Context filter, optimizes Agent input |
| **ExternalSkillLoader** | Three-tier skill loading (builtin/external/user), auto-detects and injects best practices with priority-based override. Supports phase-based injection (planning, implementation, retry) |

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
        MEGA --> MP_GEN[Generate mega-plan.json<br/>+ design_doc.json]
        MP_GEN --> MP_REVIEW[Unified Review Display]
        MP_REVIEW --> MP_EDIT{Edit?}
        MP_EDIT -->|Yes| MP_MODIFY["/plan-cascade:mega-edit"]
        MP_MODIFY --> MP_REVIEW
        MP_EDIT -->|No| MP_APPROVE["/plan-cascade:mega-approve"]
    end

    subgraph "Hybrid Worktree Flow"
        HW --> HW_CREATE[Create Worktree + Branch]
        HW_CREATE --> HW_PRD[Generate PRD<br/>+ design_doc.json]
        HW_PRD --> HW_REVIEW[Unified Review Display]
    end

    subgraph "Hybrid Auto Flow"
        HA --> HA_GEN[Analyze Task + Generate PRD<br/>+ design_doc.json]
        HA_GEN --> HA_REVIEW[Unified Review Display]
    end

    MP_APPROVE --> BATCH_EXEC
    HW_REVIEW --> PRD_EDIT
    HA_REVIEW --> PRD_EDIT

    subgraph "PRD Review"
        PRD_EDIT{Edit?}
        PRD_EDIT -->|Yes| PRD_MODIFY["/plan-cascade:edit"]
        PRD_MODIFY --> PRD_REVIEW2[Unified Review Display]
        PRD_REVIEW2 --> PRD_EDIT
        PRD_EDIT -->|No| APPROVE["/plan-cascade:approve"]
    end

    subgraph "Execution Phase"
        APPROVE --> AGENT_CFG[Parse Agent Configuration<br/>--agent, --impl-agent, --verify]
        AGENT_CFG --> EXEC_MODE{Execution Mode?}
        EXEC_MODE -->|Manual| MANUAL[Manual Batch Progression]
        EXEC_MODE -->|"--auto-run"| AUTO[Auto-Iteration Loop]

        AUTO --> BATCH_EXEC
        MANUAL --> BATCH_EXEC

        BATCH_EXEC[Execute Current Batch] --> CTX[Load Context<br/>Design Doc + External Skills]
        CTX --> RESOLVE[Resolve Agent per Story<br/>Priority Chain]
        RESOLVE --> PARALLEL[Start Agents in Parallel<br/>Display Agent Assignment]
        PARALLEL --> WAIT[Wait via TaskOutput]
        WAIT --> VERIFY{AI Verify?<br/>--verify}
        VERIFY -->|Yes| VGATE[AI Verification Gate<br/>Detect Skeleton Code]
        VERIFY -->|No| QG
        VGATE --> QG{Quality Gate}
        QG -->|Pass| NEXT{Next Batch?}
        QG -->|Fail| RETRY{Retry?}
        RETRY -->|Yes| RETRY_AGENT[Select Retry Agent<br/>+ Error Context]
        RETRY_AGENT --> PARALLEL
        RETRY -->|No| FAIL[Mark Failed]
        NEXT -->|Yes| BATCH_EXEC
        NEXT -->|No| DONE[Execution Complete]
    end

    subgraph "Completion Phase"
        DONE --> COMPLETE["/plan-cascade:complete or<br/>/plan-cascade:mega-complete"]
        COMPLETE --> MERGE[Merge to Target Branch]
        MERGE --> CLEANUP[Cleanup Worktree]
    end
```

### Key Changes from Previous Version

| Aspect | Previous | Current |
|--------|----------|---------|
| **Design Doc** | Not shown | Auto-generated at each level |
| **Review Display** | "Display PRD Preview" | "Unified Review Display" (PRD + Design Doc) |
| **Agent Config** | Not shown | Explicit `--agent`, `--impl-agent`, `--verify` parsing |
| **Agent Assignment** | Implicit | "Resolve Agent per Story" with priority chain |
| **Verification** | Not shown | Optional "AI Verification Gate" |
| **Retry** | Simple retry | "Select Retry Agent + Error Context" |
| **Wait Mechanism** | Implicit | "Wait via TaskOutput" |

---

## 4. Auto Strategy Workflow

The `/plan-cascade:auto` command provides AI-driven automatic strategy selection based on structured task analysis.

### Strategy Selection Flowchart

```mermaid
flowchart TD
    A["/plan-cascade:auto<br/>Task Description"] --> B[Gather Project Context]
    B --> C[AI Self-Assessment Analysis]

    C --> D[Structured Task Analysis]

    D --> E{Analyze Dimensions}
    E --> E1[Scope: Functional areas?]
    E --> E2[Complexity: Dependencies?]
    E --> E3[Risk: Break existing code?]
    E --> E4[Parallelization benefit?]

    E1 --> F[Output Structured JSON]
    E2 --> F
    E3 --> F
    E4 --> F

    F --> G{Strategy Decision}

    G -->|"4+ areas, multiple features"| H[MEGA_PLAN]
    G -->|"2-3 areas + high risk"| I[HYBRID_WORKTREE]
    G -->|"2-3 areas, 3-7 steps"| J[HYBRID_AUTO]
    G -->|"1 area, 1-2 steps, low risk"| K[DIRECT]

    H --> L["/plan-cascade:mega-plan"]
    I --> M["/plan-cascade:hybrid-worktree"]
    J --> N["/plan-cascade:hybrid-auto"]
    K --> O[Direct Execution]

    L --> P[Multi-Feature Orchestration]
    M --> Q[Isolated Development]
    N --> R[PRD + Story Execution]
    O --> S[Task Complete]
```

### AI Self-Assessment Output

The AI outputs structured analysis in JSON format:

```json
{
  "task_analysis": {
    "functional_areas": ["auth", "api", "frontend"],
    "estimated_stories": 5,
    "has_dependencies": true,
    "requires_architecture_decisions": true,
    "risk_level": "medium",
    "parallelization_benefit": "significant"
  },
  "strategy_decision": {
    "strategy": "HYBRID_AUTO",
    "confidence": 0.85,
    "reasoning": "Task involves 3 functional areas with dependencies..."
  }
}
```

### Strategy Selection Guidelines

| Analysis Result | Strategy | Example |
|----------------|----------|---------|
| 1 functional area, 1-2 steps, low risk | **DIRECT** | "Fix the typo in README" |
| 2-3 functional areas, 3-7 steps, has dependencies | **HYBRID_AUTO** | "Implement user authentication with OAuth" |
| HYBRID_AUTO + high risk or experimental | **HYBRID_WORKTREE** | "Experimental refactoring of payment module" |
| 4+ functional areas, multiple independent features | **MEGA_PLAN** | "Build e-commerce platform with users, products, cart, orders" |

---

## 5. Design Document System

Plan Cascade automatically generates technical design documents (`design_doc.json`) alongside PRDs to provide architectural context during story execution.

### Two-Level Architecture

```mermaid
graph TB
    subgraph "Level 1: Project Design"
        PDD[Project design_doc.json]
        PDD --> ARCH[Architecture Overview]
        PDD --> PATTERNS[Cross-Feature Patterns]
        PDD --> PADRS[Project ADRs<br/>ADR-001, ADR-002...]
        PDD --> FMAP[Feature Mappings]
    end

    subgraph "Level 2: Feature Design"
        FDD[Feature design_doc.json]
        FDD --> COMP[Feature Components]
        FDD --> API[Feature APIs]
        FDD --> FADRS[Feature ADRs<br/>ADR-F001, ADR-F002...]
        FDD --> SMAP[Story Mappings]
    end

    PDD -.->|inheritance| FDD
    PATTERNS -.->|referenced by| COMP
    PADRS -.->|extended by| FADRS
```

### Design Document Schema

```json
{
  "metadata": {
    "created_at": "ISO-8601",
    "version": "1.0.0",
    "source": "ai-generated|user-provided|converted",
    "prd_reference": "prd.json",
    "parent_design_doc": "path/to/project/design_doc.json"
  },
  "overview": {
    "title": "Project/Feature Title",
    "summary": "Summary description",
    "goals": ["Goal 1", "Goal 2"],
    "non_goals": ["Non-goal 1"]
  },
  "architecture": {
    "components": [{
      "name": "ComponentName",
      "description": "Description",
      "responsibilities": ["Responsibility 1"],
      "dependencies": ["DependencyComponent"],
      "files": ["src/file.py"]
    }],
    "data_flow": "Data flow description",
    "patterns": [{
      "name": "PatternName",
      "description": "Description",
      "rationale": "Why this pattern"
    }]
  },
  "interfaces": {
    "apis": [...],
    "data_models": [...]
  },
  "decisions": [{
    "id": "ADR-001",
    "title": "Decision Title",
    "context": "Background context",
    "decision": "The decision made",
    "rationale": "Why this decision",
    "alternatives_considered": ["Alternative 1"],
    "status": "accepted"
  }],
  "story_mappings": {
    "story-001": {
      "components": ["ComponentA"],
      "decisions": ["ADR-001"],
      "interfaces": ["API-1"]
    }
  },
  "feature_mappings": {
    "feature-001": {
      "patterns": ["PatternA"],
      "decisions": ["ADR-001"]
    }
  }
}
```

### Auto-Generation Flow

```mermaid
flowchart TD
    subgraph "Mega Plan Flow"
        MP[mega-plan.json] --> PDD[Generate Project design_doc.json]
        PDD --> F1[Feature 1 Worktree]
        PDD --> F2[Feature 2 Worktree]
        F1 --> PRD1[prd.json]
        F2 --> PRD2[prd.json]
        PRD1 --> FDD1[Feature design_doc.json<br/>inherits from Project]
        PRD2 --> FDD2[Feature design_doc.json<br/>inherits from Project]
    end

    subgraph "Hybrid Auto/Worktree Flow"
        PRD[prd.json] --> FDD[Generate Feature design_doc.json]
    end
```

### External Design Document Import

All three main commands support importing external design documents:

```bash
# mega-plan: 2nd argument
/plan-cascade:mega-plan "Build e-commerce" ./architecture.md

# hybrid-auto: 2nd argument
/plan-cascade:hybrid-auto "Implement auth" ./auth-design.md

# hybrid-worktree: 4th argument
/plan-cascade:hybrid-worktree fix-auth main "Fix auth" ./design.md
```

Supported formats: Markdown (.md), JSON (.json), HTML (.html)

### Context Injection Flow

```mermaid
flowchart LR
    DD[design_doc.json] --> CF[ContextFilter]
    CF --> |story_mappings| SC[Story Context]
    SC --> AE[AgentExecutor]
    AE --> |Design-aware prompt| Agent

    subgraph "Story Context"
        SC --> COMP[Relevant Components]
        SC --> DEC[Relevant Decisions]
        SC --> PAT[Architectural Patterns]
    end
```

### Three-Tier External Skills System

Plan Cascade uses a three-tier skill priority system to provide framework-specific best practices:

```mermaid
flowchart TD
    subgraph "Tier 1: Builtin Skills (Priority 1-50)"
        BS[builtin-skills/]
        BS --> PY[python/]
        BS --> GO[go/]
        BS --> JAVA[java/]
        BS --> TS[typescript/]
    end

    subgraph "Tier 2: External Skills (Priority 51-100)"
        ES[external-skills/]
        ES --> VS[vercel/ - React/Next.js]
        ES --> VUE[vue/ - Vue/Nuxt]
        ES --> RS[rust/ - Rust]
    end

    subgraph "Tier 3: User Skills (Priority 101-200)"
        UC[.plan-cascade/skills.json]
        UC --> LOCAL[Local Path Skills]
        UC --> REMOTE[Remote URL Skills]
    end

    subgraph "Skill Loading"
        PJ[package.json] --> ESL[ExternalSkillLoader]
        CT[Cargo.toml] --> ESL
        PP[pyproject.toml] --> ESL
        ESL --> |detect & dedupe| MERGE{Priority Merge}
        MERGE --> |higher wins| CF2[ContextFilter]
        CF2 --> SC2[Story Context]
    end
```

**Priority Tiers:**

| Tier | Priority Range | Source | Description |
|------|----------------|--------|-------------|
| Builtin | 1-50 | `builtin-skills/` | Python, Go, Java, TypeScript best practices bundled with Plan Cascade |
| External | 51-100 | `external-skills/` | Framework skills from Git submodules (React, Vue, Rust) |
| User | 101-200 | `.plan-cascade/skills.json` | Custom skills from local paths or remote URLs |

**Same-name Override:** When skills share the same name, higher priority wins.

---

## 6. Mega Plan Workflow

Suitable for large project development containing multiple related feature modules.

### Use Cases

| Type | Scenario | Example |
|------|----------|---------|
| ✅ Suitable | Multi-module new project development | Build SaaS platform (user + subscription + billing + admin) |
| ✅ Suitable | Large-scale refactoring involving multiple subsystems | Monolith to microservices architecture |
| ✅ Suitable | Feature group development | E-commerce platform (users, products, cart, orders) |
| ❌ Not suitable | Single feature development | Only implement user authentication (use Hybrid Ralph) |
| ❌ Not suitable | Bug fixes | Fix login page form validation issue |

### Command Parameters

```bash
/plan-cascade:mega-plan <project-description> [design-doc-path]

# Examples:
/plan-cascade:mega-plan "Build e-commerce platform"
/plan-cascade:mega-plan "Build e-commerce platform" ./architecture.md
```

| Parameter | Description |
|-----------|-------------|
| `project-description` | Required. Project description for feature decomposition |
| `design-doc-path` | Optional. External design document to import (.md/.json/.html) |

### Detailed Flowchart

```mermaid
flowchart TD
    A["<b>/plan-cascade:mega-plan</b><br/>project-desc [design-doc]"] --> A0[Step 0: Configure .gitignore]
    A0 --> B[Step 1: Parse Arguments]
    B --> C[Step 2: Check Existing Mega Plan]
    C --> D[Step 3: Analyze Project Requirements]
    D --> E[Step 4: Generate mega-plan.json]

    E --> F{External Design Doc?}
    F -->|Yes| F1[Convert .md/.json/.html<br/>to design_doc.json]
    F -->|No| F2[Step 5: Auto-Generate<br/>Project design_doc.json]
    F1 --> G
    F2 --> G

    G[Step 6: Create Supporting Files<br/>mega-findings.md, .mega-status.json]
    G --> H[Calculate Execution Batches]
    H --> I[Step 7: Ask Execution Mode<br/>Auto / Manual]
    I --> J[Step 8: Display Unified Review<br/>unified-review.py --mode mega]

    J --> K{User Action}
    K -->|Edit| L["/plan-cascade:mega-edit"]
    L --> J
    K -->|Approve| M["/plan-cascade:mega-approve"]

    subgraph "mega-approve Execution"
        M --> N[Parse --auto-prd --agent --prd-agent --impl-agent]
        N --> O[Create Batch N Worktrees]
        O --> P[Generate PRDs for Batch<br/>via selected PRD Agent]
        P --> Q[Execute Stories for Batch<br/>via selected Impl Agent]
        Q --> R[Wait via TaskOutput]
        R --> S{Batch Complete?}
        S -->|Yes| T[Merge Batch to Target Branch]
        T --> U[Cleanup Batch Worktrees]
        U --> V{More Batches?}
        V -->|Yes| O
        V -->|No| W[All Complete]
    end

    W --> X["/plan-cascade:mega-complete"]
    X --> Y[Final Cleanup]
```

### Files Created

| File | Location | Description |
|------|----------|-------------|
| `mega-plan.json` | User data dir or project root | Project plan with features |
| `design_doc.json` | Project root | Project-level technical design |
| `mega-findings.md` | Project root | Shared findings across features |
| `.mega-status.json` | State dir or project root | Execution status |

### Recovery

If interrupted:
```bash
/plan-cascade:mega-resume --auto-prd
```

---

## 7. Hybrid Worktree Workflow

Suitable for single complex feature development requiring branch isolation.

**Important**: This command only handles worktree creation and PRD/design doc generation. Story execution is handled by `/plan-cascade:approve`.

### Use Cases

| Type | Scenario | Example |
|------|----------|---------|
| ✅ Suitable | Complete feature with multiple subtasks | User authentication (registration + login + password reset) |
| ✅ Suitable | Experimental feature requiring branch isolation | New payment channel integration test |
| ✅ Suitable | Medium-scale refactoring (5-20 files) | API layer unified error handling |
| ❌ Not suitable | Simple single-file modification | Modify a component's style |
| ❌ Not suitable | Quick prototype validation | Verify if a library is usable |

### Command Parameters

```bash
/plan-cascade:hybrid-worktree <task-name> <target-branch> <prd-or-description> [design-doc-path]

# Examples:
/plan-cascade:hybrid-worktree fix-auth main "Fix authentication bug"
/plan-cascade:hybrid-worktree fix-auth main ./existing-prd.json
/plan-cascade:hybrid-worktree fix-auth main "Fix auth" ./design-spec.md
```

| Parameter | Description |
|-----------|-------------|
| `task-name` | Required. Name for worktree and branch |
| `target-branch` | Required. Branch to merge into when complete |
| `prd-or-description` | Required. Existing PRD file path OR task description |
| `design-doc-path` | Optional. External design document to import |

### Detailed Flowchart

```mermaid
flowchart TD
    A["<b>/plan-cascade:hybrid-worktree</b><br/>task-name target-branch prd-or-desc [design-doc]"] --> A0[Step 0: Configure .gitignore]
    A0 --> B[Step 1: Parse Parameters]
    B --> C[Step 2: Detect OS and Shell<br/>Cross-platform support]
    C --> D[Step 3: Verify Git Repository]
    D --> E[Step 4: Detect Default Branch]
    E --> F[Step 5: Set Variables via PathResolver]

    F --> G[Step 6: Check for Project design_doc.json]
    G --> H{Worktree Exists?}
    H -->|Yes| I[Navigate to Existing Worktree]
    H -->|No| J[Create Git Worktree + Branch]

    J --> K[Initialize Planning Files<br/>findings.md, progress.txt]
    K --> L{Copy Project design_doc.json?}
    L -->|Yes| L1[Copy to Worktree]
    L -->|No| M
    L1 --> M
    I --> M

    M[Step 7: Determine PRD Mode]
    M --> N{PRD_ARG is file?}
    N -->|Yes| O[Load PRD from File]
    N -->|No| P[Generate PRD via Task Agent]

    O --> Q
    P --> Q[Wait via TaskOutput]
    Q --> R[Validate PRD]

    R --> S{External Design Doc?}
    S -->|Yes| S1[Convert External Doc]
    S -->|No| S2[Auto-Generate Feature design_doc.json]
    S1 --> T
    S2 --> T

    T[Create story_mappings<br/>Link stories to components/decisions]
    T --> U[Update .hybrid-execution-context.md]
    U --> V[Display Unified Review<br/>unified-review.py --mode hybrid]
    V --> W[Show Worktree Summary]

    W --> X{User Action}
    X -->|Edit| Y["/plan-cascade:edit"]
    Y --> V
    X -->|Approve| Z["/plan-cascade:approve"]

    Z --> EXEC[Execute Stories<br/>See Section 9]
    EXEC --> AA["/plan-cascade:hybrid-complete"]
    AA --> AB[Merge to Target Branch]
    AB --> AC[Delete Worktree]
```

### Design Document Inheritance

When a project-level `design_doc.json` exists:

```json
{
  "metadata": {
    "parent_design_doc": "../design_doc.json",
    "level": "feature"
  },
  "inherited_context": {
    "patterns": ["PatternName"],
    "decisions": ["ADR-001"],
    "shared_models": ["SharedModel"]
  },
  "story_mappings": {
    "story-001": {
      "components": ["ComponentA"],
      "decisions": ["ADR-F001"]
    }
  }
}
```

### Recovery

If interrupted:
```bash
/plan-cascade:hybrid-resume --auto
```

---

## 8. Hybrid Auto Workflow

Suitable for quick development of simple features without Worktree isolation.

**Important**: This command only handles PRD and design doc generation. Story execution is handled by `/plan-cascade:approve`.

### Command Parameters

```bash
/plan-cascade:hybrid-auto <task-description> [design-doc-path] [--agent <name>]

# Examples:
/plan-cascade:hybrid-auto "Add password reset functionality"
/plan-cascade:hybrid-auto "Implement auth" ./auth-design.md
/plan-cascade:hybrid-auto "Fix bug" --agent=codex
```

| Parameter | Description |
|-----------|-------------|
| `task-description` | Required. Task description for PRD generation |
| `design-doc-path` | Optional. External design document to import |
| `--agent` | Optional. Agent for PRD generation (default: claude-code) |

### Detailed Flowchart

```mermaid
flowchart TD
    A["<b>/plan-cascade:hybrid-auto</b><br/>task-desc [design-doc] [--agent]"] --> A0[Step 0: Configure .gitignore]
    A0 --> B[Step 1: Parse Arguments]
    B --> C[Step 1.1: Resolve PRD Agent<br/>claude-code / codex / aider]

    C --> D{CLI Agent Available?}
    D -->|No| D1[Fallback to claude-code]
    D -->|Yes| E
    D1 --> E

    E[Step 2: Generate PRD via Agent]
    E --> F[Step 3: Wait via TaskOutput]
    F --> G[Step 4: Validate PRD Structure]

    G --> H{External Design Doc?}
    H -->|Yes| I[Step 4.5.1: Convert External Doc<br/>.md / .json / .html]
    H -->|No| J[Step 4.5.2: Auto-Generate<br/>design_doc.json]
    I --> K
    J --> K

    K[Create story_mappings<br/>Link stories to components]
    K --> L[Wait via TaskOutput]
    L --> M[Step 5: Display Unified Review<br/>unified-review.py --mode hybrid]

    M --> N[Step 6: Confirm Generation Complete]
    N --> O{User Action}

    O -->|Edit| P["/plan-cascade:edit"]
    P --> M
    O -->|Approve| Q["/plan-cascade:approve"]
    O -->|"Approve+Auto"| R["/plan-cascade:approve --auto-run"]

    Q --> EXEC[Execute Stories<br/>See Section 9]
    R --> EXEC
```

### Generated Files

| File | Description |
|------|-------------|
| `prd.json` | Product requirements with stories |
| `design_doc.json` | Technical design with story_mappings |

### Recovery

If interrupted:
```bash
/plan-cascade:hybrid-resume --auto
```

---

## 9. Approve and Execute Workflow

The `/plan-cascade:approve` command handles story execution with multi-agent support.

### Command Parameters

```bash
/plan-cascade:approve [options]

Options:
  --agent <name>        Global agent override (all stories)
  --impl-agent <name>   Agent for implementation phase
  --retry-agent <name>  Agent for retry phase
  --verify              Enable AI verification gate
  --verify-agent <name> Agent for verification (default: claude-code)
  --no-fallback         Disable automatic fallback to claude-code
  --auto-run            Start execution immediately
```

### Agent Priority Chain

```
1. --agent parameter           (Highest - global override)
2. --impl-agent parameter      (Phase-specific override)
3. story.agent in PRD          (Story-level specification)
4. Story type inference:
   - bugfix, fix → codex
   - refactor, cleanup → aider
   - test, spec → claude-code
   - feature, add → claude-code
5. phase_defaults in agents.json
6. fallback_chain in agents.json
7. claude-code                 (Always available fallback)
```

### Detailed Flowchart

```mermaid
flowchart TD
    A["<b>/plan-cascade:approve</b><br/>[--agent] [--impl-agent] [--verify] [--auto-run]"] --> B[Step 1: Detect OS and Shell]
    B --> C[Step 2: Parse Agent Parameters]
    C --> D[Step 2.5: Load agents.json Config]
    D --> E[Step 3: Verify PRD Exists]
    E --> F[Step 4: Read and Validate PRD]

    F --> G[Step 4.5: Check for design_doc.json]
    G --> H[Display Design Document Summary<br/>Components, Patterns, ADRs, Mappings]

    H --> I[Step 4.6: Detect External Skills<br/>ExternalSkillLoader]
    I --> J[Display Loaded Skills Summary]

    J --> K[Step 5: Calculate Execution Batches<br/>Based on Dependencies]
    K --> L[Step 6: Choose Execution Mode<br/>Auto / Manual]
    L --> M[Step 7: Initialize progress.txt]

    M --> N[Step 8: Launch Batch Agents]

    subgraph "Step 8: Agent Resolution & Launch"
        N --> O[8.1: Resolve Agent per Story<br/>Priority Chain]
        O --> P[8.2: Check Agent Availability<br/>CLI: which/where]
        P --> Q{Available?}
        Q -->|No + Fallback| R[Use Next in Chain]
        Q -->|No + No Fallback| S[ERROR]
        Q -->|Yes| T[8.3: Build Story Prompt<br/>+ Design Context<br/>+ External Skills]
        R --> Q
        T --> U[8.4: Launch Agent<br/>Task tool or CLI]
        U --> V[Display Agent Launch Summary]
    end

    V --> W[Step 9: Wait for Batch Completion]

    subgraph "Step 9: Wait & Verify"
        W --> X[9.1: Wait via TaskOutput<br/>per story]
        X --> Y[9.2: Verify Completion<br/>Read progress.txt]
        Y --> Z{--verify enabled?}
        Z -->|Yes| AA[9.2.6: AI Verification Gate<br/>Detect Skeleton Code]
        Z -->|No| AB
        AA --> AB{All Pass?}
        AB -->|No| AC[9.2.5: Retry with Different Agent<br/>+ Error Context]
        AC --> U
        AB -->|Yes| AD[9.3: Progress to Next Batch]
    end

    AD --> AE{More Batches?}
    AE -->|Yes + Auto| N
    AE -->|Yes + Manual| AF[Ask User Confirmation]
    AF --> N
    AE -->|No| AG[Step 10: Show Final Status]
```

### AI Verification Gate

When `--verify` is enabled, each completed story is verified:

```
[VERIFIED] story-001 - All acceptance criteria implemented
[VERIFY_FAILED] story-002 - Skeleton code detected: function has only 'pass'
```

Detection rules:
- Functions with only `pass`, `...`, or `raise NotImplementedError`
- TODO/FIXME comments in new code
- Placeholder return values without logic
- Empty function/method bodies

### Agent Display in Progress

```
=== Batch 1 Launched ===

Stories and assigned agents:
  - story-001: claude-code (task-tool)
  - story-002: aider (cli) [refactor detected]
  - story-003: codex (cli) [bugfix detected]

⚠️ Agent fallbacks:
  - story-004: aider → claude-code (aider CLI not found)

Waiting for completion...
```

### Progress Log Format

```
[2026-01-28 10:30:00] story-001: [START] via codex (pid:12345)
[2026-01-28 10:30:05] story-001: Progress update...
[2026-01-28 10:35:00] story-001: [COMPLETE] via codex
[2026-01-28 10:35:01] story-001: [VERIFIED] All criteria met
```

---

## 10. Auto-Iteration Workflow

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

    N1 --> CTX[Load Story Context<br/>Design Doc + External Skills]
    N2 --> CTX
    N3 --> CTX
    CTX --> O[Start Agents in Parallel]

    O --> P[Wait via TaskOutput]
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

    Y -->|✓| VERIFY{AI Verify?}
    Y -->|✗ and required| X
    Y -->|✗ but optional| VERIFY

    VERIFY -->|Yes| VGATE[AI Verification Gate]
    VERIFY -->|No| T
    VGATE -->|Pass| T
    VGATE -->|Fail| X

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

## 11. Path Storage Modes

Plan Cascade supports two path storage modes for runtime files:

### New Mode (Default)

Runtime files are stored in a user-specific directory, keeping the project root clean:

| Platform | User Data Directory |
|----------|---------------------|
| **Windows** | `%APPDATA%/plan-cascade/<project-id>/` |
| **Unix/macOS** | `~/.plan-cascade/<project-id>/` |

Where `<project-id>` is a unique identifier based on project name and path hash (e.g., `my-project-a1b2c3d4`).

**File Locations in New Mode:**

| File Type | Location |
|-----------|----------|
| `prd.json` | `<user-dir>/prd.json` (or worktree if in worktree mode) |
| `mega-plan.json` | `<user-dir>/mega-plan.json` |
| `.mega-status.json` | `<user-dir>/.state/.mega-status.json` |
| `.iteration-state.json` | `<user-dir>/.state/` |
| Worktrees | `<user-dir>/.worktree/<task-name>/` |
| `design_doc.json` | **Project root** (user-visible) |
| `progress.txt` | **Working directory** (user-visible) |
| `findings.md` | **Working directory** (user-visible) |

### Legacy Mode

All files are stored in the project root (backward compatible):

| File | Location |
|------|----------|
| `prd.json` | `<project-root>/prd.json` |
| `mega-plan.json` | `<project-root>/mega-plan.json` |
| `.mega-status.json` | `<project-root>/.mega-status.json` |
| Worktrees | `<project-root>/.worktree/<task-name>/` |

### Checking Current Mode

```bash
python3 -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; r=PathResolver(Path.cwd()); print('Mode:', 'legacy' if r.is_legacy_mode() else 'new'); print('PRD path:', r.get_prd_path())"
```

---

## 12. Data Flow and State Files

```mermaid
graph TB
    subgraph "Input"
        U[User Description] --> CMD[Command Parser]
        CFG[agents.json] --> CMD
        EXT[External Design Doc<br/>.md/.json/.html] -.-> CMD
    end

    subgraph "Planning Files"
        CMD --> PRD[prd.json<br/>PRD Document]
        CMD --> MP[mega-plan.json<br/>Project Plan]
        CMD --> DD[design_doc.json<br/>Design Document]
    end

    subgraph "Execution State"
        PRD --> AS[.agent-status.json<br/>Agent Status]
        PRD --> IS[.iteration-state.json<br/>Iteration State]
        PRD --> RS[.retry-state.json<br/>Retry State]
        MP --> MS[.mega-status.json<br/>Mega Plan Status]
    end

    subgraph "Shared Context"
        AS --> FD[findings.md<br/>Findings Record]
        AS --> MF[mega-findings.md<br/>Project Findings]
        AS --> PG[progress.txt<br/>Progress Log]
    end

    subgraph "Agent Output"
        AS --> AO[.agent-outputs/<br/>├─ story-001.log<br/>├─ story-001.prompt.txt<br/>├─ story-001.verify.md<br/>└─ story-001.result.json]
    end

    subgraph "Cache"
        AD[.agent-detection.json<br/>Agent Detection Cache]
        GCF[.state/gate-cache.json<br/>Gate Result Cache]
        LK[.locks/<br/>File Locks]
    end

    subgraph "Context Recovery"
        HEC[.hybrid-execution-context.md]
        MEC[.mega-execution-context.md]
    end

    DD --> CF[ContextFilter]
    CF --> AS

    style PRD fill:#e1f5fe
    style MP fill:#e1f5fe
    style DD fill:#e1f5fe
    style AS fill:#fff3e0
    style IS fill:#fff3e0
    style RS fill:#fff3e0
    style MS fill:#fff3e0
    style FD fill:#e8f5e9
    style MF fill:#e8f5e9
    style PG fill:#e8f5e9
```

### File Descriptions

| File | Type | Description |
|------|------|-------------|
| `prd.json` | Planning | PRD document, contains goals, stories, dependencies |
| `mega-plan.json` | Planning | Project-level plan, manages multiple Features |
| `design_doc.json` | Planning | Technical design document, architecture and decisions |
| `agents.json` | Configuration | Agent configuration, includes phase defaults and fallback chains |
| `findings.md` | Shared | Agent findings record, supports tag filtering |
| `mega-findings.md` | Shared | Project-level findings (mega-plan mode) |
| `progress.txt` | Shared | Progress timeline, includes Agent execution info |
| `.agent-status.json` | State | Agent running/completed/failed status |
| `.iteration-state.json` | State | Auto-iteration progress and batch results |
| `.retry-state.json` | State | Retry history and failure records |
| `.mega-status.json` | State | Mega-plan execution status |
| `.agent-detection.json` | Cache | Cross-platform Agent detection results (1-hour TTL) |
| `.state/gate-cache.json` | Cache | Gate execution results cache (keyed by git commit + tree hash) |
| `.hybrid-execution-context.md` | Context | Hybrid task context for AI recovery after session interruption |
| `.mega-execution-context.md` | Context | Mega-plan context for AI recovery after session interruption |
| `.agent-outputs/` | Output | Agent logs, prompts, verification reports, and result files |

---

## 13. Dual-Mode Architecture

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

---

## 14. Multi-Agent Collaboration Architecture

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

### Supported Agents

| Agent | Type | Best For |
|-------|------|----------|
| `claude-code` | task-tool | General purpose (default, always available) |
| `codex` | cli | Bug fixes, quick implementations |
| `aider` | cli | Refactoring, code improvements |
| `amp-code` | cli | Alternative implementations |
| `cursor-cli` | cli | Cursor editor integration |

### Command Parameters

**For `/plan-cascade:approve` (story execution):**

| Parameter | Description | Example |
|-----------|-------------|---------|
| `--agent` | Global agent override (all stories) | `--agent=codex` |
| `--impl-agent` | Implementation phase agent | `--impl-agent=claude-code` |
| `--retry-agent` | Retry phase agent | `--retry-agent=aider` |
| `--verify` | Enable AI verification gate | `--verify` |
| `--verify-agent` | Verification agent | `--verify-agent=claude-code` |
| `--no-fallback` | Disable auto-fallback on failure | `--no-fallback` |

**For `/plan-cascade:mega-approve` (feature execution):**

| Parameter | Description | Example |
|-----------|-------------|---------|
| `--agent` | Global agent override | `--agent=claude-code` |
| `--prd-agent` | PRD generation agent | `--prd-agent=codex` |
| `--impl-agent` | Implementation phase agent | `--impl-agent=aider` |
| `--auto-prd` | Auto-generate PRDs and execute | `--auto-prd` |

**For `/plan-cascade:hybrid-auto` (PRD generation):**

| Parameter | Description | Example |
|-----------|-------------|---------|
| `--agent` | Agent for PRD generation | `--agent=codex` |

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

### Agent Configuration File (agents.json)

```json
{
  "default_agent": "claude-code",
  "agents": {
    "claude-code": {"type": "task-tool"},
    "codex": {"type": "cli", "command": "codex"},
    "aider": {"type": "cli", "command": "aider"}
  },
  "phase_defaults": {
    "implementation": {
      "default_agent": "claude-code",
      "fallback_chain": ["codex", "aider"],
      "story_type_overrides": {
        "refactor": "aider",
        "bugfix": "codex"
      }
    }
  }
}
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
