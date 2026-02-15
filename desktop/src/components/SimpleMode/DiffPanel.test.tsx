/**
 * DiffPanel Component Tests
 *
 * Tests for the DiffPanel component:
 * - Renders header with title and refresh button
 * - Shows "Not a git repository" when workspacePath is null
 * - Displays Git Changes and Tool Changes sections
 * - Extracts tool changes from streaming output correctly
 * - Handles graceful degradation for non-git directories
 * - fileDiffToContents reconstructs old/new content correctly
 *
 * Story-005: Integrate Diffs Panel into SimpleMode
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { DiffPanel, extractToolChanges, fileDiffToContents } from './DiffPanel';
import type { StreamLine } from '../../store/execution';
import type { FileDiff } from '../../lib/diffParser';

// --------------------------------------------------------------------------
// Mocks
// --------------------------------------------------------------------------

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string }) => opts?.defaultValue || key,
  }),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Mock the Command.create from @tauri-apps/plugin-shell
const mockExecute = vi.fn();

vi.mock('@tauri-apps/plugin-shell', () => ({
  Command: {
    create: vi.fn(() => ({
      execute: mockExecute,
    })),
  },
}));

// Mock EnhancedDiffViewer to avoid rendering complexities
vi.mock('../ClaudeCodeMode/EnhancedDiffViewer', () => ({
  EnhancedDiffViewer: ({ filePath, oldContent, newContent }: {
    filePath?: string;
    oldContent: string;
    newContent: string;
  }) => (
    <div data-testid="enhanced-diff-viewer" data-file-path={filePath}>
      <span data-testid="old-content">{oldContent}</span>
      <span data-testid="new-content">{newContent}</span>
    </div>
  ),
}));

// --------------------------------------------------------------------------
// Test Helpers
// --------------------------------------------------------------------------

function createStreamLine(
  id: number,
  content: string,
  type: StreamLine['type'] = 'tool'
): StreamLine {
  return { id, content, type, timestamp: Date.now() };
}

const defaultProps = {
  streamingOutput: [] as StreamLine[],
  workspacePath: '/test/project',
};

// --------------------------------------------------------------------------
// Unit Tests: extractToolChanges
// --------------------------------------------------------------------------

describe('extractToolChanges', () => {
  it('returns empty array for empty input', () => {
    expect(extractToolChanges([])).toEqual([]);
  });

  it('extracts Write tool changes from tool lines', () => {
    const lines: StreamLine[] = [
      createStreamLine(1, '[tool:Write] /src/index.ts content here', 'tool'),
    ];
    const changes = extractToolChanges(lines);
    expect(changes).toHaveLength(1);
    expect(changes[0].toolName).toBe('Write');
    expect(changes[0].filePath).toBe('/src/index.ts');
  });

  it('extracts Edit tool changes from tool lines', () => {
    const lines: StreamLine[] = [
      createStreamLine(1, '[tool:Edit] /src/app.tsx replacing content', 'tool'),
    ];
    const changes = extractToolChanges(lines);
    expect(changes).toHaveLength(1);
    expect(changes[0].toolName).toBe('Edit');
    expect(changes[0].filePath).toBe('/src/app.tsx');
  });

  it('deduplicates changes for the same tool+filePath', () => {
    const lines: StreamLine[] = [
      createStreamLine(1, '[tool:Write] /src/index.ts first write', 'tool'),
      createStreamLine(2, '[tool:Write] /src/index.ts second write', 'tool'),
    ];
    const changes = extractToolChanges(lines);
    expect(changes).toHaveLength(1);
  });

  it('keeps changes for different file paths', () => {
    const lines: StreamLine[] = [
      createStreamLine(1, '[tool:Write] /src/a.ts content', 'tool'),
      createStreamLine(2, '[tool:Write] /src/b.ts content', 'tool'),
    ];
    const changes = extractToolChanges(lines);
    expect(changes).toHaveLength(2);
  });

  it('ignores non-tool and non-tool_result lines', () => {
    const lines: StreamLine[] = [
      createStreamLine(1, '[tool:Write] /src/a.ts content', 'text'),
      createStreamLine(2, 'Some info message', 'info'),
    ];
    const changes = extractToolChanges(lines);
    expect(changes).toHaveLength(0);
  });

  it('truncates preview to 120 characters', () => {
    const longContent = 'a'.repeat(200);
    const lines: StreamLine[] = [
      createStreamLine(1, `[tool:Write] /src/long.ts ${longContent}`, 'tool'),
    ];
    const changes = extractToolChanges(lines);
    expect(changes[0].preview.length).toBeLessThanOrEqual(123); // 120 + '...'
  });
});

// --------------------------------------------------------------------------
// Unit Tests: fileDiffToContents
// --------------------------------------------------------------------------

describe('fileDiffToContents', () => {
  it('reconstructs old and new content from hunks', () => {
    const fileDiff: FileDiff = {
      filePath: 'test.ts',
      changeType: 'modified',
      hunks: [
        {
          oldStart: 1,
          oldCount: 3,
          newStart: 1,
          newCount: 3,
          lines: [
            { type: 'context', content: 'line1', oldLineNumber: 1, newLineNumber: 1 },
            { type: 'removed', content: 'old-line2', oldLineNumber: 2 },
            { type: 'added', content: 'new-line2', newLineNumber: 2 },
            { type: 'context', content: 'line3', oldLineNumber: 3, newLineNumber: 3 },
          ],
        },
      ],
    };

    const { oldContent, newContent } = fileDiffToContents(fileDiff);
    expect(oldContent).toBe('line1\nold-line2\nline3');
    expect(newContent).toBe('line1\nnew-line2\nline3');
  });

  it('handles empty hunks', () => {
    const fileDiff: FileDiff = {
      filePath: 'empty.ts',
      changeType: 'added',
      hunks: [],
    };

    const { oldContent, newContent } = fileDiffToContents(fileDiff);
    expect(oldContent).toBe('');
    expect(newContent).toBe('');
  });

  it('handles added-only files', () => {
    const fileDiff: FileDiff = {
      filePath: 'new.ts',
      changeType: 'added',
      hunks: [
        {
          oldStart: 0,
          oldCount: 0,
          newStart: 1,
          newCount: 2,
          lines: [
            { type: 'added', content: 'line1', newLineNumber: 1 },
            { type: 'added', content: 'line2', newLineNumber: 2 },
          ],
        },
      ],
    };

    const { oldContent, newContent } = fileDiffToContents(fileDiff);
    expect(oldContent).toBe('');
    expect(newContent).toBe('line1\nline2');
  });
});

// --------------------------------------------------------------------------
// Component Tests
// --------------------------------------------------------------------------

describe('DiffPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockExecute.mockReset();
  });

  it('renders the header with title and refresh button', async () => {
    mockExecute.mockResolvedValue({ code: 0, stdout: '', stderr: '' });

    render(<DiffPanel {...defaultProps} />);

    expect(screen.getByText('Changes')).toBeInTheDocument();
    expect(screen.getByTitle('Refresh')).toBeInTheDocument();
  });

  it('shows "Not a git repository" when workspacePath is null', async () => {
    render(<DiffPanel {...defaultProps} workspacePath={null} />);

    await waitFor(() => {
      expect(screen.getByText('Not a git repository')).toBeInTheDocument();
    });
  });

  it('shows "No changes detected" when git diff returns empty output', async () => {
    mockExecute.mockResolvedValue({ code: 0, stdout: '', stderr: '' });

    render(<DiffPanel {...defaultProps} />);

    await waitFor(() => {
      const noChangesElements = screen.getAllByText('No changes detected');
      expect(noChangesElements.length).toBeGreaterThanOrEqual(1);
    });
  });

  it('renders git diffs when git diff returns output', async () => {
    const diffOutput = [
      'diff --git a/src/test.ts b/src/test.ts',
      'index abc1234..def5678 100644',
      '--- a/src/test.ts',
      '+++ b/src/test.ts',
      '@@ -1,3 +1,3 @@',
      ' line1',
      '-old-line2',
      '+new-line2',
      ' line3',
    ].join('\n');

    mockExecute.mockResolvedValue({ code: 0, stdout: diffOutput, stderr: '' });

    render(<DiffPanel {...defaultProps} />);

    await waitFor(() => {
      const viewers = screen.getAllByTestId('enhanced-diff-viewer');
      expect(viewers.length).toBeGreaterThanOrEqual(1);
      expect(viewers[0]).toHaveAttribute('data-file-path', 'src/test.ts');
    });
  });

  it('handles git diff failure (not a git repo)', async () => {
    mockExecute.mockResolvedValue({
      code: 128,
      stdout: '',
      stderr: 'fatal: not a git repository',
    });

    render(<DiffPanel {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getByText('Not a git repository')).toBeInTheDocument();
    });
  });

  it('handles shell errors gracefully', async () => {
    mockExecute.mockRejectedValue(new Error('Command not found'));

    render(<DiffPanel {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getByText('Not a git repository')).toBeInTheDocument();
    });
  });

  it('renders tool changes from streaming output', async () => {
    mockExecute.mockResolvedValue({ code: 0, stdout: '', stderr: '' });

    const streamingOutput: StreamLine[] = [
      createStreamLine(1, '[tool:Write] /src/new-file.ts export function hello() {}', 'tool'),
      createStreamLine(2, '[tool:Edit] /src/existing.ts replacing old content', 'tool'),
    ];

    render(<DiffPanel {...defaultProps} streamingOutput={streamingOutput} />);

    await waitFor(() => {
      expect(screen.getByText('Write')).toBeInTheDocument();
      expect(screen.getByText('Edit')).toBeInTheDocument();
      expect(screen.getByText('/src/new-file.ts')).toBeInTheDocument();
      expect(screen.getByText('/src/existing.ts')).toBeInTheDocument();
    });
  });

  it('shows loading state while fetching diffs', async () => {
    // Make the mock never resolve to keep loading state
    mockExecute.mockImplementation(() => new Promise(() => {}));

    render(<DiffPanel {...defaultProps} />);

    expect(screen.getByText('Loading...')).toBeInTheDocument();
  });

  it('re-fetches diffs when refresh button is clicked', async () => {
    mockExecute.mockResolvedValue({ code: 0, stdout: '', stderr: '' });

    render(<DiffPanel {...defaultProps} />);

    await waitFor(() => {
      expect(mockExecute).toHaveBeenCalledTimes(1);
    });

    const refreshButton = screen.getByTitle('Refresh');
    fireEvent.click(refreshButton);

    await waitFor(() => {
      expect(mockExecute).toHaveBeenCalledTimes(2);
    });
  });

  it('shows Git Changes and Tool Changes section headers', async () => {
    mockExecute.mockResolvedValue({ code: 0, stdout: '', stderr: '' });

    render(<DiffPanel {...defaultProps} />);

    await waitFor(() => {
      expect(screen.getByText('Git Changes')).toBeInTheDocument();
      expect(screen.getByText('Tool Changes')).toBeInTheDocument();
    });
  });
});
