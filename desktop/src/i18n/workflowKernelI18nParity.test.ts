import { describe, expect, it } from 'vitest';
import type { WorkflowEventKind } from '../types/workflowKernel';
import enSimpleMode from './locales/en/simpleMode.json';
import zhSimpleMode from './locales/zh/simpleMode.json';
import jaSimpleMode from './locales/ja/simpleMode.json';

const EVENT_KIND_KEYS: Record<WorkflowEventKind, true> = {
  session_opened: true,
  mode_transitioned: true,
  mode_session_linked: true,
  input_submitted: true,
  context_appended: true,
  plan_edited: true,
  plan_execution_started: true,
  plan_step_retried: true,
  operation_cancelled: true,
  session_recovered: true,
  checkpoint_created: true,
  quality_run_started: true,
  quality_gate_updated: true,
  quality_run_completed: true,
  quality_decision_required: true,
  quality_decision_applied: true,
};

const EVENT_KINDS = Object.keys(EVENT_KIND_KEYS) as WorkflowEventKind[];
const REQUIRED_REASON_CODES = [
  ...EVENT_KINDS,
  'cancelled_by_user',
  'mode_transitioned_with_input',
  'mode_start_failed',
  'mode_session_link_failed',
  'interrupted_by_restart',
  'unknown_reason',
] as const;

function asRecord(value: unknown): Record<string, unknown> {
  if (value && typeof value === 'object') {
    return value as Record<string, unknown>;
  }
  return {};
}

function getKernelTree(localeTree: unknown): Record<string, unknown> {
  const workflow = asRecord(localeTree).workflow;
  const progress = asRecord(workflow).progress;
  return asRecord(asRecord(progress).kernel);
}

function expectLocalizedValue(localeName: string, keyPath: string, leafKey: string, value: unknown): void {
  expect(typeof value, `${localeName} missing ${keyPath}.${leafKey}`).toBe('string');
  expect((value as string).trim().length, `${localeName} has empty ${keyPath}.${leafKey}`).toBeGreaterThan(0);
}

describe('workflow kernel i18n parity', () => {
  const locales: Array<{ name: string; tree: unknown }> = [
    { name: 'en', tree: enSimpleMode },
    { name: 'zh', tree: zhSimpleMode },
    { name: 'ja', tree: jaSimpleMode },
  ];

  it('contains eventKind labels for all workflow event kinds', () => {
    for (const locale of locales) {
      const kernel = getKernelTree(locale.tree);
      const eventKind = asRecord(kernel.eventKind);
      for (const kind of EVENT_KINDS) {
        expectLocalizedValue(locale.name, 'eventKind', kind, eventKind[kind]);
      }
    }
  });

  it('contains reasonCode labels for required reason codes', () => {
    for (const locale of locales) {
      const kernel = getKernelTree(locale.tree);
      const reasonCode = asRecord(kernel.reasonCode);
      for (const code of REQUIRED_REASON_CODES) {
        expectLocalizedValue(locale.name, 'reasonCode', code, reasonCode[code]);
      }
    }
  });
});
