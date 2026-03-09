import { createRef, forwardRef } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SimpleInputComposer } from './SimpleInputComposer';
import type { InputBoxHandle } from './InputBox';
import type { SimpleInputComposerProps } from './SimpleInputComposer';
import type { QueuedChatMessage } from './queuePersistence';

vi.mock('../shared/EffectiveContextSummary', () => ({
  EffectiveContextSummary: () => <div data-testid="effective-summary" />,
}));

vi.mock('./InterviewInputPanel', () => ({
  InterviewInputPanel: () => <div data-testid="interview-input-panel" />,
}));

vi.mock('./PlanClarifyInputPanel', () => ({
  PlanClarifyInputPanel: (props: { onSubmit: (answer: string) => void; onSkipAll: () => void }) => (
    <div>
      <button data-testid="plan-clarify-panel" onClick={() => props.onSubmit('structured answer')}>
        plan panel
      </button>
      <button data-testid="plan-clarify-skip-all" onClick={props.onSkipAll}>
        plan skip all
      </button>
    </div>
  ),
}));

vi.mock('./InputBox', () => ({
  InputBox: forwardRef(function MockInputBox(props: { placeholder?: string; onSubmit: () => void }, _ref) {
    return (
      <div>
        <div data-testid="input-placeholder">{props.placeholder}</div>
        <button data-testid="input-submit" onClick={props.onSubmit}>
          submit
        </button>
      </div>
    );
  }),
}));

function makeProps(overrides?: Partial<SimpleInputComposerProps>): SimpleInputComposerProps {
  return {
    t: (key, opts) => (opts?.defaultValue as string) || key,
    workflowMode: 'chat',
    workflowPhase: 'idle',
    isRunning: false,
    taskInterviewingPhase: false,
    planClarifyingPhase: false,
    hasStructuredInterviewQuestion: false,
    hasStructuredPlanClarifyQuestion: false,
    hasTextInterviewQuestion: false,
    taskPendingQuestion: null,
    planPendingQuestion: null,
    onStructuredInterviewSubmit: vi.fn(),
    onStructuredPlanClarifySubmit: vi.fn(),
    onSkipInterviewQuestion: vi.fn(),
    onSkipPlanClarifyQuestion: vi.fn(),
    onSkipPlanClarification: vi.fn(),
    isInterviewSubmitting: false,
    inputBoxRef: createRef<InputBoxHandle>(),
    description: '',
    onDescriptionChange: vi.fn(),
    onSubmit: vi.fn(),
    inputDisabled: false,
    canQueueWhileRunning: false,
    inputLoading: false,
    attachments: [],
    workspaceReferences: [],
    onAttach: vi.fn(),
    onRemoveAttachment: vi.fn(),
    onWorkspaceReferencesChange: vi.fn(),
    workspacePath: '/tmp/workspace',
    activeAgentName: null,
    onClearAgent: vi.fn(),
    queuedChatMessages: [],
    onRemoveQueuedChatMessage: vi.fn(),
    onMoveQueuedChatMessage: vi.fn(),
    onSetQueuedChatMessagePriority: vi.fn(),
    onRetryQueuedChatMessage: vi.fn(),
    onClearQueuedChatMessages: vi.fn(),
    maxQueuedChatMessages: 3,
    ...overrides,
  };
}

function createQueuedMessage(overrides?: Partial<QueuedChatMessage>): QueuedChatMessage {
  return {
    id: 'q1',
    sessionId: 'session-1',
    prompt: 'queued prompt',
    submitAsFollowUp: true,
    mode: 'chat',
    attempts: 0,
    attachments: [],
    references: [],
    priority: 'normal',
    status: 'pending',
    enqueueSeq: 0,
    createdAt: new Date('2026-01-01T00:00:00.000Z').toISOString(),
    lastError: null,
    ...overrides,
  };
}

describe('SimpleInputComposer', () => {
  it('renders queued messages and removes one on click', () => {
    const onRemoveQueuedChatMessage = vi.fn();
    const props = makeProps({
      queuedChatMessages: [createQueuedMessage()],
      onRemoveQueuedChatMessage,
    });

    render(<SimpleInputComposer {...props} />);

    expect(screen.getByText('queued prompt')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Remove'));
    expect(onRemoveQueuedChatMessage).toHaveBeenCalledWith('q1');
  });

  it('wires queue control actions to callbacks', () => {
    const onMoveQueuedChatMessage = vi.fn();
    const onSetQueuedChatMessagePriority = vi.fn();
    const onRetryQueuedChatMessage = vi.fn();
    const onClearQueuedChatMessages = vi.fn();

    const props = makeProps({
      queuedChatMessages: [createQueuedMessage({ status: 'blocked', lastError: 'switch failed' })],
      onMoveQueuedChatMessage,
      onSetQueuedChatMessagePriority,
      onRetryQueuedChatMessage,
      onClearQueuedChatMessages,
    });

    render(<SimpleInputComposer {...props} />);

    fireEvent.click(screen.getByTitle('Move to top'));
    expect(onMoveQueuedChatMessage).toHaveBeenCalledWith('q1', 'top');

    fireEvent.click(screen.getByTitle('Move to bottom'));
    expect(onMoveQueuedChatMessage).toHaveBeenCalledWith('q1', 'bottom');

    fireEvent.click(screen.getByTitle('high'));
    expect(onSetQueuedChatMessagePriority).toHaveBeenCalledWith('q1', 'high');

    fireEvent.click(screen.getByTitle('Retry'));
    expect(onRetryQueuedChatMessage).toHaveBeenCalledWith('q1');

    fireEvent.click(screen.getByText('Clear All'));
    expect(onClearQueuedChatMessages).toHaveBeenCalledTimes(1);
  });

  it('renders queue operation labels from i18n keys', () => {
    const tSpy = vi.fn((key: string) => key);
    const props = makeProps({
      t: tSpy,
      queuedChatMessages: [createQueuedMessage()],
    });

    render(<SimpleInputComposer {...props} />);

    expect(screen.getByText('workflow.queue.clearAll')).toBeInTheDocument();
    expect(tSpy).toHaveBeenCalledWith(
      'workflow.queue.clearAll',
      expect.objectContaining({ defaultValue: 'Clear All' }),
    );
    expect(tSpy).toHaveBeenCalledWith(
      'workflow.queue.moveTop',
      expect.objectContaining({ defaultValue: 'Move to top' }),
    );
  });

  it('shows plan clarify skip-all and calls handler', () => {
    const onSkipPlanClarification = vi.fn();
    const props = makeProps({
      workflowMode: 'plan',
      planClarifyingPhase: true,
      hasStructuredPlanClarifyQuestion: true,
      planPendingQuestion: {
        questionId: 'clarify-1',
        question: 'Need more details?',
        hint: 'extra context',
        inputType: 'text',
      },
      onSkipPlanClarification,
    });

    render(<SimpleInputComposer {...props} />);

    fireEvent.click(screen.getByTestId('plan-clarify-skip-all'));
    expect(onSkipPlanClarification).toHaveBeenCalledTimes(1);
  });

  it('renders structured plan clarify panel and routes submit', () => {
    const onStructuredPlanClarifySubmit = vi.fn();
    const props = makeProps({
      workflowMode: 'plan',
      planClarifyingPhase: true,
      hasStructuredPlanClarifyQuestion: true,
      planPendingQuestion: {
        questionId: 'clarify-2',
        question: 'Choose one',
        hint: null,
        inputType: 'single_select',
        options: ['A', 'B'],
      },
      onStructuredPlanClarifySubmit,
    });

    render(<SimpleInputComposer {...props} />);

    fireEvent.click(screen.getByTestId('plan-clarify-panel'));
    expect(onStructuredPlanClarifySubmit).toHaveBeenCalledWith('structured answer');
  });
});
