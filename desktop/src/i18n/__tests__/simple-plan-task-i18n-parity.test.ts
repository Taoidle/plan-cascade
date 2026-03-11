import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import enSimpleMode from '../locales/en/simpleMode.json';
import zhSimpleMode from '../locales/zh/simpleMode.json';
import jaSimpleMode from '../locales/ja/simpleMode.json';
import enPlanMode from '../locales/en/planMode.json';
import zhPlanMode from '../locales/zh/planMode.json';
import jaPlanMode from '../locales/ja/planMode.json';
import enDebugMode from '../locales/en/debugMode.json';
import zhDebugMode from '../locales/zh/debugMode.json';
import jaDebugMode from '../locales/ja/debugMode.json';

type LocaleTree = Record<string, unknown>;

function flattenLeafKeys(tree: unknown, prefix = ''): Map<string, string> {
  const result = new Map<string, string>();
  if (!tree || typeof tree !== 'object' || Array.isArray(tree)) return result;
  const entries = Object.entries(tree as Record<string, unknown>);
  for (const [key, value] of entries) {
    const next = prefix ? `${prefix}.${key}` : key;
    if (typeof value === 'string') {
      result.set(next, value);
      continue;
    }
    const nested = flattenLeafKeys(value, next);
    for (const [nestedKey, nestedValue] of nested) {
      result.set(nestedKey, nestedValue);
    }
  }
  return result;
}

function extractPlaceholders(input: string): string[] {
  const placeholders = new Set<string>();
  const pattern = /\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g;
  let match: RegExpExecArray | null = null;
  while (true) {
    match = pattern.exec(input);
    if (!match) break;
    if (match[1]) placeholders.add(match[1]);
  }
  return [...placeholders].sort();
}

function getNestedString(tree: LocaleTree, key: string): string {
  const value = key.split('.').reduce<unknown>((acc, segment) => {
    if (!acc || typeof acc !== 'object' || Array.isArray(acc)) return undefined;
    return (acc as LocaleTree)[segment];
  }, tree);
  return typeof value === 'string' ? value : '';
}

function expectParityForNamespace(params: { namespace: string; en: LocaleTree; zh: LocaleTree; ja: LocaleTree }): void {
  const enMap = flattenLeafKeys(params.en);
  const zhMap = flattenLeafKeys(params.zh);
  const jaMap = flattenLeafKeys(params.ja);

  const keySets = [new Set(enMap.keys()), new Set(zhMap.keys()), new Set(jaMap.keys())];
  const allKeys = new Set<string>();
  for (const set of keySets) {
    for (const key of set) allKeys.add(key);
  }

  for (const key of allKeys) {
    expect(enMap.has(key), `[${params.namespace}] missing key '${key}' in en`).toBe(true);
    expect(zhMap.has(key), `[${params.namespace}] missing key '${key}' in zh`).toBe(true);
    expect(jaMap.has(key), `[${params.namespace}] missing key '${key}' in ja`).toBe(true);
  }

  for (const key of enMap.keys()) {
    const enValue = enMap.get(key) ?? '';
    const zhValue = zhMap.get(key) ?? '';
    const jaValue = jaMap.get(key) ?? '';

    const enVars = extractPlaceholders(enValue);
    const zhVars = extractPlaceholders(zhValue);
    const jaVars = extractPlaceholders(jaValue);

    expect(zhVars, `[${params.namespace}] interpolation mismatch on '${key}' (zh vs en)`).toEqual(enVars);
    expect(jaVars, `[${params.namespace}] interpolation mismatch on '${key}' (ja vs en)`).toEqual(enVars);
  }
}

function collectRawTextNodes(source: string): string[] {
  const matches = source.matchAll(/<[A-Za-z][^>]*>\s*([^<{][^<{]*?)\s*<\/[A-Za-z]/g);
  const values: string[] = [];
  for (const match of matches) {
    const value = (match[1] ?? '').trim();
    if (!value) continue;
    values.push(value);
  }
  return values;
}

function assertNoRawUiStrings(filePath: string): void {
  const content = fs.readFileSync(filePath, 'utf8');
  const rawNodes = collectRawTextNodes(content).filter((value) => {
    if (['↑', '↓', '⇡', '⇣', 'H', 'N', 'L', '&#x2022;'].includes(value)) return false;
    if (/^[#0-9]+$/.test(value)) return false;
    return /[A-Za-z\u4e00-\u9fff\u3040-\u30ff]/.test(value);
  });

  expect(rawNodes, `unexpected raw UI text in ${path.basename(filePath)}`).toEqual([]);
  expect(content, `bare showToast string in ${path.basename(filePath)}`).not.toMatch(/showToast\(\s*['"`]/);
}

describe('simple plan/task i18n parity', () => {
  it('keeps simpleMode locale keys and placeholders aligned', () => {
    expectParityForNamespace({
      namespace: 'simpleMode',
      en: enSimpleMode as LocaleTree,
      zh: zhSimpleMode as LocaleTree,
      ja: jaSimpleMode as LocaleTree,
    });
  });

  it('keeps planMode locale keys and placeholders aligned', () => {
    expectParityForNamespace({
      namespace: 'planMode',
      en: enPlanMode as LocaleTree,
      zh: zhPlanMode as LocaleTree,
      ja: jaPlanMode as LocaleTree,
    });
  });

  it('keeps debugMode locale keys and placeholders aligned', () => {
    expectParityForNamespace({
      namespace: 'debugMode',
      en: enDebugMode as LocaleTree,
      zh: zhDebugMode as LocaleTree,
      ja: jaDebugMode as LocaleTree,
    });
  });

  it('prevents new raw UI strings in queue/plan editor components', () => {
    const currentDir = path.dirname(fileURLToPath(import.meta.url));
    const projectRoot = path.resolve(currentDir, '../../..');
    const guardedFiles = [
      path.join(projectRoot, 'src/components/SimpleMode/SimpleInputComposer.tsx'),
      path.join(projectRoot, 'src/components/SimpleMode/useQueuedChatMessages.ts'),
      path.join(projectRoot, 'src/components/SimpleMode/WorkflowCards/PlanCard.tsx'),
      path.join(projectRoot, 'src/components/SimpleMode/WorkflowCards/ModeHandoffCard.tsx'),
    ];

    for (const guardedFile of guardedFiles) {
      assertNoRawUiStrings(guardedFile);
    }
  });

  it('keeps en/zh/ja translations usable for queue and plan core copy', () => {
    const sampleKeys = [
      { namespace: 'simpleMode', key: 'workflow.queue.clearAll' },
      { namespace: 'simpleMode', key: 'workflow.queue.autoSwitchFailed' },
      { namespace: 'simpleMode', key: 'workflow.modeHandoffCard.title' },
      { namespace: 'simpleMode', key: 'workflow.modeHandoffCard.summaryKinds.task_prd' },
      { namespace: 'planMode', key: 'plan.approveAndExecute' },
      { namespace: 'planMode', key: 'plan.validation.blockTitle' },
      { namespace: 'debugMode', key: 'cards.patchReview.approve' },
      { namespace: 'debugMode', key: 'cards.incidentSummary.defaultTitle' },
    ] as const;

    for (const sample of sampleKeys) {
      const en = getNestedString(
        (sample.namespace === 'simpleMode'
          ? enSimpleMode
          : sample.namespace === 'planMode'
            ? enPlanMode
            : enDebugMode) as LocaleTree,
        sample.key,
      );
      const zh = getNestedString(
        (sample.namespace === 'simpleMode'
          ? zhSimpleMode
          : sample.namespace === 'planMode'
            ? zhPlanMode
            : zhDebugMode) as LocaleTree,
        sample.key,
      );
      const ja = getNestedString(
        (sample.namespace === 'simpleMode'
          ? jaSimpleMode
          : sample.namespace === 'planMode'
            ? jaPlanMode
            : jaDebugMode) as LocaleTree,
        sample.key,
      );
      expect(en.length, `[${sample.namespace}] missing en text for ${sample.key}`).toBeGreaterThan(0);
      expect(zh.length, `[${sample.namespace}] missing zh text for ${sample.key}`).toBeGreaterThan(0);
      expect(ja.length, `[${sample.namespace}] missing ja text for ${sample.key}`).toBeGreaterThan(0);
      expect(zh, `[${sample.namespace}] zh fallback detected for ${sample.key}`).not.toBe(sample.key);
      expect(ja, `[${sample.namespace}] ja fallback detected for ${sample.key}`).not.toBe(sample.key);
    }
  });

  it('prevents hard-coded persona display names in workflow orchestrator', () => {
    const currentDir = path.dirname(fileURLToPath(import.meta.url));
    const projectRoot = path.resolve(currentDir, '../../..');
    const orchestratorPath = path.join(projectRoot, 'src/store/workflowOrchestrator.ts');
    const content = fs.readFileSync(orchestratorPath, 'utf8');

    expect(content).not.toMatch(/displayName:\s*'Business Analyst'/);
    expect(content).not.toMatch(/displayName:\s*'Senior Engineer'/);
    expect(content).not.toMatch(/displayName:\s*'Product Manager'/);
    expect(content).not.toMatch(/displayName:\s*'Software Architect'/);
    expect(content).not.toMatch(/displayName:\s*'Tech Lead'/);
  });

  it('keeps task persona localization keys available in en/zh/ja', () => {
    const personaKeys = [
      'workflow.persona.TechLead',
      'workflow.persona.SeniorEngineer',
      'workflow.persona.BusinessAnalyst',
      'workflow.persona.ProductManager',
      'workflow.persona.SoftwareArchitect',
      'workflow.persona.Developer',
      'workflow.persona.QaEngineer',
    ] as const;

    for (const key of personaKeys) {
      const en = getNestedString(enSimpleMode as LocaleTree, key);
      const zh = getNestedString(zhSimpleMode as LocaleTree, key);
      const ja = getNestedString(jaSimpleMode as LocaleTree, key);
      expect(en.length, `missing en persona key: ${key}`).toBeGreaterThan(0);
      expect(zh.length, `missing zh persona key: ${key}`).toBeGreaterThan(0);
      expect(ja.length, `missing ja persona key: ${key}`).toBeGreaterThan(0);
    }
  });
});
