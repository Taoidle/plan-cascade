# Plan Cascade Desktop Kernel System Design Document

**Version**: 1.0.0
**Date**: 2026-03-08
**Scope**: Simple mode Chat / Plan / Task lifecycle, transcript, output, and cross-mode context ownership

---

## Table of Contents

1. [Workflow Kernel SSOT Contract](#1-workflow-kernel-ssot-contract)
2. [Simple Plan/Task Production V2 Specification](#2-simple-plantask-production-v2-specification)
3. [Workflow Plan V2 Contract](#3-workflow-plan-v2-contract)
4. [ADR: Workflow Kernel Authority V2](#4-adr-workflow-kernel-authority-v2)
5. [Core Principles of Kernel Ownership](#5-core-principles-of-kernel-ownership)

---

## 1. Workflow Kernel SSOT Contract

> Source: `workflow-kernel-ssot.md`

### 1.1 Goals

Ensure that `workflowKernel` is the Single Source of Truth (SSOT) for:

- Active root session + active mode
- Lifecycle state / pending prompts
- Transcript content for each mode
- Right panel output content
- Runtime/session binding metadata used for routing transcript updates
- Cross-mode context ledger and per-mode entry handoff snapshots

### 1.2 Ownership Rules

1. **Lifecycle truth must come from kernel snapshots**:
   - `session.status`
   - `session.activeMode`
   - `session.modeSnapshots.chat.phase`
   - `session.modeSnapshots.task.phase`
   - `session.modeSnapshots.plan.phase`
   - `session.modeSnapshots.task.pendingInterview`
   - `session.modeSnapshots.plan.pendingClarification`

2. **Transcript truth must come from kernel transcript state**:
   - `workflowKernel.modeTranscriptsBySession[rootSessionId][mode]`
   - UI must NOT render from `execution.streamingOutput`
   - UI must NOT render from `simpleSessionStore`

3. **Output truth must come from transcript derivation**:
   - `ChatTranscript`
   - Right panel `Output`
   - Git tool output extraction in Simple mode

4. **`executionStore` is only for runtime control**:
   - Allowed: pause/resume/cancel, runtime ids, usage, transport state
   - Not allowed: Simple mode transcript truth

5. **`simpleSessionStore` is only a UI cache**:
   - Allowed: drafts, attachments, unread flags, scroll/UI affordances
   - Not allowed: transcript truth or transcript revisions

6. **Transcript routing must resolve explicit root sessions**:
   - Route by `rootSessionId`
   - Or route by `bindingSessionId -> rootSessionId`
   - Unresolved routes must be discarded and reported
   - Unresolved routes must NEVER fall back to `activeRootSessionId`

7. **Cross-mode context truth is owned by the kernel**:
   - Root `handoffContext` is a persistent cross-mode ledger
   - `modeSnapshots.chat.entryHandoff`
   - `modeSnapshots.plan.entryHandoff`
   - `modeSnapshots.task.entryHandoff`
   - Mode entry handoffs are frozen at transition/start, not recalculated in frontend

8. **Cross-mode transcript visibility must be authored by the kernel**:
   - Each successful mode transition can attach a `mode_handoff_card`
   - UI must render from kernel transcripts like any other workflow card
   - Frontend toasts can supplement UX, but cannot be the source of truth for imported content

### 1.3 Implementation Notes

1. `SimpleModeShell` must NOT synthesize foreground vs cached transcripts
2. `workflow-mode-transcript-updated` updates kernel transcript cache, not `simpleSessionStore`
3. Chat user messages must be written to kernel transcript with stable `lineId`, `turnId`, `turnBoundary: 'user'`
4. Chat edit/rollback/regenerate/fork must use kernel transcript line ids
5. Plan/Task cards and step/story output must enter UI only through kernel transcript
6. If lifecycle conflicts are observed, kernel state wins, other stores are considered stale
7. `transition_mode` / `transition_and_submit_input` derive target `entryHandoff` internally:
   - `chat -> plan/task` uses chat transcript turns
   - `plan/task -> chat` uses structured summary items from root ledger
8. `PlanModeSession` / `TaskModeSession` persist `kernelSessionId`; startup commands must read handoffs from that root session, not wait for `linkModeSession`
9. Plan/Task summary publishing back to Chat must happen in backend commands, not frontend orchestrator
10. Chat prompt assembly can read `chat.entryHandoff`, but only as a read-only kernel snapshot

### 1.4 Verification Checklist

- [ ] `SimpleModeShell` only renders intermediate transcripts from kernel transcript cache
- [ ] Right panel output renders from the same transcript source as middle panel
- [ ] `simpleSessionStore` does not contain transcript content or transcript revision state
- [ ] `executionStore` transcript state is not used as Simple mode rendering authority
- [ ] Chat first user message renders as actual content, not synthesized `User`
- [ ] Plan/Task transcript appears on first entry, no session switch/re-entry needed
- [ ] Transcript updates do not leak across root sessions
- [ ] Typecheck passes
- [ ] `chat -> plan/task` first entry sees imported context, no re-entry needed
- [ ] `plan/task -> chat` next chat turn sees structured summary from kernel `entryHandoff`

---

## 2. Simple Plan/Task Production V2 Specification

> Source: `Simple-Plan-Task-Production-V2.md`

### 2.1 Goals

This iteration hardens the Simple page Chat/Plan/Task flow to production grade by:

- Unifying phase truth to workflow kernel snapshots
- Replacing main polling flows with kernel event push
- Restoring broken Task gate and Plan step-output data paths
- Hard-cutting legacy compatibility dual-track
- Removing dead/unused legacy UI paths

### 2.2 Runtime Flags

There are no runtime rollout flags for Simple Plan/Task lifecycle or card pipeline.
- Kernel snapshot authority is always on
- Card rendering is typed payload only

### 2.3 Backend Contract

#### Commands

- `workflow_link_mode_session(session_id, mode, mode_session_id)`
  - Binds Plan/Task backend session ID to kernel session for traceability and recovery

#### Events

- `workflow-kernel-updated`
  - Emitted after kernel/session changes, contains `{ sessionState, revision, source }`
  - Frontend subscribes once and applies updates incrementally

### 2.4 Frontend Data Paths

#### Kernel-Driven Phase State

- `SimpleMode` uses `workflowKernel.session.modeSnapshots` to get `chat/plan/task` phases
- `SimpleMode` no longer calls orchestrator runtime hydration hooks to mirror kernel phase/snapshot questions
- Workflow cards for gated interactive operations now read directly from kernel session snapshots
- No runtime feature flags control kernel authority

#### Task Gate Results

- `workflowOrchestrator.subscribeToProgressEvents` maps `payload.gateResults` to:
  - `qualityGateResults[storyId].gates`
  - `qualityGateResults[storyId].overallStatus`
- Gate evidence is preserved even in failure paths

#### Plan Step Output

- `planMode.fetchStepOutput(stepId)` wraps `get_step_output`
- `planOrchestrator` injects `plan_step_output` cards after `step_completed`
- Empty/failed output fetches emit visible error cards (not silently dropped)

### 2.5 Kernel SSOT Hardening (2024-03-04)

- Removed shell to orchestrator runtime mirroring (`syncRuntimeFromKernel` hooks)
- Removed `syncRuntimeFromKernel` action from Task/Plan orchestrators
- Mode linking during startup now returns from command, frontend does not wait for `linkModeSession`
- Kernel owns all phase truth, frontend hooks removed

---

## 3. Workflow Plan V2 Contract

> Source: `workflow-plan-v2-contract.md`

### 3.1 Plan Mode Overview

Plan Mode provides structured step-by-step planning with batch execution capabilities.

### 3.2 Plan Structure

```
Plan
├── Stories (user-visible milestones)
│   ├── Story 1
│   │   ├── Step 1.1
│   │   ├── Step 1.2
│   │   └── Step 1.3
│   └── Story 2
│       ├── Step 2.1
│       └── Step 2.2
└── Output (aggregated results)
```

### 3.3 Plan Execution Contract

1. **Story-level granularity**: Stories are the unit of user visibility and checkpoint
2. **Step execution**: Steps within a story execute sequentially
3. **Quality gates**: Each step can have associated quality gate validation
4. **Output capture**: Step outputs are captured and aggregated at story level

### 3.4 Handoff Context

When transitioning from Chat to Plan:

- Chat transcript turns are packaged as `entryHandoff`
- `handoffContext` contains structured summary items
- Plan prompt assembly reads from `chat.entryHandoff`

When transitioning from Plan to Chat:

- Plan summary is published as a workflow card
- Structured summary items flow back to Chat transcript
- Chat continues with full context awareness

---

## 4. ADR: Workflow Kernel Authority V2

> Source: `ADR-kernel-authority-v2.md`

### 4.1 Decision

The Workflow Kernel is the **Single Source of Truth (SSOT)** for all runtime state in Simple mode (Chat/Plan/Task).

### 4.2 Motivation

Previous architecture allowed multiple sources of truth:
- `executionStore` contained runtime state
- `simpleSessionStore` contained UI state
- Kernel contained kernel state

This led to synchronization issues and state drift.

### 4.3 Consequences

| Before | After |
|--------|-------|
| Multiple state sources | Single kernel authority |
| Polling for updates | Event-driven push |
| State reconciliation | Direct kernel reads |
| Complex sync logic | Simplified data flow |

### 4.4 Migration Path

1. Phase 1: Dual-write to kernel and legacy stores (for rollback)
2. Phase 2: Frontend reads from kernel only
3. Phase 3: Remove legacy store sync logic
4. Phase 4: Remove legacy stores entirely

---

## 5. Core Principles of Kernel Ownership

### 5.1 Lifecycle Truth

All lifecycle-related state must originate from kernel snapshots:

```typescript
// WRONG: Reading from execution store
const phase = executionStore.currentPhase;

// CORRECT: Reading from kernel
const phase = workflowKernel.session.modeSnapshots.chat.phase;
```

### 5.2 Transcript Truth

All transcript rendering must come from kernel transcripts:

```typescript
// WRONG: Rendering from streaming output
messages.map(msg => <Message content={msg.streamingText} />);

// CORRECT: Rendering from kernel transcript
const transcript = workflowKernel.modeTranscriptsBySession[rootSessionId]['chat'];
transcript.lines.map(line => <Message content={line.content} />);
```

### 5.3 Output Truth

Output panels must derive from transcript:

```typescript
// WRONG: Storing output separately
const output = executionStore.currentOutput;

// CORRECT: Deriving from transcript
const output = deriveOutputFromTranscript(workflowKernel.currentTranscript);
```

### 5.4 Event Consumption

Frontend must subscribe to kernel events:

```typescript
// Subscribe once at app mount
subscribe('workflow-kernel-updated', (event) => {
  workflowKernel.applyUpdate(event.sessionState);
});
```

### 5.5 No Side Channels

Do not bypass kernel for state:

- No direct writes to `executionStore` for Simple mode state
- No recalculation of `entryHandoff` in frontend
- No fallback to `activeRootSessionId` for routing

---

## Appendix: Migration Checklist

- [ ] Remove `syncRuntimeFromKernel` hooks
- [ ] Remove `executionStore` transcript reads in Simple mode
- [ ] Remove `simpleSessionStore` transcript state
- [ ] Ensure all mode transitions use kernel events
- [ ] Verify `entryHandoff` is computed in backend
- [ ] Test Chat → Plan → Chat round-trip
- [ ] Test Chat → Task → Chat round-trip
- [ ] Verify no state leakage across sessions
