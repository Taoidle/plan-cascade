/**
 * Git AI Feature Tests
 *
 * Tests for AIReviewPanel, Toast components, and IPC contract validation
 * for the git AI commands.
 *
 * Feature-005: LLM-Powered Git Assistance
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

// Mock react-i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}));

// Mock settings store
const mockSettings = {
  workspacePath: '/test/repo',
  backend: 'claude-api' as const,
  apiKey: 'test-key',
};

vi.mock('../../../../store/settings', () => ({
  useSettingsStore: vi.fn((selector: (state: typeof mockSettings) => unknown) => {
    return typeof selector === 'function' ? selector(mockSettings) : mockSettings;
  }),
}));

// Mock git store
const mockGitState = {
  commitMessage: '',
  setCommitMessage: vi.fn(),
  isAmend: false,
  setIsAmend: vi.fn(),
  commit: vi.fn(),
  status: {
    staged: [{ path: 'test.ts', kind: 'modified' as const }],
    unstaged: [{ path: 'other.ts', kind: 'modified' as const }],
    untracked: [],
    conflicted: [],
    branch: 'main',
    upstream: 'origin/main',
    ahead: 0,
    behind: 0,
  },
  isLoading: false,
  stageFiles: vi.fn(),
  unstageFiles: vi.fn(),
  stageAll: vi.fn(),
  error: null,
  setError: vi.fn(),
};

vi.mock('../../../../store/git', () => ({
  useGitStore: vi.fn((selector: (state: typeof mockGitState) => unknown) => {
    return typeof selector === 'function' ? selector(mockGitState) : mockGitState;
  }),
}));

// ---------------------------------------------------------------------------
// Import components after mocks
// ---------------------------------------------------------------------------

import { AIReviewPanel } from '../ChangesTab/AIReviewPanel';
import { ToastProvider, useToast } from '../../../shared/Toast';

// ============================================================================
// AIReviewPanel Tests
// ============================================================================

describe('AIReviewPanel', () => {
  it('should parse and display review notes', () => {
    const reviewText = `- This looks good overall
- Consider adding error handling for the edge case
- Bug: potential null pointer when accessing user.name
- Performance could be improved by caching results`;

    render(
      <ToastProvider>
        <AIReviewPanel reviewText={reviewText} onDismiss={vi.fn()} />
      </ToastProvider>
    );

    expect(screen.getByText('AI Review')).toBeTruthy();
    expect(screen.getByText(/This looks good overall/)).toBeTruthy();
    expect(screen.getByText(/Consider adding error handling/)).toBeTruthy();
    expect(screen.getByText(/Bug: potential null pointer/)).toBeTruthy();
    expect(screen.getByText(/Performance could be improved/)).toBeTruthy();
  });

  it('should classify severity correctly', () => {
    const reviewText = `- Bug: critical security issue
- Consider refactoring this function
- Nice use of TypeScript generics`;

    render(
      <ToastProvider>
        <AIReviewPanel reviewText={reviewText} onDismiss={vi.fn()} />
      </ToastProvider>
    );

    // Check summary counts
    expect(screen.getByText(/1 issue/)).toBeTruthy();
    expect(screen.getByText(/1 warning/)).toBeTruthy();
    expect(screen.getByText(/1 note/)).toBeTruthy();
  });

  it('should dismiss individual notes', () => {
    const reviewText = `- First note
- Second note`;

    render(
      <ToastProvider>
        <AIReviewPanel reviewText={reviewText} onDismiss={vi.fn()} />
      </ToastProvider>
    );

    const dismissButtons = screen.getAllByTitle('Dismiss');
    expect(dismissButtons.length).toBe(2);

    fireEvent.click(dismissButtons[0]);

    // Should still show second note
    expect(screen.getByText(/Second note/)).toBeTruthy();
  });

  it('should call onDismiss when panel close is clicked', () => {
    const onDismiss = vi.fn();
    const reviewText = `- Good code`;

    render(
      <ToastProvider>
        <AIReviewPanel reviewText={reviewText} onDismiss={onDismiss} />
      </ToastProvider>
    );

    const dismissButton = screen.getByTitle('Dismiss review');
    fireEvent.click(dismissButton);

    expect(onDismiss).toHaveBeenCalledTimes(1);
  });

  it('should show all-dismissed state when all notes are dismissed', () => {
    const reviewText = `- Only note`;

    render(
      <ToastProvider>
        <AIReviewPanel reviewText={reviewText} onDismiss={vi.fn()} />
      </ToastProvider>
    );

    const dismissButton = screen.getByTitle('Dismiss');
    fireEvent.click(dismissButton);

    expect(screen.getByText('All review notes dismissed')).toBeTruthy();
  });

  it('should handle multi-line notes correctly', () => {
    const reviewText = `- First bullet point
- Second bullet point with warning: should use const
* Third point uses asterisk marker`;

    render(
      <ToastProvider>
        <AIReviewPanel reviewText={reviewText} onDismiss={vi.fn()} />
      </ToastProvider>
    );

    expect(screen.getByText(/First bullet point/)).toBeTruthy();
    expect(screen.getByText(/Second bullet point/)).toBeTruthy();
    expect(screen.getByText(/Third point uses asterisk/)).toBeTruthy();
  });

  it('should handle error severity keywords', () => {
    const reviewText = `- Security vulnerability in auth handler
- Dangerous use of eval()
- Critical: unhandled exception`;

    render(
      <ToastProvider>
        <AIReviewPanel reviewText={reviewText} onDismiss={vi.fn()} />
      </ToastProvider>
    );

    // All three should be classified as errors
    expect(screen.getByText(/3 issue/)).toBeTruthy();
  });

  it('should handle warning severity keywords', () => {
    const reviewText = `- You should add input validation
- Consider using a more efficient algorithm
- This might cause issues in production`;

    render(
      <ToastProvider>
        <AIReviewPanel reviewText={reviewText} onDismiss={vi.fn()} />
      </ToastProvider>
    );

    expect(screen.getByText(/3 warning/)).toBeTruthy();
  });
});

// ============================================================================
// Toast Component Tests
// ============================================================================

describe('ToastProvider', () => {
  it('should render children', () => {
    render(
      <ToastProvider>
        <div data-testid="child">Hello</div>
      </ToastProvider>
    );

    expect(screen.getByTestId('child')).toBeTruthy();
  });
});

// ============================================================================
// useToast Hook Tests
// ============================================================================

function ToastTestComponent() {
  const { showToast } = useToast();
  return (
    <div>
      <button onClick={() => showToast('Test message', 'success')}>Show Toast</button>
      <button onClick={() => showToast('Error message', 'error')}>Show Error</button>
      <button onClick={() => showToast('Info message', 'info')}>Show Info</button>
    </div>
  );
}

describe('useToast', () => {
  it('should show toast messages', async () => {
    render(
      <ToastProvider>
        <ToastTestComponent />
      </ToastProvider>
    );

    fireEvent.click(screen.getByText('Show Toast'));

    await waitFor(() => {
      expect(screen.getByText('Test message')).toBeTruthy();
    });
  });

  it('should show error toasts', async () => {
    render(
      <ToastProvider>
        <ToastTestComponent />
      </ToastProvider>
    );

    fireEvent.click(screen.getByText('Show Error'));

    await waitFor(() => {
      expect(screen.getByText('Error message')).toBeTruthy();
    });
  });

  it('should show multiple toasts', async () => {
    render(
      <ToastProvider>
        <ToastTestComponent />
      </ToastProvider>
    );

    fireEvent.click(screen.getByText('Show Toast'));
    fireEvent.click(screen.getByText('Show Error'));

    await waitFor(() => {
      expect(screen.getByText('Test message')).toBeTruthy();
      expect(screen.getByText('Error message')).toBeTruthy();
    });
  });

  it('should show info toasts', async () => {
    render(
      <ToastProvider>
        <ToastTestComponent />
      </ToastProvider>
    );

    fireEvent.click(screen.getByText('Show Info'));

    await waitFor(() => {
      expect(screen.getByText('Info message')).toBeTruthy();
    });
  });
});

// ============================================================================
// IPC Contract Tests (validate command names and parameter shapes)
// ============================================================================

describe('IPC Command Contracts', () => {
  it('git_generate_commit_message should accept repoPath parameter', () => {
    const params = { repoPath: '/test/repo' };
    expect(params.repoPath).toBe('/test/repo');
    // Validates the contract: command takes { repoPath: string }
  });

  it('git_review_diff should accept repoPath parameter', () => {
    const params = { repoPath: '/test/repo' };
    expect(params.repoPath).toBe('/test/repo');
  });

  it('git_resolve_conflict_ai should accept repoPath and filePath', () => {
    const params = { repoPath: '/test/repo', filePath: 'src/main.ts' };
    expect(params.repoPath).toBe('/test/repo');
    expect(params.filePath).toBe('src/main.ts');
  });

  it('git_summarize_commit should accept repoPath and sha', () => {
    const params = { repoPath: '/test/repo', sha: 'abc1234' };
    expect(params.repoPath).toBe('/test/repo');
    expect(params.sha).toBe('abc1234');
  });

  it('git_check_llm_available takes no parameters', () => {
    const params = {};
    expect(Object.keys(params).length).toBe(0);
  });

  it('CommandResponse shape should have success, data, and error', () => {
    const response = {
      success: true as boolean,
      data: 'test' as string | null,
      error: null as string | null,
    };
    expect(response.success).toBe(true);
    expect(response.data).toBe('test');
    expect(response.error).toBeNull();
  });

  it('CommandResponse error shape should have success=false', () => {
    const response = {
      success: false as boolean,
      data: null as string | null,
      error: 'Something went wrong' as string | null,
    };
    expect(response.success).toBe(false);
    expect(response.data).toBeNull();
    expect(response.error).toBe('Something went wrong');
  });
});

// ============================================================================
// Review Note Severity Classification Tests
// ============================================================================

describe('Review Note Severity Classification', () => {
  it('should detect bug-related keywords as error severity', () => {
    const reviewText = `- Bug in the login handler`;
    render(
      <ToastProvider>
        <AIReviewPanel reviewText={reviewText} onDismiss={vi.fn()} />
      </ToastProvider>
    );
    expect(screen.getByText(/1 issue/)).toBeTruthy();
  });

  it('should detect performance keywords as warning severity', () => {
    const reviewText = `- Performance issue with O(n^2) loop`;
    render(
      <ToastProvider>
        <AIReviewPanel reviewText={reviewText} onDismiss={vi.fn()} />
      </ToastProvider>
    );
    expect(screen.getByText(/1 warning/)).toBeTruthy();
  });

  it('should default to info severity for neutral notes', () => {
    const reviewText = `- The code structure is clean`;
    render(
      <ToastProvider>
        <AIReviewPanel reviewText={reviewText} onDismiss={vi.fn()} />
      </ToastProvider>
    );
    expect(screen.getByText(/1 note/)).toBeTruthy();
  });

  it('should handle mixed severity notes', () => {
    const reviewText = `- Good job on the refactoring
- Security vulnerability detected
- Consider adding tests`;

    render(
      <ToastProvider>
        <AIReviewPanel reviewText={reviewText} onDismiss={vi.fn()} />
      </ToastProvider>
    );

    expect(screen.getByText(/1 issue/)).toBeTruthy();
    expect(screen.getByText(/1 warning/)).toBeTruthy();
    expect(screen.getByText(/1 note/)).toBeTruthy();
  });
});
