import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { InterviewInputPanel } from './InterviewInputPanel';
import type { InterviewQuestionCardData } from '../../types/workflowCard';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string; count?: number }) => {
      if (opts?.defaultValue) {
        return opts.defaultValue.replace('{{count}}', String(opts.count ?? ''));
      }
      return key;
    },
  }),
}));

function renderPanel(question: InterviewQuestionCardData) {
  const onSubmit = vi.fn();
  const onSkip = vi.fn();
  const view = render(<InterviewInputPanel question={question} onSubmit={onSubmit} onSkip={onSkip} loading={false} />);
  return { onSubmit, onSkip, ...view };
}

describe('InterviewInputPanel', () => {
  it('does not carry multi-select answers across questions', () => {
    const { onSubmit, rerender } = renderPanel({
      questionId: 'multi-1',
      question: 'Choose first set',
      hint: null,
      required: true,
      inputType: 'multi_select',
      options: ['alpha', 'beta', 'gamma'],
      allowCustom: false,
      questionNumber: 1,
      totalQuestions: 2,
    });

    fireEvent.click(screen.getByText('alpha'));
    fireEvent.click(screen.getByText('gamma'));
    fireEvent.click(screen.getByText('workflow.interview.submitCount'));

    expect(onSubmit).toHaveBeenLastCalledWith('alpha, gamma');

    rerender(
      <InterviewInputPanel
        question={{
          questionId: 'multi-2',
          question: 'Choose second set',
          hint: null,
          required: true,
          inputType: 'multi_select',
          options: ['delta', 'epsilon', 'zeta'],
          allowCustom: false,
          questionNumber: 2,
          totalQuestions: 2,
        }}
        onSubmit={onSubmit}
        onSkip={vi.fn()}
        loading={false}
      />,
    );

    fireEvent.click(screen.getByText('delta'));
    fireEvent.click(screen.getByText('zeta'));
    fireEvent.click(screen.getByText('workflow.interview.submitCount'));

    expect(onSubmit).toHaveBeenLastCalledWith('delta, zeta');
  });

  it('does not carry single-select answers across questions', () => {
    const { onSubmit, rerender } = renderPanel({
      questionId: 'single-1',
      question: 'Choose first option',
      hint: null,
      required: true,
      inputType: 'single_select',
      options: ['one', 'two'],
      allowCustom: false,
      questionNumber: 1,
      totalQuestions: 2,
    });

    fireEvent.click(screen.getByText('two'));
    fireEvent.click(screen.getByText('workflow.interview.submit'));
    expect(onSubmit).toHaveBeenLastCalledWith('two');

    rerender(
      <InterviewInputPanel
        question={{
          questionId: 'single-2',
          question: 'Choose second option',
          hint: null,
          required: true,
          inputType: 'single_select',
          options: ['three', 'four'],
          allowCustom: false,
          questionNumber: 2,
          totalQuestions: 2,
        }}
        onSubmit={onSubmit}
        onSkip={vi.fn()}
        loading={false}
      />,
    );

    const submitButton = screen.getByText('workflow.interview.submit');
    expect(submitButton).toBeDisabled();

    fireEvent.click(screen.getByText('four'));
    fireEvent.click(submitButton);
    expect(onSubmit).toHaveBeenLastCalledWith('four');
  });
});
