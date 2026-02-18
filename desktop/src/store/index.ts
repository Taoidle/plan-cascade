/**
 * Store Module Exports
 *
 * Central export point for all Zustand stores.
 */

export { useModeStore, MODES, MODE_LABELS, MODE_DESCRIPTIONS } from './mode';
export type { Mode, BreadcrumbItem, TransitionDirection } from './mode';

export { useExecutionStore } from './execution';
export type {
  ExecutionStatus,
  Strategy,
  Story,
  Batch,
  ExecutionResult,
  ExecutionHistoryItem,
} from './execution';

export { useSettingsStore } from './settings';
export type { Backend, Theme } from './settings';

export { usePRDStore } from './prd';
export type {
  AgentType,
  StoryStatus,
  ExecutionStrategy,
  PRDStory,
  QualityGate,
  WorktreeConfig,
  PRDDraft,
  PRD,
} from './prd';

export { useClaudeCodeStore } from './claudeCode';
export type {
  ToolCallStatus,
  ToolType,
  ToolCallParameters,
  ToolCallResult,
  ToolCall,
  MessageRole,
  Message,
  Conversation,
} from './claudeCode';

export { useProjectsStore } from './projects';
export type {
  Project,
  Session,
  SessionDetails,
  MessageSummary,
  ResumeResult,
  ProjectSortBy,
  CommandResponse,
} from '../types/project';

export { useAgentsStore, getFilteredAgents } from './agents';
export type {
  Agent,
  AgentWithStats,
  AgentCreateRequest,
  AgentUpdateRequest,
  AgentRun,
  AgentRunList,
  AgentStats,
  RunStatus,
} from '../types/agent';

export { useSpecInterviewStore, getPhaseLabel, getPhaseOrder } from './specInterview';
export type {
  InterviewPhase,
  InterviewQuestion,
  InterviewHistoryEntry,
  InterviewSession,
  InterviewConfig,
  CompiledSpec,
  CompileOptions,
} from './specInterview';

export { useRecoveryStore, EXECUTION_MODE_LABELS } from './recovery';
export type {
  ExecutionMode,
  IncompleteTask,
  RestoredContext,
  ResumeResult as RecoveryResumeResult,
  ResumeEvent,
  RecoveryState,
} from './recovery';

export { useDesignDocStore } from './designDoc';
export type {
  DesignDocLevel,
  DecisionStatus,
  DesignDocMetadata,
  Overview,
  DesignComponent,
  DesignPattern,
  DesignDecision,
  FeatureMapping,
  Architecture,
  Interfaces,
  DesignDoc,
  GenerateResult,
  ImportResult,
  ImportWarning,
  GenerationInfo,
  GenerateOptions,
} from './designDoc';

export { useSkillMemoryStore } from './skillMemory';
export type {
  SkillSourceFilter,
  MemoryCategoryFilter,
  SkillMemoryTab,
} from './skillMemory';

export { useEmbeddingStore } from './embedding';
export type { EmbeddingState } from './embedding';

export { useTaskModeStore } from './taskMode';
export type {
  ExecutionMode as TaskExecutionMode,
  RiskLevel,
  Benefit,
  TaskModeSessionStatus,
  StrategyAnalysis as TaskStrategyAnalysis,
  TaskStory,
  TaskPrd,
  TaskModeSession,
  BatchExecutionProgress,
  TaskExecutionStatus,
  ExecutionReport as TaskExecutionReport,
  StoryQualityGateResults,
  GateResult,
  DimensionScore,
  TaskModeState,
} from './taskMode';

export { useExecutionReportStore } from './executionReport';
export type {
  ReportSummary,
  RadarDimension,
  TimelineEntry,
  AgentPerformance,
  ExecutionReportModel,
  ExecutionReportState,
} from './executionReport';

export { useKnowledgeStore } from './knowledge';
export type { KnowledgeState } from './knowledge';

export { useArtifactsStore } from './artifacts';
export type { ArtifactsState, ScopeFilter } from './artifacts';

export { useGitStore } from './git';
