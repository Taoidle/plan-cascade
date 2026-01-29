[English](System-Architecture.md)

# Plan Cascade - 系统架构与流程设计

**版本**: 4.1.0
**最后更新**: 2026-01-29

本文档包含 Plan Cascade 的详细架构图、流程图和系统设计。

---

## 目录

1. [三层架构](#1-三层架构)
2. [核心组件](#2-核心组件)
3. [完整工作流](#3-完整工作流)
4. [Auto 自动策略流程](#4-auto-自动策略流程)
5. [Mega Plan 流程](#5-mega-plan-流程)
6. [Hybrid Worktree 流程](#6-hybrid-worktree-流程)
7. [Hybrid Auto 流程](#7-hybrid-auto-流程)
8. [自动迭代流程](#8-自动迭代流程)
9. [数据流与状态文件](#9-数据流与状态文件)
10. [双模式架构](#10-双模式架构)
11. [多 Agent 协同架构](#11-多-agent-协同架构)

---

## 1. 三层架构

```mermaid
graph TB
    subgraph "Level 1: Mega Plan 项目级"
        MP[mega-plan.json] --> F1[Feature 1]
        MP --> F2[Feature 2]
        MP --> F3[Feature 3]
    end

    subgraph "Level 2: Hybrid Ralph 功能级"
        F1 --> W1[Worktree 1]
        F2 --> W2[Worktree 2]
        F3 --> W3[Worktree 3]
        W1 --> PRD1[prd.json]
        W2 --> PRD2[prd.json]
        W3 --> PRD3[prd.json]
    end

    subgraph "Level 3: Stories 故事级"
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

### 层级详解

| 层级 | 名称 | 职责 | 产物 |
|------|------|------|------|
| **Level 1** | Mega Plan | 项目级编排，管理多个 Feature 的依赖和执行顺序 | `mega-plan.json` |
| **Level 2** | Hybrid Ralph | 功能级开发，在独立 Worktree 中执行，自动生成 PRD | `prd.json`, `findings.md` |
| **Level 3** | Stories | 故事级执行，由 Agent 并行处理，支持质量门控和重试 | 代码变更, `progress.txt` |

---

## 2. 核心组件

```mermaid
graph LR
    subgraph "编排层"
        O[Orchestrator<br/>编排器]
        IL[IterationLoop<br/>迭代循环]
    end

    subgraph "执行层"
        AE[AgentExecutor<br/>Agent执行器]
        PM[PhaseManager<br/>阶段管理器]
        CPD[CrossPlatformDetector<br/>跨平台检测]
    end

    subgraph "质量层"
        QG[QualityGate<br/>质量门控]
        RM[RetryManager<br/>重试管理器]
    end

    subgraph "状态层"
        SM[StateManager<br/>状态管理器]
        CF[ContextFilter<br/>上下文过滤]
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

### 组件说明

| 组件 | 职责 |
|------|------|
| **Orchestrator** | 核心编排器，协调所有组件 |
| **IterationLoop** | 自动迭代循环，管理批次执行 |
| **AgentExecutor** | Agent 执行抽象，支持多种 Agent |
| **PhaseManager** | 阶段管理，根据阶段选择 Agent |
| **QualityGate** | 质量门控，验证代码质量 |
| **RetryManager** | 重试管理，处理失败重试 |
| **StateManager** | 状态管理，持久化执行状态 |
| **ContextFilter** | 上下文过滤，优化 Agent 输入 |

---

## 3. 完整工作流

```mermaid
flowchart TB
    subgraph "入口选择"
        START{项目规模?}
        START -->|多功能模块| MEGA["/plan-cascade:mega-plan"]
        START -->|单功能+隔离| HW["/plan-cascade:hybrid-worktree"]
        START -->|简单功能| HA["/plan-cascade:hybrid-auto"]
    end

    subgraph "Mega Plan 流程"
        MEGA --> MP_GEN[生成 mega-plan.json]
        MP_GEN --> MP_EDIT{编辑?}
        MP_EDIT -->|是| MP_MODIFY["/plan-cascade:mega-edit"]
        MP_MODIFY --> MP_GEN
        MP_EDIT -->|否| MP_APPROVE["/plan-cascade:mega-approve"]
        MP_APPROVE --> MP_BATCH[按批次创建 Worktree]
        MP_BATCH --> MP_PRD[每个 Feature 生成 PRD]
    end

    subgraph "Hybrid Worktree 流程"
        HW --> HW_CREATE[创建 Worktree + 分支]
        HW_CREATE --> HW_PRD["/plan-cascade:hybrid-auto 生成 PRD"]
    end

    subgraph "Hybrid Auto 流程"
        HA --> HA_GEN[分析任务 + 生成 PRD]
    end

    MP_PRD --> PRD_REVIEW
    HW_PRD --> PRD_REVIEW
    HA_GEN --> PRD_REVIEW

    subgraph "PRD 审核"
        PRD_REVIEW[显示 PRD 预览]
        PRD_REVIEW --> PRD_EDIT{编辑?}
        PRD_EDIT -->|是| PRD_MODIFY["/plan-cascade:edit"]
        PRD_MODIFY --> PRD_REVIEW
        PRD_EDIT -->|否| APPROVE["/plan-cascade:approve"]
    end

    subgraph "执行阶段"
        APPROVE --> EXEC_MODE{执行模式?}
        EXEC_MODE -->|手动| MANUAL[手动推进批次]
        EXEC_MODE -->|自动| AUTO[自动迭代循环]

        AUTO --> BATCH[执行当前批次]
        MANUAL --> BATCH
        BATCH --> PARALLEL[并行启动 Agent]
        PARALLEL --> WAIT[等待完成]
        WAIT --> QG{质量门控}
        QG -->|通过| NEXT{下一批次?}
        QG -->|失败| RETRY{重试?}
        RETRY -->|是| BATCH
        RETRY -->|否| FAIL[标记失败]
        NEXT -->|是| BATCH
        NEXT -->|否| DONE[执行完成]
    end

    subgraph "完成阶段"
        DONE --> COMPLETE["/plan-cascade:complete 或<br/>/plan-cascade:mega-complete"]
        COMPLETE --> MERGE[合并到目标分支]
        MERGE --> CLEANUP[清理 Worktree]
    end
```

---

## 4. Auto 自动策略流程

`/plan-cascade:auto` 命令提供基于任务分析的 AI 驱动自动策略选择。

### 策略选择流程图

```mermaid
flowchart TD
    A["/plan-cascade:auto<br/>任务描述"] --> B[收集项目上下文]
    B --> C[AI 策略分析]

    C --> D{关键词检测}

    D -->|"platform, system,<br/>architecture, 3+ 模块"| E[MEGA_PLAN]
    D -->|"implement, create +<br/>experimental, refactor"| F[HYBRID_WORKTREE]
    D -->|"implement, create,<br/>build, feature"| G[HYBRID_AUTO]
    D -->|"fix, update, simple<br/>或默认"| H[DIRECT]

    E --> I["/plan-cascade:mega-plan"]
    F --> J["/plan-cascade:hybrid-worktree"]
    G --> K["/plan-cascade:hybrid-auto"]
    H --> L[直接执行]

    I --> M[多功能编排]
    J --> N[隔离开发]
    K --> O[PRD + Story 执行]
    L --> P[任务完成]
```

### 策略检测规则

| 优先级 | 策略 | 关键词 | 条件 |
|--------|------|--------|------|
| 1 | **MEGA_PLAN** | platform, system, architecture, microservices | 或列举 3+ 个独立模块 |
| 2 | **HYBRID_WORKTREE** | (功能关键词) + experimental, refactor, isolated | 两个条件同时满足 |
| 3 | **HYBRID_AUTO** | implement, create, build, feature, api | 无隔离关键词 |
| 4 | **DIRECT** | fix, typo, update, simple, single | 默认回退 |

### 策略映射示例

| 任务描述 | 检测到的关键词 | 选择的策略 |
|----------|----------------|------------|
| "修复 README 中的拼写错误" | fix, typo | DIRECT |
| "实现 OAuth 用户认证" | implement, authentication | HYBRID_AUTO |
| "实验性重构支付模块" | refactoring + experimental | HYBRID_WORKTREE |
| "构建电商平台：用户、商品、购物车、订单" | platform + 4 个模块 | MEGA_PLAN |

---

## 5. Mega Plan 流程

适用于包含多个相关功能模块的大型项目开发。

### 适用场景

| 类型 | 场景 | 示例 |
|------|------|------|
| ✅ 适用 | 多功能模块的新项目开发 | 构建 SaaS 平台（用户 + 订阅 + 计费 + 后台） |
| ✅ 适用 | 涉及多子系统的大规模重构 | 单体应用重构为微服务架构 |
| ✅ 适用 | 功能群开发 | 电商平台（用户、商品、购物车、订单） |
| ❌ 不适用 | 单个功能开发 | 仅实现用户认证（用 Hybrid Ralph） |
| ❌ 不适用 | Bug 修复 | 修复登录页表单验证问题 |

### 批次间顺序执行

```
mega-approve (第1次) → 启动 Batch 1
    ↓ Batch 1 完成
mega-approve (第2次) → 合并 Batch 1 → 从更新后的分支创建 Batch 2
    ↓ Batch 2 完成
mega-approve (第3次) → 合并 Batch 2 → ...
    ↓ 所有批次完成
mega-complete → 清理计划文件
```

### 详细流程图

```mermaid
flowchart TD
    A["<b>/plan-cascade:mega-plan</b><br/>电商平台：用户、商品、订单"] --> B[分析项目需求]
    B --> C[识别功能模块]
    C --> D[生成 Feature 列表]
    D --> E[分析 Feature 依赖]
    E --> F[生成 mega-plan.json]

    F --> G{用户操作}
    G -->|编辑| H["/plan-cascade:mega-edit"]
    H --> F
    G -->|批准| I["<b>/plan-cascade:mega-approve</b><br/>(第1次)"]

    I --> J[创建 Batch 1 Worktrees]
    J --> K[Batch 1: 基础设施]

    subgraph "Feature 并行开发 (Batch 1)"
        K --> L1["Feature: 用户系统<br/>Worktree: .worktrees/user"]
        K --> L2["Feature: 商品系统<br/>Worktree: .worktrees/product"]

        L1 --> M1[自动生成 PRD]
        L2 --> M2[自动生成 PRD]

        M1 --> N1[执行 Stories<br/>+ 质量门控 + 重试]
        M2 --> N2[执行 Stories<br/>+ 质量门控 + 重试]
    end

    N1 --> O1[Feature 完成]
    N2 --> O2[Feature 完成]

    O1 --> P["<b>/plan-cascade:mega-approve</b><br/>(第2次)"]
    O2 --> P
    P --> P1[合并 Batch 1 到目标分支]
    P1 --> P2[从更新后的分支创建 Batch 2]
    P2 --> Q[Batch 2: 订单系统<br/>依赖用户+商品]

    Q --> R[继续执行...]
    R --> S[所有 Feature 完成]
    S --> T["<b>/plan-cascade:mega-complete</b>"]
    T --> U[清理计划文件]
    U --> V[清理所有 Worktrees]
```

---

## 6. Hybrid Worktree 流程

适用于需要分支隔离的单个复杂功能开发。

### 适用场景

| 类型 | 场景 | 示例 |
|------|------|------|
| ✅ 适用 | 包含多子任务的完整功能 | 用户认证（注册 + 登录 + 密码重置） |
| ✅ 适用 | 需要分支隔离的实验功能 | 新支付渠道集成测试 |
| ✅ 适用 | 中等规模重构（5-20 文件） | API 层统一错误处理改造 |
| ❌ 不适用 | 简单单文件修改 | 修改一个组件的样式 |
| ❌ 不适用 | 快速原型验证 | 验证某个库是否可用 |

### 详细流程图

```mermaid
flowchart TD
    A["<b>/plan-cascade:hybrid-worktree</b><br/>feature-auth main 用户认证"] --> B[创建 Git 分支]
    B --> C[创建 Worktree 目录]
    C --> D[初始化规划文件]
    D --> E["<b>/plan-cascade:hybrid-auto</b><br/>生成 PRD"]

    E --> F[分析任务描述]
    F --> G[扫描代码库结构]
    G --> H[生成 prd.json]
    H --> I[显示 PRD 预览]

    I --> J{用户操作}
    J -->|编辑| K["/plan-cascade:edit"]
    K --> I
    J -->|批准| L["<b>/plan-cascade:approve</b>"]

    L --> M{执行模式}
    M -->|"--auto-run"| N[自动迭代模式]
    M -->|手动| O[手动模式]

    subgraph "自动迭代"
        N --> P[执行 Batch 1]
        P --> Q[并行 Agent 执行]
        Q --> R[质量门控检查]
        R --> S{通过?}
        S -->|是| T{还有批次?}
        S -->|否| U[智能重试]
        U --> Q
        T -->|是| P
        T -->|否| V[全部完成]
    end

    subgraph "手动模式"
        O --> W[执行 Batch 1]
        W --> X["/plan-cascade:status 查看进度"]
        X --> Y[手动推进下一批次]
        Y --> W
    end

    V --> Z["<b>/plan-cascade:hybrid-complete</b>"]
    Z --> AA[合并到 main 分支]
    AA --> AB[删除 Worktree]
```

---

## 7. Hybrid Auto 流程

适用于简单功能的快速开发，无需 Worktree 隔离。

### 详细流程图

```mermaid
flowchart TD
    A["<b>/plan-cascade:hybrid-auto</b><br/>添加密码重置功能"] --> B[解析任务描述]
    B --> C[分析代码库上下文]
    C --> D{生成 PRD}

    D --> E[Goal: 主要目标]
    D --> F[Objectives: 子目标列表]
    D --> G[Stories: 用户故事]

    G --> H[Story 1: 设计 API]
    G --> I[Story 2: 实现后端]
    G --> J[Story 3: 添加邮件]
    G --> K[Story 4: 前端页面]

    H --> L[依赖分析]
    I --> L
    J --> L
    K --> L

    L --> M[生成执行批次]
    M --> N["Batch 1: Story 1<br/>Batch 2: Story 2, 3<br/>Batch 3: Story 4"]

    N --> O[显示 PRD 预览]
    O --> P{用户操作}

    P -->|编辑| Q["/plan-cascade:edit"]
    Q --> O
    P -->|批准| R["<b>/plan-cascade:approve</b>"]
    P -->|"批准+自动"| S["<b>/plan-cascade:approve --auto-run</b>"]

    R --> T[手动执行模式]
    S --> U[自动迭代模式]

    subgraph "执行详情"
        T --> V[启动 Batch 1]
        U --> V
        V --> W["Agent 并行执行<br/>(支持多种 Agent)"]
        W --> X[质量门控]
        X --> Y{检查结果}
        Y -->|typecheck ❌| Z[重试 + 失败上下文]
        Y -->|test ❌| Z
        Y -->|通过 ✓| AA[推进下一批次]
        Z --> W
        AA --> V
    end

    AA --> AB[所有 Stories 完成]
    AB --> AC[显示执行摘要]
```

---

## 8. 自动迭代流程

`/plan-cascade:approve --auto-run` 或 `/plan-cascade:auto-run` 命令启动的自动迭代循环：

```mermaid
flowchart TD
    A[开始自动迭代] --> B[加载配置]
    B --> C{迭代模式}

    C -->|until_complete| D[循环直到全部完成]
    C -->|max_iterations| E[最多执行 N 次]
    C -->|batch_complete| F[仅执行当前批次]

    D --> G[初始化迭代状态]
    E --> G
    F --> G

    G --> H[获取当前批次 Stories]
    H --> I{有待执行?}

    I -->|否| J[检查完成条件]
    I -->|是| K[解析 Agent 分配]

    K --> L[阶段: Implementation]
    L --> M{Agent 选择}
    M --> N1[Story类型: feature → claude-code]
    M --> N2[Story类型: bugfix → codex]
    M --> N3[Story类型: refactor → aider]

    N1 --> O[并行启动 Agents]
    N2 --> O
    N3 --> O

    O --> P[轮询等待<br/>poll_interval: 10s]
    P --> Q{Story 完成?}

    Q -->|运行中| P
    Q -->|完成| R{质量门控启用?}
    Q -->|超时| S[记录超时失败]

    R -->|否| T[标记完成]
    R -->|是| U[执行质量检查]

    U --> V{TypeCheck}
    V -->|✓| W{Tests}
    V -->|✗| X[记录失败详情]

    W -->|✓| Y{Lint}
    W -->|✗| X

    Y -->|✓| T
    Y -->|✗ 且必需| X
    Y -->|✗ 但可选| T

    X --> Z{可重试?}
    S --> Z

    Z -->|是| AA[构建重试 Prompt]
    Z -->|否| AB[标记最终失败]

    AA --> AC[注入失败上下文]
    AC --> AD[选择重试 Agent]
    AD --> O

    T --> AE[更新迭代状态]
    AB --> AE

    AE --> AF{批次完成?}
    AF -->|否| H
    AF -->|是| AG[推进到下一批次]

    AG --> AH{还有批次?}
    AH -->|是| H
    AH -->|否| J

    J --> AI{全部成功?}
    AI -->|是| AJ[状态: COMPLETED]
    AI -->|否| AK[状态: FAILED]

    AJ --> AL[保存最终状态]
    AK --> AL
    AL --> AM[生成执行报告]
```

### 迭代模式

| 模式 | 说明 |
|------|------|
| `until_complete` | 持续执行直到所有 Story 完成（默认） |
| `max_iterations` | 执行最多 N 次迭代后停止 |
| `batch_complete` | 仅执行当前批次后停止 |

---

## 9. 数据流与状态文件

```mermaid
graph TB
    subgraph "输入"
        U[用户描述] --> CMD[命令解析]
        CFG[agents.json] --> CMD
    end

    subgraph "规划文件"
        CMD --> PRD[prd.json<br/>PRD文档]
        CMD --> MP[mega-plan.json<br/>项目计划]
    end

    subgraph "执行状态"
        PRD --> AS[.agent-status.json<br/>Agent状态]
        PRD --> IS[.iteration-state.json<br/>迭代状态]
        PRD --> RS[.retry-state.json<br/>重试状态]
    end

    subgraph "共享上下文"
        AS --> FD[findings.md<br/>发现记录]
        AS --> PG[progress.txt<br/>进度日志]
    end

    subgraph "Agent 输出"
        AS --> AO[.agent-outputs/<br/>├─ story-001.log<br/>├─ story-001.prompt.txt<br/>└─ story-001.result.json]
    end

    subgraph "缓存"
        AD[.agent-detection.json<br/>Agent检测缓存]
        LK[.locks/<br/>文件锁]
    end

    style PRD fill:#e1f5fe
    style MP fill:#e1f5fe
    style AS fill:#fff3e0
    style IS fill:#fff3e0
    style RS fill:#fff3e0
    style FD fill:#e8f5e9
    style PG fill:#e8f5e9
```

### 文件说明

| 文件 | 类型 | 说明 |
|------|------|------|
| `prd.json` | 规划 | PRD 文档，包含目标、故事、依赖关系 |
| `mega-plan.json` | 规划 | 项目级计划，管理多个 Feature |
| `agents.json` | 配置 | Agent 配置，包含阶段默认和降级链 |
| `findings.md` | 共享 | Agent 发现记录，支持标签过滤 |
| `progress.txt` | 共享 | 进度时间线，包含 Agent 执行信息 |
| `.agent-status.json` | 状态 | Agent 运行/完成/失败状态 |
| `.iteration-state.json` | 状态 | 自动迭代进度和批次结果 |
| `.retry-state.json` | 状态 | 重试历史和失败记录 |
| `.agent-detection.json` | 缓存 | 跨平台 Agent 检测结果（1小时TTL） |
| `.agent-outputs/` | 输出 | Agent 日志、Prompt 和结果文件 |

---

## 10. 双模式架构

### 模式切换设计

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Plan Cascade                                     │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─────────────────────────┐     ┌─────────────────────────┐           │
│   │      简单模式            │     │      专家模式            │           │
│   │                         │     │                         │           │
│   │  用户输入描述            │     │  用户输入描述            │           │
│   │       ↓                 │     │       ↓                 │           │
│   │  AI 自动判断策略         │     │  生成 PRD (可编辑)       │           │
│   │       ↓                 │     │       ↓                 │           │
│   │  自动生成 PRD           │     │  用户 Review/修改        │           │
│   │       ↓                 │     │       ↓                 │           │
│   │  自动执行               │     │  选择策略/Agent          │           │
│   │       ↓                 │     │       ↓                 │           │
│   │  完成                   │     │  执行                   │           │
│   └─────────────────────────┘     └─────────────────────────┘           │
│                                                                          │
│                              共享核心                                    │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │  Orchestrator │ PRDGenerator │ QualityGate │ AgentExecutor      │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 双工作模式架构

**核心理念：Plan Cascade = 大脑（编排），执行层 = 手（工具执行）**

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Plan Cascade                                   │
│                    (编排层 - 两种模式共享)                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │                    编排引擎 (共享)                                │   │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │   │
│   │  │ PRD 生成器  │  │ 依赖分析器  │  │  批次调度器 │              │   │
│   │  └─────────────┘  └─────────────┘  └─────────────┘              │   │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │   │
│   │  │ 状态管理器  │  │ 质量门控    │  │  重试管理   │              │   │
│   │  └─────────────┘  └─────────────┘  └─────────────┘              │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                              │                                           │
│                    ┌─────────┴─────────┐                                │
│                    │  执行层选择        │                                │
│                    └─────────┬─────────┘                                │
│              ┌───────────────┴───────────────┐                          │
│              ▼                               ▼                          │
│   ┌─────────────────────────┐    ┌─────────────────────────┐           │
│   │    独立编排模式          │    │  Claude Code GUI 模式   │           │
│   ├─────────────────────────┤    ├─────────────────────────┤           │
│   │                         │    │                         │           │
│   │   内置工具执行引擎       │    │   Claude Code CLI       │           │
│   │   ┌───────────────┐     │    │   ┌───────────────┐     │           │
│   │   │ Read/Write    │     │    │   │ Claude Code   │     │           │
│   │   │ Edit/Bash     │     │    │   │ 执行工具      │     │           │
│   │   │ Glob/Grep     │     │    │   │ (stream-json) │     │           │
│   │   └───────────────┘     │    │   └───────────────┘     │           │
│   │          │              │    │          │              │           │
│   │          ▼              │    │          ▼              │           │
│   │   ┌───────────────┐     │    │   ┌───────────────┐     │           │
│   │   │ LLM 抽象层    │     │    │   │ Plan Cascade  │     │           │
│   │   │ (多种选择)    │     │    │   │ 可视化界面    │     │           │
│   │   └───────────────┘     │    │   └───────────────┘     │           │
│   │          │              │    │                         │           │
│   │   ┌──────┴──────┐       │    │                         │           │
│   │   ▼      ▼      ▼       │    │                         │           │
│   │ Claude Claude OpenAI    │    │                         │           │
│   │ Max    API    etc.      │    │                         │           │
│   │                         │    │                         │           │
│   └─────────────────────────┘    └─────────────────────────┘           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘

两种模式都支持：PRD 驱动开发、批次执行、质量门控、状态追踪
```

### 独立编排模式架构详解

```
┌─────────────────────────────────────────────────────────────────────────┐
│                       独立编排模式                                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─ 编排层 ─────────────────────────────────────────────────────────┐   │
│  │                                                                    │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                │   │
│  │  │ 意图分类器  │  │ 策略分析器  │  │  PRD 生成器 │                │   │
│  │  │ Intent     │  │ Strategy   │  │ PRDGenerator│                │   │
│  │  │ Classifier │  │ Analyzer   │  │             │                │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                │   │
│  │         │               │               │                          │   │
│  │         └───────────────┴───────────────┘                          │   │
│  │                         │                                          │   │
│  │                         ▼                                          │   │
│  │  ┌─────────────────────────────────────────────────────────────┐  │   │
│  │  │                   Orchestrator                               │  │   │
│  │  │  • 批次依赖分析                                              │  │   │
│  │  │  • 并行执行协调                                              │  │   │
│  │  │  • 质量门控检查                                              │  │   │
│  │  │  • 重试管理                                                  │  │   │
│  │  └─────────────────────────────────────────────────────────────┘  │   │
│  │                         │                                          │   │
│  └─────────────────────────┼──────────────────────────────────────────┘   │
│                            ▼                                              │
│  ┌─ 执行层 ─────────────────────────────────────────────────────────┐   │
│  │                                                                    │   │
│  │  ┌─────────────────────────────────────────────────────────────┐  │   │
│  │  │                   ReAct 执行引擎                             │  │   │
│  │  │                                                              │  │   │
│  │  │   ┌─────────┐     ┌─────────┐     ┌─────────┐               │  │   │
│  │  │   │  Think  │ ──→ │   Act   │ ──→ │ Observe │ ──→ (循环)    │  │   │
│  │  │   │  (LLM)  │     │ (工具)  │     │ (结果)  │               │  │   │
│  │  │   └─────────┘     └─────────┘     └─────────┘               │  │   │
│  │  │                                                              │  │   │
│  │  └─────────────────────────────────────────────────────────────┘  │   │
│  │                         │                                          │   │
│  │                         ▼                                          │   │
│  │  ┌─────────────────────────────────────────────────────────────┐  │   │
│  │  │                   工具执行引擎                               │  │   │
│  │  │                                                              │  │   │
│  │  │   ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐   │  │   │
│  │  │   │  Read  │ │ Write  │ │  Edit  │ │  Bash  │ │  Glob  │   │  │   │
│  │  │   └────────┘ └────────┘ └────────┘ └────────┘ └────────┘   │  │   │
│  │  │   ┌────────┐ ┌────────┐                                     │  │   │
│  │  │   │  Grep  │ │   LS   │                                     │  │   │
│  │  │   └────────┘ └────────┘                                     │  │   │
│  │  │                                                              │  │   │
│  │  └─────────────────────────────────────────────────────────────┘  │   │
│  │                                                                    │   │
│  └────────────────────────────────────────────────────────────────────┘   │
│                            │                                              │
│                            ▼                                              │
│  ┌─ LLM 层 ─────────────────────────────────────────────────────────┐   │
│  │                                                                    │   │
│  │  ┌─────────────────────────────────────────────────────────────┐  │   │
│  │  │                   LLM 抽象层                                 │  │   │
│  │  │              (只提供思考，不执行工具)                        │  │   │
│  │  └─────────────────────────────────────────────────────────────┘  │   │
│  │                         │                                          │   │
│  │       ┌─────────────────┼─────────────────┐                       │   │
│  │       ▼                 ▼                 ▼                       │   │
│  │  ┌─────────┐       ┌─────────┐       ┌─────────┐                 │   │
│  │  │ Claude  │       │ Claude  │       │ OpenAI  │                 │   │
│  │  │   Max   │       │   API   │       │ DeepSeek│                 │   │
│  │  │(via CC) │       │         │       │ Ollama  │                 │   │
│  │  └─────────┘       └─────────┘       └─────────┘                 │   │
│  │                                                                    │   │
│  └────────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 11. 多 Agent 协同架构

```
┌─────────────────────────────────────────────────────────────────────────┐
│                       多 Agent 协同架构                                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   Plan Cascade 编排层                                                    │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │  Orchestrator → AgentExecutor → PhaseAgentManager               │   │
│   │       │              │               │                           │   │
│   │       │              │               └─ 阶段/类型 → Agent 映射   │   │
│   │       │              └─ 解析最佳 Agent                           │   │
│   │       └─ 调度 Story 执行                                        │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                              │                                           │
│              ┌───────────────┴───────────────┐                          │
│              ▼                               ▼                          │
│   ┌─────────────────────────┐    ┌─────────────────────────┐           │
│   │    独立编排模式          │    │  Claude Code GUI 模式   │           │
│   │                         │    │                         │           │
│   │   默认 Agent:            │    │   默认 Agent:            │           │
│   │   内置 ReAct 引擎        │    │   Claude Code CLI       │           │
│   │                         │    │                         │           │
│   │   可选 CLI Agents:       │    │   可选 CLI Agents:       │           │
│   │   codex, aider, amp...  │    │   codex, aider, amp...  │           │
│   │                         │    │                         │           │
│   └─────────────────────────┘    └─────────────────────────┘           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 阶段化 Agent 分配

| 阶段 | 默认 Agent | 降级链 | Story 类型覆盖 |
|------|-----------|--------|----------------|
| `planning` | codex | claude-code | - |
| `implementation` | claude-code | codex, aider | bugfix→codex, refactor→aider |
| `retry` | claude-code | aider | - |
| `refactor` | aider | claude-code | - |
| `review` | claude-code | codex | - |

### Agent 优先级解析

```
1. --agent 命令参数              # 最高优先级（全局覆盖）
2. 阶段覆盖 --impl-agent 等      # 阶段特定覆盖
3. Story 中指定的 agent          # story.agent 字段
4. Story 类型覆盖               # bugfix → codex, refactor → aider
5. 阶段默认 Agent               # phase_defaults 配置
6. 降级链                       # fallback_chain
7. claude-code                  # 终极回退（始终可用）
```

---

## 附录：两种工作模式对比

| 特性 | 独立编排模式 | Claude Code GUI 模式 |
|------|--------------|----------------------|
| 编排层 | Plan Cascade | Plan Cascade |
| 工具执行 | Plan Cascade 自己执行 | Claude Code CLI 执行 |
| LLM 来源 | Claude Max/API, OpenAI, DeepSeek, Ollama | Claude Code |
| PRD 驱动 | ✅ 完整支持 | ✅ 完整支持 |
| 批次执行 | ✅ 完整支持 | ✅ 完整支持 |
| 离线可用 | ✅ (使用 Ollama) | ❌ |
| 适用场景 | 需要其他 LLM 或离线使用 | 有 Claude Max/Code 订阅 |

| 组件 | 独立编排模式 | Claude Code GUI 模式 |
|------|--------------|----------------------|
| PRD 生成 | Plan Cascade (LLM) | Plan Cascade (Claude Code) |
| 依赖分析 | Plan Cascade | Plan Cascade |
| 批次调度 | Plan Cascade | Plan Cascade |
| Story 执行 | Plan Cascade (ReAct) | Claude Code CLI |
| 工具调用 | 内置工具引擎 | Claude Code |
| 状态追踪 | Plan Cascade | Plan Cascade |
| 质量门控 | Plan Cascade | Plan Cascade |
