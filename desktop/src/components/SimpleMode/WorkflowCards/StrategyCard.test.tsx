import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

const translations: Record<string, string> = {
  'workflow.strategy.title': 'Strategy Analysis',
  'workflow.strategy.confidence': '{{pct}}% confidence',
  'workflow.strategy.risk': 'Risk',
  'workflow.strategy.stories': 'Stories',
  'workflow.strategy.parallel': 'Parallel',
  'workflow.strategy.modelBadge': 'Model: {{model}}',
  'workflow.strategy.values.strategy.task': 'Task',
  'workflow.strategy.values.risk.medium': 'Medium',
  'workflow.strategy.values.parallel.moderate': 'Moderate',
};

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: () => {},
  },
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      const template = translations[key] ?? key;
      return template.replace(/\{\{(\w+)\}\}/g, (_match, token: string) => String(options?.[token] ?? ''));
    },
  }),
}));

import { StrategyCard } from './StrategyCard';

describe('StrategyCard', () => {
  it('localizes strategy dimensions and shows model badge for LLM-backed analysis', () => {
    render(
      <StrategyCard
        data={{
          strategy: 'task',
          confidence: 0.84,
          reasoning: 'Use structured execution.',
          riskLevel: 'medium',
          estimatedStories: 4,
          parallelizationBenefit: 'moderate',
          functionalAreas: ['frontend'],
          recommendations: [],
          model: 'minimax:MiniMax-M2.5',
          recommendationSource: 'llm_enhanced',
        }}
      />,
    );

    expect(screen.getByText('Task')).toBeInTheDocument();
    expect(screen.getByText('Medium')).toBeInTheDocument();
    expect(screen.getByText('Moderate')).toBeInTheDocument();
    expect(screen.getByText('Model: minimax:MiniMax-M2.5')).toBeInTheDocument();
  });
});
