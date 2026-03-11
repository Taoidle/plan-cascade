import { create } from 'zustand';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import i18n from '../i18n';
import { collectPreferredBrowserEvidence, getDebugBrowserBridgeStatus } from '../lib/browserDebugApi';
import { writeDebugArtifact } from '../lib/debugArtifactsApi';
import { useDebugModeStore } from './debugMode';
import { useWorkflowKernelStore } from './workflowKernel';
import { useContextSourcesStore } from './contextSources';
import { selectKernelDebugRuntime } from './workflowKernelSelectors';
import { isDebugPhaseBusy } from './workflowPhaseModel';
import { okResult, failResult, type ActionResult } from '../types/actionResult';
import type {
  DebugCapabilitySnapshot,
  DebugExecutionReport,
  DebugModeSession,
  DebugProgressPayload,
  DebugToolCategory,
} from '../types/debugMode';
import type {
  BrowserRuntimeCardData,
  ConsoleErrorCardData,
  DebugIntakeCardData,
  FixCandidateCardData,
  IncidentSummaryCardData,
  NetworkTraceCardData,
  PerformanceTraceCardData,
  PatchReviewCardData,
  ReproductionStatusCardData,
  RootCauseCardData,
  SignalSummaryCardData,
  SourceMappingCardData,
  VerificationCardData,
} from '../types/workflowCard';
import { injectDebugCard } from './debugOrchestrator/cardInjection';

interface DebugOrchestratorState {
  sessionId: string | null;
  phase: string;
  taskDescription: string;
  report: DebugExecutionReport | null;
  isBusy: boolean;
  isCancelling: boolean;
  _progressUnlisten: UnlistenFn | null;
  capabilitySnapshot: DebugCapabilitySnapshot | null;

  startDebugWorkflow: (
    description: string,
    kernelSessionId?: string | null,
  ) => Promise<{ modeSessionId: string | null }>;
  submitClarification: (answer: string) => Promise<{ ok: boolean; errorCode?: string | null }>;
  approvePatch: () => Promise<ActionResult>;
  rejectPatch: (reason?: string) => Promise<ActionResult>;
  collectBrowserEvidence: (stage?: 'baseline' | 'verification') => Promise<ActionResult>;
  refreshAnalysis: () => Promise<ActionResult>;
  runVerification: () => Promise<ActionResult>;
  cancelWorkflow: () => Promise<void>;
  ensureTerminalSummaryCardFromKernel: () => Promise<void>;
  resetWorkflow: () => void;
}

function buildDebugContextSources(sessionId?: string | null) {
  const contextSourcesStore = useContextSourcesStore.getState();
  contextSourcesStore.setMemorySessionId(sessionId?.trim() || null);
  return contextSourcesStore.buildConfig();
}

function allowedToolCategories(
  snapshot: DebugCapabilitySnapshot | null,
  session: DebugModeSession,
): DebugToolCategory[] {
  if (snapshot) {
    return Array.from(
      new Set(
        snapshot.tools
          .filter((tool) => tool.allowed && tool.toolCategory)
          .map((tool) => tool.toolCategory as DebugToolCategory),
      ),
    ).slice(0, 6) as DebugToolCategory[];
  }

  return session.state.capabilityProfile === 'prod_observe_only'
    ? ['debug:logs', 'debug:metrics', 'debug:trace']
    : ['debug:logs', 'debug:browser', 'debug:test_runner'];
}

function injectBootstrapCards(session: DebugModeSession, capabilitySnapshot: DebugCapabilitySnapshot | null) {
  const state = session.state;
  injectDebugCard('debug_intake_card', {
    title: state.title || i18n.t('debugMode:intake.defaultTitle', { defaultValue: 'New debug case' }),
    symptomSummary: state.symptomSummary,
    expectedBehavior: state.expectedBehavior || '',
    actualBehavior: state.actualBehavior || '',
    reproSteps: state.reproSteps,
    environment: state.environment,
    severity: state.severity,
    affectedSurface: state.affectedSurface,
    recentChanges: state.recentChanges,
    targetUrlOrEntry: state.targetUrlOrEntry,
  } satisfies DebugIntakeCardData);

  injectDebugCard('signal_summary_card', {
    summary: state.symptomSummary,
    environment: state.environment,
    severity: state.severity,
    toolCategories: allowedToolCategories(capabilitySnapshot, session),
    evidenceCount: state.evidenceRefs.length,
    highlights: state.affectedSurface,
  } satisfies SignalSummaryCardData);
}

function summarizeConsoleEntries(
  entries: Array<{ level: string; args: string[]; timestamp?: string | null }>,
): Array<{ level: string; message: string; timestamp?: string | null }> {
  return entries
    .slice(-6)
    .map((entry) => ({
      level: entry.level || 'log',
      message: entry.args.join(' '),
      timestamp: entry.timestamp ?? null,
    }))
    .filter((entry) => entry.message.trim().length > 0);
}

function summarizeNetworkHighlights(
  events: Array<{
    method?: string | null;
    url?: string | null;
    status?: number | null;
    ok?: boolean | null;
    error?: string | null;
  }>,
): string[] {
  return events
    .filter((event) => event.ok === false || (typeof event.status === 'number' && event.status >= 400) || event.error)
    .slice(-5)
    .map((event) => {
      const parts = [
        event.method || 'GET',
        event.url || '(unknown url)',
        typeof event.status === 'number' ? String(event.status) : null,
        event.error || null,
      ].filter(Boolean);
      return parts.join(' · ');
    });
}

function extractSourceMappingHints(
  consoleEntries: Array<{ message: string }>,
  targetUrl: string | null,
  extraHints: string[] = [],
): string[] {
  const hints = new Set<string>();
  const patterns = [/https?:\/\/[^\s)]+/g, /\b[\w./-]+\.(?:js|jsx|ts|tsx|vue)(?::\d+(?::\d+)?)?/g];
  for (const entry of consoleEntries) {
    for (const pattern of patterns) {
      const matches = entry.message.match(pattern) ?? [];
      for (const match of matches) {
        hints.add(match);
      }
    }
  }
  if (targetUrl) {
    hints.add(targetUrl);
  }
  for (const extraHint of extraHints) {
    if (!extraHint) continue;
    for (const pattern of patterns) {
      const matches = extraHint.match(pattern) ?? [];
      for (const match of matches) {
        hints.add(match);
      }
    }
  }
  return Array.from(hints).slice(0, 6);
}

async function injectBrowserEvidenceCards(
  session: DebugModeSession,
  stage: 'baseline' | 'verification' = 'baseline',
): Promise<void> {
  const bridgeStatus = await getDebugBrowserBridgeStatus();
  const targetUrl = session.state.targetUrlOrEntry;

  injectDebugCard('browser_runtime_card', {
    bridgeKind: bridgeStatus.kind,
    targetUrl,
    serverName: bridgeStatus.serverName,
    capabilities: bridgeStatus.capabilities,
    notes: bridgeStatus.notes,
    recommendedCatalogItemId: bridgeStatus.recommendedCatalogItemId,
  } satisfies BrowserRuntimeCardData);

  if (!targetUrl) {
    injectDebugCard('reproduction_status_card', {
      status: 'pending',
      summary: i18n.t('debugMode:cards.reproduction.missingUrl', {
        defaultValue: 'Provide a page URL to let Debug capture browser evidence automatically.',
      }),
      reproductionSteps: session.state.reproSteps,
      browserArtifacts: [],
    } satisfies ReproductionStatusCardData);
    return;
  }

  if (!bridgeStatus.builtinBrowserAvailable && !bridgeStatus.devtoolsConnected) {
    injectDebugCard('reproduction_status_card', {
      status: 'failed',
      summary: i18n.t('debugMode:cards.reproduction.bridgeOnly', {
        defaultValue:
          'No browser bridge is ready for automatic capture on this machine. Connect Chrome DevTools MCP or enable the built-in Browser tool.',
      }),
      reproductionSteps: session.state.reproSteps,
      browserArtifacts: bridgeStatus.capabilities,
    } satisfies ReproductionStatusCardData);
    return;
  }

  const evidence = await collectPreferredBrowserEvidence(targetUrl, bridgeStatus);
  if (!evidence) {
    injectDebugCard('reproduction_status_card', {
      status: 'failed',
      summary: i18n.t('debugMode:cards.reproduction.captureFailed', {
        defaultValue: 'Automatic browser evidence capture failed. Check the page URL and browser availability.',
      }),
      reproductionSteps: session.state.reproSteps,
      browserArtifacts: [],
    } satisfies ReproductionStatusCardData);
    return;
  }

  const consoleEntries = summarizeConsoleEntries(evidence.consoleEntries);
  const networkHighlights = summarizeNetworkHighlights(evidence.networkEvents);
  const sourceHints = extractSourceMappingHints(consoleEntries, evidence.currentUrl, [
    JSON.stringify(evidence.domSnapshot ?? {}),
    evidence.pageTitle ?? '',
    ...evidence.scriptUrls,
    ...evidence.sourceMapUrls,
    ...evidence.resolvedSourceFiles,
  ]);
  const collectedAt = new Date().toISOString();
  const captureLabel =
    evidence.captureSource === 'devtools_mcp'
      ? i18n.t('debugMode:bridgeKinds.devtools_mcp', { defaultValue: 'Chrome DevTools MCP' })
      : i18n.t('debugMode:bridgeKinds.builtin_browser', { defaultValue: 'Built-in Browser tool' });

  injectDebugCard('reproduction_status_card', {
    status: consoleEntries.length > 0 || networkHighlights.length > 0 ? 'confirmed' : 'partial',
    summary: i18n.t('debugMode:cards.reproduction.captured', {
      defaultValue: 'Browser evidence was captured from the target page.',
    }),
    reproductionSteps: session.state.reproSteps,
    browserArtifacts: [
      captureLabel,
      evidence.pageTitle || '',
      evidence.currentUrl || targetUrl,
      `${evidence.consoleEntries.length} console entries`,
      `${evidence.networkEvents.length} network events`,
      evidence.screenshotCaptured
        ? i18n.t('debugMode:cards.reproduction.screenshotCaptured', {
            defaultValue: 'Screenshot captured',
          })
        : '',
    ].filter(Boolean),
  } satisfies ReproductionStatusCardData);

  const browserArtifactPaths: Partial<Record<'network' | 'source_mapping' | 'performance', string>> = {};
  const writeBrowserArtifact = async (
    kind: 'network' | 'source_mapping' | 'performance',
    payload: Record<string, unknown>,
  ) => {
    const fileName = `${kind}-${stage}-${new Date().toISOString().replace(/[:.]/g, '-')}.json`;
    const result = await writeDebugArtifact(session.sessionId, fileName, JSON.stringify(payload, null, 2));
    if (result.success && result.data) {
      browserArtifactPaths[kind] = result.data.path;
    }
  };

  if (consoleEntries.length > 0) {
    injectDebugCard('console_error_card', {
      currentUrl: evidence.currentUrl,
      collectedAt,
      entries: consoleEntries,
    } satisfies ConsoleErrorCardData);
    await useDebugModeStore
      .getState()
      .attachEvidence(
        'Browser console',
        consoleEntries.map((entry) => `${entry.level}: ${entry.message}`).join('\n'),
        `${evidence.captureSource}:console:${stage}`,
        session.sessionId,
        {
          stage,
          captureSource: evidence.captureSource,
          currentUrl: evidence.currentUrl,
          entryCount: consoleEntries.length,
          blockingEntryCount: consoleEntries.filter((entry) => ['error', 'warn'].includes(entry.level.toLowerCase()))
            .length,
        },
      );
  }

  if (networkHighlights.length > 0 || evidence.networkEvents.length > 0) {
    await writeBrowserArtifact('network', {
      stage,
      collectedAt,
      captureSource: evidence.captureSource,
      currentUrl: evidence.currentUrl,
      harSummary: evidence.harSummary,
      events: evidence.networkEvents,
    });
    injectDebugCard('network_trace_card', {
      currentUrl: evidence.currentUrl,
      collectedAt,
      totalEvents: evidence.networkEvents.length,
      failedEvents: evidence.networkEvents.filter(
        (event) => event.ok === false || (typeof event.status === 'number' && event.status >= 400) || !!event.error,
      ).length,
      highlights: networkHighlights,
      slowestRequests: evidence.harSummary?.slowestRequests ?? [],
      harCaptured: !!evidence.harSummary,
    } satisfies NetworkTraceCardData);
    await useDebugModeStore
      .getState()
      .attachEvidence(
        'Browser network',
        networkHighlights.join('\n') || `${evidence.networkEvents.length} network events captured`,
        `${evidence.captureSource}:network:${stage}`,
        session.sessionId,
        {
          stage,
          captureSource: evidence.captureSource,
          currentUrl: evidence.currentUrl,
          totalEventCount: evidence.networkEvents.length,
          failedEventCount: evidence.networkEvents.filter(
            (event) => event.ok === false || (typeof event.status === 'number' && event.status >= 400) || !!event.error,
          ).length,
          artifactPath: browserArtifactPaths.network ?? null,
        },
      );
  }

  if (sourceHints.length > 0) {
    await writeBrowserArtifact('source_mapping', {
      stage,
      collectedAt,
      captureSource: evidence.captureSource,
      currentUrl: evidence.currentUrl,
      candidateFiles: sourceHints,
      bundleScripts: evidence.scriptUrls,
      sourceMapUrls: evidence.sourceMapUrls,
      resolvedSources: evidence.resolvedSourceFiles,
      stackFrames: evidence.stackFrames,
      matchedSourceMaps: evidence.matchedSourceMapUrls,
      originalPositionHints: evidence.originalPositionHints,
    });
    injectDebugCard('source_mapping_card', {
      source: evidence.captureSource,
      summary: i18n.t('debugMode:cards.sourceMapping.summary', {
        defaultValue:
          'These browser-side source hints can help trace the failure back to the frontend bundle or route.',
      }),
      candidateFiles: sourceHints,
      bundleScripts: evidence.scriptUrls,
      sourceMapUrls: evidence.sourceMapUrls,
      resolvedSources: evidence.resolvedSourceFiles,
      stackFrames: evidence.stackFrames.map(
        (frame) => `${frame.url}${frame.line ? `:${frame.line}` : ''}${frame.column ? `:${frame.column}` : ''}`,
      ),
      matchedSourceMaps: evidence.matchedSourceMapUrls,
      originalPositionHints: evidence.originalPositionHints,
    } satisfies SourceMappingCardData);
    await useDebugModeStore
      .getState()
      .attachEvidence(
        'Browser source mapping',
        sourceHints.join('\n'),
        `${evidence.captureSource}:source_mapping:${stage}`,
        session.sessionId,
        {
          stage,
          captureSource: evidence.captureSource,
          currentUrl: evidence.currentUrl,
          candidateFileCount: sourceHints.length,
          candidateFiles: sourceHints,
          sourceMapCount: evidence.sourceMapUrls.length,
          sourceMapUrls: evidence.sourceMapUrls,
          bundleScriptCount: evidence.scriptUrls.length,
          bundleScripts: evidence.scriptUrls,
          resolvedSources: evidence.resolvedSourceFiles,
          stackFrames: evidence.stackFrames.map((frame) => ({
            url: frame.url,
            line: frame.line,
            column: frame.column,
            raw: frame.raw,
          })),
          matchedSourceMaps: evidence.matchedSourceMapUrls,
          originalPositionHints: evidence.originalPositionHints,
          artifactPath: browserArtifactPaths.source_mapping ?? null,
        },
      );
  }

  if (evidence.performanceSummary) {
    await writeBrowserArtifact('performance', {
      stage,
      collectedAt,
      captureSource: evidence.captureSource,
      currentUrl: evidence.currentUrl,
      summary: evidence.performanceSummary,
      entries: evidence.performanceEntries,
    });
    injectDebugCard('performance_trace_card', {
      currentUrl: evidence.currentUrl,
      collectedAt,
      summary: evidence.performanceSummary.summary,
      metrics: evidence.performanceSummary.metrics,
      longTasks: evidence.performanceSummary.longTasks,
    } satisfies PerformanceTraceCardData);
    await useDebugModeStore
      .getState()
      .attachEvidence(
        'Browser performance',
        evidence.performanceSummary.summary,
        `${evidence.captureSource}:performance:${stage}`,
        session.sessionId,
        {
          stage,
          captureSource: evidence.captureSource,
          currentUrl: evidence.currentUrl,
          metricCount: evidence.performanceSummary.metrics.length,
          longTaskCount: evidence.performanceSummary.longTasks.length,
          artifactPath: browserArtifactPaths.performance ?? null,
        },
      );
  }

  const refreshedSession = await useDebugModeStore.getState().getSessionSnapshot(session.sessionId);
  if (refreshedSession) {
    injectStateCards(refreshedSession);
  }
}

function injectStateCards(session: DebugModeSession) {
  const state = session.state;
  if (state.activeHypotheses.length > 0) {
    injectDebugCard(
      'hypothesis_card',
      {
        hypotheses: state.activeHypotheses,
        recommendedNextChecks: Array.from(
          new Set(state.activeHypotheses.flatMap((hypothesis) => hypothesis.nextChecks)),
        ).slice(0, 6),
      },
      false,
    );
  }
  if (state.selectedRootCause) {
    injectDebugCard('root_cause_card', state.selectedRootCause satisfies RootCauseCardData);
  }
  if (state.fixProposal) {
    injectDebugCard(
      state.pendingApproval ? 'patch_review_card' : 'fix_candidate_card',
      state.pendingApproval
        ? ({
            title: i18n.t('debugMode:cards.patchReview.title', { defaultValue: 'Patch review required' }),
            summary: state.fixProposal.summary,
            riskLevel: state.fixProposal.riskLevel,
            filesOrSystemsTouched: state.fixProposal.filesOrSystemsTouched,
            verificationPlan: state.fixProposal.verificationPlan,
            patchPreviewRef: state.fixProposal.patchPreviewRef,
            patchOperations: state.fixProposal.patchOperations,
            requiredCapabilityClass: 'mutate',
            approvalDescription: state.pendingApproval.description,
          } satisfies PatchReviewCardData)
        : ({
            ...state.fixProposal,
            requiresApproval: false,
          } satisfies FixCandidateCardData),
      !!state.pendingApproval,
    );
  }
  if (state.verificationReport) {
    injectDebugCard('verification_card', state.verificationReport satisfies VerificationCardData);
  }
}

export const useDebugOrchestratorStore = create<DebugOrchestratorState>((set, get) => ({
  sessionId: null,
  phase: 'intaking',
  taskDescription: '',
  report: null,
  capabilitySnapshot: null,
  isBusy: false,
  isCancelling: false,
  _progressUnlisten: null,

  startDebugWorkflow: async (description, kernelSessionId) => {
    const settings = (await import('./settings')).useSettingsStore.getState();
    const session = await useDebugModeStore
      .getState()
      .enterDebugMode(
        description,
        settings.debugDefaultEnvironment,
        undefined,
        undefined,
        undefined,
        settings.workspacePath || undefined,
        buildDebugContextSources(kernelSessionId),
        i18n.language,
        kernelSessionId,
      );
    if (!session) {
      set({ isBusy: false });
      return { modeSessionId: null };
    }
    const capabilitySnapshot = await useDebugModeStore.getState().getCapabilitySnapshot(session.sessionId);

    const progressUnlisten = await listen<DebugProgressPayload>('debug-progress', (event) => {
      const payload = event.payload;
      if (payload.sessionId !== session.sessionId) return;
      set({ phase: payload.phase, isBusy: isDebugPhaseBusy(payload.phase) });
    });

    injectBootstrapCards(session, capabilitySnapshot);
    injectStateCards(session);
    void injectBrowserEvidenceCards(session);
    set({
      sessionId: session.sessionId,
      phase: session.state.phase,
      taskDescription: description,
      capabilitySnapshot,
      isBusy: isDebugPhaseBusy(session.state.phase),
      _progressUnlisten: progressUnlisten,
    });

    return { modeSessionId: session.sessionId };
  },

  submitClarification: async (answer) => {
    const sessionId = get().sessionId;
    const session = await useDebugModeStore
      .getState()
      .submitClarification(
        answer,
        undefined,
        undefined,
        undefined,
        undefined,
        buildDebugContextSources(sessionId),
        i18n.language,
        sessionId,
      );
    if (!session) {
      return { ok: false, errorCode: 'submit_failed' };
    }
    injectStateCards(session);
    set({ phase: session.state.phase });
    return { ok: true };
  },

  approvePatch: async () => {
    const sessionId = get().sessionId;
    const settings = (await import('./settings')).useSettingsStore.getState();
    const session = await useDebugModeStore
      .getState()
      .approvePatch(
        undefined,
        undefined,
        undefined,
        settings.workspacePath || undefined,
        buildDebugContextSources(sessionId),
        i18n.language,
        sessionId,
      );
    if (!session) {
      return failResult('debug_patch_approval_failed', 'Failed to approve debug patch');
    }
    injectStateCards(session);
    set({ phase: session.state.phase });
    return okResult();
  },

  rejectPatch: async (reason = 'rejected_by_user') => {
    const session = await useDebugModeStore.getState().rejectPatch(reason, get().sessionId);
    if (!session) {
      return failResult('debug_patch_rejection_failed', 'Failed to reject debug patch');
    }
    set({ phase: session.state.phase });
    return okResult();
  },

  collectBrowserEvidence: async (stage = 'baseline') => {
    const sessionId = get().sessionId;
    if (!sessionId) {
      return failResult('debug_browser_evidence_missing_session', 'No active debug session');
    }
    const session = await useDebugModeStore.getState().getSessionSnapshot(sessionId);
    if (!session) {
      return failResult('debug_browser_evidence_missing_snapshot', 'Failed to load debug session');
    }
    if (!session.state.targetUrlOrEntry) {
      return failResult('debug_browser_evidence_missing_url', 'No target URL configured for browser evidence');
    }
    await injectBrowserEvidenceCards(session, stage);
    return okResult();
  },

  refreshAnalysis: async () => {
    const sessionId = get().sessionId;
    if (!sessionId) {
      return failResult('debug_refresh_analysis_missing_session', 'No active debug session');
    }
    const session = await useDebugModeStore.getState().retryPhase('gathering_signal', sessionId);
    if (!session) {
      return failResult('debug_refresh_analysis_failed', 'Failed to refresh debug analysis');
    }
    injectStateCards(session);
    set({ phase: session.state.phase });
    return okResult();
  },

  runVerification: async () => {
    const sessionId = get().sessionId;
    if (!sessionId) {
      return failResult('debug_run_verification_missing_session', 'No active debug session');
    }
    const sessionSnapshot = await useDebugModeStore.getState().getSessionSnapshot(sessionId);
    if (sessionSnapshot?.state.targetUrlOrEntry) {
      await injectBrowserEvidenceCards(sessionSnapshot, 'verification');
    }
    const session = await useDebugModeStore.getState().retryPhase('verifying', sessionId);
    if (!session) {
      return failResult('debug_run_verification_failed', 'Failed to run debug verification');
    }
    injectStateCards(session);
    set({ phase: session.state.phase });
    return okResult();
  },

  cancelWorkflow: async () => {
    const sessionId = get().sessionId;
    if (!sessionId) return;
    set({ isCancelling: true });
    await useDebugModeStore.getState().cancelOperation(sessionId);
    set({ isCancelling: false });
  },

  ensureTerminalSummaryCardFromKernel: async () => {
    const session = useWorkflowKernelStore.getState().session;
    const runtime = selectKernelDebugRuntime(session);
    if (!runtime.linkedSessionId) return;
    const report = await useDebugModeStore.getState().fetchReport(runtime.linkedSessionId);
    if (!report) return;
    injectDebugCard('incident_summary_card', {
      title:
        report.caseId || i18n.t('debugMode:cards.incidentSummary.defaultTitle', { defaultValue: 'Debug case summary' }),
      environment: session?.modeSnapshots.debug?.environment ?? 'dev',
      severity: session?.modeSnapshots.debug?.severity ?? 'medium',
      summary: report.summary,
      rootCauseConclusion: report.rootCauseConclusion,
      fixApplied: report.fixApplied,
      verificationSummary: report.verification?.summary ?? null,
      residualRisks: report.residualRisks,
    } satisfies IncidentSummaryCardData);
    set({ report, isBusy: false });
  },

  resetWorkflow: () => {
    get()._progressUnlisten?.();
    set({
      sessionId: null,
      phase: 'intaking',
      taskDescription: '',
      report: null,
      capabilitySnapshot: null,
      isBusy: false,
      isCancelling: false,
      _progressUnlisten: null,
    });
  },
}));
