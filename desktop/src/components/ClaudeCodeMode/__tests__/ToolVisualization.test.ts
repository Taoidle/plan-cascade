/**
 * Tool Visualization Component Tests
 *
 * Unit tests for the new tool call visualization components.
 * These tests validate the core logic without requiring DOM rendering.
 *
 * Note: Add vitest or jest to package.json to run these tests:
 * pnpm add -D vitest @testing-library/react @testing-library/user-event
 */

import { truncatePath, truncateCommand, truncateAtWordBoundary } from '../TruncatedText';
import { parseAnsiText, get256Color } from '../AnsiOutput';
import { buildFileTree } from '../GlobResultViewer';
import { highlightPattern } from '../GrepResultViewer';
import { computeDiff, getLanguageFromPath } from '../EnhancedDiffViewer';
import { calculateTimeline, calculateStatistics } from '../ExecutionTimeline';
import type { ToolCall } from '../../../store/claudeCode';

// ============================================================================
// TruncatedText Tests
// ============================================================================

describe('TruncatedText utilities', () => {
  describe('truncatePath', () => {
    it('should return path unchanged if within limit', () => {
      const path = '/short/path.txt';
      expect(truncatePath(path, 60)).toBe(path);
    });

    it('should truncate long paths preserving filename', () => {
      const path = '/very/long/nested/path/to/some/deeply/buried/file.tsx';
      const result = truncatePath(path, 30);
      expect(result).toContain('file.tsx');
      expect(result.startsWith('...')).toBe(true);
    });

    it('should handle paths with backslashes (Windows)', () => {
      const path = 'C:\\Users\\user\\documents\\project\\src\\file.ts';
      const result = truncatePath(path, 30);
      expect(result).toContain('file.ts');
    });
  });

  describe('truncateCommand', () => {
    it('should return command unchanged if within limit', () => {
      const cmd = 'git status';
      expect(truncateCommand(cmd, 80)).toBe(cmd);
    });

    it('should truncate at pipe if present', () => {
      const cmd = 'cat file.txt | grep pattern | head -10';
      const result = truncateCommand(cmd, 25);
      expect(result.endsWith('...')).toBe(true);
    });
  });

  describe('truncateAtWordBoundary', () => {
    it('should truncate at word boundary', () => {
      const text = 'This is a long text that needs truncation';
      const result = truncateAtWordBoundary(text, 20);
      expect(result.endsWith('...')).toBe(true);
      expect(result.length).toBeLessThanOrEqual(20);
    });

    it('should return text unchanged if within limit', () => {
      const text = 'Short text';
      expect(truncateAtWordBoundary(text, 100)).toBe(text);
    });
  });
});

// ============================================================================
// AnsiOutput Tests
// ============================================================================

describe('AnsiOutput utilities', () => {
  describe('parseAnsiText', () => {
    it('should parse plain text', () => {
      const result = parseAnsiText('Hello World');
      expect(result).toHaveLength(1);
      expect(result[0].spans[0].text).toBe('Hello World');
    });

    it('should parse multiple lines', () => {
      const result = parseAnsiText('Line 1\nLine 2\nLine 3');
      expect(result).toHaveLength(3);
      expect(result[0].lineNumber).toBe(1);
      expect(result[2].lineNumber).toBe(3);
    });

    it('should parse basic ANSI colors', () => {
      // Red text: \x1b[31m
      const result = parseAnsiText('\x1b[31mRed Text\x1b[0m');
      expect(result[0].spans).toHaveLength(1);
      expect(result[0].spans[0].style.color).toBeDefined();
    });

    it('should handle bold formatting', () => {
      const result = parseAnsiText('\x1b[1mBold\x1b[0m');
      expect(result[0].spans[0].style.bold).toBe(true);
    });

    it('should reset styles', () => {
      const result = parseAnsiText('\x1b[31mRed\x1b[0mNormal');
      expect(result[0].spans).toHaveLength(2);
      expect(result[0].spans[1].style.color).toBeUndefined();
    });
  });

  describe('get256Color', () => {
    it('should return standard colors for 0-15', () => {
      expect(get256Color(0)).toBe('#000000'); // Black
      expect(get256Color(1)).toBe('#cd0000'); // Red
      expect(get256Color(15)).toBe('#ffffff'); // Bright White
    });

    it('should return grayscale for 232-255', () => {
      const gray232 = get256Color(232);
      expect(gray232.startsWith('#')).toBe(true);
      expect(gray232).toHaveLength(7); // #RRGGBB
    });

    it('should return color cube values for 16-231', () => {
      const color = get256Color(100);
      expect(color.startsWith('#')).toBe(true);
      expect(color).toHaveLength(7);
    });
  });
});

// ============================================================================
// GlobResultViewer Tests
// ============================================================================

describe('GlobResultViewer utilities', () => {
  describe('buildFileTree', () => {
    it('should build tree from flat file list', () => {
      const files = ['src/index.ts', 'src/utils/helper.ts', 'src/utils/format.ts'];
      const tree = buildFileTree(files);

      expect(tree.fileCount).toBe(3);
      expect(tree.children.has('src')).toBe(true);
    });

    it('should handle mixed path separators', () => {
      const files = ['src/file1.ts', 'src\\file2.ts'];
      const tree = buildFileTree(files);
      expect(tree.fileCount).toBe(2);
    });

    it('should create nested directory structure', () => {
      const files = ['a/b/c/file.ts'];
      const tree = buildFileTree(files);

      let node = tree.children.get('a');
      expect(node).toBeDefined();
      expect(node!.isDirectory).toBe(true);

      node = node!.children.get('b');
      expect(node).toBeDefined();

      node = node!.children.get('c');
      expect(node).toBeDefined();
    });
  });
});

// ============================================================================
// GrepResultViewer Tests
// ============================================================================

describe('GrepResultViewer utilities', () => {
  describe('highlightPattern', () => {
    it('should return plain text when no pattern', () => {
      const result = highlightPattern('Hello World', '');
      expect(result).toHaveLength(1);
    });

    it('should highlight matching patterns', () => {
      const result = highlightPattern('Hello World', 'World');
      expect(result.length).toBeGreaterThan(1);
    });

    it('should be case insensitive', () => {
      const result = highlightPattern('Hello WORLD', 'world');
      expect(result.length).toBeGreaterThan(1);
    });

    it('should handle regex patterns', () => {
      const result = highlightPattern('test123test456', 'test\\d+');
      expect(result.length).toBeGreaterThanOrEqual(1);
    });
  });
});

// ============================================================================
// EnhancedDiffViewer Tests
// ============================================================================

describe('EnhancedDiffViewer utilities', () => {
  describe('computeDiff', () => {
    it('should detect no changes', () => {
      const oldLines = ['line 1', 'line 2'];
      const newLines = ['line 1', 'line 2'];
      const diff = computeDiff(oldLines, newLines);

      expect(diff.every((d) => d.type === 'unchanged')).toBe(true);
    });

    it('should detect additions', () => {
      const oldLines = ['line 1'];
      const newLines = ['line 1', 'line 2'];
      const diff = computeDiff(oldLines, newLines);

      expect(diff.some((d) => d.type === 'added')).toBe(true);
    });

    it('should detect removals', () => {
      const oldLines = ['line 1', 'line 2'];
      const newLines = ['line 1'];
      const diff = computeDiff(oldLines, newLines);

      expect(diff.some((d) => d.type === 'removed')).toBe(true);
    });

    it('should handle empty inputs', () => {
      const diff = computeDiff([], []);
      expect(diff).toHaveLength(0);
    });

    it('should detect modifications', () => {
      const oldLines = ['const x = 1;'];
      const newLines = ['const x = 2;'];
      const diff = computeDiff(oldLines, newLines);

      // Should have both added and removed
      expect(diff.some((d) => d.type === 'added')).toBe(true);
      expect(diff.some((d) => d.type === 'removed')).toBe(true);
    });
  });

  describe('getLanguageFromPath', () => {
    it('should detect TypeScript', () => {
      expect(getLanguageFromPath('file.ts')).toBe('typescript');
      expect(getLanguageFromPath('file.tsx')).toBe('typescript');
    });

    it('should detect JavaScript', () => {
      expect(getLanguageFromPath('file.js')).toBe('javascript');
      expect(getLanguageFromPath('file.jsx')).toBe('javascript');
    });

    it('should detect Python', () => {
      expect(getLanguageFromPath('script.py')).toBe('python');
    });

    it('should return text for unknown extensions', () => {
      expect(getLanguageFromPath('file.unknown')).toBe('text');
    });

    it('should handle no extension', () => {
      expect(getLanguageFromPath('Makefile')).toBe('text');
    });
  });
});

// ============================================================================
// ExecutionTimeline Tests
// ============================================================================

describe('ExecutionTimeline utilities', () => {
  const mockToolCalls: ToolCall[] = [
    {
      id: '1',
      name: 'Read',
      parameters: { file_path: '/test.ts' },
      status: 'completed',
      startedAt: '2024-01-01T10:00:00.000Z',
      completedAt: '2024-01-01T10:00:01.000Z',
      duration: 1000,
      result: { success: true },
    },
    {
      id: '2',
      name: 'Edit',
      parameters: { file_path: '/test.ts' },
      status: 'completed',
      startedAt: '2024-01-01T10:00:01.000Z',
      completedAt: '2024-01-01T10:00:03.000Z',
      duration: 2000,
      result: { success: true },
    },
    {
      id: '3',
      name: 'Bash',
      parameters: { command: 'npm test' },
      status: 'failed',
      startedAt: '2024-01-01T10:00:03.000Z',
      completedAt: '2024-01-01T10:00:04.000Z',
      duration: 1000,
      result: { success: false, error: 'Tests failed' },
    },
  ];

  describe('calculateTimeline', () => {
    it('should calculate timeline from tool calls', () => {
      const { bars, duration, laneCount } = calculateTimeline(mockToolCalls);

      expect(bars).toHaveLength(3);
      expect(duration).toBeGreaterThan(0);
      expect(laneCount).toBeGreaterThanOrEqual(1);
    });

    it('should handle empty tool calls', () => {
      const { bars, duration, laneCount } = calculateTimeline([]);

      expect(bars).toHaveLength(0);
      expect(duration).toBe(0);
      expect(laneCount).toBe(0);
    });

    it('should assign lanes to overlapping operations', () => {
      const overlapping: ToolCall[] = [
        {
          id: '1',
          name: 'Read',
          parameters: {},
          status: 'completed',
          startedAt: '2024-01-01T10:00:00.000Z',
          completedAt: '2024-01-01T10:00:05.000Z',
          duration: 5000,
        },
        {
          id: '2',
          name: 'Grep',
          parameters: {},
          status: 'completed',
          startedAt: '2024-01-01T10:00:02.000Z',
          completedAt: '2024-01-01T10:00:04.000Z',
          duration: 2000,
        },
      ];

      const { bars, laneCount } = calculateTimeline(overlapping);

      expect(laneCount).toBe(2); // Should need 2 lanes
      expect(bars[0].lane).not.toBe(bars[1].lane);
    });
  });

  describe('calculateStatistics', () => {
    it('should calculate statistics from bars', () => {
      const { bars, duration } = calculateTimeline(mockToolCalls);
      const stats = calculateStatistics(bars, duration);

      expect(stats.totalExecutionTime).toBe(4000); // 1000 + 2000 + 1000
      expect(stats.averageDuration).toBeCloseTo(4000 / 3);
      expect(stats.longestOperation).toBeDefined();
      expect(stats.longestOperation!.duration).toBe(2000);
    });

    it('should handle empty bars', () => {
      const stats = calculateStatistics([], 0);

      expect(stats.totalExecutionTime).toBe(0);
      expect(stats.parallelEfficiency).toBe(0);
      expect(stats.longestOperation).toBeNull();
    });
  });
});

// ============================================================================
// Mock describe/expect/it for environments without test runners
// ============================================================================

// These are placeholders that will be replaced by actual test runner (vitest/jest)
declare function describe(name: string, fn: () => void): void;
declare function it(name: string, fn: () => void): void;
interface ExpectMatchers<T> {
  toBe(expected: T): void;
  toHaveLength(length: number): void;
  toContain(item: unknown): void;
  toBeDefined(): void;
  toBeUndefined(): void;
  toBeNull(): void;
  toBeGreaterThan(value: number): void;
  toBeGreaterThanOrEqual(value: number): void;
  toBeLessThanOrEqual(value: number): void;
  toBeCloseTo(value: number, precision?: number): void;
  toEqual(expected: T): void;
  not: ExpectMatchers<T>;
}
declare function expect<T>(actual: T): ExpectMatchers<T>;
