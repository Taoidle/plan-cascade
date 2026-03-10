import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import type { PromptTemplate } from '../../types/prompt';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (key.startsWith('promptCategories.')) {
        const parts = key.split('.');
        return parts[parts.length - 1] ?? key;
      }
      return (opts?.defaultValue as string) || key;
    },
    i18n: { language: 'en' },
  }),
}));

const mockStore = {
  prompts: [] as PromptTemplate[],
  loading: false,
  fetchPrompts: vi.fn(),
  openDialog: vi.fn(),
  togglePin: vi.fn(),
  recordUse: vi.fn(),
  setPendingInsert: vi.fn(),
};

vi.mock('../../store/prompts', () => ({
  usePromptsStore: vi.fn((selector: (state: typeof mockStore) => unknown) => selector(mockStore)),
}));

import { PromptPanel } from './PromptPanel';

function createPrompt(overrides: Partial<PromptTemplate> = {}): PromptTemplate {
  return {
    id: 'prompt-1',
    title: 'Prompt One',
    content: 'prompt content',
    description: null,
    category: '',
    tags: [],
    variables: [],
    is_builtin: false,
    is_pinned: false,
    use_count: 0,
    last_used_at: null,
    created_at: null,
    updated_at: null,
    ...overrides,
  };
}

describe('PromptPanel', () => {
  beforeEach(() => {
    mockStore.prompts = [];
    mockStore.loading = false;
    mockStore.fetchPrompts.mockClear();
    mockStore.openDialog.mockClear();
    mockStore.togglePin.mockClear();
    mockStore.recordUse.mockClear();
    mockStore.setPendingInsert.mockClear();
  });

  it('keeps showing other prompts after one prompt becomes recent', () => {
    mockStore.prompts = [
      createPrompt({ id: 'recent', title: 'Recent Prompt', use_count: 1 }),
      createPrompt({ id: 'other-1', title: 'Other Prompt 1' }),
      createPrompt({ id: 'other-2', title: 'Other Prompt 2' }),
    ];

    render(<PromptPanel />);

    expect(screen.getByText('Recent Prompt')).toBeInTheDocument();
    expect(screen.getByText('Other Prompt 1')).toBeInTheDocument();
    expect(screen.getByText('Other Prompt 2')).toBeInTheDocument();
  });

  it('inserts the selected prompt content', () => {
    mockStore.prompts = [createPrompt({ id: 'insert-me', title: 'Insert Me', content: 'hello world' })];

    render(<PromptPanel />);
    fireEvent.click(screen.getByText('Insert'));

    expect(mockStore.recordUse).toHaveBeenCalledWith('insert-me');
    expect(mockStore.setPendingInsert).toHaveBeenCalledWith('hello world');
  });
});
