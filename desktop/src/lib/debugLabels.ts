import i18n from '../i18n';
import type { DebugState } from '../types/debugMode';

type DebugStateLike = Pick<
  DebugState,
  | 'phase'
  | 'environment'
  | 'severity'
  | 'pendingApproval'
  | 'selectedRootCause'
  | 'fixProposal'
  | 'verificationReport'
  | 'symptomSummary'
>;

export function localizeDebugSeverity(severity: string | null | undefined): string | null {
  const normalized = (severity || '').trim().toLowerCase();
  if (!normalized) return null;
  return i18n.t(`simpleMode:sidebar.debug.severity.${normalized}`, { defaultValue: normalized });
}

export function localizeDebugEnvironment(environment: string | null | undefined): string | null {
  const normalized = (environment || '').trim().toLowerCase();
  if (!normalized) return null;
  return i18n.t(`simpleMode:sidebar.debug.environment.${normalized}`, { defaultValue: normalized });
}

export function localizeDebugPhase(phase: string | null | undefined): string | null {
  const normalized = (phase || '').trim().toLowerCase();
  if (!normalized) return null;
  return i18n.t(`simpleMode:sidebar.debug.phase.${normalized}`, { defaultValue: normalized });
}

export function summarizeDebugCase(debug: DebugStateLike | null | undefined, fallback?: string | null): string | null {
  const summary =
    debug?.verificationReport?.summary ||
    debug?.selectedRootCause?.conclusion ||
    debug?.fixProposal?.summary ||
    debug?.symptomSummary ||
    fallback ||
    null;
  const trimmed = (summary || '').trim();
  if (!trimmed) return null;
  return trimmed.length > 120 ? `${trimmed.slice(0, 117)}...` : trimmed;
}

export function buildDebugStateChips(
  debug: DebugStateLike | null | undefined,
  options?: {
    includeEnvironment?: boolean;
    includeSeverity?: boolean;
    includePhase?: boolean;
    includeDerivedStatus?: boolean;
    max?: number;
  },
): string[] {
  if (!debug) return [];

  const includeEnvironment = options?.includeEnvironment ?? true;
  const includeSeverity = options?.includeSeverity ?? true;
  const includePhase = options?.includePhase ?? true;
  const includeDerivedStatus = options?.includeDerivedStatus ?? true;
  const max = options?.max ?? 6;

  const chips = [
    includeEnvironment ? localizeDebugEnvironment(debug.environment) : null,
    includeSeverity ? localizeDebugSeverity(debug.severity) : null,
    includePhase ? localizeDebugPhase(debug.phase) : null,
    includeDerivedStatus && debug.pendingApproval
      ? i18n.t('simpleMode:sidebar.debug.pendingApproval', { defaultValue: 'Approval' })
      : null,
    includeDerivedStatus && debug.selectedRootCause
      ? i18n.t('simpleMode:sidebar.debug.rootCauseReady', { defaultValue: 'Root Cause' })
      : null,
    includeDerivedStatus && debug.verificationReport
      ? i18n.t('simpleMode:sidebar.debug.verified', { defaultValue: 'Verified' })
      : null,
  ].filter((value): value is string => Boolean(value));

  return Array.from(new Set(chips)).slice(0, max);
}
