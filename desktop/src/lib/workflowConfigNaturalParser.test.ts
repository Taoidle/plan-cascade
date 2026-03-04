import { describe, expect, it } from 'vitest';
import { parseWorkflowConfigNatural } from './workflowConfigNaturalParser';

describe('workflowConfigNaturalParser', () => {
  it('parses zh override phrase with parallel and tdd', () => {
    const result = parseWorkflowConfigNatural('使用 6 个并行代理并启用 TDD', 'zh-CN');
    expect(result.updates.maxParallel).toBe(6);
    expect(result.updates.tddMode).toBe('flexible');
    expect(result.matched).toEqual(expect.arrayContaining(['maxParallel=6', 'tddMode=flexible']));
    expect(result.unmatched).toEqual([]);
  });

  it('parses en override phrase with strict tdd and flow level', () => {
    const result = parseWorkflowConfigNatural('Use 3 parallel agents with strict TDD and full flow', 'en-US');
    expect(result.updates.maxParallel).toBe(3);
    expect(result.updates.tddMode).toBe('strict');
    expect(result.updates.flowLevel).toBe('full');
  });

  it('parses ja override phrase with quality and interview toggles', () => {
    const result = parseWorkflowConfigNatural(
      '並列 5 エージェントでTDDを有効、品質ゲートを無効、要件インタビューを有効',
      'ja-JP',
    );
    expect(result.updates.maxParallel).toBe(5);
    expect(result.updates.tddMode).toBe('flexible');
    expect(result.updates.qualityGatesEnabled).toBe(false);
    expect(result.updates.specInterviewEnabled).toBe(true);
  });

  it('returns unmatched clause when no override is recognized', () => {
    const result = parseWorkflowConfigNatural('just do your best', 'en-US');
    expect(result.updates).toEqual({});
    expect(result.matched).toEqual([]);
    expect(result.unmatched).toEqual(['just do your best']);
  });
});
