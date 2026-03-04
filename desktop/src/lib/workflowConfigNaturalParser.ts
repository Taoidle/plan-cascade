import type { WorkflowConfig } from '../types/workflowCard';

export interface WorkflowConfigNaturalParseResult {
  updates: Partial<WorkflowConfig>;
  matched: string[];
  unmatched: string[];
}

interface LocaleLexicon {
  parallelKeywords: string[];
  agentKeywords: string[];
  tddKeywords: string[];
  tddStrictKeywords: string[];
  disableKeywords: string[];
  enableKeywords: string[];
  flowQuickKeywords: string[];
  flowFullKeywords: string[];
  flowStandardKeywords: string[];
  qualityKeywords: string[];
  interviewKeywords: string[];
}

const EN_LEXICON: LocaleLexicon = {
  parallelKeywords: ['parallel', 'concurrent', 'parallelism'],
  agentKeywords: ['agent', 'agents', 'worker', 'workers'],
  tddKeywords: ['tdd', 'test-driven', 'test driven'],
  tddStrictKeywords: ['strict', 'strict mode', 'strictly'],
  disableKeywords: ['off', 'disable', 'disabled', 'without', 'skip', 'no', "don't", 'do not'],
  enableKeywords: ['enable', 'enabled', 'on', 'with', 'use'],
  flowQuickKeywords: ['quick', 'fast', 'rapid', 'lean'],
  flowFullKeywords: ['full', 'thorough', 'complete', 'comprehensive'],
  flowStandardKeywords: ['standard', 'default', 'normal'],
  qualityKeywords: ['quality', 'quality gate', 'quality gates'],
  interviewKeywords: ['interview', 'spec interview', 'requirements interview'],
};

const ZH_LEXICON: LocaleLexicon = {
  parallelKeywords: ['并行', '並行', '并发', '並發'],
  agentKeywords: ['代理', '智能体', '智能體', 'agent'],
  tddKeywords: ['tdd', '测试驱动', '測試驅動'],
  tddStrictKeywords: ['严格', '嚴格'],
  disableKeywords: ['关闭', '關閉', '禁用', '不开启', '不启用', '不要', '跳过', '不需要'],
  enableKeywords: ['开启', '開啟', '启用', '啟用', '打开', '打開'],
  flowQuickKeywords: ['快速', '极速', '迅速'],
  flowFullKeywords: ['完整', '全面', '彻底', '徹底'],
  flowStandardKeywords: ['标准', '標準', '默认', '默認', '普通'],
  qualityKeywords: ['质量', '品質', '质量门禁', '品質ゲート'],
  interviewKeywords: ['访谈', '訪談', '规格访谈', '規格訪談', '需求访谈', '需求訪談'],
};

const JA_LEXICON: LocaleLexicon = {
  parallelKeywords: ['並列', '並行', '同時'],
  agentKeywords: ['エージェント', 'agent', 'ワーカー'],
  tddKeywords: ['tdd', 'テスト駆動'],
  tddStrictKeywords: ['厳格', 'strict'],
  disableKeywords: ['無効', 'オフ', 'しない', 'なし', 'スキップ'],
  enableKeywords: ['有効', 'オン', '使う', '使用'],
  flowQuickKeywords: ['クイック', '高速', '短縮'],
  flowFullKeywords: ['フル', '完全', '徹底'],
  flowStandardKeywords: ['標準', '通常', 'デフォルト'],
  qualityKeywords: ['品質', '品質ゲート'],
  interviewKeywords: ['インタビュー', '要件インタビュー', '仕様インタビュー'],
};

function unique(values: string[]): string[] {
  return [...new Set(values)];
}

function mergeLexicons(...lexicons: LocaleLexicon[]): LocaleLexicon {
  return {
    parallelKeywords: unique(lexicons.flatMap((item) => item.parallelKeywords)),
    agentKeywords: unique(lexicons.flatMap((item) => item.agentKeywords)),
    tddKeywords: unique(lexicons.flatMap((item) => item.tddKeywords)),
    tddStrictKeywords: unique(lexicons.flatMap((item) => item.tddStrictKeywords)),
    disableKeywords: unique(lexicons.flatMap((item) => item.disableKeywords)),
    enableKeywords: unique(lexicons.flatMap((item) => item.enableKeywords)),
    flowQuickKeywords: unique(lexicons.flatMap((item) => item.flowQuickKeywords)),
    flowFullKeywords: unique(lexicons.flatMap((item) => item.flowFullKeywords)),
    flowStandardKeywords: unique(lexicons.flatMap((item) => item.flowStandardKeywords)),
    qualityKeywords: unique(lexicons.flatMap((item) => item.qualityKeywords)),
    interviewKeywords: unique(lexicons.flatMap((item) => item.interviewKeywords)),
  };
}

function resolveLexicon(locale?: string): LocaleLexicon {
  const normalized = (locale ?? 'en').toLowerCase();
  if (normalized.startsWith('zh')) {
    return mergeLexicons(EN_LEXICON, ZH_LEXICON);
  }
  if (normalized.startsWith('ja')) {
    return mergeLexicons(EN_LEXICON, JA_LEXICON);
  }
  return EN_LEXICON;
}

function hasAny(text: string, keywords: string[]): boolean {
  return keywords.some((keyword) => text.includes(keyword));
}

function indexOfAny(text: string, keywords: string[]): number {
  let min = Number.POSITIVE_INFINITY;
  for (const keyword of keywords) {
    const index = text.indexOf(keyword);
    if (index >= 0 && index < min) {
      min = index;
    }
  }
  return Number.isFinite(min) ? min : -1;
}

function escapeRegex(keyword: string): string {
  return keyword.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function toAlternation(keywords: string[]): string {
  return keywords
    .filter((keyword) => keyword.trim().length > 0)
    .sort((left, right) => right.length - left.length)
    .map((keyword) => escapeRegex(keyword))
    .join('|');
}

function inferDisableEnable(text: string, lexicon: LocaleLexicon): 'enable' | 'disable' | null {
  const disableIdx = indexOfAny(text, lexicon.disableKeywords);
  const enableIdx = indexOfAny(text, lexicon.enableKeywords);

  if (disableIdx === -1 && enableIdx === -1) return null;
  if (disableIdx === -1) return 'enable';
  if (enableIdx === -1) return 'disable';

  return disableIdx <= enableIdx ? 'disable' : 'enable';
}

function clampParallel(value: number): number {
  return Math.min(Math.max(value, 1), 16);
}

function parseParallel(text: string, lexicon: LocaleLexicon): number | null {
  if (!hasAny(text, lexicon.parallelKeywords)) return null;

  const parallelPattern = toAlternation(lexicon.parallelKeywords);
  const agentPattern = toAlternation(lexicon.agentKeywords);
  const numberThenParallel = new RegExp(
    `(\\d{1,2})\\s*(?:个|個|つ|名|台|人|x)?\\s*(?:${parallelPattern})(?:\\s*(?:${agentPattern}))?`,
    'iu',
  );
  const parallelThenNumber = new RegExp(`(?:${parallelPattern})(?:\\s*(?:${agentPattern}))?\\s*(\\d{1,2})`, 'iu');
  const agentThenNumber = new RegExp(`(?:${agentPattern})\\s*(\\d{1,2})\\s*(?:${parallelPattern})`, 'iu');

  const match = text.match(numberThenParallel) ?? text.match(parallelThenNumber) ?? text.match(agentThenNumber);
  if (!match?.[1]) return null;
  return clampParallel(Number.parseInt(match[1], 10));
}

function splitClauses(text: string): string[] {
  const separators = /(?:\b(?:and|with|plus)\b|并且|同时|且|以及|そして|かつ|および|[，,。；;、\n])+/giu;
  return text
    .split(separators)
    .map((segment) => segment.trim())
    .filter((segment) => segment.length > 0);
}

function applyFlowLevel(text: string, lexicon: LocaleLexicon): WorkflowConfig['flowLevel'] | null {
  const quickIdx = indexOfAny(text, lexicon.flowQuickKeywords);
  const fullIdx = indexOfAny(text, lexicon.flowFullKeywords);
  const standardIdx = indexOfAny(text, lexicon.flowStandardKeywords);
  const hits = [
    { mode: 'quick' as const, idx: quickIdx },
    { mode: 'full' as const, idx: fullIdx },
    { mode: 'standard' as const, idx: standardIdx },
  ]
    .filter((item) => item.idx >= 0)
    .sort((left, right) => left.idx - right.idx);

  return hits.length > 0 ? hits[0].mode : null;
}

function parseClause(clause: string, lexicon: LocaleLexicon): WorkflowConfigNaturalParseResult {
  const updates: Partial<WorkflowConfig> = {};
  const matched: string[] = [];

  const maxParallel = parseParallel(clause, lexicon);
  if (maxParallel !== null) {
    updates.maxParallel = maxParallel;
    matched.push(`maxParallel=${maxParallel}`);
  }

  if (hasAny(clause, lexicon.tddKeywords)) {
    if (hasAny(clause, lexicon.tddStrictKeywords)) {
      updates.tddMode = 'strict';
      matched.push('tddMode=strict');
    } else {
      const toggle = inferDisableEnable(clause, lexicon);
      if (toggle === 'disable') {
        updates.tddMode = 'off';
        matched.push('tddMode=off');
      } else {
        updates.tddMode = 'flexible';
        matched.push('tddMode=flexible');
      }
    }
  }

  const flowLevel = applyFlowLevel(clause, lexicon);
  if (flowLevel) {
    updates.flowLevel = flowLevel;
    matched.push(`flowLevel=${flowLevel}`);
  }

  if (hasAny(clause, lexicon.qualityKeywords)) {
    const toggle = inferDisableEnable(clause, lexicon);
    if (toggle === 'disable') {
      updates.qualityGatesEnabled = false;
      matched.push('qualityGatesEnabled=false');
    } else if (toggle === 'enable') {
      updates.qualityGatesEnabled = true;
      matched.push('qualityGatesEnabled=true');
    }
  }

  if (hasAny(clause, lexicon.interviewKeywords)) {
    const toggle = inferDisableEnable(clause, lexicon);
    if (toggle === 'disable') {
      updates.specInterviewEnabled = false;
      matched.push('specInterviewEnabled=false');
    } else {
      updates.specInterviewEnabled = true;
      matched.push('specInterviewEnabled=true');
    }
  }

  return {
    updates,
    matched,
    unmatched: matched.length === 0 ? [clause] : [],
  };
}

export function parseWorkflowConfigNatural(text: string, locale?: string): WorkflowConfigNaturalParseResult {
  const normalized = text.trim().toLowerCase();
  const updates: Partial<WorkflowConfig> = {};
  const matched: string[] = [];
  const unmatched: string[] = [];

  if (!normalized) {
    return { updates, matched, unmatched };
  }

  const lexicon = resolveLexicon(locale);
  const clauses = splitClauses(normalized);

  for (const clause of clauses.length > 0 ? clauses : [normalized]) {
    const clauseResult = parseClause(clause, lexicon);
    Object.assign(updates, clauseResult.updates);
    matched.push(...clauseResult.matched);
    if (clauseResult.unmatched.length > 0) {
      unmatched.push(...clauseResult.unmatched);
    }
  }

  const dedupedMatched = unique(matched);
  const dedupedUnmatched = unique(unmatched);

  if (dedupedMatched.length === 0 && dedupedUnmatched.length === 0) {
    dedupedUnmatched.push(text.trim());
  }

  return { updates, matched: dedupedMatched, unmatched: dedupedUnmatched };
}
