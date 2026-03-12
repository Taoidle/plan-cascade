import type { HandoffContextBundle } from './workflowKernel';
import type { ModeQualitySnapshot } from './workflowQuality';

export type DebugLifecyclePhase =
  | 'intaking'
  | 'clarifying'
  | 'gathering_signal'
  | 'reproducing'
  | 'hypothesizing'
  | 'testing_hypothesis'
  | 'identifying_root_cause'
  | 'proposing_fix'
  | 'patch_review'
  | 'patching'
  | 'verifying'
  | 'completed'
  | 'failed'
  | 'cancelled';

export type DebugSeverity = 'low' | 'medium' | 'high' | 'critical';
export type DebugEnvironment = 'dev' | 'staging' | 'prod';
export type DebugCapabilityClass = 'observe' | 'experiment' | 'mutate';
export type DebugCapabilityProfile = 'dev_full' | 'staging_limited' | 'prod_observe_only';
export type DebugBrowserBridgeKind = 'devtools_mcp' | 'builtin_browser' | 'unavailable';
export type DebugPatchOperationKind = 'replace_text' | 'write_file';

export type DebugToolCategory =
  | 'debug:logs'
  | 'debug:db_read'
  | 'debug:db_write'
  | 'debug:cache_read'
  | 'debug:cache_write'
  | 'debug:queue'
  | 'debug:metrics'
  | 'debug:trace'
  | 'debug:k8s'
  | 'debug:browser'
  | 'debug:runbook'
  | 'debug:test_runner';

export interface DebugIntakeFieldMap {
  title: string;
  symptom: string;
  expectedBehavior: string;
  actualBehavior: string;
  reproSteps: string[];
  environment: DebugEnvironment;
  affectedSurface: string[];
  recentChanges: string;
  supportingArtifacts: string[];
  targetUrlOrEntry: string | null;
}

export interface DebugEvidenceRef {
  id: string;
  kind: string;
  title: string;
  summary: string;
  source: string;
  createdAt: string;
  metadata: Record<string, unknown>;
}

export interface DebugHypothesis {
  id: string;
  statement: string;
  confidence: number;
  supportingEvidenceIds: string[];
  contradictingEvidenceIds: string[];
  nextChecks: string[];
  status: 'candidate' | 'testing' | 'confirmed' | 'rejected';
}

export interface RootCauseReport {
  conclusion: string;
  supportingEvidenceIds: string[];
  contradictions: string[];
  confidence: number;
  impactScope: string[];
  recommendedDirection: string;
}

export interface DebugPatchOperation {
  id: string;
  kind: DebugPatchOperationKind;
  filePath: string;
  description: string;
  findText?: string | null;
  replaceText?: string | null;
  content?: string | null;
  createIfMissing?: boolean;
  expectedOccurrences?: number | null;
}

export interface FixProposal {
  summary: string;
  changeScope: string[];
  riskLevel: DebugSeverity;
  filesOrSystemsTouched: string[];
  manualApprovalsRequired: string[];
  verificationPlan: string[];
  patchPreviewRef: string | null;
  patchOperations?: DebugPatchOperation[];
}

export interface VerificationReport {
  summary: string;
  checks: Array<{
    id: string;
    label: string;
    status: 'passed' | 'failed' | 'skipped';
    details?: string | null;
  }>;
  residualRisks: string[];
  artifacts: string[];
}

export interface DebugPendingApproval {
  kind: 'patch_review' | 'dangerous_tool';
  title: string;
  description: string;
  requiredActions: string[];
}

export interface DebugState {
  caseId: string | null;
  phase: DebugLifecyclePhase;
  severity: DebugSeverity;
  environment: DebugEnvironment;
  symptomSummary: string;
  title: string | null;
  projectPath?: string | null;
  expectedBehavior: string | null;
  actualBehavior: string | null;
  reproSteps: string[];
  affectedSurface: string[];
  recentChanges: string | null;
  targetUrlOrEntry: string | null;
  evidenceRefs: DebugEvidenceRef[];
  activeHypotheses: DebugHypothesis[];
  selectedRootCause: RootCauseReport | null;
  fixProposal: FixProposal | null;
  pendingApproval: DebugPendingApproval | null;
  verificationReport: VerificationReport | null;
  pendingPrompt: string | null;
  capabilityProfile: DebugCapabilityProfile;
  toolBlockReason: string | null;
  backgroundStatus?: string | null;
  lastCheckpointId?: string | null;
  entryHandoff?: HandoffContextBundle;
  quality?: ModeQualitySnapshot | null;
}

export interface DebugRuntimeCapabilities {
  profile: DebugCapabilityProfile;
  allowedClasses: DebugCapabilityClass[];
  allowedToolCategories: DebugToolCategory[];
  approvalRequiredFor: DebugCapabilityClass[];
}

export interface DebugToolCapability {
  toolName: string;
  description: string;
  source: 'builtin' | 'runtime' | string;
  capabilityClass: DebugCapabilityClass;
  toolCategory: DebugToolCategory | null;
  debugCategories: DebugToolCategory[];
  environmentAllowlist: DebugEnvironment[];
  writeBehavior?: string | null;
  allowed: boolean;
  requiresApproval: boolean;
  blockedReason: string | null;
  rationale: string;
}

export interface DebugCapabilitySnapshot {
  profile: DebugCapabilityProfile;
  runtimeCapabilities: DebugRuntimeCapabilities;
  tools: DebugToolCapability[];
}

export interface DebugBrowserBridgeStatus {
  kind: DebugBrowserBridgeKind;
  builtinBrowserAvailable: boolean;
  devtoolsCatalogInstalled: boolean;
  devtoolsConnected: boolean;
  serverId: string | null;
  serverName: string | null;
  capabilities: string[];
  connectedToolNames: string[];
  recommendedCatalogItemId: string | null;
  notes: string[];
}

export interface DebugModeSession {
  sessionId: string;
  kernelSessionId?: string | null;
  projectPath?: string | null;
  state: DebugState;
  createdAt: string;
  updatedAt: string;
}

export interface DebugExecutionReport {
  caseId: string | null;
  summary: string;
  rootCauseConclusion: string | null;
  fixApplied: boolean;
  verification: VerificationReport | null;
  residualRisks: string[];
}

export interface DebugProgressPayload {
  sessionId: string;
  phase: DebugLifecyclePhase;
  cardType?: string | null;
  message?: string | null;
  data?: Record<string, unknown> | null;
}

export interface DebugArtifactDescriptor {
  path: string;
  fileName: string;
  kind: string;
  contentType: string;
  sizeBytes: number;
  updatedAt: string;
  description: string;
}

export interface DebugArtifactContent {
  artifact: DebugArtifactDescriptor;
  data: number[];
}
