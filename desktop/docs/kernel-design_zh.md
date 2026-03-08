# Plan Cascade Desktop 内核系统设计文档

**版本**: 1.0.0
**日期**: 2026-03-08
**范围**: Simple mode Chat / Plan / Task lifecycle, transcript, output, and cross-mode context ownership

---

## 目录

1. [工作流内核 SSOT 合约](#1-工作流内核-ssot-合约)
2. [Simple Plan/Task 生产 V2 规范](#2-simple-plantask-生产-v2-规范)
3. [工作流计划 V2 合约](#3-工作流计划-v2-合约)
4. [ADR: 工作流内核权威 V2](#4-adr-工作流内核权威-v2)
5. [内核所有权的核心原则](#5-内核所有权的核心原则)

---

## 1. 工作流内核 SSOT 合约

> 来源: `workflow-kernel-ssot.md`

### 1.1 目标

确保 `workflowKernel` 是以下内容的唯一真相源 (SSOT)：

- 活动根会话 + 活动模式
- 生命周期状态 / 待处理提示
- 每模式的 transcript 内容
- 右侧面板输出内容
- 用于路由 transcript 更新的运行时/会话绑定元数据
- 跨模式上下文账本和每模式入口交接快照

### 1.2 所有权规则

1. **生命周期真相必须来自内核快照**:
   - `session.status`
   - `session.activeMode`
   - `session.modeSnapshots.chat.phase`
   - `session.modeSnapshots.task.phase`
   - `session.modeSnapshots.plan.phase`
   - `session.modeSnapshots.task.pendingInterview`
   - `session.modeSnapshots.plan.pendingClarification`

2. **Transcript 真相必须来自内核 transcript 状态**:
   - `workflowKernel.modeTranscriptsBySession[rootSessionId][mode]`
   - UI 必须不从 `execution.streamingOutput` 渲染
   - UI 必须不从 `simpleSessionStore` 渲染

3. **输出真相必须来自 transcript 派生**:
   - `ChatTranscript`
   - 右侧面板 `Output`
   - Simple 模式下的 Git 工具输出提取

4. **`executionStore` 仅用于运行时控制**:
   - 允许: pause/resume/cancel, runtime ids, usage, transport state
   - 不允许: Simple 模式的 transcript 真相

5. **`simpleSessionStore` 仅是 UI 缓存**:
   - 允许: drafts, attachments, unread flags, scroll/UI affordances
   - 不允许: transcript 真相或 transcript 修订

6. **Transcript 路由必须解析明确的根会话**:
   - 按 `rootSessionId` 路由
   - 或按 `bindingSessionId -> rootSessionId` 路由
   - 未解析的路由必须丢弃并报告
   - 未解析的路由永远不能回退到 `activeRootSessionId`

7. **跨模式上下文真相由内核拥有**:
   - 根 `handoffContext` 是持久的跨模式账本
   - `modeSnapshots.chat.entryHandoff`
   - `modeSnapshots.plan.entryHandoff`
   - `modeSnapshots.task.entryHandoff`
   - 模式入口交接在转换/启动时冻结，不得在前端重新计算

8. **跨模式 transcript 可见性必须由内核编写**:
   - 每次成功的模式转换可以附加一个 `mode_handoff_card`
   - UI 必须像渲染任何其他工作流卡片一样从内核 transcript 渲染
   - 前端 toast 可以补充 UX，但不能作为导入内容的真相来源

### 1.3 实现说明

1. `SimpleModeShell` 不得合成前台 vs 缓存 transcript
2. `workflow-mode-transcript-updated` 更新内核 transcript 缓存，而非 `simpleSessionStore`
3. Chat 用户消息必须用稳定的 `lineId`、`turnId`、`turnBoundary: 'user'` 写入内核 transcript
4. Chat 编辑/回滚/重新生成/分叉必须使用内核 transcript line ids
5. Plan/Task 卡片和步骤/故事输出必须仅通过内核 transcript 进入 UI
6. 如果观察到生命周期冲突，内核状态获胜，其他存储被视为过时
7. `transition_mode` / `transition_and_submit_input` 在内核内部派生目标 `entryHandoff`:
   - `chat -> plan/task` 使用 chat transcript turns
   - `plan/task -> chat` 使用来自根账本的结构化摘要项
8. `PlanModeSession` / `TaskModeSession` 持久化 `kernelSessionId`; 启动命令必须从该根会话读取交接，不得等待 `linkModeSession`
9. Plan/Task 摘要发布回 Chat 必须在后端命令中发生，而非前端编排器
10. Chat prompt 组装可以读取 `chat.entryHandoff`，但仅作为只读内核快照

### 1.4 验证清单

- [ ] `SimpleModeShell` 仅从内核 transcript 缓存渲染中间 transcript
- [ ] 右侧面板输出从与中间面板相同的 transcript 源渲染
- [ ] `simpleSessionStore` 不包含 transcript 内容或 transcript 修订状态
- [ ] `executionStore` transcript 状态不用作 Simple 模式渲染权威
- [ ] Chat 第一个用户消息渲染为实际内容，而非合成 `User`
- [ ] Plan/Task transcript 在首次进入时出现，无需会话切换/重新进入
- [ ] Transcript 更新不会跨根会话泄漏
- [ ] Typecheck 通过
- [ ] `chat -> plan/task` 首次进入看到导入的上下文，无需重新进入
- [ ] `plan/task -> chat` 下一个聊天 turn 看到来自内核 `entryHandoff` 的结构化摘要

---

## 2. Simple Plan/Task 生产 V2 规范

> 来源: `Simple-Plan-Task-Production-V2.md`

### 2.1 目标

本次迭代通过以下方式将 Simple 页面 Chat/Plan/Task 流程硬化为生产级：

- 统一阶段真相到工作流内核快照
- 用内核事件推送替换主要轮询流程
- 恢复损坏的 Task gate 和 Plan step-output 数据路径
- 硬切遗留兼容性双轨
- 移除死/未使用的遗留 UI 路径

### 2.2 运行时标志

Simple Plan/Task 生命周期或卡片管道没有运行时 rollout 标志。
- 内核快照权威始终开启
- 卡片渲染仅为类型化 payload

### 2.3 后端合约

#### 命令

- `workflow_link_mode_session(session_id, mode, mode_session_id)`
  - 将 Plan/Task 后端会话 ID 绑定到内核会话以实现可追踪性和恢复

#### 事件

- `workflow-kernel-updated`
  - 在内核/会话变更后发出，包含 `{ sessionState, revision, source }`
  - 前端订阅一次并增量应用更新

### 2.4 前端数据路径

#### 内核驱动的阶段状态

- `SimpleMode` 使用 `workflowKernel.session.modeSnapshots` 获取 `chat/plan/task` 阶段
- `SimpleMode` 不再调用 orchestrator 运行时水合 hooks 来镜像内核阶段/问题快照
- 门控交互操作的工作流卡片现在直接读取内核会话快照
- 没有运行时特性标志控制内核权威

#### Task Gate 结果

- `workflowOrchestrator.subscribeToProgressEvents` 将 `payload.gateResults` 映射到:
  - `qualityGateResults[storyId].gates`
  - `qualityGateResults[storyId].overallStatus`
- 即使在失败路径中，gate 证据也被保留

#### Plan Step Output

- `planMode.fetchStepOutput(stepId)` 包装 `get_step_output`
- `planOrchestrator` 在 `step_completed` 后注入 `plan_step_output` 卡片
- 空/失败的输出获取发出可见错误卡片（而非静默丢弃）

### 2.5 内核 SSOT 硬化 (2026-03-04)

- 移除 shell 到 orchestrator 运行时镜像 (`syncRuntimeFromKernel` hooks)
- 从 Task/Plan orchestrators 移除 `syncRuntimeFromKernel` action
- 启动期间的模式链接现在从命令返回和内核链接快照解析模式会话 ID
- Plan/Task 模式存储仅是命令客户端 (`isLoading/isCancelling/error/_requestId`)，不持久化运行时业务状态
- Plan/Task orchestrators 仅保留 UI drafts + 执行投影; 生命周期阶段和链接会话真相由内核驱动
- 移除 `planMode.sessionPhase`、`taskMode.sessionStatus`、`isPlanMode`、`isTaskMode` 和相关兼容性镜像

#### 类型化卡片管道

- `execution.appendCard(payload)` 始终存储 `line.cardPayload`
- `ChatTranscript` 仅从 `line.cardPayload` 渲染卡片
- 移除 JSON 解析回退
- 遗留历史卡片行没有类型化 payload 的被丢弃并持久化为清理记��

#### Chat Transcript 性能硬化

- `ChatTranscript` 现在通过索引范围 (`userLineIndex`, `assistantStartIndex`, `assistantEndIndex`) 而非每个 turn 克隆 assistant 数组来派生 `TurnViewModel`
- 长会话 (`>=50` turns) 使用 turn 级虚拟化 (`@tanstack/react-virtual`)，动态行测量和 overscan
- `SimpleModeShell` 中的导出图片流程在捕获前暂时强制全 transcript 渲染 (`forceFullRender`)，然后恢复正常的虚拟化渲染和之前的滚动位置
- `useWorkflowModeSwitchGuard` 不再依赖渲染时 `streamingOutput` props; 它仅在模式切换操作时读取最新执行快照

### 2.6 移除的遗留 UI

从活动代码库移除的未使用组件:

- `src/components/SimpleMode/WorkflowProgressPanel.tsx`
- `src/components/SimpleMode/ProgressView.tsx`
- `src/components/SimpleMode/PlanClarifyInputArea.tsx`
- `src/components/SimpleMode/StructuredInputOverlay.tsx`
- `src/components/TaskMode/*` (整个未使用的文件夹)

---

## 3. 工作流计划 V2 合约

> 来源: `workflow-plan-v2-contract.md`

### 3.1 执行策略

`Plan.executionConfig` 现在支持:

- `maxParallel: number`
- `retry.enabled: boolean`
- `retry.maxAttempts: number` (首次尝试后重试)
- `retry.backoffMs: number`
- `retry.failBatchOnExhausted: boolean`

**默认策略**:

- `maxParallel=4`
- `retry.enabled=true`
- `retry.maxAttempts=2`
- `retry.backoffMs=800`
- `retry.failBatchOnExhausted=true`

### 3.2 Step Output V2

`StepOutput` 字段:

- `summary`
- `fullContent`
- `artifacts`
- `truncated`
- `originalLength`
- `shownLength`
- `criteriaMet`
- `qualityState: complete | incomplete`
- `incompleteReason: string | null`
- `attemptCount`
- `toolEvidence[]`

**质量门规则**:

- 空输出拒绝
- 待操作叙述 ("let me...", "让我...") 拒绝
- 非平凡步骤的短非结构化输出拒绝
- 未满足的完成标准拒绝

被拒绝的输出视为步骤失败并进入重试策略

### 3.3 进度遥测 V2

`plan-mode-progress` 事件包括:

- `runId`
- `eventSeq`
- `source`
- `dropReason`

生命周期所有权不来自这些遥测事件

### 3.4 终端报告 V2

`PlanExecutionReport` 字段:

- `terminalState: completed | failed | cancelled`
- `finalConclusionMarkdown`
- `highlights[]`
- `nextActions[]`
- `retryStats.totalRetries`
- `retryStats.stepsRetried`
- `retryStats.exhaustedFailures`
- `runId`

`completed/failed/cancelled` 都需要终端报告 payload

### 3.5 批处理门

执行模型:

1. 运行当前批次的步骤
2. 自动重试失败/不完整的步骤，达到策略限制
3. 如果仍有失败且 `failBatchOnExhausted=true`，发出 `batch_blocked` 并停止后续批次

这防止了"前一批失败但下一批仍运行"的行为

---

## 4. ADR: 工作流内核权威 V2

> 来源: `ADR-kernel-authority-v2.md`

### 4.1 上下文

模式阶段状态之前有双重真相源:
- 前端遗留 orchestrator 阶段值
- 工作流内核 `modeSnapshots`

前端定期将遗留阶段推回内核 (`syncModePhase`)，这在恢复或快速模式转换期间引入漂移风险和竞态条件

### 4.2 决策

使工作流内核 `modeSnapshots` 成为 UI 渲染和模式状态检查的权威阶段源

**变更**:
- 移除前端阶段回填桥接 (`syncModePhase`)
- 直接从内核快照读取 task/plan/chat 阶段
- 恢复时，运行快照完整性修复并在 `SessionRecovered` 事件 payload 中发出诊断元数据 (`snapshotIntegrity`, `repairedFields`)

### 4.3 后果

- Chat/Plan/Task 阶段状态的单一真相源
- 恢复可观察且对不完整 snapshot payloads 自愈
- 遗留 orchestrator 状态保持操作输入，但不再覆盖内核权威

---

## 5. 内核所有权的核心原则

### 5.1 单一真相源 (SSOT)

内核是以下内容的唯一权威:
- 会话生命周期状态
- 模式阶段 (`chat.phase`, `plan.phase`, `task.phase`)
- Transcript 内容
- 跨模式上下文交接

### 5.2 前端只读原则

- 前端不得合成内核状态
- 前端不得覆盖内核权威
- 前端只能消费内核推送的事件更新

### 5.3 模式切换原则

- 模式转换时，上下文在转换开始时冻结
- 交接内容由内核生成，前端不参与计算
- 模式入口时，前端直接消费内核快照

### 5.4 验证要求

所有变更必须通过以下验证:
- 内核状态始终是最新的
- UI 渲染与内核状态一致
- 跨会话边界的状态持久化正确

---

## 相关文档

- [整体架构设计](./architecture-design.md)
- [内存与技能设计](./memory-skill-design.md)
- [代码库索引设计](./codebase-index-design.md)
