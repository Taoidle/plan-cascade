[English](System-Architecture.md)

# Plan Cascade - 系统架构与流程设计

**版本**: 4.3.2
**最后更新**: 2026-02-02

本文档包含 Plan Cascade 的详细架构图、流程图和系统设计。

---

## 目录

1. [三层架构](#1-三层架构)
2. [核心组件](#2-核心组件)
3. [完整工作流](#3-完整工作流)
4. [Auto 自动策略流程](#4-auto-自动策略流程)
5. [设计文档系统](#5-设计文档系统)
6. [Mega Plan 流程](#6-mega-plan-流程)
7. [Hybrid Worktree 流程](#7-hybrid-worktree-流程)
8. [Hybrid Auto 流程](#8-hybrid-auto-流程)
9. [Approve 执行流程](#9-approve-执行流程)
10. [自动迭代流程](#10-自动迭代流程)
11. [路径存储模式](#11-路径存储模式)
12. [数据流与状态文件](#12-数据流与状态文件)
13. [双模式架构](#13-双模式架构)
14. [多 Agent 协同架构](#14-多-agent-协同架构)

---

## 1. 三层架构

```mermaid
graph TB
    subgraph "Level 1: Mega Plan 项目级"
        MP[mega-plan.json] --> DD1[design_doc.json<br/>项目级]
        MP --> F1[Feature 1]
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
        PRD1 --> DD2[design_doc.json<br/>功能级]
        PRD2 --> DD3[design_doc.json<br/>功能级]
        PRD3 --> DD4[design_doc.json<br/>功能级]
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

    DD1 -.->|继承| DD2
    DD1 -.->|继承| DD3
    DD1 -.->|继承| DD4
```

### 层级详解

| 层级 | 名称 | 职责 | 产物 |
|------|------|------|------|
| **Level 1** | Mega Plan | 项目级编排，管理多个 Feature 的依赖和执行顺序 | `mega-plan.json`, `design_doc.json` (项目级) |
| **Level 2** | Hybrid Ralph | 功能级开发，在独立 Worktree 中执行，自动生成 PRD 和设计文档 | `prd.json`, `design_doc.json` (功能级), `findings.md` |
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
        VG[VerificationGate<br/>实现验证]
        RM[RetryManager<br/>重试管理器]
        GC[GateCache<br/>门控缓存]
        EP[ErrorParser<br/>错误解析器]
        CFD[ChangedFilesDetector<br/>变更文件检测]
    end

    subgraph "状态层"
        SM[StateManager<br/>状态管理器]
        CF[ContextFilter<br/>上下文过滤]
        ESL[ExternalSkillLoader<br/>外部技能]
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

### 组件说明

| 组件 | 职责 |
|------|------|
| **Orchestrator** | 核心编排器，协调所有组件 |
| **IterationLoop** | 自动迭代循环，管理批次执行 |
| **AgentExecutor** | Agent 执行抽象，支持多种 Agent |
| **PhaseManager** | 阶段管理，根据阶段选择 Agent |
| **QualityGate** | 质量门控，支持三阶段执行：PRE_VALIDATION（FORMAT）、VALIDATION（TYPECHECK、TEST、LINT 并行）、POST_VALIDATION（CODE_REVIEW、IMPLEMENTATION_VERIFY）。支持快速失败、增量检查和缓存 |
| **FormatGate** | 代码格式化门控（PRE_VALIDATION），根据项目类型使用 ruff/prettier/cargo fmt/gofmt 自动格式化代码。支持仅检查模式 |
| **CodeReviewGate** | AI 驱动的代码审查（POST_VALIDATION），评估 5 个维度：代码质量（25分）、命名清晰度（20分）、复杂度（20分）、模式遵循（20分）、安全性（15分）。遇到严重问题时阻止通过 |
| **VerificationGate** | AI 驱动的实现验证，检测骨架代码并验证验收标准 |
| **RetryManager** | 重试管理，处理失败重试，传递结构化错误上下文 |
| **GateCache** | 门控结果缓存，基于 git commit + 工作树哈希，避免重复检查 |
| **ErrorParser** | 结构化错误解析，支持 mypy、ruff、pytest、eslint、tsc，提取 ErrorInfo |
| **ChangedFilesDetector** | 基于 Git 的变更检测，用于增量门控执行 |
| **StateManager** | 状态管理，持久化执行状态 |
| **ContextFilter** | 上下文过滤，优化 Agent 输入 |
| **ExternalSkillLoader** | 三层技能加载（内置/外部/用户），自动检测并按优先级覆盖注入最佳实践。支持阶段化注入（planning、implementation、retry） |

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
        MEGA --> MP_GEN[生成 mega-plan.json<br/>+ design_doc.json]
        MP_GEN --> MP_REVIEW[统一审查显示]
        MP_REVIEW --> MP_EDIT{编辑?}
        MP_EDIT -->|是| MP_MODIFY["/plan-cascade:mega-edit"]
        MP_MODIFY --> MP_REVIEW
        MP_EDIT -->|否| MP_APPROVE["/plan-cascade:mega-approve"]
    end

    subgraph "Hybrid Worktree 流程"
        HW --> HW_CREATE[创建 Worktree + 分支]
        HW_CREATE --> HW_PRD[生成 PRD<br/>+ design_doc.json]
        HW_PRD --> HW_REVIEW[统一审查显示]
    end

    subgraph "Hybrid Auto 流程"
        HA --> HA_GEN[分析任务 + 生成 PRD<br/>+ design_doc.json]
        HA_GEN --> HA_REVIEW[统一审查显示]
    end

    MP_APPROVE --> BATCH_EXEC
    HW_REVIEW --> PRD_EDIT
    HA_REVIEW --> PRD_EDIT

    subgraph "PRD 审核"
        PRD_EDIT{编辑?}
        PRD_EDIT -->|是| PRD_MODIFY["/plan-cascade:edit"]
        PRD_MODIFY --> PRD_REVIEW2[统一审查显示]
        PRD_REVIEW2 --> PRD_EDIT
        PRD_EDIT -->|否| APPROVE["/plan-cascade:approve"]
    end

    subgraph "执行阶段"
        APPROVE --> AGENT_CFG[解析 Agent 配置<br/>--agent, --impl-agent, --verify]
        AGENT_CFG --> EXEC_MODE{执行模式?}
        EXEC_MODE -->|手动| MANUAL[手动推进批次]
        EXEC_MODE -->|"--auto-run"| AUTO[自动迭代循环]

        AUTO --> BATCH_EXEC
        MANUAL --> BATCH_EXEC

        BATCH_EXEC[执行当前批次] --> CTX[加载上下文<br/>设计文档 + 外部技能]
        CTX --> RESOLVE[解析每个 Story 的 Agent<br/>优先级链]
        RESOLVE --> PARALLEL[并行启动 Agent<br/>显示 Agent 分配]
        PARALLEL --> WAIT[通过 TaskOutput 等待]
        WAIT --> VERIFY{AI 验证?<br/>--verify}
        VERIFY -->|是| VGATE[AI 验证门<br/>检测骨架代码]
        VERIFY -->|否| QG
        VGATE --> QG{质量门控}
        QG -->|通过| NEXT{下一批次?}
        QG -->|失败| RETRY{重试?}
        RETRY -->|是| RETRY_AGENT[选择重试 Agent<br/>+ 错误上下文]
        RETRY_AGENT --> PARALLEL
        RETRY -->|否| FAIL[标记失败]
        NEXT -->|是| BATCH_EXEC
        NEXT -->|否| DONE[执行完成]
    end

    subgraph "完成阶段"
        DONE --> COMPLETE["/plan-cascade:complete 或<br/>/plan-cascade:mega-complete"]
        COMPLETE --> MERGE[合并到目标分支]
        MERGE --> CLEANUP[清理 Worktree]
    end
```

### 与旧版本的主要变化

| 方面 | 旧版本 | 当前版本 |
|------|--------|----------|
| **设计文档** | 未显示 | 每个层级自动生成 |
| **审查显示** | "显示 PRD 预览" | "统一审查显示"（PRD + 设计文档） |
| **Agent 配置** | 未显示 | 显式解析 `--agent`, `--impl-agent`, `--verify` |
| **Agent 分配** | 隐式 | "解析每个 Story 的 Agent" + 优先级链 |
| **验证** | 未显示 | 可选的 "AI 验证门" |
| **重试** | 简单重试 | "选择重试 Agent + 错误上下文" |
| **等待机制** | 隐式 | "通过 TaskOutput 等待" |

---

## 4. Auto 自动策略流程

`/plan-cascade:auto` 命令提供基于结构化任务分析的 AI 驱动自动策略选择。

### 策略选择流程图

```mermaid
flowchart TD
    A["/plan-cascade:auto<br/>任务描述"] --> B[收集项目上下文]
    B --> C[AI 自评估分析]

    C --> D[结构化任务分析]

    D --> E{分析维度}
    E --> E1[范围: 涉及多少功能区域?]
    E --> E2[复杂度: 有子任务依赖?]
    E --> E3[风险: 可能破坏现有代码?]
    E --> E4[并行化收益?]

    E1 --> F[输出结构化 JSON]
    E2 --> F
    E3 --> F
    E4 --> F

    F --> G{策略决策}

    G -->|"4+ 区域, 多个功能"| H[MEGA_PLAN]
    G -->|"2-3 区域 + 高风险"| I[HYBRID_WORKTREE]
    G -->|"2-3 区域, 3-7 步"| J[HYBRID_AUTO]
    G -->|"1 区域, 1-2 步, 低风险"| K[DIRECT]

    H --> L["/plan-cascade:mega-plan"]
    I --> M["/plan-cascade:hybrid-worktree"]
    J --> N["/plan-cascade:hybrid-auto"]
    K --> O[直接执行]

    L --> P[多功能编排]
    M --> Q[隔离开发]
    N --> R[PRD + Story 执行]
    O --> S[任务完成]
```

### AI 自评估输出

AI 输出结构化 JSON 格式的分析结果：

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
    "reasoning": "任务涉及 3 个功能区域，有依赖关系..."
  }
}
```

### 策略选择指南

| 分析结果 | 策略 | 示例 |
|----------|------|------|
| 1 个功能区域, 1-2 步, 低风险 | **DIRECT** | "修复 README 中的拼写错误" |
| 2-3 个功能区域, 3-7 步, 有依赖 | **HYBRID_AUTO** | "实现 OAuth 用户认证" |
| HYBRID_AUTO + 高风险或实验性 | **HYBRID_WORKTREE** | "实验性重构支付模块" |
| 4+ 功能区域, 多个独立功能 | **MEGA_PLAN** | "构建电商平台：用户、商品、购物车、订单" |

---

## 5. 设计文档系统

Plan Cascade 自动生成技术设计文档 (`design_doc.json`)，与 PRD 并行，在故事执行时提供架构上下文。

### 两级架构

```mermaid
graph TB
    subgraph "Level 1: 项目级设计"
        PDD[项目 design_doc.json]
        PDD --> ARCH[架构概览]
        PDD --> PATTERNS[跨功能模式]
        PDD --> PADRS[项目 ADRs<br/>ADR-001, ADR-002...]
        PDD --> FMAP[功能映射]
    end

    subgraph "Level 2: 功能级设计"
        FDD[功能 design_doc.json]
        FDD --> COMP[功能组件]
        FDD --> API[功能 APIs]
        FDD --> FADRS[功能 ADRs<br/>ADR-F001, ADR-F002...]
        FDD --> SMAP[Story 映射]
    end

    PDD -.->|继承| FDD
    PATTERNS -.->|被引用| COMP
    PADRS -.->|被扩展| FADRS
```

### 设计文档 Schema

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
    "title": "项目/功能标题",
    "summary": "摘要描述",
    "goals": ["目标1", "目标2"],
    "non_goals": ["非目标1"]
  },
  "architecture": {
    "components": [{
      "name": "组件名称",
      "description": "描述",
      "responsibilities": ["职责1"],
      "dependencies": ["依赖组件"],
      "files": ["src/file.py"]
    }],
    "data_flow": "数据流描述",
    "patterns": [{
      "name": "模式名称",
      "description": "描述",
      "rationale": "采用理由"
    }]
  },
  "interfaces": {
    "apis": [...],
    "data_models": [...]
  },
  "decisions": [{
    "id": "ADR-001",
    "title": "决策标题",
    "context": "背景上下文",
    "decision": "决策内容",
    "rationale": "决策理由",
    "alternatives_considered": ["备选方案1"],
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

### 自动生成流程

```mermaid
flowchart TD
    subgraph "Mega Plan 流程"
        MP[mega-plan.json] --> PDD[生成项目 design_doc.json]
        PDD --> F1[Feature 1 Worktree]
        PDD --> F2[Feature 2 Worktree]
        F1 --> PRD1[prd.json]
        F2 --> PRD2[prd.json]
        PRD1 --> FDD1[功能 design_doc.json<br/>继承自项目]
        PRD2 --> FDD2[功能 design_doc.json<br/>继承自项目]
    end

    subgraph "Hybrid Auto/Worktree 流程"
        PRD[prd.json] --> FDD[生成功能 design_doc.json]
    end
```

### 外部设计文档导入

三个主要命令都支持导入外部设计文档：

```bash
# mega-plan: 第2个参数
/plan-cascade:mega-plan "构建电商平台" ./architecture.md

# hybrid-auto: 第2个参数
/plan-cascade:hybrid-auto "实现用户认证" ./auth-design.md

# hybrid-worktree: 第4个参数
/plan-cascade:hybrid-worktree fix-auth main "修复认证" ./design.md
```

支持格式: Markdown (.md), JSON (.json), HTML (.html)

### 上下文注入流程

```mermaid
flowchart LR
    DD[design_doc.json] --> CF[ContextFilter]
    CF --> |story_mappings| SC[Story 上下文]
    SC --> AE[AgentExecutor]
    AE --> |设计感知提示词| Agent

    subgraph "Story 上下文"
        SC --> COMP[相关组件]
        SC --> DEC[相关决策]
        SC --> PAT[架构模式]
    end
```

### 三层外部技能系统

Plan Cascade 使用三层技能优先级系统提供框架特定的最佳实践：

```mermaid
flowchart TD
    subgraph "第一层：内置技能 (优先级 1-50)"
        BS[builtin-skills/]
        BS --> PY[python/]
        BS --> GO[go/]
        BS --> JAVA[java/]
        BS --> TS[typescript/]
    end

    subgraph "第二层：外部技能 (优先级 51-100)"
        ES[external-skills/]
        ES --> VS[vercel/ - React/Next.js]
        ES --> VUE[vue/ - Vue/Nuxt]
        ES --> RS[rust/ - Rust]
    end

    subgraph "第三层：用户技能 (优先级 101-200)"
        UC[.plan-cascade/skills.json]
        UC --> LOCAL[本地路径技能]
        UC --> REMOTE[远程 URL 技能]
    end

    subgraph "技能加载"
        PJ[package.json] --> ESL[ExternalSkillLoader]
        CT[Cargo.toml] --> ESL
        PP[pyproject.toml] --> ESL
        ESL --> |检测与去重| MERGE{优先级合并}
        MERGE --> |高优先级覆盖| CF2[ContextFilter]
        CF2 --> SC2[Story 上下文]
    end
```

**优先级层次：**

| 层次 | 优先级范围 | 来源 | 描述 |
|------|------------|------|------|
| 内置 | 1-50 | `builtin-skills/` | Python、Go、Java、TypeScript 最佳实践，随 Plan Cascade 分发 |
| 外部 | 51-100 | `external-skills/` | 来自 Git 子模块的框架技能（React、Vue、Rust） |
| 用户 | 101-200 | `.plan-cascade/skills.json` | 来自本地路径或远程 URL 的自定义技能 |

**同名覆盖：** 当技能同名时，高优先级覆盖低优先级。

---

## 6. Mega Plan 流程

适用于包含多个相关功能模块的大型项目开发。

### 适用场景

| 类型 | 场景 | 示例 |
|------|------|------|
| ✅ 适用 | 多功能模块的新项目开发 | 构建 SaaS 平台（用户 + 订阅 + 计费 + 后台） |
| ✅ 适用 | 涉及多子系统的大规模重构 | 单体应用重构为微服务架构 |
| ✅ 适用 | 功能群开发 | 电商平台（用户、商品、购物车、订单） |
| ❌ 不适用 | 单个功能开发 | 仅实现用户认证（用 Hybrid Ralph） |
| ❌ 不适用 | Bug 修复 | 修复登录页表单验证问题 |

### 命令参数

```bash
/plan-cascade:mega-plan <项目描述> [设计文档路径]

# 示例：
/plan-cascade:mega-plan "构建电商平台"
/plan-cascade:mega-plan "构建电商平台" ./architecture.md
```

| 参数 | 说明 |
|------|------|
| `项目描述` | 必填。用于功能分解的项目描述 |
| `设计文档路径` | 可选。要导入的外部设计文档（.md/.json/.html） |

### 详细流程图

```mermaid
flowchart TD
    A["<b>/plan-cascade:mega-plan</b><br/>项目描述 [设计文档]"] --> A0[Step 0: 配置 .gitignore]
    A0 --> B[Step 1: 解析参数]
    B --> C[Step 2: 检查现有 Mega Plan]
    C --> D[Step 3: 分析项目需求]
    D --> E[Step 4: 生成 mega-plan.json]

    E --> F{外部设计文档?}
    F -->|是| F1[转换 .md/.json/.html<br/>为 design_doc.json]
    F -->|否| F2[Step 5: 自动生成<br/>项目 design_doc.json]
    F1 --> G
    F2 --> G

    G[Step 6: 创建支持文件<br/>mega-findings.md, .mega-status.json]
    G --> H[计算执行批次]
    H --> I[Step 7: 询问执行模式<br/>自动 / 手动]
    I --> J[Step 8: 显示统一审查<br/>unified-review.py --mode mega]

    J --> K{用户操作}
    K -->|编辑| L["/plan-cascade:mega-edit"]
    L --> J
    K -->|批准| M["/plan-cascade:mega-approve"]

    subgraph "mega-approve 执行"
        M --> N[解析 --auto-prd --agent --prd-agent --impl-agent]
        N --> O[创建批次 N 的 Worktrees]
        O --> P[为批次生成 PRDs<br/>使用选定的 PRD Agent]
        P --> Q[为批次执行 Stories<br/>使用选定的 Impl Agent]
        Q --> R[通过 TaskOutput 等待]
        R --> S{批次完成?}
        S -->|是| T[合并批次到目标分支]
        T --> U[清理批次 Worktrees]
        U --> V{还有批次?}
        V -->|是| O
        V -->|否| W[全部完成]
    end

    W --> X["/plan-cascade:mega-complete"]
    X --> Y[最终清理]
```

### 创建的文件

| 文件 | 位置 | 说明 |
|------|------|------|
| `mega-plan.json` | 用户数据目录或项目根目录 | 包含 Features 的项目计划 |
| `design_doc.json` | 项目根目录 | 项目级技术设计 |
| `mega-findings.md` | 项目根目录 | 跨 Feature 共享发现 |
| `.mega-status.json` | 状态目录或项目根目录 | 执行状态 |

### 恢复

如果中断：
```bash
/plan-cascade:mega-resume --auto-prd
```

---

## 7. Hybrid Worktree 流程

适用于需要分支隔离的单个复杂功能开发。

**重要**：此命令只处理 Worktree 创建和 PRD/设计文档生成。Story 执行由 `/plan-cascade:approve` 处理。

### 适用场景

| 类型 | 场景 | 示例 |
|------|------|------|
| ✅ 适用 | 包含多子任务的完整功能 | 用户认证（注册 + 登录 + 密码重置） |
| ✅ 适用 | 需要分支隔离的实验功能 | 新支付渠道集成测试 |
| ✅ 适用 | 中等规模重构（5-20 文件） | API 层统一错误处理改造 |
| ❌ 不适用 | 简单单文件修改 | 修改一个组件的样式 |
| ❌ 不适用 | 快速原型验证 | 验证某个库是否可用 |

### 命令参数

```bash
/plan-cascade:hybrid-worktree <任务名> <目标分支> <PRD或描述> [设计文档路径]

# 示例：
/plan-cascade:hybrid-worktree fix-auth main "修复认证 bug"
/plan-cascade:hybrid-worktree fix-auth main ./existing-prd.json
/plan-cascade:hybrid-worktree fix-auth main "修复认证" ./design-spec.md
```

| 参数 | 说明 |
|------|------|
| `任务名` | 必填。Worktree 和分支的名称 |
| `目标分支` | 必填。完成后合并到的分支 |
| `PRD或描述` | 必填。现有 PRD 文件路径或任务描述 |
| `设计文档路径` | 可选。要导入的外部设计文档 |

### 详细流程图

```mermaid
flowchart TD
    A["<b>/plan-cascade:hybrid-worktree</b><br/>任务名 目标分支 PRD或描述 [设计文档]"] --> A0[Step 0: 配置 .gitignore]
    A0 --> B[Step 1: 解析参数]
    B --> C[Step 2: 检测操作系统和 Shell<br/>跨平台支持]
    C --> D[Step 3: 验证 Git 仓库]
    D --> E[Step 4: 检测默认分支]
    E --> F[Step 5: 通过 PathResolver 设置变量]

    F --> G[Step 6: 检查项目 design_doc.json]
    G --> H{Worktree 存在?}
    H -->|是| I[导航到现有 Worktree]
    H -->|否| J[创建 Git Worktree + 分支]

    J --> K[初始化规划文件<br/>findings.md, progress.txt]
    K --> L{复制项目 design_doc.json?}
    L -->|是| L1[复制到 Worktree]
    L -->|否| M
    L1 --> M
    I --> M

    M[Step 7: 确定 PRD 模式]
    M --> N{PRD_ARG 是文件?}
    N -->|是| O[从文件加载 PRD]
    N -->|否| P[通过 Task Agent 生成 PRD]

    O --> Q
    P --> Q[通过 TaskOutput 等待]
    Q --> R[验证 PRD]

    R --> S{外部设计文档?}
    S -->|是| S1[转换外部文档]
    S -->|否| S2[自动生成功能 design_doc.json]
    S1 --> T
    S2 --> T

    T[创建 story_mappings<br/>将 Stories 链接到组件/决策]
    T --> U[更新 .hybrid-execution-context.md]
    U --> V[显示统一审查<br/>unified-review.py --mode hybrid]
    V --> W[显示 Worktree 摘要]

    W --> X{用户操作}
    X -->|编辑| Y["/plan-cascade:edit"]
    Y --> V
    X -->|批准| Z["/plan-cascade:approve"]

    Z --> EXEC[执行 Stories<br/>参见第 9 节]
    EXEC --> AA["/plan-cascade:hybrid-complete"]
    AA --> AB[合并到目标分支]
    AB --> AC[删除 Worktree]
```

### 设计文档继承

当项目级 `design_doc.json` 存在时：

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

### 恢复

如果中断：
```bash
/plan-cascade:hybrid-resume --auto
```

---

## 8. Hybrid Auto 流程

适用于简单功能的快速开发，无需 Worktree 隔离。

**重要**：此命令只处理 PRD 和设计文档生成。Story 执行由 `/plan-cascade:approve` 处理。

### 命令参数

```bash
/plan-cascade:hybrid-auto <任务描述> [设计文档路径] [--agent <名称>]

# 示例：
/plan-cascade:hybrid-auto "添加密码重置功能"
/plan-cascade:hybrid-auto "实现认证" ./auth-design.md
/plan-cascade:hybrid-auto "修复 bug" --agent=codex
```

| 参数 | 说明 |
|------|------|
| `任务描述` | 必填。用于 PRD 生成的任务描述 |
| `设计文档路径` | 可选。要导入的外部设计文档 |
| `--agent` | 可选。PRD 生成使用的 Agent（默认：claude-code） |

### 详细流程图

```mermaid
flowchart TD
    A["<b>/plan-cascade:hybrid-auto</b><br/>任务描述 [设计文档] [--agent]"] --> A0[Step 0: 配置 .gitignore]
    A0 --> B[Step 1: 解析参数]
    B --> C[Step 1.1: 解析 PRD Agent<br/>claude-code / codex / aider]

    C --> D{CLI Agent 可用?}
    D -->|否| D1[降级到 claude-code]
    D -->|是| E
    D1 --> E

    E[Step 2: 通过 Agent 生成 PRD]
    E --> F[Step 3: 通过 TaskOutput 等待]
    F --> G[Step 4: 验证 PRD 结构]

    G --> H{外部设计文档?}
    H -->|是| I[Step 4.5.1: 转换外部文档<br/>.md / .json / .html]
    H -->|否| J[Step 4.5.2: 自动生成<br/>design_doc.json]
    I --> K
    J --> K

    K[创建 story_mappings<br/>将 Stories 链接到组件]
    K --> L[通过 TaskOutput 等待]
    L --> M[Step 5: 显示统一审查<br/>unified-review.py --mode hybrid]

    M --> N[Step 6: 确认生成完成]
    N --> O{用户操作}

    O -->|编辑| P["/plan-cascade:edit"]
    P --> M
    O -->|批准| Q["/plan-cascade:approve"]
    O -->|"批准+自动"| R["/plan-cascade:approve --auto-run"]

    Q --> EXEC[执行 Stories<br/>参见第 9 节]
    R --> EXEC
```

### 生成的文件

| 文件 | 说明 |
|------|------|
| `prd.json` | 包含 Stories 的产品需求 |
| `design_doc.json` | 包含 story_mappings 的技术设计 |

### 恢复

如果中断：
```bash
/plan-cascade:hybrid-resume --auto
```

---

## 9. Approve 执行流程

`/plan-cascade:approve` 命令处理 Story 执行，支持多 Agent 协作。

### 命令参数

```bash
/plan-cascade:approve [选项]

选项：
  --agent <名称>        全局 Agent 覆盖（所有 Stories）
  --impl-agent <名称>   实现阶段 Agent
  --retry-agent <名称>  重试阶段 Agent
  --verify              启用 AI 验证门
  --verify-agent <名称> 验证 Agent（默认：claude-code）
  --no-review           禁用 AI 代码审查（默认启用）
  --review-agent <名称> 代码审查 Agent（默认：claude-code）
  --no-fallback         禁用自动降级到 claude-code
  --auto-run            立即开始执行
```

### Agent 优先级链

```
1. --agent 参数           （最高 - 全局覆盖）
2. --impl-agent 参数      （阶段特定覆盖）
3. PRD 中的 story.agent   （Story 级指定）
4. Story 类型推断：
   - bugfix, fix → codex
   - refactor, cleanup → aider
   - test, spec → claude-code
   - feature, add → claude-code
5. agents.json 中的 phase_defaults
6. agents.json 中的 fallback_chain
7. claude-code            （始终可用的回退）
```

### 详细流程图

```mermaid
flowchart TD
    A["<b>/plan-cascade:approve</b><br/>[--agent] [--impl-agent] [--verify] [--auto-run]"] --> B[Step 1: 检测操作系统和 Shell]
    B --> C[Step 2: 解析 Agent 参数]
    C --> D[Step 2.5: 加载 agents.json 配置]
    D --> E[Step 3: 验证 PRD 存在]
    E --> F[Step 4: 读取并验证 PRD]

    F --> G[Step 4.5: 检查 design_doc.json]
    G --> H[显示设计文档摘要<br/>组件、模式、ADRs、映射]

    H --> I[Step 4.6: 检测外部技能<br/>ExternalSkillLoader]
    I --> J[显示加载的技能摘要]

    J --> K[Step 5: 计算执行批次<br/>基于依赖关系]
    K --> L[Step 6: 选择执行模式<br/>自动 / 手动]
    L --> M[Step 7: 初始化 progress.txt]

    M --> N[Step 8: 启动批次 Agents]

    subgraph "Step 8: Agent 解析与启动"
        N --> O[8.1: 为每个 Story 解析 Agent<br/>优先级链]
        O --> P[8.2: 检查 Agent 可用性<br/>CLI: which/where]
        P --> Q{可用?}
        Q -->|否 + 降级| R[使用链中下一个]
        Q -->|否 + 禁用降级| S[错误]
        Q -->|是| T[8.3: 构建 Story Prompt<br/>+ 设计上下文<br/>+ 外部技能]
        R --> Q
        T --> U[8.4: 启动 Agent<br/>Task 工具或 CLI]
        U --> V[显示 Agent 启动摘要]
    end

    V --> W[Step 9: 等待批次完成]

    subgraph "Step 9: 等待与验证"
        W --> X[9.1: 通过 TaskOutput 等待<br/>每个 Story]
        X --> Y[9.2: 验证完成<br/>读取 progress.txt]
        Y --> FMT[9.2.1: FORMAT 门控<br/>PRE_VALIDATION]
        FMT --> QGV[9.2.2: TYPECHECK + TEST + LINT<br/>VALIDATION - 并行]
        QGV --> Z{启用 --verify?}
        Z -->|是| AA[9.2.6: AI 验证门<br/>检测骨架代码]
        Z -->|否| CR
        AA --> CR[9.2.7: CODE_REVIEW 门控<br/>POST_VALIDATION]
        CR --> AB{全部通过?}
        AB -->|否| AC[9.2.5: 使用不同 Agent 重试<br/>+ 错误上下文]
        AC --> U
        AB -->|是| AD[9.3: 推进到下一批次]
    end

    AD --> AE{还有批次?}
    AE -->|是 + 自动| N
    AE -->|是 + 手动| AF[询问用户确认]
    AF --> N
    AE -->|否| AG[Step 10: 显示最终状态]
```

### 质量门控执行顺序

质量门控分三个阶段执行：

```
┌─────────────────────────────────────────────────────────────────┐
│ PRE_VALIDATION (顺序执行)                                        │
│   └── FORMAT: 自动格式化代码 (ruff/prettier/cargo fmt/gofmt)    │
│       └── 格式化后使缓存失效                                     │
├─────────────────────────────────────────────────────────────────┤
│ VALIDATION (并行执行)                                            │
│   ├── TYPECHECK: mypy/tsc/cargo check                           │
│   ├── TEST: pytest/jest/cargo test                              │
│   └── LINT: ruff/eslint/clippy                                  │
├─────────────────────────────────────────────────────────────────┤
│ POST_VALIDATION (并行执行)                                       │
│   ├── IMPLEMENTATION_VERIFY: AI 骨架代码检测                    │
│   └── CODE_REVIEW: AI 5维度代码审查                             │
└─────────────────────────────────────────────────────────────────┘
```

### AI 验证门

当启用 `--verify` 时，验证每个完成的 Story：

```
[VERIFIED] story-001 - 所有验收标准已实现
[VERIFY_FAILED] story-002 - 检测到骨架代码：函数只有 'pass'
```

检测规则：
- 只有 `pass`、`...` 或 `raise NotImplementedError` 的函数
- 新代码中的 TODO/FIXME 注释
- 没有逻辑的占位符返回值
- 空的函数/方法体

### AI 代码审查门控

默认情况下，AI 代码审查会在每个 Story 完成后运行。使用 `--no-review` 禁用。

**审查维度（总计 100 分）：**

| 维度 | 分值 | 关注点 |
|------|------|--------|
| 代码质量 | 25 | 错误处理、资源管理、边界情况 |
| 命名清晰度 | 20 | 变量/函数命名、代码可读性 |
| 复杂度 | 20 | 圈复杂度、嵌套深度 |
| 模式遵循 | 20 | 架构模式、设计文档合规 |
| 安全性 | 15 | OWASP 漏洞、输入验证 |

**进度标记：**
```
[REVIEW_PASSED] story-001 - Score: 85/100
[REVIEW_ISSUES] story-002 - Score: 60/100 - 2 critical findings
```

**阻止条件：**
- 分数低于阈值（默认：70）
- 存在严重级别发现（当 `block_on_critical=true` 时）
- 置信度低于 0.7

### 进度中的 Agent 显示

```
=== 批次 1 已启动 ===

Stories 和分配的 Agents：
  - story-001: claude-code (task-tool)
  - story-002: aider (cli) [检测到 refactor]
  - story-003: codex (cli) [检测到 bugfix]

⚠️ Agent 降级：
  - story-004: aider → claude-code (aider CLI 未找到)

等待完成...
```

### 进度日志格式

```
[2026-01-28 10:30:00] story-001: [START] via codex (pid:12345)
[2026-01-28 10:30:05] story-001: 进度更新...
[2026-01-28 10:35:00] story-001: [COMPLETE] via codex
[2026-01-28 10:35:01] story-001: [VERIFIED] 所有标准满足
```

---

## 10. 自动迭代流程

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

    N1 --> CTX[加载 Story 上下文<br/>设计文档 + 外部技能]
    N2 --> CTX
    N3 --> CTX
    CTX --> O[并行启动 Agents]

    O --> P[通过 TaskOutput 等待]
    P --> Q{Story 完成?}

    Q -->|运行中| P
    Q -->|完成| R{质量门控启用?}
    Q -->|超时| S[记录超时失败]

    R -->|否| T[标记完成]
    R -->|是| U[执行质量门控]

    U --> FMT2{FORMAT<br/>PRE_VALIDATION}
    FMT2 -->|✓| V{TypeCheck}
    FMT2 -->|✗| X[记录失败详情]

    V -->|✓| W{Tests}
    V -->|✗| X

    W -->|✓| Y{Lint}
    W -->|✗| X

    Y -->|✓| VERIFY{AI 验证?}
    Y -->|✗ 且必需| X
    Y -->|✗ 但可选| VERIFY

    VERIFY -->|是| VGATE[AI 验证门]
    VERIFY -->|否| CR2
    VGATE -->|通过| CR2{CODE_REVIEW<br/>POST_VALIDATION}
    VGATE -->|失败| X

    CR2 -->|✓ 或禁用| T
    CR2 -->|✗ 严重| X

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

## 11. 路径存储模式

Plan Cascade 支持两种运行时文件的路径存储模式：

### 新模式（默认）

运行时文件存储在用户特定目录，保持项目根目录整洁：

| 平台 | 用户数据目录 |
|------|--------------|
| **Windows** | `%APPDATA%/plan-cascade/<project-id>/` |
| **Unix/macOS** | `~/.plan-cascade/<project-id>/` |

其中 `<project-id>` 是基于项目名称和路径哈希的唯一标识符（例如：`my-project-a1b2c3d4`）。

**新模式下的文件位置：**

| 文件类型 | 位置 |
|----------|------|
| `prd.json` | `<user-dir>/prd.json`（或 worktree 模式下在 worktree 中） |
| `mega-plan.json` | `<user-dir>/mega-plan.json` |
| `.mega-status.json` | `<user-dir>/.state/.mega-status.json` |
| `.iteration-state.json` | `<user-dir>/.state/` |
| Worktrees | `<user-dir>/.worktree/<task-name>/` |
| `design_doc.json` | **项目根目录**（用户可见） |
| `progress.txt` | **工作目录**（用户可见） |
| `findings.md` | **工作目录**（用户可见） |

### 旧模式

所有文件存储在项目根目录（向后兼容）：

| 文件 | 位置 |
|------|------|
| `prd.json` | `<project-root>/prd.json` |
| `mega-plan.json` | `<project-root>/mega-plan.json` |
| `.mega-status.json` | `<project-root>/.mega-status.json` |
| Worktrees | `<project-root>/.worktree/<task-name>/` |

### 检查当前模式

```bash
python3 -c "from plan_cascade.state.path_resolver import PathResolver; from pathlib import Path; r=PathResolver(Path.cwd()); print('模式:', 'legacy' if r.is_legacy_mode() else 'new'); print('PRD 路径:', r.get_prd_path())"
```

---

## 12. 数据流与状态文件

```mermaid
graph TB
    subgraph "输入"
        U[用户描述] --> CMD[命令解析]
        CFG[agents.json] --> CMD
        EXT[外部设计文档<br/>.md/.json/.html] -.-> CMD
    end

    subgraph "规划文件"
        CMD --> PRD[prd.json<br/>PRD文档]
        CMD --> MP[mega-plan.json<br/>项目计划]
        CMD --> DD[design_doc.json<br/>设计文档]
    end

    subgraph "执行状态"
        PRD --> AS[.agent-status.json<br/>Agent状态]
        PRD --> IS[.iteration-state.json<br/>迭代状态]
        PRD --> RS[.retry-state.json<br/>重试状态]
        MP --> MS[.mega-status.json<br/>Mega Plan状态]
    end

    subgraph "共享上下文"
        AS --> FD[findings.md<br/>发现记录]
        AS --> MF[mega-findings.md<br/>项目发现]
        AS --> PG[progress.txt<br/>进度日志]
    end

    subgraph "Agent 输出"
        AS --> AO[.agent-outputs/<br/>├─ story-001.log<br/>├─ story-001.prompt.txt<br/>├─ story-001.verify.md<br/>└─ story-001.result.json]
    end

    subgraph "缓存"
        AD[.agent-detection.json<br/>Agent检测缓存]
        GCF[.state/gate-cache.json<br/>门控结果缓存]
        LK[.locks/<br/>文件锁]
    end

    subgraph "上下文恢复"
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

### 文件说明

| 文件 | 类型 | 说明 |
|------|------|------|
| `prd.json` | 规划 | PRD 文档，包含目标、故事、依赖关系 |
| `mega-plan.json` | 规划 | 项目级计划，管理多个 Feature |
| `design_doc.json` | 规划 | 技术设计文档，架构和决策 |
| `agents.json` | 配置 | Agent 配置，包含阶段默认和降级链 |
| `findings.md` | 共享 | Agent 发现记录，支持标签过滤 |
| `mega-findings.md` | 共享 | 项目级发现记录（mega-plan 模式） |
| `progress.txt` | 共享 | 进度时间线，包含 Agent 执行信息 |
| `.agent-status.json` | 状态 | Agent 运行/完成/失败状态 |
| `.iteration-state.json` | 状态 | 自动迭代进度和批次结果 |
| `.retry-state.json` | 状态 | 重试历史和失败记录 |
| `.mega-status.json` | 状态 | Mega-plan 执行状态 |
| `.agent-detection.json` | 缓存 | 跨平台 Agent 检测结果（1小时TTL） |
| `.state/gate-cache.json` | 缓存 | 门控执行结果缓存（基于 git commit + 工作树哈希） |
| `.hybrid-execution-context.md` | 上下文 | Hybrid 任务上下文，用于会话中断后 AI 恢复 |
| `.mega-execution-context.md` | 上下文 | Mega-plan 上下文，用于会话中断后 AI 恢复 |
| `.agent-outputs/` | 输出 | Agent 日志、Prompt、验证报告和结果文件 |

---

## 13. 双模式架构

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

---

## 14. 多 Agent 协同架构

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

### 支持的 Agents

| Agent | 类型 | 最适用于 |
|-------|------|----------|
| `claude-code` | task-tool | 通用任务（默认，始终可用） |
| `codex` | cli | Bug 修复、快速实现 |
| `aider` | cli | 重构、代码改进 |
| `amp-code` | cli | 替代实现方案 |
| `cursor-cli` | cli | Cursor 编辑器集成 |

### 命令参数

**`/plan-cascade:approve`（Story 执行）：**

| 参数 | 说明 | 示例 |
|------|------|------|
| `--agent` | 全局 Agent 覆盖（所有 Stories） | `--agent=codex` |
| `--impl-agent` | 实现阶段 Agent | `--impl-agent=claude-code` |
| `--retry-agent` | 重试阶段 Agent | `--retry-agent=aider` |
| `--verify` | 启用 AI 验证门 | `--verify` |
| `--verify-agent` | 验证 Agent | `--verify-agent=claude-code` |
| `--no-review` | 禁用 AI 代码审查（默认启用） | `--no-review` |
| `--review-agent` | 代码审查 Agent | `--review-agent=claude-code` |
| `--no-fallback` | 禁用失败自动降级 | `--no-fallback` |

**`/plan-cascade:mega-approve`（Feature 执行）：**

| 参数 | 说明 | 示例 |
|------|------|------|
| `--agent` | 全局 Agent 覆盖 | `--agent=claude-code` |
| `--prd-agent` | PRD 生成 Agent | `--prd-agent=codex` |
| `--impl-agent` | 实现阶段 Agent | `--impl-agent=aider` |
| `--auto-prd` | 自动生成 PRD 并执行 | `--auto-prd` |

**`/plan-cascade:hybrid-auto`（PRD 生成）：**

| 参数 | 说明 | 示例 |
|------|------|------|
| `--agent` | PRD 生成 Agent | `--agent=codex` |

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

### Agent 配置文件 (agents.json)

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
