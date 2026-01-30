/**
 * MessageActions Component Tests
 * Story 011-4: Message Actions (Copy, Regenerate, Edit & Resend)
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { renderHook, act } from '@testing-library/react';
import { MessageActions, useMessageActions } from '../MessageActions';
import { Message } from '../../../store/claudeCode';

// Mock i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => {
      const translations: Record<string, string> = {
        'messageActions.copy': 'Copy',
        'messageActions.copied': 'Copied!',
        'messageActions.regenerate': 'Regenerate',
        'messageActions.edit': 'Edit',
        'messageActions.fork': 'Fork',
        'messageActions.editHint': 'Ctrl+Enter to save, Escape to cancel',
        'messageActions.cancel': 'Cancel',
        'messageActions.saveAndResend': 'Save & Resend',
      };
      return translations[key] || key;
    },
  }),
}));

describe('MessageActions', () => {
  const createMessage = (role: 'user' | 'assistant'): Message => ({
    id: '1',
    role,
    content: 'Test message',
    timestamp: new Date().toISOString(),
  });

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders copy button for all messages', () => {
    const message = createMessage('user');
    render(
      <MessageActions
        message={message}
        onCopy={vi.fn().mockResolvedValue(undefined)}
      />
    );

    expect(screen.getByTitle('Copy')).toBeInTheDocument();
  });

  it('calls onCopy when copy button is clicked', async () => {
    const onCopy = vi.fn().mockResolvedValue(undefined);
    const message = createMessage('user');
    render(<MessageActions message={message} onCopy={onCopy} />);

    fireEvent.click(screen.getByTitle('Copy'));
    await waitFor(() => {
      expect(onCopy).toHaveBeenCalled();
    });
  });

  it('shows "Copied!" feedback after copy', async () => {
    const onCopy = vi.fn().mockResolvedValue(undefined);
    const message = createMessage('user');
    render(<MessageActions message={message} onCopy={onCopy} />);

    fireEvent.click(screen.getByTitle('Copy'));
    await waitFor(() => {
      expect(screen.getByTitle('Copied!')).toBeInTheDocument();
    });
  });

  it('renders regenerate button for assistant messages', () => {
    const message = createMessage('assistant');
    render(
      <MessageActions
        message={message}
        onCopy={vi.fn().mockResolvedValue(undefined)}
        onRegenerate={vi.fn().mockResolvedValue(undefined)}
      />
    );

    expect(screen.getByTitle('Regenerate')).toBeInTheDocument();
  });

  it('does not render regenerate button for user messages', () => {
    const message = createMessage('user');
    render(
      <MessageActions
        message={message}
        onCopy={vi.fn().mockResolvedValue(undefined)}
        onRegenerate={vi.fn().mockResolvedValue(undefined)}
      />
    );

    expect(screen.queryByTitle('Regenerate')).not.toBeInTheDocument();
  });

  it('renders edit button for user messages', () => {
    const message = createMessage('user');
    render(
      <MessageActions
        message={message}
        onCopy={vi.fn().mockResolvedValue(undefined)}
        onEdit={vi.fn().mockResolvedValue(undefined)}
      />
    );

    expect(screen.getByTitle('Edit')).toBeInTheDocument();
  });

  it('does not render edit button for assistant messages', () => {
    const message = createMessage('assistant');
    render(
      <MessageActions
        message={message}
        onCopy={vi.fn().mockResolvedValue(undefined)}
        onEdit={vi.fn().mockResolvedValue(undefined)}
      />
    );

    expect(screen.queryByTitle('Edit')).not.toBeInTheDocument();
  });

  it('renders fork button when onFork is provided', () => {
    const message = createMessage('user');
    render(
      <MessageActions
        message={message}
        onCopy={vi.fn().mockResolvedValue(undefined)}
        onFork={vi.fn()}
      />
    );

    expect(screen.getByTitle('Fork')).toBeInTheDocument();
  });

  it('shows loading state during regeneration', () => {
    const message = createMessage('assistant');
    render(
      <MessageActions
        message={message}
        onCopy={vi.fn().mockResolvedValue(undefined)}
        onRegenerate={vi.fn().mockResolvedValue(undefined)}
        isRegenerating={true}
      />
    );

    const regenButton = screen.getByTitle('Regenerate');
    expect(regenButton).toBeDisabled();
  });
});

describe('useMessageActions', () => {
  const createMessages = (): Message[] => [
    { id: '1', role: 'user', content: 'Hello', timestamp: '2024-01-01' },
    { id: '2', role: 'assistant', content: 'Hi there!', timestamp: '2024-01-01' },
    { id: '3', role: 'user', content: 'How are you?', timestamp: '2024-01-01' },
    { id: '4', role: 'assistant', content: 'I am good!', timestamp: '2024-01-01' },
  ];

  it('copies message content to clipboard', async () => {
    const messages = createMessages();
    const { result } = renderHook(() =>
      useMessageActions(messages, vi.fn(), vi.fn())
    );

    await act(async () => {
      await result.current.copyMessage(messages[0]);
    });

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('Hello');
  });

  it('regenerates message by removing it and resending', async () => {
    const messages = createMessages();
    const sendMessage = vi.fn().mockResolvedValue(undefined);
    const updateMessages = vi.fn();

    const { result } = renderHook(() =>
      useMessageActions(messages, sendMessage, updateMessages)
    );

    await act(async () => {
      await result.current.regenerateMessage('2'); // assistant message
    });

    expect(updateMessages).toHaveBeenCalled();
    expect(sendMessage).toHaveBeenCalledWith('Hello'); // last user message
  });

  it('edits message and resends', async () => {
    const messages = createMessages();
    const sendMessage = vi.fn().mockResolvedValue(undefined);
    const updateMessages = vi.fn();

    const { result } = renderHook(() =>
      useMessageActions(messages, sendMessage, updateMessages)
    );

    await act(async () => {
      await result.current.editMessage('1', 'Updated message');
    });

    expect(updateMessages).toHaveBeenCalled();
    expect(sendMessage).toHaveBeenCalledWith('Updated message');
  });

  it('tracks regenerating state', async () => {
    const messages = createMessages();
    const sendMessage = vi.fn().mockImplementation(
      () => new Promise((resolve) => setTimeout(resolve, 100))
    );
    const updateMessages = vi.fn();

    const { result } = renderHook(() =>
      useMessageActions(messages, sendMessage, updateMessages)
    );

    expect(result.current.isRegenerating).toBe(false);
    expect(result.current.regeneratingMessageId).toBeNull();
  });
});
