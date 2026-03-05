import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { PlanClarifyInputPanel } from './PlanClarifyInputPanel';
import type { PlanClarifyQuestionCardData } from '../../types/planModeCard';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { defaultValue?: string }) => opts?.defaultValue ?? key,
  }),
}));

function renderPanel(question: PlanClarifyQuestionCardData) {
  const onSubmit = vi.fn();
  const onSkipQuestion = vi.fn();
  const onSkipAll = vi.fn();

  render(
    <PlanClarifyInputPanel
      question={question}
      onSubmit={onSubmit}
      onSkipQuestion={onSkipQuestion}
      onSkipAll={onSkipAll}
      loading={false}
    />,
  );

  return { onSubmit, onSkipQuestion, onSkipAll };
}

describe('PlanClarifyInputPanel', () => {
  it('submits boolean answers with one click', () => {
    const { onSubmit } = renderPanel({
      questionId: 'q-boolean',
      question: 'Enable strict mode?',
      hint: null,
      inputType: 'boolean',
    });

    fireEvent.click(screen.getByText('Yes'));
    expect(onSubmit).toHaveBeenCalledWith('yes');
  });

  it('submits single select answers with one click', () => {
    const { onSubmit } = renderPanel({
      questionId: 'q-select',
      question: 'Choose target',
      hint: null,
      inputType: 'single_select',
      options: ['alpha', 'beta'],
    });

    fireEvent.click(screen.getByText('beta'));
    expect(onSubmit).toHaveBeenCalledWith('beta');
  });

  it('submits text answers on Enter', () => {
    const { onSubmit } = renderPanel({
      questionId: 'q-text',
      question: 'Describe scope',
      hint: null,
      inputType: 'text',
    });

    const input = screen.getByPlaceholderText('Type your answer...');
    fireEvent.change(input, { target: { value: '  details  ' } });
    fireEvent.keyDown(input, { key: 'Enter' });

    expect(onSubmit).toHaveBeenCalledWith('details');
  });

  it('submits textarea answers on Ctrl/Cmd + Enter', () => {
    const { onSubmit } = renderPanel({
      questionId: 'q-textarea',
      question: 'Explain constraints',
      hint: null,
      inputType: 'textarea',
    });

    const textarea = screen.getByPlaceholderText('Type your answer...');
    fireEvent.change(textarea, { target: { value: 'More context' } });
    fireEvent.keyDown(textarea, { key: 'Enter', ctrlKey: true });

    expect(onSubmit).toHaveBeenCalledWith('More context');
  });
});
