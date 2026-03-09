import { describe, expect, it } from 'vitest';
import { estimatePromptTokensFallback, estimateTokensRough, formatTokenCount } from './tokenBudget';

describe('tokenBudget utilities', () => {
  it('estimates tokens with text and non-text attachments', () => {
    const result = estimatePromptTokensFallback(
      'abcd',
      [
        {
          id: 'a',
          name: 'readme.md',
          path: 'readme.md',
          size: 8,
          type: 'text',
          inlineContent: 'abcdefgh',
        },
        {
          id: 'b',
          name: 'diagram.png',
          path: 'diagram.png',
          size: 100,
          type: 'image',
          inlinePreview: 'data:image/png;base64,test',
        },
      ],
      [],
      100,
    );

    expect(result.prompt_tokens).toBe(1);
    expect(result.attachment_tokens).toBe(50);
    expect(result.estimated_tokens).toBe(51);
    expect(result.remaining_tokens).toBe(49);
    expect(result.exceeds_budget).toBe(false);
  });

  it('reports budget overflow', () => {
    const result = estimatePromptTokensFallback('a'.repeat(600), [], [], 100);
    expect(result.exceeds_budget).toBe(true);
    expect(result.remaining_tokens).toBeLessThan(0);
  });

  it('formats token counts compactly', () => {
    expect(formatTokenCount(999)).toBe('999');
    expect(formatTokenCount(1250)).toBe('1.3k');
    expect(formatTokenCount(23000)).toBe('23k');
    expect(formatTokenCount(2_300_000)).toBe('2.3m');
    expect(formatTokenCount(-1500)).toBe('-1.5k');
    expect(estimateTokensRough('abcd')).toBe(1);
  });
});
