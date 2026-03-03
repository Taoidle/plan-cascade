import { createRef, forwardRef } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SimpleInputComposer } from './SimpleInputComposer';
import type { InputBoxHandle } from './InputBox';
import type { SimpleInputComposerProps } from './SimpleInputComposer';

vi.mock('../shared/EffectiveContextSummary', () => ({
  EffectiveContextSummary: () => <div data-testid="effective-summary" />,
}));

vi.mock('./InterviewInputPanel', () => ({
  InterviewInputPanel: () => <div data-testid="interview-input-panel" />,
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
    hasTextInterviewQuestion: false,
    hasPlanClarifyQuestion: false,
    taskPendingQuestion: null,
    planPendingQuestion: null,
    onStructuredInterviewSubmit: vi.fn(),
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
    onAttach: vi.fn(),
    onRemoveAttachment: vi.fn(),
    workspacePath: '/tmp/workspace',
    activeAgentName: null,
    onClearAgent: vi.fn(),
    queuedChatMessages: [],
    onRemoveQueuedChatMessage: vi.fn(),
    maxQueuedChatMessages: 3,
    ...overrides,
  };
}

describe('SimpleInputComposer', () => {
  it('renders queued messages and removes one on click', () => {
    const onRemoveQueuedChatMessage = vi.fn();
    const props = makeProps({
      queuedChatMessages: [{ id: 'q1', prompt: 'queued prompt', submitAsFollowUp: true }],
      onRemoveQueuedChatMessage,
    });

    render(<SimpleInputComposer {...props} />);

    expect(screen.getByText('queued prompt')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Remove'));
    expect(onRemoveQueuedChatMessage).toHaveBeenCalledWith('q1');
  });

  it('shows plan clarify skip-all and calls handler', () => {
    const onSkipPlanClarification = vi.fn();
    const props = makeProps({
      workflowMode: 'plan',
      planClarifyingPhase: true,
      hasPlanClarifyQuestion: true,
      planPendingQuestion: {
        questionId: 'clarify-1',
        question: 'Need more details?',
        hint: 'extra context',
        inputType: 'text',
      },
      onSkipPlanClarification,
    });

    render(<SimpleInputComposer {...props} />);

    fireEvent.click(screen.getByText('Skip All'));
    expect(onSkipPlanClarification).toHaveBeenCalledTimes(1);
  });
});
